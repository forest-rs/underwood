// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Paragraph-local source provenance without per-observation range vectors.
//!
//! This module owns the authoritative projection-to-leaf relation retained by
//! one paragraph scene. Geometry records keep compact projected spans and
//! positions; borrowed iterators materialize revision-bound observations only
//! at the public boundary.

use super::*;
use core::mem::size_of;

/// One compact range in projected paragraph UTF-8 coordinates.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct SourceSpan {
    pub(super) start: u32,
    pub(super) end: u32,
}

impl SourceSpan {
    pub(super) const fn new(start: u32, end: u32) -> Self {
        Self { start, end }
    }

    pub(super) const fn is_empty(self) -> bool {
        self.start == self.end
    }
}

impl From<Range<u32>> for SourceSpan {
    fn from(range: Range<u32>) -> Self {
        Self::new(range.start, range.end)
    }
}

/// One compact position in projected paragraph UTF-8 coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct SourcePosition {
    pub(super) offset: u32,
    pub(super) affinity: TextAffinity,
}

impl SourcePosition {
    pub(super) const fn new(offset: u32, affinity: TextAffinity) -> Self {
        Self { offset, affinity }
    }
}

/// Source represented by a retained record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SourceReference {
    /// One projected paragraph interval mapped through the relation runs.
    Projected(SourceSpan),
    /// One complete semantic leaf stored in the source map.
    Leaf(u32),
}

#[derive(Clone, Debug)]
pub(super) struct ParagraphSourceMap {
    source_len: u32,
    projected_len: u32,
    identity: bool,
    relations: Vec<SourceRelation>,
    leaves: Vec<SourceLeaf>,
}

#[derive(Clone, Copy, Debug)]
struct SourceRelation {
    kind: ProjectionKind,
    source: SourceSpan,
    projected: SourceSpan,
}

#[derive(Clone, Debug)]
struct SourceLeaf {
    paragraph: SourceSpan,
    text: TextId,
    semantic: SemanticId,
    source: SourceLeafKind,
}

#[derive(Clone, Copy, Debug)]
enum SourceLeafKind {
    Snapshot {
        start: u32,
    },
    Composition {
        id: CompositionId,
        epoch: crate::CompositionEpoch,
        start: u32,
    },
}

impl ParagraphSourceMap {
    pub(super) fn from_projection(projection: &Projection) -> Result<Self, SceneError> {
        let source_len = u32::try_from(projection.mapping.source_text().len()).map_err(|_| {
            SceneError::for_paragraph(SceneErrorKind::SourceCoverage, projection.paragraph)
        })?;
        let projected_len = u32::try_from(projection.mapping.text().len()).map_err(|_| {
            SceneError::for_paragraph(SceneErrorKind::SourceCoverage, projection.paragraph)
        })?;
        let identity = projection.mapping.is_identity();
        let relations = if identity {
            Vec::new()
        } else {
            projection
                .mapping
                .segments()
                .map(|relation| SourceRelation {
                    kind: relation.kind(),
                    source: relation.source().into(),
                    projected: relation.projected().into(),
                })
                .collect()
        };
        let mut covered = 0_u32;
        let mut leaves = Vec::with_capacity(projection.spans.len());
        for span in &projection.spans {
            if span.paragraph.start != covered || span.paragraph.end < span.paragraph.start {
                return Err(SceneError::for_paragraph(
                    SceneErrorKind::SourceCoverage,
                    projection.paragraph,
                ));
            }
            leaves.push(SourceLeaf {
                paragraph: span.paragraph.clone().into(),
                text: span.text,
                semantic: span.semantic,
                source: match span.source {
                    LeafSpanSource::Snapshot { start } => SourceLeafKind::Snapshot { start },
                    LeafSpanSource::Composition { id, epoch, start } => {
                        SourceLeafKind::Composition { id, epoch, start }
                    }
                },
            });
            covered = span.paragraph.end;
        }
        if covered != source_len || leaves.is_empty() && source_len != 0 {
            return Err(SceneError::for_paragraph(
                SceneErrorKind::SourceCoverage,
                projection.paragraph,
            ));
        }
        Ok(Self {
            source_len,
            projected_len,
            identity,
            relations,
            leaves,
        })
    }

    pub(super) fn span(
        &self,
        range: Range<u32>,
        paragraph: ParagraphId,
    ) -> Result<SourceSpan, SceneError> {
        if range.start > range.end || range.end > self.projected_len {
            return Err(SceneError::for_source(
                SceneErrorKind::SourceCoverage,
                paragraph,
                range,
            ));
        }
        Ok(range.into())
    }

    pub(super) fn ranges(&self, reference: SourceReference) -> LocalRanges<'_> {
        match reference {
            SourceReference::Projected(span) => self.ranges_for_span(span),
            SourceReference::Leaf(index) => self.ranges_for_leaf(index),
        }
    }

    pub(super) fn ranges_for_span(&self, span: SourceSpan) -> LocalRanges<'_> {
        let source = self.source_range(span);
        let indices = self.leaf_indices_for_source(source);
        LocalRanges::new(self, source, indices.start, indices.end)
    }

    pub(super) fn leaf_indices_for_span(&self, span: SourceSpan) -> Range<usize> {
        self.leaf_indices_for_source(self.source_range(span))
    }

    pub(super) fn semantic_for_span(&self, span: SourceSpan) -> Option<SemanticId> {
        let owner_affinity = if span.is_empty() {
            TextAffinity::Upstream
        } else {
            TextAffinity::Downstream
        };
        let owner = self.source_position(span.start, owner_affinity);
        let transformed = !span.is_empty()
            && self.relations.iter().any(|relation| {
                relation.kind != ProjectionKind::Identity
                    && !relation.projected.is_empty()
                    && relation.projected.start <= span.start
                    && span.end <= relation.projected.end
            });
        if span.is_empty() || transformed {
            return self
                .leaf_for_position(owner, TextAffinity::Downstream)
                .or_else(|| self.leaf_for_position(owner, TextAffinity::Upstream))
                .map(|index| self.leaves[index].semantic);
        }

        let source = self.source_range(span);
        let indices = self.leaf_indices_for_source(source);
        let first = self.leaves.get(indices.start)?.semantic;
        self.leaves[indices]
            .iter()
            .all(|leaf| leaf.semantic == first)
            .then_some(first)
    }

    pub(super) fn ranges_for_leaf(&self, index: u32) -> LocalRanges<'_> {
        let index = usize::try_from(index).unwrap_or(usize::MAX);
        let Some(leaf) = self.leaves.get(index) else {
            return LocalRanges::new(self, SourceSpan::default(), 0, 0);
        };
        LocalRanges::new(self, leaf.paragraph, index, index + 1)
    }

    pub(super) fn materialize_position(&self, position: SourcePosition) -> LocalPosition {
        let source = self.source_position(position.offset, position.affinity);
        self.leaves[self
            .leaf_for_position(source, position.affinity)
            .expect("validated retained source positions keep a source leaf")]
        .local_position(source, position.affinity)
    }

    pub(super) fn source_position_for_local(
        &self,
        position: LocalPosition,
    ) -> Option<SourcePosition> {
        let affinity = match position {
            LocalPosition::Snapshot { affinity, .. }
            | LocalPosition::Composition { affinity, .. } => affinity,
        };
        let find = |leaf: &SourceLeaf| {
            let start = match (leaf.source, position) {
                (
                    SourceLeafKind::Snapshot { start },
                    LocalPosition::Snapshot { text, byte, .. },
                ) if leaf.text == text => start_and_relative_byte(leaf, start, byte),
                (
                    SourceLeafKind::Composition { id, epoch, start },
                    LocalPosition::Composition {
                        id: position_id,
                        epoch: position_epoch,
                        byte,
                        ..
                    },
                ) if id == position_id && epoch == position_epoch => {
                    start_and_relative_byte(leaf, start, byte)
                }
                _ => None,
            }?;
            Some(SourcePosition::new(
                self.projected_position(start, affinity),
                affinity,
            ))
        };
        match affinity {
            TextAffinity::Upstream => self.leaves.iter().rev().find_map(find),
            TextAffinity::Downstream => self.leaves.iter().find_map(find),
        }
    }

    pub(super) fn leaf_count(&self) -> usize {
        self.leaves.len()
    }

    pub(super) fn leaf_index_for_text(&self, text: TextId) -> Option<usize> {
        self.leaves.iter().position(|leaf| leaf.text == text)
    }

    pub(super) fn leaf_range(&self, index: usize) -> Option<LocalRange> {
        self.leaves
            .get(index)
            .map(|leaf| leaf.local_range(leaf.paragraph))
    }

    pub(super) fn rebind_composition(&mut self, id: CompositionId, epoch: crate::CompositionEpoch) {
        for leaf in &mut self.leaves {
            if let SourceLeafKind::Composition {
                id: leaf_id,
                epoch: leaf_epoch,
                ..
            } = &mut leaf.source
            {
                *leaf_id = id;
                *leaf_epoch = epoch;
            }
        }
    }

    pub(super) fn accounted_owned_bytes(&self) -> usize {
        size_of::<SourceRelation>()
            .saturating_mul(self.relations.capacity())
            .saturating_add(size_of::<SourceLeaf>().saturating_mul(self.leaves.capacity()))
    }

    fn source_range(&self, projected: SourceSpan) -> SourceSpan {
        if projected.is_empty() {
            let source = self.source_position(projected.start, TextAffinity::Upstream);
            return SourceSpan::new(source, source);
        }
        let start = self.source_position(projected.start, TextAffinity::Downstream);
        let end = self.source_position(projected.end, TextAffinity::Upstream);
        SourceSpan::new(start.min(end), start.max(end))
    }

    fn leaf_indices_for_source(&self, source: SourceSpan) -> Range<usize> {
        if source.is_empty() {
            let index = self
                .leaf_for_position(source.start, TextAffinity::Upstream)
                .or_else(|| self.leaf_for_position(source.start, TextAffinity::Downstream));
            return index.map_or(0..0, |index| index..index + 1);
        }
        let front = self
            .leaves
            .partition_point(|leaf| leaf.paragraph.end <= source.start);
        let back = self
            .leaves
            .partition_point(|leaf| leaf.paragraph.start < source.end);
        front..back
    }

    fn source_position(&self, projected: u32, affinity: TextAffinity) -> u32 {
        if self.identity {
            return projected;
        }
        if let Some(relation) = self.relations.iter().find(|relation| {
            relation.projected.start < projected && projected < relation.projected.end
        }) {
            return match relation.kind {
                ProjectionKind::Identity => {
                    relation.source.start + (projected - relation.projected.start)
                }
                ProjectionKind::Replacement | ProjectionKind::Collapsed => match affinity {
                    TextAffinity::Upstream => relation.source.end,
                    TextAffinity::Downstream => relation.source.start,
                },
                ProjectionKind::Inserted => relation.source.start,
                ProjectionKind::Omitted => unreachable!("omitted runs have no interior"),
            };
        }
        match affinity {
            TextAffinity::Upstream => self
                .relations
                .iter()
                .rev()
                .find(|relation| {
                    !relation.projected.is_empty() && relation.projected.end <= projected
                })
                .map_or(0, |relation| relation.source.end),
            TextAffinity::Downstream => self
                .relations
                .iter()
                .find(|relation| {
                    !relation.projected.is_empty() && relation.projected.start >= projected
                })
                .map_or(self.source_len, |relation| relation.source.start),
        }
    }

    fn projected_position(&self, source: u32, affinity: TextAffinity) -> u32 {
        if self.identity {
            return source;
        }
        let mut inserted = self.relations.iter().filter(|relation| {
            relation.kind == ProjectionKind::Inserted && relation.source.start == source
        });
        if let Some(first) = inserted.next() {
            return inserted.fold(
                match affinity {
                    TextAffinity::Upstream => first.projected.start,
                    TextAffinity::Downstream => first.projected.end,
                },
                |position, relation| match affinity {
                    TextAffinity::Upstream => position.min(relation.projected.start),
                    TextAffinity::Downstream => position.max(relation.projected.end),
                },
            );
        }
        if let Some(relation) = self
            .relations
            .iter()
            .find(|relation| relation.source.start < source && source < relation.source.end)
        {
            return match relation.kind {
                ProjectionKind::Identity => {
                    relation.projected.start + (source - relation.source.start)
                }
                ProjectionKind::Replacement | ProjectionKind::Collapsed => match affinity {
                    TextAffinity::Upstream => relation.projected.end,
                    TextAffinity::Downstream => relation.projected.start,
                },
                ProjectionKind::Omitted => relation.projected.start,
                ProjectionKind::Inserted => unreachable!("inserted runs have no source interior"),
            };
        }
        match affinity {
            TextAffinity::Upstream => self
                .relations
                .iter()
                .rev()
                .find(|relation| !relation.source.is_empty() && relation.source.end <= source)
                .map_or(0, |relation| relation.projected.end),
            TextAffinity::Downstream => self
                .relations
                .iter()
                .find(|relation| !relation.source.is_empty() && relation.source.start >= source)
                .map_or(self.projected_len, |relation| relation.projected.start),
        }
    }

    fn leaf_for_position(&self, source: u32, affinity: TextAffinity) -> Option<usize> {
        let match_leaf = |leaf: &SourceLeaf| match affinity {
            TextAffinity::Upstream => {
                (leaf.paragraph.start < source && source <= leaf.paragraph.end)
                    || (leaf.paragraph.is_empty() && leaf.paragraph.end == source)
            }
            TextAffinity::Downstream => {
                (leaf.paragraph.start <= source && source < leaf.paragraph.end)
                    || (leaf.paragraph.is_empty() && leaf.paragraph.start == source)
            }
        };
        match affinity {
            TextAffinity::Upstream => self.leaves.iter().rposition(match_leaf),
            TextAffinity::Downstream => self.leaves.iter().position(match_leaf),
        }
        .or_else(|| {
            self.leaves
                .iter()
                .position(|leaf| leaf.paragraph.start <= source && source <= leaf.paragraph.end)
        })
    }
}

impl SourceLeaf {
    fn local_range(&self, source: SourceSpan) -> LocalRange {
        let relative_start = source.start.saturating_sub(self.paragraph.start);
        let relative_end = source.end.saturating_sub(self.paragraph.start);
        match self.source {
            SourceLeafKind::Snapshot { start } => LocalRange::Snapshot {
                text: self.text,
                bytes: (start + relative_start)..(start + relative_end),
            },
            SourceLeafKind::Composition { id, epoch, start } => LocalRange::Composition {
                id,
                epoch,
                bytes: (start + relative_start)..(start + relative_end),
            },
        }
    }

    fn local_position(&self, source: u32, affinity: TextAffinity) -> LocalPosition {
        let relative = source.saturating_sub(self.paragraph.start);
        match self.source {
            SourceLeafKind::Snapshot { start } => LocalPosition::Snapshot {
                text: self.text,
                byte: start + relative,
                affinity,
            },
            SourceLeafKind::Composition { id, epoch, start } => LocalPosition::Composition {
                id,
                epoch,
                byte: start + relative,
                affinity,
            },
        }
    }
}

fn start_and_relative_byte(leaf: &SourceLeaf, start: u32, byte: u32) -> Option<u32> {
    let relative = byte.checked_sub(start)?;
    (relative <= leaf.paragraph.end - leaf.paragraph.start)
        .then_some(leaf.paragraph.start + relative)
}

/// Borrowed exact-size iterator over one source observation.
#[derive(Clone, Debug)]
pub(super) struct LocalRanges<'a> {
    map: &'a ParagraphSourceMap,
    source: SourceSpan,
    front: usize,
    back: usize,
}

impl<'a> LocalRanges<'a> {
    const fn new(
        map: &'a ParagraphSourceMap,
        source: SourceSpan,
        front: usize,
        back: usize,
    ) -> Self {
        Self {
            map,
            source,
            front,
            back,
        }
    }

    fn range_for(&self, index: usize) -> LocalRange {
        let leaf = &self.map.leaves[index];
        let start = self.source.start.max(leaf.paragraph.start);
        let end = self.source.end.min(leaf.paragraph.end);
        leaf.local_range(SourceSpan::new(start, end))
    }
}

impl Iterator for LocalRanges<'_> {
    type Item = LocalRange;

    fn next(&mut self) -> Option<Self::Item> {
        if self.front == self.back {
            return None;
        }
        let index = self.front;
        self.front += 1;
        Some(self.range_for(index))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.back - self.front;
        (remaining, Some(remaining))
    }
}

impl DoubleEndedIterator for LocalRanges<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.front == self.back {
            return None;
        }
        self.back -= 1;
        Some(self.range_for(self.back))
    }
}

impl ExactSizeIterator for LocalRanges<'_> {}
