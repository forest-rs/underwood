// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Persistent paragraph-scene summaries and allocation-free positioned traversal.
//!
//! This module owns document-scale scene structure. Paragraph geometry remains
//! local and immutable; traversal context supplies block origins and global
//! record ordinals without rewriting retained records.

use super::*;
use crate::RegionAttempt;
use core::mem::size_of;

// Paragraph indices are `u32`; a balanced tree over every representable
// paragraph therefore needs at most 32 pending right siblings.
const MAX_SPINE_DEPTH: usize = 32;

#[derive(Clone, Debug)]
pub(super) struct ParagraphSceneSegment {
    pub(super) paragraph: ParagraphId,
    pub(super) geometry: Arc<CachedGeometry>,
    pub(super) region_transcript: Option<RegionTranscript>,
}

impl ParagraphSceneSegment {
    pub(super) fn new(
        paragraph: ParagraphId,
        geometry: Arc<CachedGeometry>,
        region_transcript: Option<RegionTranscript>,
    ) -> Self {
        Self {
            paragraph,
            geometry,
            region_transcript,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct SceneSummary {
    pub(super) paragraphs: usize,
    pub(super) block_extent: f64,
    pub(super) lines: usize,
    pub(super) fragments: usize,
    pub(super) clusters: usize,
    pub(super) carets: usize,
    pub(super) movements: usize,
    pub(super) texts: usize,
    pub(super) semantics: usize,
    pub(super) min_x: f64,
    pub(super) max_x: f64,
    pub(super) min_y: f64,
    pub(super) max_y: f64,
    pub(super) first_baseline: Option<f64>,
    pub(super) last_baseline: Option<f64>,
    pub(super) region_start: Option<RegionCursor>,
    pub(super) region_end: Option<RegionCursor>,
    pub(super) region_attempts: usize,
    pub(super) region_height_rejections: usize,
    pub(super) region_chain_valid: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct SceneRegionBinding {
    pub(super) start: RegionCursor,
    pub(super) end: RegionCursor,
    pub(super) attempts: usize,
    pub(super) height_rejections: usize,
}

impl Default for SceneSummary {
    fn default() -> Self {
        Self {
            paragraphs: 0,
            block_extent: 0.0,
            lines: 0,
            fragments: 0,
            clusters: 0,
            carets: 0,
            movements: 0,
            texts: 0,
            semantics: 0,
            min_x: 0.0,
            max_x: 0.0,
            min_y: 0.0,
            max_y: 0.0,
            first_baseline: None,
            last_baseline: None,
            region_start: None,
            region_end: None,
            region_attempts: 0,
            region_height_rejections: 0,
            region_chain_valid: true,
        }
    }
}

impl SceneSummary {
    fn from_segment(segment: &ParagraphSceneSegment) -> Self {
        let geometry = &segment.geometry;
        let region_start = segment
            .region_transcript
            .as_ref()
            .map(RegionTranscript::start);
        let region_end = segment
            .region_transcript
            .as_ref()
            .map(RegionTranscript::end);
        let region_attempts = segment
            .region_transcript
            .as_ref()
            .map_or(0, |transcript| transcript.attempts().len());
        let region_height_rejections = segment.region_transcript.as_ref().map_or(0, |transcript| {
            transcript
                .attempts()
                .iter()
                .filter(|attempt| attempt.outcome() == RegionAttemptOutcome::HeightRejected)
                .count()
        });
        let mut min_x = 0.0_f64;
        let mut max_x = 0.0_f64;
        let mut min_y = 0.0_f64;
        let mut max_y = geometry.height;
        for line in geometry.lines.iter() {
            min_x = min_x.min(line.bounds.x0);
            max_x = max_x.max(line.bounds.x0 + line.advance);
            min_y = min_y.min(line.bounds.y0);
            max_y = max_y.max(line.bounds.y1);
        }
        Self {
            paragraphs: 1,
            block_extent: geometry.height,
            lines: geometry.lines.len(),
            fragments: geometry.fragments.len(),
            clusters: geometry.clusters.len(),
            carets: geometry.carets.len(),
            movements: geometry.movements.len(),
            texts: geometry.texts.len(),
            semantics: geometry.semantics.len(),
            min_x,
            max_x,
            min_y,
            max_y,
            first_baseline: geometry.lines.first().map(|line| line.baseline),
            last_baseline: geometry.lines.last().map(|line| line.baseline),
            region_start,
            region_end,
            region_attempts,
            region_height_rejections,
            region_chain_valid: true,
        }
    }

    fn combine(left: Self, right: Self, normal_flow: bool) -> Self {
        let right_origin = if normal_flow { left.block_extent } else { 0.0 };
        let region_chain_valid = left.region_chain_valid
            && right.region_chain_valid
            && match (
                left.region_start,
                left.region_end,
                right.region_start,
                right.region_end,
            ) {
                (None, None, None, None) => true,
                (Some(_), Some(left_end), Some(right_start), Some(_)) => left_end == right_start,
                _ => false,
            };
        Self {
            paragraphs: left.paragraphs.saturating_add(right.paragraphs),
            block_extent: if normal_flow {
                left.block_extent + right.block_extent
            } else {
                left.block_extent.max(right.block_extent)
            },
            lines: left.lines.saturating_add(right.lines),
            fragments: left.fragments.saturating_add(right.fragments),
            clusters: left.clusters.saturating_add(right.clusters),
            carets: left.carets.saturating_add(right.carets),
            movements: left.movements.saturating_add(right.movements),
            texts: left.texts.saturating_add(right.texts),
            semantics: left.semantics.saturating_add(right.semantics),
            min_x: left.min_x.min(right.min_x),
            max_x: left.max_x.max(right.max_x),
            min_y: left.min_y.min(right.min_y + right_origin),
            max_y: left.max_y.max(right.max_y + right_origin),
            first_baseline: left
                .first_baseline
                .or_else(|| right.first_baseline.map(|value| value + right_origin)),
            last_baseline: right
                .last_baseline
                .map(|value| value + right_origin)
                .or(left.last_baseline),
            region_start: left.region_start.or(right.region_start),
            region_end: right.region_end.or(left.region_end),
            region_attempts: left.region_attempts.saturating_add(right.region_attempts),
            region_height_rejections: left
                .region_height_rejections
                .saturating_add(right.region_height_rejections),
            region_chain_valid,
        }
    }
}

#[derive(Debug)]
enum SceneNode {
    Leaf {
        summary: SceneSummary,
        segment: Arc<ParagraphSceneSegment>,
    },
    Branch {
        summary: SceneSummary,
        height: u8,
        left: Arc<Self>,
        right: Arc<Self>,
    },
}

impl SceneNode {
    fn summary(&self) -> SceneSummary {
        match self {
            Self::Leaf { summary, .. } | Self::Branch { summary, .. } => *summary,
        }
    }

    fn branch(left: Arc<Self>, right: Arc<Self>, normal_flow: bool) -> Self {
        Self::Branch {
            summary: SceneSummary::combine(left.summary(), right.summary(), normal_flow),
            height: left.height().max(right.height()).saturating_add(1),
            left,
            right,
        }
    }

    fn height(&self) -> u8 {
        match self {
            Self::Leaf { .. } => 1,
            Self::Branch { height, .. } => *height,
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct SceneSpine {
    root: Option<Arc<SceneNode>>,
    normal_flow: bool,
}

impl SceneSpine {
    pub(super) fn empty(normal_flow: bool) -> Self {
        Self {
            root: None,
            normal_flow,
        }
    }

    pub(super) fn from_segments(
        segments: &[Arc<ParagraphSceneSegment>],
        normal_flow: bool,
    ) -> Self {
        Self {
            root: build_balanced(segments, normal_flow),
            normal_flow,
        }
    }

    pub(super) fn summary(&self) -> SceneSummary {
        self.root
            .as_deref()
            .map(SceneNode::summary)
            .unwrap_or_default()
    }

    pub(super) const fn is_normal_flow(&self) -> bool {
        self.normal_flow
    }

    pub(super) fn segment(&self, index: usize) -> Option<&Arc<ParagraphSceneSegment>> {
        let mut node = self.root.as_deref()?;
        let mut index = index;
        loop {
            match node {
                SceneNode::Leaf { segment, .. } => return (index == 0).then_some(segment),
                SceneNode::Branch { left, right, .. } => {
                    let left_count = left.summary().paragraphs;
                    if index < left_count {
                        node = left;
                    } else {
                        index -= left_count;
                        node = right;
                    }
                }
            }
        }
    }

    pub(super) fn positioned_segment(&self, index: usize) -> Option<PositionedSegment<'_>> {
        self.positioned_record(index, |summary| summary.paragraphs)
            .map(|(position, _)| position)
    }

    pub(super) fn positioned_segment_at_block(&self, block: f64) -> Option<PositionedSegment<'_>> {
        if !self.normal_flow {
            return None;
        }
        let root = self.root.as_deref()?;
        let block = if block.is_nan() {
            return None;
        } else {
            block.clamp(0.0, root.summary().block_extent)
        };
        let mut node = root;
        let mut local_block = block;
        let mut position = SegmentPosition::default();
        loop {
            match node {
                SceneNode::Leaf { segment, .. } => {
                    return Some(PositionedSegment { segment, position });
                }
                SceneNode::Branch { left, right, .. } => {
                    let left_summary = left.summary();
                    if local_block < left_summary.block_extent
                        || right.summary().block_extent == 0.0
                    {
                        node = left;
                    } else {
                        local_block -= left_summary.block_extent;
                        position.advance(left_summary, true);
                        node = right;
                    }
                }
            }
        }
    }

    pub(super) fn replace(
        &self,
        index: usize,
        segment: Arc<ParagraphSceneSegment>,
    ) -> Option<Self> {
        Some(Self {
            root: Some(replace_node(
                self.root.as_ref()?,
                index,
                segment,
                self.normal_flow,
            )?),
            normal_flow: self.normal_flow,
        })
    }

    pub(super) fn replace_range(
        &self,
        start: usize,
        segments: &[Arc<ParagraphSceneSegment>],
    ) -> Option<Self> {
        if segments.is_empty() {
            return (start <= self.summary().paragraphs).then(|| self.clone());
        }
        let end = start.checked_add(segments.len())?;
        (end <= self.summary().paragraphs).then_some(())?;
        Some(Self {
            root: Some(replace_node_range(
                self.root.as_ref()?,
                start,
                segments,
                self.normal_flow,
            )?),
            normal_flow: self.normal_flow,
        })
    }

    pub(super) fn append(&self, segment: Arc<ParagraphSceneSegment>) -> Self {
        let leaf = Arc::new(SceneNode::Leaf {
            summary: SceneSummary::from_segment(&segment),
            segment,
        });
        Self {
            root: Some(match &self.root {
                Some(root) => append_node(root, leaf, self.normal_flow),
                None => leaf,
            }),
            normal_flow: self.normal_flow,
        }
    }

    pub(super) fn segments(&self) -> SpineSegments<'_> {
        SpineSegments::new(self.root.as_deref(), self.normal_flow)
    }

    pub(super) fn region_attempts(&self) -> SpineRegionAttempts<'_> {
        SpineRegionAttempts::new(self)
    }

    pub(super) fn accounted_node_bytes(&self) -> usize {
        accounted_node_bytes(self.summary().paragraphs)
    }

    pub(super) fn unshared_node_bytes_from(&self, previous: Option<&Self>) -> usize {
        let Some(current) = &self.root else {
            return 0;
        };
        let Some(previous) = previous.filter(|previous| previous.normal_flow == self.normal_flow)
        else {
            return self.accounted_node_bytes();
        };
        let Some(previous) = &previous.root else {
            return self.accounted_node_bytes();
        };
        unshared_node_count(current, previous).saturating_mul(size_of::<SceneNode>())
    }

    pub(super) fn positioned_line(&self, index: usize) -> Option<PositionedLine<'_>> {
        self.positioned_record(index, |summary| summary.lines)
            .map(|(position, local)| PositionedLine { position, local })
    }

    pub(super) fn positioned_fragment(&self, index: usize) -> Option<PositionedFragment<'_>> {
        self.positioned_record(index, |summary| summary.fragments)
            .map(|(position, local)| PositionedFragment { position, local })
    }

    pub(super) fn positioned_movement(&self, index: usize) -> Option<PositionedMovement<'_>> {
        self.positioned_record(index, |summary| summary.movements)
            .map(|(position, local)| PositionedMovement {
                position,
                _local: local,
            })
    }

    pub(super) fn positioned_text(&self, index: usize) -> Option<PositionedText<'_>> {
        self.positioned_record(index, |summary| summary.texts)
            .map(|(position, local)| PositionedText { position, local })
    }

    fn positioned_record(
        &self,
        index: usize,
        count: impl Fn(SceneSummary) -> usize,
    ) -> Option<(PositionedSegment<'_>, usize)> {
        let mut node = self.root.as_deref()?;
        let mut index = index;
        let mut position = SegmentPosition::default();
        loop {
            match node {
                SceneNode::Leaf { segment, .. } => {
                    return (index < count(node.summary()))
                        .then_some((PositionedSegment { segment, position }, index));
                }
                SceneNode::Branch { left, right, .. } => {
                    let left_summary = left.summary();
                    let left_count = count(left_summary);
                    if index < left_count {
                        node = left;
                    } else {
                        index -= left_count;
                        position.advance(left_summary, self.normal_flow);
                        node = right;
                    }
                }
            }
        }
    }
}

fn accounted_node_bytes(paragraphs: usize) -> usize {
    if paragraphs == 0 {
        return 0;
    }
    size_of::<SceneNode>().saturating_mul(paragraphs.saturating_mul(2).saturating_sub(1))
}

fn unshared_node_count(current: &Arc<SceneNode>, previous: &Arc<SceneNode>) -> usize {
    if Arc::ptr_eq(current, previous) {
        return 0;
    }
    match (current.as_ref(), previous.as_ref()) {
        (
            SceneNode::Branch {
                left: current_left,
                right: current_right,
                ..
            },
            SceneNode::Branch {
                left: previous_left,
                right: previous_right,
                ..
            },
        ) => 1_usize
            .saturating_add(unshared_node_count(current_left, previous_left))
            .saturating_add(unshared_node_count(current_right, previous_right)),
        (SceneNode::Leaf { .. }, SceneNode::Leaf { .. }) => 1,
        _ => current
            .summary()
            .paragraphs
            .saturating_mul(2)
            .saturating_sub(1),
    }
}

fn build_balanced(
    segments: &[Arc<ParagraphSceneSegment>],
    normal_flow: bool,
) -> Option<Arc<SceneNode>> {
    match segments {
        [] => None,
        [segment] => Some(Arc::new(SceneNode::Leaf {
            summary: SceneSummary::from_segment(segment),
            segment: Arc::clone(segment),
        })),
        _ => {
            let middle = segments.len() / 2;
            let left = build_balanced(&segments[..middle], normal_flow)?;
            let right = build_balanced(&segments[middle..], normal_flow)?;
            Some(Arc::new(SceneNode::branch(left, right, normal_flow)))
        }
    }
}

fn replace_node(
    node: &Arc<SceneNode>,
    index: usize,
    segment: Arc<ParagraphSceneSegment>,
    normal_flow: bool,
) -> Option<Arc<SceneNode>> {
    match node.as_ref() {
        SceneNode::Leaf { .. } => (index == 0).then(|| {
            Arc::new(SceneNode::Leaf {
                summary: SceneSummary::from_segment(&segment),
                segment,
            })
        }),
        SceneNode::Branch { left, right, .. } => {
            let left_count = left.summary().paragraphs;
            if index < left_count {
                let replacement = replace_node(left, index, segment, normal_flow)?;
                Some(Arc::new(SceneNode::branch(
                    replacement,
                    Arc::clone(right),
                    normal_flow,
                )))
            } else {
                let replacement = replace_node(right, index - left_count, segment, normal_flow)?;
                Some(Arc::new(SceneNode::branch(
                    Arc::clone(left),
                    replacement,
                    normal_flow,
                )))
            }
        }
    }
}

fn replace_node_range(
    node: &Arc<SceneNode>,
    index: usize,
    segments: &[Arc<ParagraphSceneSegment>],
    normal_flow: bool,
) -> Option<Arc<SceneNode>> {
    match node.as_ref() {
        SceneNode::Leaf { .. } => (index == 0 && segments.len() == 1).then(|| {
            let segment = Arc::clone(&segments[0]);
            Arc::new(SceneNode::Leaf {
                summary: SceneSummary::from_segment(&segment),
                segment,
            })
        }),
        SceneNode::Branch { left, right, .. } => {
            let left_count = left.summary().paragraphs;
            if index >= left_count {
                let right = replace_node_range(right, index - left_count, segments, normal_flow)?;
                return Some(Arc::new(SceneNode::branch(
                    Arc::clone(left),
                    right,
                    normal_flow,
                )));
            }

            let left_replacements = segments.len().min(left_count - index);
            let left =
                replace_node_range(left, index, &segments[..left_replacements], normal_flow)?;
            let right = if left_replacements == segments.len() {
                Arc::clone(right)
            } else {
                replace_node_range(right, 0, &segments[left_replacements..], normal_flow)?
            };
            Some(Arc::new(SceneNode::branch(left, right, normal_flow)))
        }
    }
}

fn append_node(node: &Arc<SceneNode>, leaf: Arc<SceneNode>, normal_flow: bool) -> Arc<SceneNode> {
    match node.as_ref() {
        SceneNode::Leaf { .. } => Arc::new(SceneNode::branch(Arc::clone(node), leaf, normal_flow)),
        SceneNode::Branch { left, right, .. } => {
            let right = append_node(right, leaf, normal_flow);
            balance_right(Arc::clone(left), right, normal_flow)
        }
    }
}

fn balance_right(left: Arc<SceneNode>, right: Arc<SceneNode>, normal_flow: bool) -> Arc<SceneNode> {
    if right.height() <= left.height().saturating_add(1) {
        return Arc::new(SceneNode::branch(left, right, normal_flow));
    }
    let SceneNode::Branch {
        left: middle,
        right: outer,
        ..
    } = right.as_ref()
    else {
        unreachable!("a leaf cannot exceed its sibling by two levels")
    };
    if outer.height() >= middle.height() {
        Arc::new(SceneNode::branch(
            Arc::new(SceneNode::branch(left, Arc::clone(middle), normal_flow)),
            Arc::clone(outer),
            normal_flow,
        ))
    } else {
        let SceneNode::Branch {
            left: middle_left,
            right: middle_right,
            ..
        } = middle.as_ref()
        else {
            unreachable!("an inner leaf cannot make the right branch left-heavy")
        };
        Arc::new(SceneNode::branch(
            Arc::new(SceneNode::branch(
                left,
                Arc::clone(middle_left),
                normal_flow,
            )),
            Arc::new(SceneNode::branch(
                Arc::clone(middle_right),
                Arc::clone(outer),
                normal_flow,
            )),
            normal_flow,
        ))
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct SegmentPosition {
    pub(super) block_origin: f64,
    pub(super) paragraph_base: usize,
    pub(super) line_base: usize,
    pub(super) fragment_base: usize,
    pub(super) cluster_base: usize,
    pub(super) caret_base: usize,
    pub(super) movement_base: usize,
    pub(super) text_base: usize,
    pub(super) semantic_base: usize,
}

impl SegmentPosition {
    fn advance(&mut self, summary: SceneSummary, normal_flow: bool) {
        if normal_flow {
            self.block_origin += summary.block_extent;
        }
        self.paragraph_base = self.paragraph_base.saturating_add(summary.paragraphs);
        self.line_base = self.line_base.saturating_add(summary.lines);
        self.fragment_base = self.fragment_base.saturating_add(summary.fragments);
        self.cluster_base = self.cluster_base.saturating_add(summary.clusters);
        self.caret_base = self.caret_base.saturating_add(summary.carets);
        self.movement_base = self.movement_base.saturating_add(summary.movements);
        self.text_base = self.text_base.saturating_add(summary.texts);
        self.semantic_base = self.semantic_base.saturating_add(summary.semantics);
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct PositionedSegment<'a> {
    pub(super) segment: &'a ParagraphSceneSegment,
    pub(super) position: SegmentPosition,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct PositionedLine<'a> {
    pub(super) position: PositionedSegment<'a>,
    pub(super) local: usize,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct PositionedFragment<'a> {
    pub(super) position: PositionedSegment<'a>,
    pub(super) local: usize,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct PositionedMovement<'a> {
    pub(super) position: PositionedSegment<'a>,
    pub(super) _local: usize,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct PositionedText<'a> {
    pub(super) position: PositionedSegment<'a>,
    pub(super) local: usize,
}

#[derive(Clone, Debug)]
pub(super) struct SpineSegments<'a> {
    stack: [Option<&'a SceneNode>; MAX_SPINE_DEPTH],
    len: usize,
    position: SegmentPosition,
    normal_flow: bool,
}

impl<'a> SpineSegments<'a> {
    fn new(root: Option<&'a SceneNode>, normal_flow: bool) -> Self {
        let mut traversal = Self {
            stack: [None; MAX_SPINE_DEPTH],
            len: 0,
            position: SegmentPosition::default(),
            normal_flow,
        };
        if let Some(root) = root {
            traversal.push(root);
        }
        traversal
    }

    fn push(&mut self, node: &'a SceneNode) {
        assert!(
            self.len < MAX_SPINE_DEPTH,
            "u32-bounded paragraph spine exceeded its traversal depth"
        );
        self.stack[self.len] = Some(node);
        self.len += 1;
    }

    fn pop(&mut self) -> Option<&'a SceneNode> {
        self.len = self.len.checked_sub(1)?;
        self.stack[self.len].take()
    }
}

impl<'a> Iterator for SpineSegments<'a> {
    type Item = PositionedSegment<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(node) = self.pop() {
            match node {
                SceneNode::Leaf { summary, segment } => {
                    let positioned = PositionedSegment {
                        segment,
                        position: self.position,
                    };
                    self.position.advance(*summary, self.normal_flow);
                    return Some(positioned);
                }
                SceneNode::Branch { left, right, .. } => {
                    self.push(right);
                    self.push(left);
                }
            }
        }
        None
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.stack[..self.len]
            .iter()
            .flatten()
            .map(|node| node.summary().paragraphs)
            .sum();
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for SpineSegments<'_> {}

#[derive(Clone, Debug)]
pub(super) struct SpineRegionAttempts<'a> {
    segments: SpineSegments<'a>,
    current: Option<core::slice::Iter<'a, RegionAttempt>>,
    remaining: usize,
}

impl<'a> SpineRegionAttempts<'a> {
    fn new(spine: &'a SceneSpine) -> Self {
        Self {
            segments: spine.segments(),
            current: None,
            remaining: spine.summary().region_attempts,
        }
    }
}

impl<'a> Iterator for SpineRegionAttempts<'a> {
    type Item = &'a RegionAttempt;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(attempt) = self.current.as_mut().and_then(Iterator::next) {
                self.remaining = self.remaining.saturating_sub(1);
                return Some(attempt);
            }
            let positioned = self.segments.next()?;
            self.current = Some(
                positioned
                    .segment
                    .region_transcript
                    .as_ref()
                    .map_or([].as_slice(), RegionTranscript::attempts)
                    .iter(),
            );
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

impl ExactSizeIterator for SpineRegionAttempts<'_> {}

#[cfg(test)]
mod tests {
    use super::*;

    fn segment(paragraph: u32, height: f64) -> Arc<ParagraphSceneSegment> {
        let document = crate::DocumentId::from_bytes(*b"scene-spine-test");
        Arc::new(ParagraphSceneSegment::new(
            ParagraphId {
                document,
                index: paragraph,
            },
            Arc::new(CachedGeometry {
                features: SceneFeatures::DISPLAY,
                facts: Arc::new(CachedGeometryFacts {
                    height,
                    lines: Vec::new(),
                }),
                line_fragments: Vec::new(),
                fragments: Vec::new(),
                line_sources: CachedSidecar::new(false, Vec::new()),
                clusters: CachedSidecar::new(false, Vec::new()),
                carets: CachedSidecar::new(false, Vec::new()),
                movements: CachedSidecar::new(false, Vec::new()),
                texts: CachedSidecar::new(false, Vec::new()),
                semantics: CachedSidecar::new(false, Vec::new()),
            }),
            None,
        ))
    }

    #[test]
    fn replacement_shares_siblings_and_updates_prefix_summaries() {
        let segments = [segment(0, 10.0), segment(1, 20.0), segment(2, 30.0)];
        let spine = SceneSpine::from_segments(&segments, true);
        let replacement = segment(1, 25.0);
        let changed = spine
            .replace(1, Arc::clone(&replacement))
            .expect("paragraph exists");

        assert!(Arc::ptr_eq(
            spine.segment(0).expect("first segment exists"),
            changed.segment(0).expect("first segment remains")
        ));
        assert!(Arc::ptr_eq(
            spine.segment(2).expect("last segment exists"),
            changed.segment(2).expect("last segment remains")
        ));
        assert!(Arc::ptr_eq(
            changed.segment(1).expect("replacement exists"),
            &replacement
        ));
        let origins: Vec<_> = changed
            .segments()
            .map(|positioned| positioned.position.block_origin)
            .collect();
        assert_eq!(origins, [0.0, 10.0, 35.0]);
        assert_eq!(changed.summary().block_extent, 65.0);
        assert_eq!(
            spine.accounted_node_bytes(),
            5 * size_of::<SceneNode>(),
            "three paragraphs retain three leaves and two branch nodes"
        );
        assert_eq!(
            spine.unshared_node_bytes_from(Some(&spine)),
            0,
            "an exact spine retains no new nodes"
        );
        assert_eq!(
            changed.unshared_node_bytes_from(Some(&spine)),
            3 * size_of::<SceneNode>(),
            "replacing the middle paragraph retains one new leaf and its two-node root path"
        );
    }

    #[test]
    fn range_replacement_copies_each_covered_path_once() {
        let segments: Vec<_> = (0..64).map(|paragraph| segment(paragraph, 10.0)).collect();
        let spine = SceneSpine::from_segments(&segments, false);
        let replacements: Vec<_> = (20..28).map(|paragraph| segment(paragraph, 20.0)).collect();
        let changed = spine
            .replace_range(20, &replacements)
            .expect("range is represented");

        assert!(Arc::ptr_eq(
            spine.segment(19).expect("prefix exists"),
            changed.segment(19).expect("prefix remains")
        ));
        assert!(Arc::ptr_eq(
            spine.segment(28).expect("suffix exists"),
            changed.segment(28).expect("suffix remains")
        ));
        for (offset, replacement) in replacements.iter().enumerate() {
            assert!(Arc::ptr_eq(
                changed.segment(20 + offset).expect("replacement exists"),
                replacement
            ));
        }
        assert!(
            changed.unshared_node_bytes_from(Some(&spine))
                < replacements
                    .len()
                    .saturating_mul(MAX_SPINE_DEPTH)
                    .saturating_mul(size_of::<SceneNode>()),
            "one range update must share paths instead of applying independent replacements"
        );
    }

    #[test]
    fn append_remains_balanced_and_shares_the_complete_prefix() {
        let initial: Vec<_> = (0..1_024)
            .map(|paragraph| segment(paragraph, 10.0))
            .collect();
        let spine = SceneSpine::from_segments(&initial, true);
        let appended = segment(1_024, 20.0);
        let changed = spine.append(Arc::clone(&appended));

        assert!(Arc::ptr_eq(
            spine.segment(511).expect("old prefix exists"),
            changed.segment(511).expect("old prefix remains")
        ));
        assert!(Arc::ptr_eq(
            changed.segment(1_024).expect("appended segment exists"),
            &appended
        ));
        assert_eq!(changed.summary().paragraphs, 1_025);
        assert_eq!(changed.summary().block_extent, 10_260.0);
        assert!(
            changed.root.as_deref().expect("root exists").height() <= 12,
            "sequential append must preserve logarithmic depth"
        );
    }
}
