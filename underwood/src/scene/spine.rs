// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Persistent paragraph-scene summaries and allocation-free positioned traversal.
//!
//! This module owns document-scale scene structure. Paragraph geometry remains
//! local and immutable; traversal context supplies block origins and global
//! record ordinals without rewriting retained records.

use super::*;
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

#[derive(Clone, Copy, Debug, Default)]
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
}

impl SceneSummary {
    fn from_segment(segment: &ParagraphSceneSegment) -> Self {
        let geometry = &segment.geometry;
        let mut min_x = 0.0_f64;
        let mut max_x = 0.0_f64;
        let mut min_y = 0.0_f64;
        let mut max_y = geometry.height;
        for line in &geometry.lines {
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
        }
    }

    fn combine(left: Self, right: Self, normal_flow: bool) -> Self {
        let right_origin = if normal_flow { left.block_extent } else { 0.0 };
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
            left,
            right,
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

    pub(super) fn segments(&self) -> SpineSegments<'_> {
        SpineSegments::new(self.root.as_deref(), self.normal_flow)
    }

    pub(super) fn accounted_node_bytes(&self) -> usize {
        self.root.as_deref().map_or(0, accounted_node_bytes)
    }

    pub(super) fn positioned_line(&self, index: usize) -> Option<PositionedLine<'_>> {
        self.positioned_record(index, |summary| summary.lines)
            .map(|(position, local)| PositionedLine { position, local })
    }

    pub(super) fn positioned_fragment(&self, index: usize) -> Option<PositionedFragment<'_>> {
        self.positioned_record(index, |summary| summary.fragments)
            .map(|(position, local)| PositionedFragment { position, local })
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

fn accounted_node_bytes(node: &SceneNode) -> usize {
    size_of::<SceneNode>().saturating_add(match node {
        SceneNode::Leaf { .. } => 0,
        SceneNode::Branch { left, right, .. } => {
            accounted_node_bytes(left).saturating_add(accounted_node_bytes(right))
        }
    })
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
                height,
                lines: Vec::new(),
                fragments: Vec::new(),
                clusters: Vec::new(),
                carets: Vec::new(),
                movements: Vec::new(),
                texts: Vec::new(),
                semantics: Vec::new(),
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
    }
}
