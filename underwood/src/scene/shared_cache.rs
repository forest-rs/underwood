// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Bounded exact reuse of identity-free paragraph preparation.
//!
//! This module owns shared preparation keys, values, lifetime, and accounting;
//! it explicitly does not own document identity, final geometry, or backend
//! preparation policy.

use super::*;
use core::mem::size_of;

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct SharedPreparationDiagnostics {
    pub(super) budget: usize,
    pub(super) entries: usize,
    pub(super) resident_bytes: usize,
    pub(super) peak_bytes: usize,
    pub(super) hits: usize,
    pub(super) misses: usize,
    pub(super) evictions: usize,
    pub(super) oversized_non_retentions: usize,
}

#[derive(Debug)]
pub(super) struct SharedPreparationCache {
    budget: usize,
    epoch: Option<u64>,
    buckets: BTreeMap<u64, Vec<SharedPreparationEntry>>,
    entries: usize,
    resident_bytes: usize,
    work: SharedPreparationWork,
}

impl SharedPreparationCache {
    pub(super) fn new(budget: usize) -> Self {
        Self {
            budget,
            epoch: None,
            buckets: BTreeMap::new(),
            entries: 0,
            resident_bytes: 0,
            work: SharedPreparationWork::default(),
        }
    }

    pub(super) fn synchronize_epoch(&mut self, epoch: Option<u64>) {
        if self.epoch != epoch {
            self.clear_entries();
            self.epoch = epoch;
        }
    }

    pub(super) const fn is_enabled(&self) -> bool {
        self.budget != 0 && self.epoch.is_some()
    }

    pub(super) fn lookup(
        &mut self,
        query: &SharedPreparationQuery<'_, '_>,
        current_use: u64,
    ) -> Option<SharedPreparationHit> {
        debug_assert_eq!(
            self.epoch,
            Some(query.epoch),
            "shared lookup must use the synchronized backend epoch"
        );
        let fingerprint = text_fingerprint(query.projection.mapping.text());
        let hit = self
            .buckets
            .get_mut(&fingerprint)
            .and_then(|bucket| bucket.iter_mut().find(|entry| entry.key.matches(query)));
        match hit {
            Some(entry) => {
                entry.last_used = current_use;
                self.work.hits = self.work.hits.saturating_add(1);
                Some(SharedPreparationHit {
                    facts: entry.facts.clone(),
                    region_transcript: entry.region_transcript.clone(),
                })
            }
            None => {
                self.work.misses = self.work.misses.saturating_add(1);
                None
            }
        }
    }

    pub(super) fn insert(
        &mut self,
        query: &SharedPreparationQuery<'_, '_>,
        facts: Arc<PreparedParagraphFacts>,
        region_transcript: Option<&RegionTranscript>,
        current_use: u64,
    ) {
        debug_assert_eq!(
            self.epoch,
            Some(query.epoch),
            "shared insertion must use the synchronized backend epoch"
        );
        let key = SharedPreparationKey::from_query(query);
        let region_transcript = region_transcript.map(RegionTranscriptTemplate::from_transcript);
        let weight = size_of::<SharedPreparationEntry>()
            .saturating_add(key.estimated_owned_bytes())
            .saturating_add(facts.estimated_owned_bytes())
            .saturating_add(
                region_transcript
                    .as_ref()
                    .map_or(0, RegionTranscriptTemplate::estimated_owned_bytes),
            );
        if weight > self.budget {
            self.work.oversized_non_retentions =
                self.work.oversized_non_retentions.saturating_add(1);
            return;
        }
        while self.resident_bytes.saturating_add(weight) > self.budget {
            if !self.evict_oldest() {
                break;
            }
        }
        let fingerprint = text_fingerprint(&key.text);
        self.buckets
            .entry(fingerprint)
            .or_default()
            .push(SharedPreparationEntry {
                key,
                facts,
                region_transcript,
                weight,
                last_used: current_use,
            });
        self.entries = self.entries.saturating_add(1);
        self.resident_bytes = self.resident_bytes.saturating_add(weight);
        self.work.peak_bytes = self.work.peak_bytes.max(self.resident_bytes);
    }

    pub(super) fn clear(&mut self) {
        self.clear_entries();
        self.epoch = None;
    }

    pub(super) const fn diagnostics(&self) -> SharedPreparationDiagnostics {
        SharedPreparationDiagnostics {
            budget: self.budget,
            entries: self.entries,
            resident_bytes: self.resident_bytes,
            peak_bytes: self.work.peak_bytes,
            hits: self.work.hits,
            misses: self.work.misses,
            evictions: self.work.evictions,
            oversized_non_retentions: self.work.oversized_non_retentions,
        }
    }

    #[cfg(test)]
    pub(super) fn replace_first_facts_for_test(&mut self, facts: Arc<PreparedParagraphFacts>) {
        let entry = self
            .buckets
            .values_mut()
            .find_map(|bucket| bucket.first_mut())
            .expect("test cache must contain one shared entry");
        entry.facts = facts;
    }

    #[cfg(test)]
    pub(super) fn collide_bucket_for_test(&mut self, source: &str, target: &str) {
        let source = text_fingerprint(source);
        let target = text_fingerprint(target);
        let mut entries = self
            .buckets
            .remove(&source)
            .expect("test source bucket must exist");
        self.buckets.entry(target).or_default().append(&mut entries);
    }

    fn evict_oldest(&mut self) -> bool {
        let mut oldest = None;
        for (fingerprint, bucket) in &self.buckets {
            for (index, entry) in bucket.iter().enumerate() {
                let candidate = (entry.last_used, *fingerprint, index);
                if oldest.is_none_or(|previous| candidate < previous) {
                    oldest = Some(candidate);
                }
            }
        }
        let Some((_, fingerprint, index)) = oldest else {
            return false;
        };
        let mut remove_bucket = false;
        if let Some(bucket) = self.buckets.get_mut(&fingerprint) {
            let entry = bucket.remove(index);
            self.entries = self.entries.saturating_sub(1);
            self.resident_bytes = self.resident_bytes.saturating_sub(entry.weight);
            remove_bucket = bucket.is_empty();
        }
        if remove_bucket {
            self.buckets.remove(&fingerprint);
        }
        self.work.evictions = self.work.evictions.saturating_add(1);
        true
    }

    fn clear_entries(&mut self) {
        self.buckets.clear();
        self.entries = 0;
        self.resident_bytes = 0;
    }
}

#[derive(Clone, Copy)]
pub(super) struct SharedPreparationQuery<'query, 'source> {
    pub(super) epoch: u64,
    pub(super) projection: &'query Projection<'source>,
    pub(super) constraint: TextConstraint,
    pub(super) region_flow: Option<&'query RegionFlow>,
    pub(super) region_cursor: Option<RegionCursor>,
}

#[derive(Clone, Debug)]
pub(super) struct SharedPreparationHit {
    pub(super) facts: Arc<PreparedParagraphFacts>,
    region_transcript: Option<RegionTranscriptTemplate>,
}

impl SharedPreparationHit {
    pub(super) fn region_transcript(
        &self,
        paragraph: ParagraphId,
        region_flow: Option<&RegionFlow>,
    ) -> Result<Option<RegionTranscript>, SceneError> {
        match (&self.region_transcript, region_flow) {
            (Some(template), Some(flow)) => Ok(Some(template.rebind(paragraph, flow)?)),
            (None, None) => Ok(None),
            _ => Err(SceneError::for_paragraph(SceneErrorKind::Flow, paragraph)),
        }
    }
}

#[derive(Debug)]
struct SharedPreparationEntry {
    key: SharedPreparationKey,
    facts: Arc<PreparedParagraphFacts>,
    region_transcript: Option<RegionTranscriptTemplate>,
    weight: usize,
    last_used: u64,
}

#[derive(Debug, Default)]
struct SharedPreparationWork {
    peak_bytes: usize,
    hits: usize,
    misses: usize,
    evictions: usize,
    oversized_non_retentions: usize,
}

#[derive(Clone, Debug, PartialEq)]
struct SharedPreparationKey {
    epoch: u64,
    text: alloc::string::String,
    analysis_styles: Vec<AnalysisStyle>,
    analysis_runs: Vec<AnalysisRun>,
    shaping_styles: Vec<ShapingStyle>,
    shaping_runs: Vec<ShapingRun>,
    inline_flow_styles: Vec<InlineFlowStyle>,
    inline_flow_runs: Vec<InlineFlowRun>,
    base_direction: BaseDirection,
    whitespace_collapse: crate::WhitespaceCollapse,
    constraint: SharedConstraintKey,
    region_flow: Option<RegionFlow>,
    region_cursor: Option<RegionCursor>,
    empty_line_height: u64,
    paint_runs: Vec<PaintRun>,
}

impl SharedPreparationKey {
    fn from_query(query: &SharedPreparationQuery<'_, '_>) -> Self {
        let projection = query.projection;
        Self {
            epoch: query.epoch,
            text: alloc::string::String::from(projection.mapping.text()),
            analysis_styles: projection.analysis_styles.clone(),
            analysis_runs: projection.analysis_runs.clone(),
            shaping_styles: projection
                .shaping_styles
                .iter()
                .map(|style| (*style).clone())
                .collect(),
            shaping_runs: projection.shaping_runs.clone(),
            inline_flow_styles: projection.inline_flow_styles.clone(),
            inline_flow_runs: projection.inline_flow_runs.clone(),
            base_direction: projection.paragraph_style.base_direction(),
            whitespace_collapse: projection.paragraph_style.whitespace_collapse(),
            constraint: SharedConstraintKey::from(query.constraint),
            region_flow: query.region_flow.cloned(),
            region_cursor: query.region_cursor,
            empty_line_height: empty_line_height_key(projection),
            paint_runs: projection.paint_runs.clone(),
        }
    }

    fn matches(&self, query: &SharedPreparationQuery<'_, '_>) -> bool {
        let projection = query.projection;
        self.epoch == query.epoch
            && self.text == projection.mapping.text()
            && self.analysis_styles == projection.analysis_styles
            && self.analysis_runs == projection.analysis_runs
            && self.shaping_styles.len() == projection.shaping_styles.len()
            && self
                .shaping_styles
                .iter()
                .zip(&projection.shaping_styles)
                .all(|(cached, projected)| cached == *projected)
            && self.shaping_runs == projection.shaping_runs
            && self.inline_flow_styles == projection.inline_flow_styles
            && self.inline_flow_runs == projection.inline_flow_runs
            && self.base_direction == projection.paragraph_style.base_direction()
            && self.whitespace_collapse == projection.paragraph_style.whitespace_collapse()
            && self.constraint == SharedConstraintKey::from(query.constraint)
            && option_ref_eq(self.region_flow.as_ref(), query.region_flow)
            && self.region_cursor == query.region_cursor
            && self.empty_line_height == empty_line_height_key(projection)
            && self.paint_runs == projection.paint_runs
    }

    fn estimated_owned_bytes(&self) -> usize {
        self.text
            .capacity()
            .saturating_add(vec_bytes::<AnalysisStyle>(self.analysis_styles.capacity()))
            .saturating_add(vec_bytes::<AnalysisRun>(self.analysis_runs.capacity()))
            .saturating_add(vec_bytes::<ShapingStyle>(self.shaping_styles.capacity()))
            .saturating_add(vec_bytes::<ShapingRun>(self.shaping_runs.capacity()))
            .saturating_add(vec_bytes::<InlineFlowStyle>(
                self.inline_flow_styles.capacity(),
            ))
            .saturating_add(vec_bytes::<InlineFlowRun>(self.inline_flow_runs.capacity()))
            .saturating_add(vec_bytes::<PaintRun>(self.paint_runs.capacity()))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SharedConstraintKey {
    MinContent,
    MaxContent,
    Wrap(u64),
}

impl From<TextConstraint> for SharedConstraintKey {
    fn from(constraint: TextConstraint) -> Self {
        match constraint {
            TextConstraint::MinContent => Self::MinContent,
            TextConstraint::MaxContent => Self::MaxContent,
            TextConstraint::Wrap(width) => Self::Wrap(width.0.to_bits()),
        }
    }
}

#[derive(Clone, Debug)]
struct RegionTranscriptTemplate {
    start: RegionCursor,
    end: RegionCursor,
    attempts: Vec<RegionAttemptTemplate>,
}

impl RegionTranscriptTemplate {
    fn from_transcript(transcript: &RegionTranscript) -> Self {
        Self {
            start: transcript.start(),
            end: transcript.end(),
            attempts: transcript
                .attempts()
                .iter()
                .map(|attempt| RegionAttemptTemplate {
                    source: attempt.source(),
                    slot: attempt.slot(),
                    line_height: attempt.line_height(),
                    outcome: attempt.outcome(),
                })
                .collect(),
        }
    }

    fn rebind(
        &self,
        paragraph: ParagraphId,
        flow: &RegionFlow,
    ) -> Result<RegionTranscript, SceneError> {
        let attempts = self
            .attempts
            .iter()
            .map(|attempt| {
                crate::RegionAttempt::try_new(
                    paragraph,
                    attempt.source.clone(),
                    attempt.slot,
                    attempt.line_height,
                    attempt.outcome,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        RegionTranscript::try_new(flow, self.start, self.end, attempts)
    }

    fn estimated_owned_bytes(&self) -> usize {
        vec_bytes::<RegionAttemptTemplate>(self.attempts.capacity())
    }
}

#[derive(Clone, Debug)]
struct RegionAttemptTemplate {
    source: Range<u32>,
    slot: crate::LineSlot,
    line_height: f64,
    outcome: RegionAttemptOutcome,
}

fn empty_line_height_key(projection: &Projection<'_>) -> u64 {
    if projection.mapping.text().is_empty() {
        projection.empty_line_height_key()
    } else {
        0
    }
}

fn option_ref_eq<T: PartialEq>(left: Option<&T>, right: Option<&T>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => left == right,
        (None, None) => true,
        _ => false,
    }
}

fn text_fingerprint(text: &str) -> u64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    text.as_bytes().iter().fold(OFFSET, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(PRIME)
    })
}

const fn vec_bytes<T>(capacity: usize) -> usize {
    size_of::<T>().saturating_mul(capacity)
}

#[cfg(test)]
mod tests {
    use super::vec_bytes;

    #[test]
    fn byte_accounting_saturates_instead_of_wrapping() {
        assert_eq!(vec_bytes::<[u8; 2]>(usize::MAX), usize::MAX);
    }
}
