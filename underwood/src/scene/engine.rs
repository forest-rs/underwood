// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Layout orchestration and coordinated retained-cache policy.
//!
//! This module owns cache identity, lifetime, and preparation sequencing; it
//! explicitly does not own semantic projection or geometry construction.

use super::*;

/// Maximum number of retained committed and composition geometry entries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CacheBudget {
    max_entries: usize,
}

impl CacheBudget {
    /// Creates a budget with the given maximum retained geometry entries.
    ///
    /// A zero budget materializes owned outputs without retaining cache entries.
    #[must_use]
    pub const fn new(max_entries: usize) -> Self {
        Self { max_entries }
    }

    /// Returns the maximum number of retained geometry entries.
    #[must_use]
    pub const fn max_entries(self) -> usize {
        self.max_entries
    }
}

/// Snapshot of coordinated retained-cache state and cumulative activity.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CacheDiagnostics {
    budget: usize,
    committed_entries: usize,
    composition_entries: usize,
    backend_entries: Option<usize>,
    peak_entries: usize,
    hits: usize,
    misses: usize,
    evictions: usize,
    releases: usize,
}

impl CacheDiagnostics {
    /// Returns the configured maximum retained geometry entries.
    #[must_use]
    pub const fn budget(self) -> usize {
        self.budget
    }

    /// Returns resident committed geometry entries.
    #[must_use]
    pub const fn committed_entries(self) -> usize {
        self.committed_entries
    }

    /// Returns resident transient-composition geometry entries.
    #[must_use]
    pub const fn composition_entries(self) -> usize {
        self.composition_entries
    }

    /// Returns all resident geometry entries.
    #[must_use]
    pub const fn current_entries(self) -> usize {
        self.committed_entries + self.composition_entries
    }

    /// Returns retained backend physics entries, when the backend reports them.
    #[must_use]
    pub const fn backend_entries(self) -> Option<usize> {
        self.backend_entries
    }

    /// Returns the highest observed resident geometry entry count.
    #[must_use]
    pub const fn peak_entries(self) -> usize {
        self.peak_entries
    }

    /// Returns paragraph-identity lookups that found a resident geometry entry.
    #[must_use]
    pub const fn hits(self) -> usize {
        self.hits
    }

    /// Returns paragraph-identity lookups that created a geometry entry.
    #[must_use]
    pub const fn misses(self) -> usize {
        self.misses
    }

    /// Returns entries removed to enforce the configured budget.
    #[must_use]
    pub const fn evictions(self) -> usize {
        self.evictions
    }

    /// Returns entries removed by explicit document or whole-cache release.
    #[must_use]
    pub const fn releases(self) -> usize {
        self.releases
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct CacheWork {
    peak_entries: usize,
    hits: usize,
    misses: usize,
    evictions: usize,
    releases: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum CacheKind {
    Committed,
    Composition,
}

/// Mutable owner of one paragraph adapter and its retained stage caches.
pub struct LayoutEngine {
    paragraphs: Box<dyn ParagraphFormation>,
    cache: BTreeMap<ParagraphId, ParagraphCache>,
    composition_cache: BTreeMap<ParagraphId, ParagraphCache>,
    recency: BTreeSet<(u64, CacheKind, ParagraphId)>,
    documents: BTreeMap<crate::DocumentId, BTreeSet<(CacheKind, ParagraphId)>>,
    clock: u64,
    budget: CacheBudget,
    cache_work: CacheWork,
}

impl core::fmt::Debug for LayoutEngine {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("LayoutEngine")
            .field("cached_paragraphs", &self.cache.len())
            .field(
                "cached_composition_paragraphs",
                &self.composition_cache.len(),
            )
            .finish_non_exhaustive()
    }
}

impl LayoutEngine {
    /// Creates an engine owning one configured paragraph adapter and cache budget.
    #[must_use]
    pub fn new(paragraphs: impl ParagraphFormation + 'static, budget: CacheBudget) -> Self {
        Self {
            paragraphs: Box::new(paragraphs),
            cache: BTreeMap::new(),
            composition_cache: BTreeMap::new(),
            recency: BTreeSet::new(),
            documents: BTreeMap::new(),
            clock: 0,
            budget,
            cache_work: CacheWork::default(),
        }
    }

    /// Prepares an immutable scene without publishing partial results on failure.
    pub fn prepare(
        &mut self,
        snapshot: &DocumentSnapshot,
        request: &SceneRequest<'_>,
    ) -> Result<SceneOutput, SceneError> {
        validate_styles(snapshot, request)?;

        let mut work = WorkReport::default();
        let mut lines = Vec::new();
        let mut fragments = Vec::new();
        let mut clusters = Vec::new();
        let mut carets = Vec::new();
        let mut movements = Vec::new();
        let mut texts = Vec::new();
        let mut semantics = Vec::new();
        let mut y_offset = 0.0;
        let region_start = request.region_flow.map(RegionFlow::cursor);
        let mut region_cursor = region_start;
        let mut region_attempts = Vec::new();

        for paragraph in snapshot.paragraphs() {
            let projection = Projection::new(paragraph, request)?;
            self.clock = self.clock.saturating_add(1);
            let access = prepare_paragraph_geometry(
                self.paragraphs.as_mut(),
                &mut self.cache,
                paragraph,
                &projection,
                request.constraint,
                request.region_flow,
                region_cursor,
                self.clock,
                &mut work,
            )?;
            self.record_access(CacheKind::Committed, &access);
            if let Some(transcript) = &access.region_transcript {
                region_attempts.extend_from_slice(transcript.attempts());
                region_cursor = Some(transcript.end());
            }
            let geometry = &self
                .cache
                .get(&paragraph.id)
                .expect("prepared committed geometry must remain resident")
                .geometry;
            materialize_geometry(
                geometry,
                snapshot.revision(),
                if request.region_flow.is_some() {
                    0.0
                } else {
                    y_offset
                },
                &mut lines,
                &mut fragments,
                &mut clusters,
                &mut carets,
                &mut movements,
                &mut texts,
                &mut semantics,
            );
            if request.region_flow.is_none() {
                y_offset += geometry.height;
            } else {
                y_offset = y_offset.max(geometry.height);
            }
            self.enforce_budget();
        }

        work.paint = StageWork {
            paragraphs: snapshot.paragraphs().len(),
            records: fragments.len(),
        };
        let metrics = TextMetrics::from_lines(&lines, y_offset);
        let region_transcript = match (request.region_flow, region_start, region_cursor) {
            (Some(flow), Some(start), Some(end)) => Some(RegionTranscript::try_new(
                flow,
                start,
                end,
                region_attempts,
            )?),
            (None, None, None) => None,
            _ => return Err(SceneError::new(SceneErrorKind::Flow)),
        };
        let output = SceneOutput {
            scene: TextScene {
                document: snapshot.id(),
                revision: snapshot.revision(),
                metrics,
                lines,
                fragments,
                clusters,
                carets,
                movements,
                texts,
                paint: request.paint.clone(),
                semantics,
            },
            work,
            region_transcript,
        };
        Ok(output)
    }

    /// Prepares one retained block through the same paragraph and scene path as a document.
    pub fn prepare_block(
        &mut self,
        snapshot: &TextBlockSnapshot,
        request: &BlockRequest<'_>,
    ) -> Result<SceneOutput, SceneError> {
        let styles = StyleMap::new(request.style.clone())
            .with_default_paragraph_style(request.paragraph_style);
        self.prepare(
            snapshot.document(),
            &match request.region_flow {
                Some(flow) => SceneRequest::new(request.constraint, &styles, request.paint)
                    .with_region_flow(flow),
                None => SceneRequest::new(request.constraint, &styles, request.paint),
            },
        )
    }

    /// Prepares a transient generated-text scene without evicting committed work.
    pub fn prepare_composition(
        &mut self,
        snapshot: &DocumentSnapshot,
        request: &SceneRequest<'_>,
        composition: &CompositionSession,
    ) -> Result<CompositionSceneOutput, SceneError> {
        validate_styles(snapshot, request)?;
        if composition.document() != snapshot.id()
            || composition.base_revision() != snapshot.revision()
        {
            return Err(SceneError::for_document(
                SceneErrorKind::InvalidComposition,
                snapshot.id(),
            ));
        }
        let target = composition.target_text().ok_or_else(|| {
            SceneError::for_document(SceneErrorKind::InvalidComposition, snapshot.id())
        })?;

        let mut work = WorkReport::default();
        let mut lines = Vec::new();
        let mut fragments = Vec::new();
        let mut clusters = Vec::new();
        let mut carets = Vec::new();
        let mut movements = Vec::new();
        let mut semantics = Vec::new();
        let mut y_offset = 0.0;
        let region_start = request.region_flow.map(RegionFlow::cursor);
        let mut region_cursor = region_start;
        let mut region_attempts = Vec::new();

        for paragraph in snapshot.paragraphs() {
            let transient = paragraph.id.index == target.paragraph;
            let projection = if transient {
                Projection::with_composition(paragraph, request, composition)?
            } else {
                Projection::new(paragraph, request)?
            };
            self.clock = self.clock.saturating_add(1);
            let (kind, access) = if transient {
                (
                    CacheKind::Composition,
                    prepare_paragraph_geometry(
                        self.paragraphs.as_mut(),
                        &mut self.composition_cache,
                        paragraph,
                        &projection,
                        request.constraint,
                        request.region_flow,
                        region_cursor,
                        self.clock,
                        &mut work,
                    )?,
                )
            } else {
                (
                    CacheKind::Committed,
                    prepare_paragraph_geometry(
                        self.paragraphs.as_mut(),
                        &mut self.cache,
                        paragraph,
                        &projection,
                        request.constraint,
                        request.region_flow,
                        region_cursor,
                        self.clock,
                        &mut work,
                    )?,
                )
            };
            self.record_access(kind, &access);
            if let Some(transcript) = &access.region_transcript {
                region_attempts.extend_from_slice(transcript.attempts());
                region_cursor = Some(transcript.end());
            }
            let geometry = match kind {
                CacheKind::Committed => self
                    .cache
                    .get(&paragraph.id)
                    .expect("prepared committed geometry must remain resident"),
                CacheKind::Composition => self
                    .composition_cache
                    .get(&paragraph.id)
                    .expect("prepared composition geometry must remain resident"),
            };
            let geometry = &geometry.geometry;
            materialize_projected_geometry(
                geometry,
                snapshot.revision(),
                if request.region_flow.is_some() {
                    0.0
                } else {
                    y_offset
                },
                &mut lines,
                &mut fragments,
                &mut clusters,
                &mut carets,
                &mut movements,
                &mut semantics,
            );
            if request.region_flow.is_none() {
                y_offset += geometry.height;
            } else {
                y_offset = y_offset.max(geometry.height);
            }
            self.enforce_budget();
        }

        work.paint = StageWork {
            paragraphs: snapshot.paragraphs().len(),
            records: fragments.len(),
        };
        let metrics = TextMetrics::from_lines(&lines, y_offset);
        let region_transcript = match (request.region_flow, region_start, region_cursor) {
            (Some(flow), Some(start), Some(end)) => Some(RegionTranscript::try_new(
                flow,
                start,
                end,
                region_attempts,
            )?),
            (None, None, None) => None,
            _ => return Err(SceneError::new(SceneErrorKind::Flow)),
        };
        let output = CompositionSceneOutput {
            scene: CompositionScene {
                document: snapshot.id(),
                revision: snapshot.revision(),
                composition: composition.id(),
                epoch: composition.epoch(),
                metrics,
                lines,
                fragments,
                clusters,
                carets,
                movements,
                paint: request.paint.clone(),
                semantics,
            },
            work,
            region_transcript,
        };
        Ok(output)
    }

    /// Releases every retained geometry and backend entry for one document.
    pub fn release_document(&mut self, document: crate::DocumentId) {
        let Some(entries) = self.documents.remove(&document) else {
            return;
        };
        let mut paragraphs = BTreeSet::new();
        for (kind, paragraph) in entries {
            let removed = match kind {
                CacheKind::Committed => self.cache.remove(&paragraph),
                CacheKind::Composition => self.composition_cache.remove(&paragraph),
            };
            if let Some(entry) = removed {
                self.recency.remove(&(entry.last_used, kind, paragraph));
                self.cache_work.releases += 1;
            }
            paragraphs.insert(paragraph);
        }
        for paragraph in paragraphs {
            self.paragraphs.release(paragraph);
        }
    }

    /// Releases all retained geometry and backend paragraph physics.
    pub fn clear_cache(&mut self) {
        self.cache_work.releases += self.cache.len() + self.composition_cache.len();
        self.cache.clear();
        self.composition_cache.clear();
        self.recency.clear();
        self.documents.clear();
        self.paragraphs.clear();
    }

    /// Returns a snapshot of coordinated cache state and cumulative activity.
    #[must_use]
    pub fn cache_diagnostics(&self) -> CacheDiagnostics {
        CacheDiagnostics {
            budget: self.budget.max_entries,
            committed_entries: self.cache.len(),
            composition_entries: self.composition_cache.len(),
            backend_entries: self.paragraphs.retained_entries(),
            peak_entries: self.cache_work.peak_entries,
            hits: self.cache_work.hits,
            misses: self.cache_work.misses,
            evictions: self.cache_work.evictions,
            releases: self.cache_work.releases,
        }
    }

    fn record_access(&mut self, kind: CacheKind, access: &CacheAccess) {
        if let Some(previous) = access.previous_use {
            self.recency.remove(&(previous, kind, access.paragraph));
            self.cache_work.hits += 1;
        } else {
            self.cache_work.misses += 1;
            self.documents
                .entry(access.paragraph.document)
                .or_default()
                .insert((kind, access.paragraph));
        }
        self.recency
            .insert((access.current_use, kind, access.paragraph));
        self.cache_work.peak_entries = self
            .cache_work
            .peak_entries
            .max(self.cache.len() + self.composition_cache.len());
    }

    fn enforce_budget(&mut self) {
        while self.cache.len() + self.composition_cache.len() > self.budget.max_entries {
            let Some((_, kind, paragraph)) = self.recency.pop_first() else {
                break;
            };
            match kind {
                CacheKind::Committed => {
                    self.cache.remove(&paragraph);
                }
                CacheKind::Composition => {
                    self.composition_cache.remove(&paragraph);
                }
            }
            if let Some(entries) = self.documents.get_mut(&paragraph.document) {
                entries.remove(&(kind, paragraph));
                if entries.is_empty() {
                    self.documents.remove(&paragraph.document);
                }
            }
            self.cache_work.evictions += 1;
            if !self.cache.contains_key(&paragraph)
                && !self.composition_cache.contains_key(&paragraph)
            {
                self.paragraphs.release(paragraph);
            }
        }
    }
}

#[derive(Clone, Debug)]
struct CacheAccess {
    paragraph: ParagraphId,
    previous_use: Option<u64>,
    current_use: u64,
    region_transcript: Option<RegionTranscript>,
}

fn prepare_paragraph_geometry(
    paragraphs: &mut dyn ParagraphFormation,
    cache: &mut BTreeMap<ParagraphId, ParagraphCache>,
    paragraph: &Paragraph,
    projection: &Projection<'_>,
    constraint: TextConstraint,
    region_flow: Option<&RegionFlow>,
    region_cursor: Option<RegionCursor>,
    current_use: u64,
    work: &mut WorkReport,
) -> Result<CacheAccess, SceneError> {
    let formation_matches = cache.get(&paragraph.id).is_some_and(|entry| {
        entry.formation_key.matches(
            paragraph.version,
            projection,
            constraint,
            region_flow,
            region_cursor,
        )
    });
    let paint_matches = cache
        .get(&paragraph.id)
        .is_some_and(|entry| entry.paint_runs == projection.paint_runs);
    let adjustment_matches = cache.get(&paragraph.id).is_some_and(|entry| {
        entry.formation_key.paragraph_style.alignment() == projection.paragraph_style.alignment()
    });
    if formation_matches && paint_matches && adjustment_matches {
        let entry = cache
            .get_mut(&paragraph.id)
            .expect("a reusable cache entry must exist");
        let previous_use = Some(entry.last_used);
        entry.last_used = current_use;
        if let Some((id, epoch)) = projection.composition_identity() {
            rebind_composition_geometry(&mut entry.geometry, id, epoch);
        }
        work.reused_paragraphs += 1;
        return Ok(CacheAccess {
            paragraph: paragraph.id,
            previous_use,
            current_use,
            region_transcript: entry.region_transcript.clone(),
        });
    }

    let shaping_styles: Vec<_> = projection
        .shaping_styles
        .iter()
        .map(|style| (*style).clone())
        .collect();
    let text_len = u32::try_from(projection.mapping.text().len())
        .map_err(|_| SceneError::for_paragraph(SceneErrorKind::SourceCoverage, paragraph.id))?;
    let constraints = match (region_flow, region_cursor) {
        (Some(flow), Some(cursor)) => ParagraphConstraints::in_regions(
            constraint,
            projection.empty_line_height(),
            flow.clone(),
            cursor,
        ),
        (None, None) => ParagraphConstraints::new(constraint, projection.empty_line_height()),
        _ => {
            release_untracked_backend(paragraphs, cache, paragraph.id);
            return Err(SceneError::for_paragraph(
                SceneErrorKind::Flow,
                paragraph.id,
            ));
        }
    };
    let output = match paragraphs.form(
        ParagraphInput::new(
            paragraph.id,
            projection.paragraph_style,
            projection.mapping.text(),
            &projection.analysis_styles,
            &projection.analysis_runs,
            &shaping_styles,
            &projection.shaping_runs,
            &projection.inline_flow_styles,
            &projection.inline_flow_runs,
            &projection.paint_runs,
        ),
        constraints,
    ) {
        Ok(output) => output,
        Err(error) => {
            release_untracked_backend(paragraphs, cache, paragraph.id);
            return Err(SceneError::from_preparation(paragraph.id, error.kind()));
        }
    };
    if output.paragraph().paragraph() != paragraph.id || output.paragraph().text_len() != text_len {
        release_untracked_backend(paragraphs, cache, paragraph.id);
        return Err(SceneError::for_paragraph(
            SceneErrorKind::SourceCoverage,
            paragraph.id,
        ));
    }
    if let Err(error) = validate_prepared(output.paragraph(), projection) {
        release_untracked_backend(paragraphs, cache, paragraph.id);
        return Err(error);
    }
    let region_transcript = match (region_flow, region_cursor, output.region_transcript()) {
        (Some(flow), Some(cursor), Some(transcript))
            if transcript.start() == cursor
                && transcript.replay(flow) == Ok(transcript.end())
                && region_output_matches(output.paragraph(), transcript, projection) =>
        {
            Some(transcript.clone())
        }
        (None, None, None) => None,
        _ => {
            release_untracked_backend(paragraphs, cache, paragraph.id);
            return Err(SceneError::for_paragraph(
                SceneErrorKind::Flow,
                paragraph.id,
            ));
        }
    };
    let slots_match = output
        .paragraph()
        .lines()
        .iter()
        .all(|line| line.slot().is_some() == region_flow.is_some());
    if !slots_match {
        release_untracked_backend(paragraphs, cache, paragraph.id);
        return Err(SceneError::for_paragraph(
            SceneErrorKind::Flow,
            paragraph.id,
        ));
    }
    record_formation_work(work, output.work());
    if projection.mapping.text().is_empty() && !formation_matches {
        work.flow.add_paragraph(1);
    }
    let geometry = match build_geometry(
        output.paragraph(),
        projection,
        constraint,
        region_transcript.as_ref(),
    ) {
        Ok(geometry) => geometry,
        Err(error) => {
            release_untracked_backend(paragraphs, cache, paragraph.id);
            return Err(error);
        }
    };
    work.adjustment.add_paragraph(if geometry.lines.is_empty() {
        1
    } else {
        geometry.lines.len()
    });
    work.geometry.add_paragraph(geometry.fragments.len());
    let formation_key = FormationKey::new(
        paragraph.version,
        alloc::string::String::from(projection.mapping.text()),
        projection.analysis_styles.clone(),
        projection.analysis_runs.clone(),
        shaping_styles,
        projection.shaping_runs.clone(),
        projection.inline_flow_styles.clone(),
        projection.inline_flow_runs.clone(),
        projection.paragraph_style,
        constraint,
        region_flow.cloned(),
        region_cursor,
        projection.empty_line_height_key(),
        projection,
    );
    let previous_use = if let Some(entry) = cache.get_mut(&paragraph.id) {
        let previous_use = Some(entry.last_used);
        entry.last_used = current_use;
        entry.formation_key = formation_key;
        entry.paint_runs = projection.paint_runs.clone();
        entry.region_transcript = region_transcript.clone();
        entry.geometry = geometry;
        previous_use
    } else {
        cache.insert(
            paragraph.id,
            ParagraphCache {
                last_used: current_use,
                formation_key,
                paint_runs: projection.paint_runs.clone(),
                region_transcript: region_transcript.clone(),
                geometry,
            },
        );
        None
    };
    Ok(CacheAccess {
        paragraph: paragraph.id,
        previous_use,
        current_use,
        region_transcript,
    })
}

fn release_untracked_backend(
    paragraphs: &mut dyn ParagraphFormation,
    cache: &BTreeMap<ParagraphId, ParagraphCache>,
    paragraph: ParagraphId,
) {
    if !cache.contains_key(&paragraph) {
        paragraphs.release(paragraph);
    }
}

#[derive(Clone, Debug, PartialEq)]
struct FormationKey {
    version: u64,
    text: alloc::string::String,
    source_map: Vec<ProjectionSourceKey>,
    analysis_styles: Vec<AnalysisStyle>,
    analysis_runs: Vec<AnalysisRun>,
    shaping_styles: Vec<ShapingStyle>,
    shaping_runs: Vec<ShapingRun>,
    inline_flow_styles: Vec<InlineFlowStyle>,
    inline_flow_runs: Vec<InlineFlowRun>,
    paragraph_style: ParagraphStyle,
    constraint: ConstraintKey,
    region_flow: Option<RegionFlow>,
    region_cursor: Option<RegionCursor>,
    empty_line_height: u64,
}

impl FormationKey {
    fn new(
        version: u64,
        text: alloc::string::String,
        analysis_styles: Vec<AnalysisStyle>,
        analysis_runs: Vec<AnalysisRun>,
        shaping_styles: Vec<ShapingStyle>,
        shaping_runs: Vec<ShapingRun>,
        inline_flow_styles: Vec<InlineFlowStyle>,
        inline_flow_runs: Vec<InlineFlowRun>,
        paragraph_style: ParagraphStyle,
        constraint: TextConstraint,
        region_flow: Option<RegionFlow>,
        region_cursor: Option<RegionCursor>,
        empty_line_height: u64,
        projection: &Projection<'_>,
    ) -> Self {
        Self {
            version,
            text,
            source_map: ProjectionSourceKey::from_projection(projection),
            analysis_styles,
            analysis_runs,
            shaping_styles,
            shaping_runs,
            inline_flow_styles,
            inline_flow_runs,
            paragraph_style,
            constraint: ConstraintKey::from(constraint),
            region_flow,
            region_cursor,
            empty_line_height,
        }
    }

    fn matches(
        &self,
        version: u64,
        projection: &Projection<'_>,
        constraint: TextConstraint,
        region_flow: Option<&RegionFlow>,
        region_cursor: Option<RegionCursor>,
    ) -> bool {
        self.version == version
            && self.text == projection.mapping.text()
            && self.source_map == ProjectionSourceKey::from_projection(projection)
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
            && self.paragraph_style.base_direction() == projection.paragraph_style.base_direction()
            && self.paragraph_style.whitespace_collapse()
                == projection.paragraph_style.whitespace_collapse()
            && self.constraint == ConstraintKey::from(constraint)
            && option_ref_eq(self.region_flow.as_ref(), region_flow)
            && self.region_cursor == region_cursor
            && self.empty_line_height == projection.empty_line_height_key()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ConstraintKey {
    MinContent,
    MaxContent,
    Wrap(u64),
}

impl From<TextConstraint> for ConstraintKey {
    fn from(constraint: TextConstraint) -> Self {
        match constraint {
            TextConstraint::MinContent => Self::MinContent,
            TextConstraint::MaxContent => Self::MaxContent,
            TextConstraint::Wrap(width) => Self::Wrap(width.0.to_bits()),
        }
    }
}

#[derive(Clone, Debug)]
struct ParagraphCache {
    last_used: u64,
    formation_key: FormationKey,
    paint_runs: Vec<PaintRun>,
    region_transcript: Option<RegionTranscript>,
    geometry: CachedGeometry,
}

fn option_ref_eq<T: PartialEq>(left: Option<&T>, right: Option<&T>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => left == right,
        (None, None) => true,
        _ => false,
    }
}

fn region_output_matches(
    paragraph: &PreparedParagraph,
    transcript: &RegionTranscript,
    projection: &Projection<'_>,
) -> bool {
    if transcript.attempts().iter().any(|attempt| {
        attempt.paragraph() != paragraph.paragraph()
            || projection
                .mapping
                .text()
                .get(attempt.source().start as usize..attempt.source().end as usize)
                .is_none()
    }) {
        return false;
    }
    if paragraph.lines().is_empty() {
        let mut accepted = transcript
            .attempts()
            .iter()
            .filter(|attempt| attempt.outcome() == RegionAttemptOutcome::Accepted);
        return projection.mapping.text().is_empty()
            && accepted.next().is_some_and(|attempt| {
                attempt.source().is_empty()
                    && attempt.line_height() == projection.empty_line_height()
            })
            && accepted.next().is_none();
    }
    let mut accepted = transcript
        .attempts()
        .iter()
        .filter(|attempt| attempt.outcome() == RegionAttemptOutcome::Accepted);
    paragraph.lines().iter().all(|line| {
        accepted.next().is_some_and(|attempt| {
            line.source() == attempt.source()
                && line.slot() == Some(attempt.slot())
                && line.height() == attempt.line_height()
        })
    }) && accepted.next().is_none()
}
