// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Snapshot-independent cached geometry and source-aware scene lowering.
//!
//! This module owns conversion from prepared paragraphs to reusable geometry;
//! it explicitly does not own shaping or public scene interaction policy.

use super::*;
use crate::adapter::{ClusterBoundary, ClusterWhitespace};
use core::{mem::size_of, ops::Deref};

#[derive(Clone, Debug)]
pub(super) struct CachedGeometry {
    pub(super) features: SceneFeatures,
    pub(super) artifact: Arc<PreparedParagraphFacts>,
    pub(super) facts: Arc<CachedGeometryFacts>,
    pub(super) source_map: Option<Arc<ParagraphSourceMap>>,
    pub(super) hit_geometry: CachedHitSidecar,
    pub(super) semantics: CachedSidecar<CachedSemantic>,
}

#[derive(Clone, Debug)]
pub(super) struct CachedGeometryFacts {
    pub(super) height: f64,
    pub(super) empty_bounds: Rect,
    pub(super) lines: Vec<CachedLine>,
    pub(super) glyphs: Vec<CachedGlyph>,
}

#[derive(Clone, Debug)]
pub(super) struct CachedSidecar<T> {
    records: Option<Arc<Vec<T>>>,
}

impl<T> CachedSidecar<T> {
    pub(super) fn new(retain: bool, records: Vec<T>) -> Self {
        debug_assert!(
            retain || records.is_empty(),
            "discarded sidecars must not be built"
        );
        Self {
            records: retain.then(|| Arc::new(records)),
        }
    }

    fn capacity(&self) -> usize {
        self.records
            .as_ref()
            .map_or(0, |records| records.capacity())
    }

    fn retain_from(&mut self, retained: &Self) {
        if self.records.is_none() {
            self.records.clone_from(&retained.records);
        }
    }
}

impl<T> Deref for CachedSidecar<T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        self.records.as_deref().map_or(&[], Vec::as_slice)
    }
}

impl<'a, T> IntoIterator for &'a CachedSidecar<T> {
    type Item = &'a T;
    type IntoIter = core::slice::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

#[derive(Clone, Debug)]
pub(super) struct CachedHitSidecar {
    records: Option<Arc<CachedHitGeometry>>,
}

#[derive(Clone, Debug)]
struct CachedHitGeometry {
    placements: Vec<CachedHitPlacement>,
}

impl CachedHitSidecar {
    pub(super) fn new(retain: bool, placements: Vec<CachedHitPlacement>) -> Self {
        debug_assert!(
            retain || placements.is_empty(),
            "discarded hit geometry must not be built"
        );
        debug_assert!(
            placements.windows(2).all(|pair| {
                pair[0].line < pair[1].line
                    || pair[0].line == pair[1].line && pair[0].inline_end <= pair[1].inline_end
            }),
            "retained hit placements must remain ordered by line and visual inline end"
        );
        Self {
            records: retain.then(|| Arc::new(CachedHitGeometry { placements })),
        }
    }

    fn capacity(&self) -> usize {
        self.records
            .as_ref()
            .map_or(0, |records| records.placements.capacity())
    }

    pub(super) fn len(&self) -> usize {
        self.records
            .as_ref()
            .map_or(0, |records| records.placements.len())
    }

    #[cfg(test)]
    pub(super) fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn retain_from(&mut self, retained: &Self) {
        if self.records.is_none() {
            self.records.clone_from(&retained.records);
        }
    }

    fn placements(&self) -> &[CachedHitPlacement] {
        self.records
            .as_deref()
            .map_or(&[], |records| records.placements.as_slice())
    }

    fn placements_for_line(&self, line: u32) -> &[CachedHitPlacement] {
        let placements = self.placements();
        let start = placements.partition_point(|placement| placement.line < line);
        let end = placements.partition_point(|placement| placement.line <= line);
        &placements[start..end]
    }
}

impl Deref for CachedGeometry {
    type Target = CachedGeometryFacts;

    fn deref(&self) -> &Self::Target {
        &self.facts
    }
}

#[derive(Clone, Debug)]
pub(super) struct CachedLine {
    pub(super) bounds: Rect,
    pub(super) adjustment: LineAdjustment,
}

#[derive(Clone, Debug)]
pub(super) struct CachedFragment {
    pub(super) line: u32,
    pub(super) run: u32,
    pub(super) glyphs: Range<u32>,
    pub(super) instance_start: usize,
    pub(super) segment: u32,
    pub(super) paint: PaintSlot,
    pub(super) paint_clip: Option<Rect>,
}

#[derive(Clone, Debug)]
pub(super) struct CachedGlyph {
    pub(super) position: Point,
    pub(super) inline_advance_adjustment: f64,
}

#[derive(Clone, Debug)]
pub(super) struct PaintTopology {
    pub(super) fragments: Vec<CachedFragment>,
}

#[derive(Clone, Copy)]
struct PaintGlyphIndex {
    line: usize,
    run: usize,
    glyph: usize,
    instance: usize,
}

const SYNTHETIC_HIT_UNIT: u32 = u32::MAX;
pub(super) const WHOLE_GLYPH_PAINT: u32 = u32::MAX;

#[derive(Clone, Copy, Debug)]
pub(super) struct CachedHitPlacement {
    line: u32,
    unit: u32,
    inline_end: f64,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct CachedCluster<'a> {
    pub(super) source: SourceSpan,
    pub(super) semantic_id: SemanticId,
    pub(super) boundary: ClusterBoundary,
    pub(super) whitespace: ClusterWhitespace,
    pub(super) bounds: Rect,
    pub(super) line: usize,
    pub(super) left: SourcePosition,
    pub(super) right: SourcePosition,
    pub(super) bidi_level: u8,
    pub(super) prepared: Option<PreparedInteractionUnitView<'a>>,
}

#[derive(Clone, Debug)]
pub(super) struct CachedSemantic {
    pub(super) semantic_id: SemanticId,
    pub(super) paragraph_role: Option<ParagraphRole>,
    pub(super) inline_role: Option<InlineRole>,
    pub(super) source: Option<SourceReference>,
    pub(super) bounds: Rect,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum LocalRange {
    Snapshot {
        text: TextId,
        bytes: Range<u32>,
    },
    Composition {
        id: CompositionId,
        epoch: crate::CompositionEpoch,
        bytes: Range<u32>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum LocalPosition {
    Snapshot {
        text: TextId,
        byte: u32,
        affinity: TextAffinity,
    },
    Composition {
        id: CompositionId,
        epoch: crate::CompositionEpoch,
        byte: u32,
        affinity: TextAffinity,
    },
}

impl CachedGeometry {
    pub(super) fn hit_clusters(&self) -> impl Iterator<Item = CachedCluster<'_>> {
        self.hit_geometry
            .placements()
            .iter()
            .filter_map(|placement| self.hit_cluster(*placement))
    }

    pub(super) fn exact_hit_cluster(&self, line: usize, inline: f64) -> Option<CachedCluster<'_>> {
        let line = u32::try_from(line).ok()?;
        let placements = self.hit_geometry.placements_for_line(line);
        let index = placements.partition_point(|placement| placement.inline_end < inline);
        let cluster = self.hit_cluster(*placements.get(index)?)?;
        (cluster.bounds.x0 <= inline && inline <= cluster.bounds.x1).then_some(cluster)
    }

    pub(super) fn closest_hit_cluster(
        &self,
        line: usize,
        inline: f64,
    ) -> Option<CachedCluster<'_>> {
        let line = u32::try_from(line).ok()?;
        let placements = self.hit_geometry.placements_for_line(line);
        let next = placements.partition_point(|placement| placement.inline_end < inline);
        let before = next.checked_sub(1);
        let after = (next < placements.len()).then_some(next);
        let mut closest = match (before, after) {
            (Some(before), Some(after)) => {
                let before_distance =
                    inline_distance_to_rect(inline, self.hit_cluster(placements[before])?.bounds);
                let after_distance =
                    inline_distance_to_rect(inline, self.hit_cluster(placements[after])?.bounds);
                if after_distance < before_distance {
                    after
                } else {
                    before
                }
            }
            (Some(index), None) | (None, Some(index)) => index,
            (None, None) => return None,
        };
        let distance =
            inline_distance_to_rect(inline, self.hit_cluster(placements[closest])?.bounds);
        while let Some(previous) = closest.checked_sub(1) {
            let previous_distance =
                inline_distance_to_rect(inline, self.hit_cluster(placements[previous])?.bounds);
            if previous_distance != distance {
                break;
            }
            closest = previous;
        }
        self.hit_cluster(placements[closest])
    }

    fn hit_cluster(&self, placement: CachedHitPlacement) -> Option<CachedCluster<'_>> {
        let line = usize::try_from(placement.line).ok()?;
        let prepared_line = self.artifact.lines().get(line);
        let prepared = (placement.unit != SYNTHETIC_HIT_UNIT)
            .then(|| {
                let unit = usize::try_from(placement.unit).ok()?;
                prepared_line?.units().nth(unit)
            })
            .flatten();
        let source = prepared.map_or_else(
            || {
                prepared_line
                    .map(|line| line.source())
                    .unwrap_or(0..0)
                    .into()
            },
            |unit| unit.source().into(),
        );
        let source_map = self
            .source_map
            .as_deref()
            .expect("hit-testing capability retains source provenance");
        let semantic_source = prepared
            .and_then(|unit| unit.slices().first())
            .map_or(source, |slice| slice.source().into());
        let semantic_id = source_map
            .semantic_for_span(semantic_source)
            .expect("validated hit units retain semantic ownership");
        let (left, right, boundary, whitespace, bidi_level) = prepared.map_or_else(
            || {
                let offset = prepared_line.map_or(0, |line| line.source().start);
                let affinity = if offset == 0 {
                    TextAffinity::Downstream
                } else {
                    TextAffinity::Upstream
                };
                let position = SourcePosition::new(offset, affinity);
                (
                    position,
                    position,
                    ClusterBoundary::None,
                    ClusterWhitespace::None,
                    0,
                )
            },
            |unit| {
                (
                    SourcePosition::new(unit.left().offset(), unit.left().affinity()),
                    SourcePosition::new(unit.right().offset(), unit.right().affinity()),
                    unit.boundary(),
                    unit.whitespace(),
                    unit.bidi_level(),
                )
            },
        );
        let bounds = if let Some(line_bounds) = self.lines.get(line).map(|line| line.bounds) {
            let inline_start = if placement.unit == 0 {
                line_bounds.x0
            } else if placement.unit == SYNTHETIC_HIT_UNIT {
                placement.inline_end
            } else {
                let previous = placement.unit.checked_sub(1)?;
                self.hit_geometry
                    .placements_for_line(placement.line)
                    .get(usize::try_from(previous).ok()?)?
                    .inline_end
            };
            Rect::new(
                inline_start,
                line_bounds.y0,
                placement.inline_end,
                line_bounds.y1,
            )
        } else {
            self.empty_bounds
        };
        Some(CachedCluster {
            source,
            semantic_id,
            boundary,
            whitespace,
            bounds,
            line,
            left,
            right,
            bidi_level,
            prepared,
        })
    }

    pub(super) fn cursor(&self) -> Option<ParagraphCursor<'_>> {
        self.features
            .has_selection()
            .then(|| ParagraphCursor::new(&self.artifact))
    }

    pub(super) fn movement_count(&self) -> usize {
        self.cursor().map_or(0, ParagraphCursor::count)
    }

    pub(super) fn caret_bounds(&self, movement: CursorMovement<'_>) -> Option<Rect> {
        let (line_bounds, inline) = match movement.caret_anchor() {
            CursorCaretAnchor::Unit { cluster, at_end } => {
                let line = u32::try_from(cluster.line).ok()?;
                let placements = self.hit_geometry.placements_for_line(line);
                let placement = placements.get(cluster.unit)?;
                let start = cluster.unit.checked_sub(1).map_or_else(
                    || self.lines.get(cluster.line).map(|line| line.bounds.x0),
                    |previous| {
                        placements
                            .get(previous)
                            .map(|placement| placement.inline_end)
                    },
                )?;
                (
                    self.lines.get(cluster.line)?.bounds,
                    if at_end { placement.inline_end } else { start },
                )
            }
            CursorCaretAnchor::LastLineStart => self
                .lines
                .last()
                .map_or((self.empty_bounds, self.empty_bounds.x0), |line| {
                    (line.bounds, line.bounds.x0)
                }),
        };
        Some(Rect::new(
            inline,
            line_bounds.y0,
            inline + 1.0,
            line_bounds.y1,
        ))
    }

    pub(super) fn retain_sidecars_from(&mut self, retained: &Self) {
        self.features = self.features.union(retained.features);
        if self.source_map.is_none() {
            self.source_map.clone_from(&retained.source_map);
        }
        self.hit_geometry.retain_from(&retained.hit_geometry);
        if retained
            .artifact
            .features()
            .contains(self.artifact.features())
        {
            self.artifact = Arc::clone(&retained.artifact);
        }
        self.semantics.retain_from(&retained.semantics);
    }

    pub(super) fn residency_bytes(&self) -> SceneResidencyBytes {
        let layout =
            vec_bytes::<CachedLine>(self.lines.capacity())
                .saturating_add(vec_bytes::<CachedGlyph>(self.glyphs.capacity()));
        let sources = self
            .source_map
            .as_ref()
            .map_or(0, |map| map.accounted_owned_bytes());
        let hit_testing = vec_bytes::<CachedHitPlacement>(self.hit_geometry.capacity());
        SceneResidencyBytes::from_categories(
            layout,
            0,
            sources,
            vec_bytes::<CachedSemantic>(self.semantics.capacity()),
            hit_testing,
            0,
            0,
            0,
        )
    }

    pub(super) fn accounted_owned_bytes(&self) -> usize {
        self.residency_bytes().total()
    }
}

impl PaintTopology {
    pub(super) fn residency_bytes(&self) -> usize {
        vec_bytes::<CachedFragment>(self.fragments.capacity())
    }
}

fn inline_distance_to_rect(inline: f64, bounds: Rect) -> f64 {
    if inline < bounds.x0 {
        bounds.x0 - inline
    } else if inline > bounds.x1 {
        inline - bounds.x1
    } else {
        0.0
    }
}

const fn vec_bytes<T>(capacity: usize) -> usize {
    size_of::<T>().saturating_mul(capacity)
}

pub(super) fn rebind_composition_geometry(
    geometry: &mut CachedGeometry,
    id: CompositionId,
    epoch: crate::CompositionEpoch,
) {
    if let Some(source_map) = &mut geometry.source_map {
        Arc::make_mut(source_map).rebind_composition(id, epoch);
    }
}

pub(super) fn build_geometry(
    prepared: &PreparedParagraph,
    projection: &Projection<'_>,
    features: SceneFeatures,
    constraint: TextConstraint,
    region_transcript: Option<&RegionTranscript>,
) -> Result<CachedGeometry, SceneError> {
    let source_map = features
        .has_sources()
        .then(|| ParagraphSourceMap::from_projection(projection))
        .transpose()?
        .map(Arc::new);
    if features.has_selection() {
        for position in prepared
            .lines()
            .iter()
            .flat_map(|line| line.units())
            .flat_map(|unit| [unit.left(), unit.right()])
        {
            let offset = position.offset();
            let offset_usize = usize::try_from(offset).map_err(|_| {
                SceneError::for_source(
                    SceneErrorKind::SourceCoverage,
                    prepared.paragraph(),
                    offset..offset,
                )
            })?;
            if !projection.mapping.text().is_char_boundary(offset_usize) {
                return Err(SceneError::for_source(
                    SceneErrorKind::SourceCoverage,
                    prepared.paragraph(),
                    offset..offset,
                ));
            }
        }
    }
    let map = source_map.as_deref();
    let empty_line_height = projection.empty_line_height();
    let empty_slot = region_transcript.and_then(|transcript| {
        transcript.attempts().iter().rev().find_map(|attempt| {
            (attempt.paragraph() == prepared.paragraph()
                && attempt.source().is_empty()
                && attempt.outcome() == RegionAttemptOutcome::Accepted)
                .then_some(attempt.slot())
        })
    });
    let empty_slot_start = empty_slot.map_or(0.0, crate::LineSlot::inline_start);
    let empty_slot_size = empty_slot
        .map(crate::LineSlot::inline_size)
        .or_else(|| constrained_inline_size(constraint));
    let empty_adjustment = resolve_line_adjustment(
        projection.paragraph_style.alignment(),
        prepared.resolved_direction(),
        LineBreakReason::End,
        0.0,
        0.0,
        0,
        empty_slot_size,
    )?;
    let empty_inline_start = empty_slot_start + empty_adjustment.inline_offset();
    let empty_block_start = empty_slot.map_or(0.0, crate::LineSlot::block_start);
    let empty_bounds = Rect::new(
        empty_inline_start,
        empty_block_start,
        empty_inline_start,
        empty_block_start + empty_line_height,
    );
    let mut line_top = 0.0;
    let mut lines = Vec::new();
    let retains_clusters = features.has_hit_testing() || features.has_selection();
    let builds_semantics = features.has_semantics();
    let hit_capacity = if !retains_clusters {
        0
    } else if prepared.lines().is_empty() {
        usize::from(projection.mapping.text().is_empty() && !projection.spans.is_empty())
    } else {
        prepared
            .lines()
            .iter()
            .map(|line| {
                let units = line.units().len();
                if units == 0 && !projection.spans.is_empty() {
                    1
                } else {
                    units
                }
            })
            .sum()
    };
    let mut hit_placements = Vec::with_capacity(hit_capacity);
    let mut semantic_bounds: Vec<Option<Rect>> = Vec::with_capacity(if builds_semantics {
        projection.spans.len()
    } else {
        0
    });
    if builds_semantics {
        semantic_bounds.resize(projection.spans.len(), None);
    }

    for line in prepared.lines() {
        let line_index = lines.len();
        let slot_start = line.slot().map_or(0.0, crate::LineSlot::inline_start);
        let slot_size = line
            .slot()
            .map(crate::LineSlot::inline_size)
            .or_else(|| constrained_inline_size(constraint));
        let adjustment = resolve_line_adjustment(
            projection.paragraph_style.alignment(),
            prepared.resolved_direction(),
            line.break_reason(),
            line.advance(),
            line.trailing_whitespace_advance(),
            line.western_justification_opportunities(),
            slot_size,
        )?;
        let inline_start = slot_start + adjustment.inline_offset();
        let current_line_top = line.slot().map_or(line_top, crate::LineSlot::block_start);
        let expansion = adjustment.opportunity_expansion();
        let opportunity_sources: Vec<_> = if expansion > 0.0 {
            line.western_justification_opportunity_sources().collect()
        } else {
            Vec::new()
        };
        let mut unit_x = inline_start;
        for (unit_index, unit) in line.units().enumerate() {
            let paragraph_source = unit.source();
            let unit_expansion = if opportunity_sources.contains(&paragraph_source) {
                expansion
            } else {
                0.0
            };
            let source = if builds_semantics || retains_clusters {
                Some(
                    map.expect("source-aware capabilities retain a source map")
                        .span(paragraph_source.clone(), prepared.paragraph())?,
                )
            } else {
                None
            };
            let mut slice_x = unit_x;
            for slice in unit.slices() {
                let next_x = slice_x + slice.advance();
                if retains_clusters {
                    let source = slice.source();
                    let local = map
                        .expect("interaction capabilities retain a source map")
                        .span(source.clone(), prepared.paragraph())?;
                    if map
                        .expect("interaction capabilities retain a source map")
                        .semantic_for_span(local)
                        .is_none()
                    {
                        return Err(SceneError::for_source(
                            SceneErrorKind::SourceCoverage,
                            prepared.paragraph(),
                            source,
                        ));
                    }
                }
                slice_x = next_x;
            }
            let adjusted_unit_advance = unit.advance() + unit_expansion;
            let next_x = unit_x + adjusted_unit_advance;
            let bounds = Rect::new(
                unit_x,
                current_line_top,
                next_x,
                current_line_top + line.height(),
            );
            if builds_semantics {
                for index in map
                    .expect("semantic capability retains a source map")
                    .leaf_indices_for_span(
                        source.expect("semantic bounds retain a projected source span"),
                    )
                {
                    semantic_bounds[index] = Some(match semantic_bounds[index] {
                        Some(current) => current.union(bounds),
                        None => bounds,
                    });
                }
            }
            if retains_clusters {
                hit_placements.push(CachedHitPlacement {
                    line: u32::try_from(line_index).map_err(|_| {
                        SceneError::for_paragraph(
                            SceneErrorKind::SourceCoverage,
                            prepared.paragraph(),
                        )
                    })?,
                    unit: u32::try_from(unit_index).map_err(|_| {
                        SceneError::for_paragraph(
                            SceneErrorKind::SourceCoverage,
                            prepared.paragraph(),
                        )
                    })?,
                    inline_end: next_x,
                });
            }
            unit_x = next_x;
        }
        if (builds_semantics || retains_clusters)
            && line.units().is_empty()
            && !projection.spans.is_empty()
        {
            let source = line.source();
            let local_source = map
                .expect("source-aware capabilities retain a source map")
                .span(source.clone(), prepared.paragraph())?;
            let bounds = Rect::new(
                inline_start,
                current_line_top,
                inline_start,
                current_line_top + line.height(),
            );
            if builds_semantics {
                for index in map
                    .expect("semantic capability retains a source map")
                    .leaf_indices_for_span(local_source)
                {
                    semantic_bounds[index] = Some(match semantic_bounds[index] {
                        Some(current) => current.union(bounds),
                        None => bounds,
                    });
                }
            }
            if retains_clusters {
                let affinity = if source.start == 0 {
                    TextAffinity::Downstream
                } else {
                    TextAffinity::Upstream
                };
                projection.source_position(source.start, affinity)?;
                if map
                    .expect("interaction capabilities retain a source map")
                    .semantic_for_span(local_source)
                    .is_none()
                {
                    return Err(SceneError::for_source(
                        SceneErrorKind::SourceCoverage,
                        prepared.paragraph(),
                        source,
                    ));
                }
                hit_placements.push(CachedHitPlacement {
                    line: u32::try_from(line_index).map_err(|_| {
                        SceneError::for_paragraph(
                            SceneErrorKind::SourceCoverage,
                            prepared.paragraph(),
                        )
                    })?,
                    unit: SYNTHETIC_HIT_UNIT,
                    inline_end: bounds.x1,
                });
            }
        }
        let adjusted_advance = line.advance()
            + expansion
                * f64::from(u32::try_from(opportunity_sources.len()).map_err(|_| {
                    SceneError::for_paragraph(SceneErrorKind::SourceCoverage, prepared.paragraph())
                })?);
        lines.push(CachedLine {
            bounds: Rect::new(
                inline_start,
                current_line_top,
                inline_start + adjusted_advance.max(1.0),
                current_line_top + line.height(),
            ),
            adjustment,
        });
        line_top = line_top.max(current_line_top + line.height());
    }
    let glyphs = build_layout_glyphs(prepared, &lines)?;

    if retains_clusters
        && prepared.lines().is_empty()
        && projection.mapping.text().is_empty()
        && !projection.spans.is_empty()
    {
        projection.source_position(0, TextAffinity::Downstream)?;
        let source = map
            .expect("interaction capabilities retain a source map")
            .span(0..0, prepared.paragraph())?;
        if map
            .expect("interaction capabilities retain a source map")
            .semantic_for_span(source)
            .is_none()
        {
            return Err(SceneError::for_source(
                SceneErrorKind::SourceCoverage,
                prepared.paragraph(),
                0..0,
            ));
        }
        hit_placements.push(CachedHitPlacement {
            line: 0,
            unit: SYNTHETIC_HIT_UNIT,
            inline_end: empty_bounds.x1,
        });
    }

    let mut semantics = Vec::new();
    if features.has_semantics()
        && !projection.spans.is_empty()
        && let Some(first_line) = lines.first()
    {
        let bounds = lines
            .iter()
            .skip(1)
            .fold(first_line.bounds, |bounds, line| bounds.union(line.bounds));
        semantics.push(CachedSemantic {
            semantic_id: projection.paragraph_semantic,
            paragraph_role: Some(projection.paragraph_role),
            inline_role: None,
            source: None,
            bounds,
        });
    }
    for (span_index, span) in projection.spans.iter().enumerate() {
        if !features.has_semantics() {
            break;
        }
        if span.leaf_len == 0
            || projection.spans[..span_index]
                .iter()
                .any(|previous| previous.text == span.text)
        {
            continue;
        }
        let bounds = projection
            .spans
            .iter()
            .zip(&semantic_bounds)
            .filter(|(candidate, _)| {
                candidate.text == span.text
                    || matches!(span.source, LeafSpanSource::Composition { .. })
                        && matches!(candidate.source, LeafSpanSource::Composition { .. })
            })
            .filter_map(|(_, bounds)| *bounds)
            .reduce(|bounds, next| bounds.union(next));
        let source = Some(SourceReference::Leaf(u32::try_from(span_index).map_err(
            |_| SceneError::for_paragraph(SceneErrorKind::SourceCoverage, prepared.paragraph()),
        )?));
        semantics.push(CachedSemantic {
            semantic_id: span.semantic,
            paragraph_role: None,
            inline_role: Some(span.role),
            source,
            bounds: bounds.unwrap_or(empty_bounds),
        });
    }
    Ok(CachedGeometry {
        features,
        artifact: prepared.shared_facts(),
        facts: Arc::new(CachedGeometryFacts {
            height: if prepared.lines().is_empty() {
                empty_bounds.y1
            } else {
                line_top
            },
            empty_bounds,
            lines,
            glyphs,
        }),
        source_map,
        hit_geometry: CachedHitSidecar::new(retains_clusters, hit_placements),
        semantics: CachedSidecar::new(features.has_semantics(), semantics),
    })
}

pub(super) fn build_paint_topology(
    prepared: &PreparedParagraph,
    projection: &Projection<'_>,
    retained: &CachedGeometry,
) -> Result<PaintTopology, SceneError> {
    build_paint_fragments(prepared, projection, &retained.glyphs)
}

fn build_layout_glyphs(
    prepared: &PreparedParagraph,
    lines: &[CachedLine],
) -> Result<Vec<CachedGlyph>, SceneError> {
    if prepared.lines().len() != lines.len() {
        return Err(SceneError::for_paragraph(
            SceneErrorKind::SourceCoverage,
            prepared.paragraph(),
        ));
    }
    let glyph_count = prepared
        .lines()
        .iter()
        .flat_map(|line| line.runs())
        .map(|run| run.glyphs().len())
        .sum();
    let mut glyphs = Vec::with_capacity(glyph_count);
    for (line, cached_line) in prepared.lines().iter().zip(lines) {
        let expansion = cached_line.adjustment.opportunity_expansion();
        let opportunity_sources: Vec<_> = if expansion > 0.0 {
            line.western_justification_opportunity_sources().collect()
        } else {
            Vec::new()
        };
        let mut opportunity_glyphs = alloc::vec![None; opportunity_sources.len()];
        let mut line_glyph_index = 0_usize;
        for run in line.runs() {
            for glyph in run.glyphs() {
                for (index, source) in opportunity_sources.iter().enumerate() {
                    if glyph.source() == *source {
                        if opportunity_glyphs[index].is_some() {
                            return Err(SceneError::for_paragraph(
                                SceneErrorKind::SourceCoverage,
                                prepared.paragraph(),
                            ));
                        }
                        opportunity_glyphs[index] = Some(line_glyph_index);
                    }
                }
                line_glyph_index += 1;
            }
        }
        if expansion > 0.0 && opportunity_glyphs.iter().any(Option::is_none) {
            return Err(SceneError::for_paragraph(
                SceneErrorKind::SourceCoverage,
                prepared.paragraph(),
            ));
        }

        let mut x = cached_line.bounds.x0;
        line_glyph_index = 0;
        for run in line.runs() {
            for glyph in run.glyphs() {
                let glyph_expansion = if opportunity_glyphs.contains(&Some(line_glyph_index)) {
                    expansion
                } else {
                    0.0
                };
                let inline_advance_adjustment = glyph_expansion;
                glyphs.push(CachedGlyph {
                    position: Point::new(
                        x + glyph.offset().x,
                        cached_line.bounds.y0 + line.baseline() - glyph.offset().y,
                    ),
                    inline_advance_adjustment,
                });
                x += glyph.advance().x + inline_advance_adjustment;
                line_glyph_index += 1;
            }
        }
    }
    Ok(glyphs)
}

fn build_paint_fragments(
    prepared: &PreparedParagraph,
    projection: &Projection<'_>,
    layout_glyphs: &[CachedGlyph],
) -> Result<PaintTopology, SceneError> {
    let mut fragments: Vec<CachedFragment> = Vec::new();
    let mut instance = 0_usize;
    for (line_index, line) in prepared.lines().iter().enumerate() {
        for (run_index, run) in line.runs().iter().enumerate() {
            let run_fragment_start = fragments.len();
            for (glyph_index, glyph) in run.glyphs().iter().enumerate() {
                let layout = layout_glyphs.get(instance).ok_or_else(|| {
                    SceneError::for_paragraph(SceneErrorKind::SourceCoverage, prepared.paragraph())
                })?;
                if let Some(segments) = glyph.paint().split_segments() {
                    for (segment_index, segment) in segments.iter().enumerate() {
                        let clip = segment
                            .clip()
                            .expect("validated split-paint segments retain explicit clips");
                        let paint_clip = Some(Rect::new(
                            layout.position.x + clip.x0,
                            layout.position.y + clip.y0,
                            layout.position.x + clip.x1,
                            layout.position.y + clip.y1,
                        ));
                        push_paint_fragment(
                            &mut fragments,
                            run_fragment_start,
                            prepared.paragraph(),
                            PaintGlyphIndex {
                                line: line_index,
                                run: run_index,
                                glyph: glyph_index,
                                instance,
                            },
                            Some(segment_index),
                            segment.slot(),
                            paint_clip,
                        )?;
                    }
                } else {
                    let source = glyph.source();
                    let paint = projection.whole_paint_slot(source.clone()).ok_or_else(|| {
                        SceneError::from_preparation_source(
                            prepared.paragraph(),
                            source,
                            PreparationErrorKind::UnsupportedPaintCoverage,
                        )
                    })?;
                    push_paint_fragment(
                        &mut fragments,
                        run_fragment_start,
                        prepared.paragraph(),
                        PaintGlyphIndex {
                            line: line_index,
                            run: run_index,
                            glyph: glyph_index,
                            instance,
                        },
                        None,
                        paint,
                        None,
                    )?;
                }
                instance += 1;
            }
        }
    }
    if instance != layout_glyphs.len() {
        return Err(SceneError::for_paragraph(
            SceneErrorKind::SourceCoverage,
            prepared.paragraph(),
        ));
    }
    Ok(PaintTopology { fragments })
}

fn push_paint_fragment(
    fragments: &mut Vec<CachedFragment>,
    run_fragment_start: usize,
    paragraph: ParagraphId,
    index: PaintGlyphIndex,
    segment: Option<usize>,
    paint: PaintSlot,
    paint_clip: Option<Rect>,
) -> Result<(), SceneError> {
    let glyph = u32::try_from(index.glyph)
        .map_err(|_| SceneError::for_paragraph(SceneErrorKind::SourceCoverage, paragraph))?;
    let preceding = fragments
        .get_mut(run_fragment_start..)
        .and_then(|run_fragments| run_fragments.last_mut());
    if segment.is_none()
        && let Some(preceding) = preceding
        && preceding.segment == WHOLE_GLYPH_PAINT
        && preceding.paint == paint
        && preceding.glyphs.end == glyph
    {
        preceding.glyphs.end =
            preceding.glyphs.end.checked_add(1).ok_or_else(|| {
                SceneError::for_paragraph(SceneErrorKind::SourceCoverage, paragraph)
            })?;
    } else {
        fragments.push(CachedFragment {
            line: u32::try_from(index.line).map_err(|_| {
                SceneError::for_paragraph(SceneErrorKind::SourceCoverage, paragraph)
            })?,
            run: u32::try_from(index.run).map_err(|_| {
                SceneError::for_paragraph(SceneErrorKind::SourceCoverage, paragraph)
            })?,
            glyphs: glyph..glyph.checked_add(1).ok_or_else(|| {
                SceneError::for_paragraph(SceneErrorKind::SourceCoverage, paragraph)
            })?,
            instance_start: index.instance,
            segment: segment.map_or(Ok(WHOLE_GLYPH_PAINT), |segment| {
                u32::try_from(segment).map_err(|_| {
                    SceneError::for_paragraph(SceneErrorKind::SourceCoverage, paragraph)
                })
            })?,
            paint,
            paint_clip,
        });
    }
    Ok(())
}

pub(super) fn fragment_identity(paragraph: ParagraphId, fragment: usize) -> u64 {
    let mut identity = 0xcbf2_9ce4_8422_2325_u64;
    for byte in paragraph.document.opaque_bytes() {
        identity = (identity ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3);
    }
    for byte in paragraph.index.to_le_bytes() {
        identity = (identity ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3);
    }
    for byte in u64::try_from(fragment).unwrap_or(u64::MAX).to_le_bytes() {
        identity = (identity ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3);
    }
    identity
}

pub(super) fn materialize_projected_source(
    range: LocalRange,
    revision: DocumentRevision,
) -> ProjectedTextSource {
    match range {
        LocalRange::Snapshot { text, bytes } => {
            ProjectedTextSource::Snapshot(SnapshotTextRange::new(revision, text, bytes))
        }
        LocalRange::Composition { id, epoch, bytes } => {
            ProjectedTextSource::Composition(crate::CompositionTextRange::new(id, epoch, bytes))
        }
    }
}

pub(super) fn projected_position(
    source_map: &ParagraphSourceMap,
    position: SourcePosition,
    revision: DocumentRevision,
) -> ProjectedTextPosition {
    match source_map.materialize_position(position) {
        LocalPosition::Snapshot {
            text,
            byte,
            affinity,
        } => ProjectedTextPosition::Snapshot(SnapshotTextPosition::new(
            revision, text, byte, affinity,
        )),
        LocalPosition::Composition {
            id,
            epoch,
            byte,
            affinity,
        } => ProjectedTextPosition::Composition(crate::CompositionTextPosition::new(
            id, epoch, byte, affinity,
        )),
    }
}

pub(super) fn materialize_optional_snapshot_range(
    source_map: &ParagraphSourceMap,
    reference: SourceReference,
    revision: DocumentRevision,
) -> Option<SnapshotTextRange> {
    let mut ranges = source_map.ranges(reference);
    let LocalRange::Snapshot { text, bytes } = ranges.next()? else {
        return None;
    };
    if ranges.next().is_some() {
        return None;
    }
    Some(SnapshotTextRange::new(revision, text, bytes))
}

pub(super) fn materialize_cursor_step(
    source_map: &ParagraphSourceMap,
    step: Option<DerivedCursorStep>,
    revision: DocumentRevision,
) -> Option<SceneCursorStep> {
    step.map(|step| SceneCursorStep {
        target: materialize_position(
            source_map,
            SourcePosition::new(step.target.offset(), step.target.affinity()),
            revision,
        ),
        source: step
            .source
            .map(SourceSpan::from)
            .map(|source| materialize_snapshot_unit(source_map, source, revision)),
    })
}

pub(super) fn materialize_range(
    range: LocalRange,
    revision: DocumentRevision,
) -> SnapshotTextRange {
    let LocalRange::Snapshot { text, bytes } = range else {
        unreachable!("committed geometry cannot contain composition source")
    };
    SnapshotTextRange::new(revision, text, bytes)
}

pub(super) fn materialize_snapshot_unit(
    source_map: &ParagraphSourceMap,
    span: SourceSpan,
    revision: DocumentRevision,
) -> SnapshotTextUnit {
    SnapshotTextUnit::new(
        source_map
            .ranges_for_span(span)
            .map(|range| materialize_range(range, revision))
            .collect(),
    )
}

pub(super) fn materialize_position(
    source_map: &ParagraphSourceMap,
    position: SourcePosition,
    revision: DocumentRevision,
) -> SnapshotTextPosition {
    let LocalPosition::Snapshot {
        text,
        byte,
        affinity,
    } = source_map.materialize_position(position)
    else {
        unreachable!("committed geometry cannot contain a composition position")
    };
    SnapshotTextPosition::new(revision, text, byte, affinity)
}
