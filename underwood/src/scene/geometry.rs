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
    pub(super) facts: Arc<CachedGeometryFacts>,
    pub(super) line_fragments: Vec<Range<usize>>,
    pub(super) fragments: Vec<CachedFragment>,
    pub(super) line_sources: CachedSidecar<Vec<LocalRange>>,
    pub(super) clusters: CachedSidecar<CachedCluster>,
    pub(super) carets: CachedSidecar<CachedCaret>,
    pub(super) movements: CachedSidecar<CachedCursorMovement>,
    pub(super) texts: CachedSidecar<LocalRange>,
    pub(super) semantics: CachedSidecar<CachedSemantic>,
}

#[derive(Clone, Debug)]
pub(super) struct CachedGeometryFacts {
    pub(super) height: f64,
    pub(super) lines: Vec<CachedLine>,
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

    #[cfg(test)]
    pub(super) fn from_records(records: Vec<T>) -> Self {
        Self {
            records: Some(Arc::new(records)),
        }
    }

    fn capacity(&self) -> usize {
        self.records
            .as_ref()
            .map_or(0, |records| records.capacity())
    }

    fn make_mut(&mut self) -> Option<&mut Vec<T>>
    where
        T: Clone,
    {
        self.records.as_mut().map(Arc::make_mut)
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

impl Deref for CachedGeometry {
    type Target = CachedGeometryFacts;

    fn deref(&self) -> &Self::Target {
        &self.facts
    }
}

#[derive(Clone, Debug)]
pub(super) struct CachedLine {
    pub(super) bounds: Rect,
    pub(super) advance: f64,
    pub(super) break_reason: LineBreakReason,
    pub(super) baseline: f64,
    pub(super) content_ascent: f64,
    pub(super) content_descent: f64,
    pub(super) adjustment: LineAdjustment,
}

#[derive(Clone, Debug)]
pub(super) struct CachedFragment {
    pub(super) id: SceneFragmentId,
    pub(super) glyphs: Vec<CachedGlyph>,
    pub(super) paint: PaintSlot,
    pub(super) transform: Affine,
    pub(super) sources: Vec<LocalRange>,
    pub(super) paint_clip: Option<Rect>,
    pub(super) font: FontData,
    pub(super) font_size: f32,
    pub(super) synthesis: FontSynthesis,
    pub(super) normalized_coords: Arc<[i16]>,
    pub(super) bidi_level: u8,
    pub(super) script: [u8; 4],
}

#[derive(Clone, Debug)]
pub(super) struct CachedGlyph {
    pub(super) instance: usize,
    pub(super) id: u32,
    pub(super) position: Point,
    pub(super) advance: Vec2,
    pub(super) sources: Vec<LocalRange>,
}

#[derive(Clone, Debug)]
pub(super) struct CachedCluster {
    pub(super) sources: Vec<LocalRange>,
    pub(super) semantic_id: SemanticId,
    pub(super) boundary: ClusterBoundary,
    pub(super) whitespace: ClusterWhitespace,
    pub(super) hit_slices: Vec<CachedHitSlice>,
    pub(super) bounds: Rect,
    pub(super) line: usize,
    pub(super) left: LocalPosition,
    pub(super) right: LocalPosition,
    pub(super) bidi_level: u8,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct CachedHitSlice {
    pub(super) semantic_id: SemanticId,
    pub(super) x0: f64,
    pub(super) x1: f64,
}

#[derive(Clone, Debug)]
pub(super) struct CachedCaret {
    pub(super) position: LocalPosition,
    pub(super) bounds: Rect,
}

#[derive(Clone, Debug)]
pub(super) struct CachedCursorMovement {
    pub(super) position: LocalPosition,
    pub(super) previous_visual: Option<CachedCursorStep>,
    pub(super) next_visual: Option<CachedCursorStep>,
    pub(super) previous_logical: Option<CachedCursorStep>,
    pub(super) next_logical: Option<CachedCursorStep>,
}

#[derive(Clone, Debug)]
pub(super) struct CachedCursorStep {
    pub(super) target: LocalPosition,
    pub(super) source: Option<Vec<LocalRange>>,
}

#[derive(Clone, Debug)]
pub(super) struct CachedSemantic {
    pub(super) semantic_id: SemanticId,
    pub(super) paragraph_role: Option<ParagraphRole>,
    pub(super) inline_role: Option<InlineRole>,
    pub(super) source: Option<Vec<LocalRange>>,
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
    pub(super) fn retain_sidecars_from(&mut self, retained: &Self) {
        self.features = self.features.union(retained.features);
        self.line_sources.retain_from(&retained.line_sources);
        self.clusters.retain_from(&retained.clusters);
        self.carets.retain_from(&retained.carets);
        self.movements.retain_from(&retained.movements);
        self.texts.retain_from(&retained.texts);
        self.semantics.retain_from(&retained.semantics);
    }

    pub(super) fn accounted_owned_bytes(&self) -> usize {
        let mut bytes = vec_bytes::<CachedLine>(self.lines.capacity())
            .saturating_add(vec_bytes::<Range<usize>>(self.line_fragments.capacity()))
            .saturating_add(vec_bytes::<CachedFragment>(self.fragments.capacity()))
            .saturating_add(vec_bytes::<CachedCluster>(self.clusters.capacity()))
            .saturating_add(vec_bytes::<CachedCaret>(self.carets.capacity()))
            .saturating_add(vec_bytes::<CachedCursorMovement>(self.movements.capacity()))
            .saturating_add(vec_bytes::<LocalRange>(self.texts.capacity()))
            .saturating_add(vec_bytes::<CachedSemantic>(self.semantics.capacity()));
        for sources in self.line_sources.iter() {
            bytes = bytes.saturating_add(vec_bytes::<LocalRange>(sources.capacity()));
        }
        for fragment in &self.fragments {
            bytes = bytes
                .saturating_add(vec_bytes::<CachedGlyph>(fragment.glyphs.capacity()))
                .saturating_add(vec_bytes::<LocalRange>(fragment.sources.capacity()))
                .saturating_add(vec_bytes::<i16>(fragment.normalized_coords.len()));
            for glyph in &fragment.glyphs {
                bytes = bytes.saturating_add(vec_bytes::<LocalRange>(glyph.sources.capacity()));
            }
        }
        for cluster in self.clusters.iter() {
            bytes = bytes
                .saturating_add(vec_bytes::<LocalRange>(cluster.sources.capacity()))
                .saturating_add(vec_bytes::<CachedHitSlice>(cluster.hit_slices.capacity()));
        }
        for movement in self.movements.iter() {
            bytes = bytes
                .saturating_add(cursor_step_bytes(movement.previous_visual.as_ref()))
                .saturating_add(cursor_step_bytes(movement.next_visual.as_ref()))
                .saturating_add(cursor_step_bytes(movement.previous_logical.as_ref()))
                .saturating_add(cursor_step_bytes(movement.next_logical.as_ref()));
        }
        for semantic in self.semantics.iter() {
            bytes = bytes.saturating_add(
                semantic
                    .source
                    .as_ref()
                    .map_or(0, |source| vec_bytes::<LocalRange>(source.capacity())),
            );
        }
        bytes
    }
}

fn cursor_step_bytes(step: Option<&CachedCursorStep>) -> usize {
    step.and_then(|step| step.source.as_ref())
        .map_or(0, |source| vec_bytes::<LocalRange>(source.capacity()))
}

const fn vec_bytes<T>(capacity: usize) -> usize {
    size_of::<T>().saturating_mul(capacity)
}

pub(super) fn rebind_composition_geometry(
    geometry: &mut CachedGeometry,
    id: CompositionId,
    epoch: crate::CompositionEpoch,
) {
    if let Some(lines) = geometry.line_sources.make_mut() {
        for sources in lines {
            rebind_ranges(sources, id, epoch);
        }
    }
    for fragment in &mut geometry.fragments {
        rebind_ranges(&mut fragment.sources, id, epoch);
        for glyph in &mut fragment.glyphs {
            rebind_ranges(&mut glyph.sources, id, epoch);
        }
    }
    if let Some(clusters) = geometry.clusters.make_mut() {
        for cluster in clusters {
            rebind_ranges(&mut cluster.sources, id, epoch);
            rebind_position(&mut cluster.left, id, epoch);
            rebind_position(&mut cluster.right, id, epoch);
        }
    }
    if let Some(carets) = geometry.carets.make_mut() {
        for caret in carets {
            rebind_position(&mut caret.position, id, epoch);
        }
    }
    if let Some(movements) = geometry.movements.make_mut() {
        for movement in movements {
            rebind_position(&mut movement.position, id, epoch);
            for step in [
                &mut movement.previous_visual,
                &mut movement.next_visual,
                &mut movement.previous_logical,
                &mut movement.next_logical,
            ]
            .into_iter()
            .flatten()
            {
                rebind_position(&mut step.target, id, epoch);
                if let Some(source) = &mut step.source {
                    rebind_ranges(source, id, epoch);
                }
            }
        }
    }
    if let Some(texts) = geometry.texts.make_mut() {
        rebind_ranges(texts, id, epoch);
    }
    if let Some(semantics) = geometry.semantics.make_mut() {
        for semantic in semantics {
            if let Some(source) = &mut semantic.source {
                rebind_ranges(source, id, epoch);
            }
        }
    }
}

pub(super) fn rebind_ranges(
    ranges: &mut [LocalRange],
    id: CompositionId,
    epoch: crate::CompositionEpoch,
) {
    for range in ranges {
        if let LocalRange::Composition {
            id: range_id,
            epoch: range_epoch,
            ..
        } = range
        {
            *range_id = id;
            *range_epoch = epoch;
        }
    }
}

pub(super) fn rebind_position(
    position: &mut LocalPosition,
    id: CompositionId,
    epoch: crate::CompositionEpoch,
) {
    if let LocalPosition::Composition {
        id: position_id,
        epoch: position_epoch,
        ..
    } = position
    {
        *position_id = id;
        *position_epoch = epoch;
    }
}

pub(super) fn build_geometry(
    prepared: &PreparedParagraph,
    projection: &Projection<'_>,
    features: SceneFeatures,
    constraint: TextConstraint,
    region_transcript: Option<&RegionTranscript>,
) -> Result<CachedGeometry, SceneError> {
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
    let mut line_sources = Vec::new();
    let mut clusters = Vec::new();
    let mut carets = Vec::new();
    let mut caret_maps = Vec::new();
    let needs_clusters =
        features.has_semantics() || features.has_hit_testing() || features.has_selection();

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
        let baseline = current_line_top + line.baseline();
        let expansion = adjustment.opportunity_expansion();
        let opportunity_sources: Vec<_> = if expansion > 0.0 {
            line.western_justification_opportunity_sources().collect()
        } else {
            Vec::new()
        };
        let mut unit_x = inline_start;
        let mut original_unit_x = 0.0;
        let mut adjusted_unit_x = 0.0;
        let mut caret_map = CaretAdjustmentMap::with_capacity(if features.has_selection() {
            line.units().len()
        } else {
            0
        });
        for unit in line.units() {
            let paragraph_source = unit.source();
            let unit_expansion = if opportunity_sources.contains(&paragraph_source) {
                expansion
            } else {
                0.0
            };
            let sources = if needs_clusters {
                projection.local_ranges(paragraph_source.clone())?
            } else {
                Vec::new()
            };
            let left = needs_clusters
                .then(|| projection.position_at(unit.left().offset(), unit.left().affinity()))
                .transpose()?;
            let right = needs_clusters
                .then(|| projection.position_at(unit.right().offset(), unit.right().affinity()))
                .transpose()?;
            let mut slice_x = unit_x;
            let mut hit_slices = Vec::with_capacity(if needs_clusters {
                unit.slices().len()
            } else {
                0
            });
            for slice in unit.slices() {
                let next_x = slice_x + slice.advance();
                let source = slice.source();
                if needs_clusters {
                    projection.local_ranges(source.clone())?;
                    hit_slices.push(CachedHitSlice {
                        semantic_id: projection.semantic_for_range(source)?,
                        x0: slice_x,
                        x1: next_x,
                    });
                }
                slice_x = next_x;
            }
            if unit_expansion > 0.0
                && let Some(last) = hit_slices.last_mut()
            {
                last.x1 += unit_expansion;
            }
            let semantic_id = if needs_clusters {
                hit_slices.first().map_or_else(
                    || projection.semantic_for_range(paragraph_source),
                    |slice| Ok(slice.semantic_id),
                )?
            } else {
                projection.paragraph_semantic
            };
            let adjusted_unit_advance = unit.advance() + unit_expansion;
            let next_x = unit_x + adjusted_unit_advance;
            let bounds = Rect::new(
                unit_x,
                current_line_top,
                next_x,
                current_line_top + line.height(),
            );
            if needs_clusters {
                clusters.push(CachedCluster {
                    sources,
                    semantic_id,
                    boundary: unit.boundary(),
                    whitespace: unit.whitespace(),
                    hit_slices,
                    bounds,
                    line: line_index,
                    left: left.expect("cluster construction resolves the left position"),
                    right: right.expect("cluster construction resolves the right position"),
                    bidi_level: unit.bidi_level(),
                });
            }
            if features.has_selection() {
                caret_map.push(
                    original_unit_x,
                    original_unit_x + unit.advance(),
                    adjusted_unit_x,
                    adjusted_unit_x + adjusted_unit_advance,
                );
            }
            original_unit_x += unit.advance();
            adjusted_unit_x += adjusted_unit_advance;
            unit_x = next_x;
        }
        caret_map.finish_empty();
        caret_maps.push(caret_map);
        if needs_clusters && line.units().is_empty() && !projection.spans.is_empty() {
            let source = line.source();
            let affinity = if source.start == 0 {
                TextAffinity::Downstream
            } else {
                TextAffinity::Upstream
            };
            let position = projection.position_at(source.start, affinity)?;
            let local_source = projection.local_ranges(source.clone())?;
            clusters.push(CachedCluster {
                semantic_id: projection.semantic_for_range(source)?,
                sources: local_source,
                boundary: ClusterBoundary::None,
                whitespace: ClusterWhitespace::None,
                hit_slices: Vec::new(),
                bounds: Rect::new(
                    inline_start,
                    current_line_top,
                    inline_start,
                    current_line_top + line.height(),
                ),
                line: line_index,
                left: position,
                right: position,
                bidi_level: 0,
            });
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
            advance: adjusted_advance,
            break_reason: line.break_reason(),
            baseline,
            content_ascent: line.content_ascent(),
            content_descent: line.content_descent(),
            adjustment,
        });
        if features.has_sources() {
            line_sources.push(projection.local_ranges(line.source())?);
        }
        line_top = line_top.max(current_line_top + line.height());
    }
    let (fragments, line_fragments) =
        build_paint_fragments(prepared, projection, features, &lines)?;

    if needs_clusters
        && prepared.lines().is_empty()
        && projection.mapping.text().is_empty()
        && !projection.spans.is_empty()
    {
        let position = projection.position_at(0, TextAffinity::Downstream)?;
        let sources = projection.local_ranges(0..0)?;
        clusters.push(CachedCluster {
            semantic_id: projection.semantic_for_range(0..0)?,
            sources,
            boundary: ClusterBoundary::None,
            whitespace: ClusterWhitespace::None,
            hit_slices: Vec::new(),
            bounds: empty_bounds,
            line: 0,
            left: position,
            right: position,
            bidi_level: 0,
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
        let mut bounds: Option<Rect> = None;
        for cluster in &clusters {
            if cluster.sources.iter().any(|source| {
                matches!(source, LocalRange::Snapshot { text, .. } if *text == span.text)
                    || matches!(span.source, LeafSpanSource::Composition { .. })
                        && matches!(source, LocalRange::Composition { .. })
            }) {
                bounds = Some(match bounds {
                    Some(current) => current.union(cluster.bounds),
                    None => cluster.bounds,
                });
            }
        }
        let source = alloc::vec![LocalRange::Snapshot {
            text: span.text,
            bytes: 0..span.leaf_len,
        }];
        semantics.push(CachedSemantic {
            semantic_id: span.semantic,
            paragraph_role: None,
            inline_role: Some(span.role),
            source: Some(source),
            bounds: bounds.unwrap_or(empty_bounds),
        });
    }

    let movements = if projection.spans.is_empty() || !features.has_navigation() {
        Vec::new()
    } else {
        prepared
            .movements()
            .iter()
            .map(|movement| {
                Ok(CachedCursorMovement {
                    position: projection.position_at(
                        movement.position().offset(),
                        movement.position().affinity(),
                    )?,
                    previous_visual: cached_cursor_step(movement.previous_visual(), projection)?,
                    next_visual: cached_cursor_step(movement.next_visual(), projection)?,
                    previous_logical: cached_cursor_step(movement.previous_logical(), projection)?,
                    next_logical: cached_cursor_step(movement.next_logical(), projection)?,
                })
            })
            .collect::<Result<Vec<_>, SceneError>>()?
    };
    for prepared_movement in prepared
        .movements()
        .iter()
        .filter(|_| features.has_selection() && !projection.spans.is_empty())
    {
        let caret = prepared_movement.caret();
        let line = usize::try_from(caret.line()).map_err(|_| {
            SceneError::for_paragraph(SceneErrorKind::SourceCoverage, prepared.paragraph())
        })?;
        let line_bounds = lines.get(line).map(|line| line.bounds).unwrap_or(Rect::new(
            empty_bounds.x0,
            empty_bounds.y0,
            empty_bounds.x0 + 1.0,
            empty_bounds.y1,
        ));
        let adjusted_inline = match caret_maps.get(line) {
            Some(map) => map.adjusted_inline(caret.inline()).ok_or_else(|| {
                SceneError::for_paragraph(SceneErrorKind::SourceCoverage, prepared.paragraph())
            })?,
            None if prepared.lines().is_empty() && caret.inline() == 0.0 => 0.0,
            None => {
                return Err(SceneError::for_paragraph(
                    SceneErrorKind::SourceCoverage,
                    prepared.paragraph(),
                ));
            }
        };
        carets.push(CachedCaret {
            position: projection.position_at(
                prepared_movement.position().offset(),
                prepared_movement.position().affinity(),
            )?,
            bounds: Rect::new(
                line_bounds.x0 + adjusted_inline,
                line_bounds.y0,
                line_bounds.x0 + adjusted_inline + 1.0,
                line_bounds.y1,
            ),
        });
    }
    let texts = if features.has_sources() {
        projection
            .spans
            .iter()
            .map(|span| span.local_range(span.paragraph.start, span.paragraph.end))
            .collect()
    } else {
        Vec::new()
    };

    Ok(CachedGeometry {
        features,
        facts: Arc::new(CachedGeometryFacts {
            height: if prepared.lines().is_empty() {
                empty_bounds.y1
            } else {
                line_top
            },
            lines,
        }),
        line_fragments,
        fragments,
        line_sources: CachedSidecar::new(features.has_sources(), line_sources),
        clusters: CachedSidecar::new(needs_clusters, clusters),
        carets: CachedSidecar::new(features.has_selection(), carets),
        movements: CachedSidecar::new(features.has_navigation(), movements),
        texts: CachedSidecar::new(features.has_sources(), texts),
        semantics: CachedSidecar::new(features.has_semantics(), semantics),
    })
}

pub(super) fn repaint_geometry(
    prepared: &PreparedParagraph,
    projection: &Projection<'_>,
    retained: &CachedGeometry,
) -> Result<CachedGeometry, SceneError> {
    let (fragments, line_fragments) =
        build_paint_fragments(prepared, projection, retained.features, &retained.lines)?;
    Ok(CachedGeometry {
        features: retained.features,
        facts: Arc::clone(&retained.facts),
        line_fragments,
        fragments,
        line_sources: retained.line_sources.clone(),
        clusters: retained.clusters.clone(),
        carets: retained.carets.clone(),
        movements: retained.movements.clone(),
        texts: retained.texts.clone(),
        semantics: retained.semantics.clone(),
    })
}

fn build_paint_fragments(
    prepared: &PreparedParagraph,
    projection: &Projection<'_>,
    features: SceneFeatures,
    lines: &[CachedLine],
) -> Result<(Vec<CachedFragment>, Vec<Range<usize>>), SceneError> {
    if prepared.lines().len() != lines.len() {
        return Err(SceneError::for_paragraph(
            SceneErrorKind::SourceCoverage,
            prepared.paragraph(),
        ));
    }
    let mut fragments: Vec<CachedFragment> = Vec::new();
    let mut line_fragments = Vec::with_capacity(lines.len());
    let mut glyph_index = 0_usize;
    for (line, cached_line) in prepared.lines().iter().zip(lines) {
        let fragment_start = fragments.len();
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
            let normalized_coords: Arc<[i16]> = Arc::from(run.normalized_coords());
            let run_fragment_start = fragments.len();
            for glyph in run.glyphs() {
                let glyph_expansion = if opportunity_glyphs.contains(&Some(line_glyph_index)) {
                    expansion
                } else {
                    0.0
                };
                let glyph_advance =
                    Vec2::new(glyph.advance().x + glyph_expansion, glyph.advance().y);
                let instance = glyph_index;
                glyph_index += 1;
                let position = Point::new(
                    x + glyph.offset().x,
                    cached_line.baseline - glyph.offset().y,
                );
                for segment in glyph.paint().segments() {
                    let sources = if features.has_sources() {
                        projection.local_ranges(segment.source())?
                    } else {
                        Vec::new()
                    };
                    let paint_clip = segment.clip().map(|clip| {
                        Rect::new(
                            position.x + clip.x0,
                            position.y + clip.y0,
                            position.x + clip.x1,
                            position.y + clip.y1,
                        )
                    });
                    let cached_glyph = CachedGlyph {
                        instance,
                        id: glyph.id(),
                        position,
                        advance: glyph_advance,
                        sources: sources.clone(),
                    };
                    let preceding = fragments
                        .get_mut(run_fragment_start..)
                        .and_then(|run_fragments| run_fragments.last_mut());
                    if paint_clip.is_none()
                        && let Some(preceding) = preceding
                        && preceding.paint_clip.is_none()
                        && preceding.paint == segment.slot()
                    {
                        preceding.glyphs.push(cached_glyph);
                        preceding.sources.extend(sources);
                    } else {
                        let id = SceneFragmentId(fragment_identity(
                            prepared.paragraph(),
                            fragments.len(),
                        ));
                        fragments.push(CachedFragment {
                            id,
                            glyphs: alloc::vec![cached_glyph],
                            paint: segment.slot(),
                            transform: Affine::IDENTITY,
                            sources,
                            paint_clip,
                            font: run.font().clone(),
                            font_size: run.font_size(),
                            synthesis: run.synthesis().clone(),
                            normalized_coords: Arc::clone(&normalized_coords),
                            bidi_level: run.bidi_level(),
                            script: run.script(),
                        });
                    }
                }
                x += glyph_advance.x;
                line_glyph_index += 1;
            }
        }
        line_fragments.push(fragment_start..fragments.len());
    }
    Ok((fragments, line_fragments))
}

pub(super) fn cached_cursor_step(
    step: Option<&crate::adapter::PreparedCursorStep>,
    projection: &Projection<'_>,
) -> Result<Option<CachedCursorStep>, SceneError> {
    step.map(|step| {
        let target = step.target();
        Ok(CachedCursorStep {
            target: projection.position_at(target.offset(), target.affinity())?,
            source: step
                .source()
                .map(|source| projection.local_ranges(source))
                .transpose()?,
        })
    })
    .transpose()
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

pub(super) fn projected_range(
    ranges: &[LocalRange],
    revision: DocumentRevision,
) -> ProjectedTextRange {
    ProjectedTextRange::new(
        ranges
            .iter()
            .map(|range| materialize_projected_source(range, revision))
            .collect(),
    )
}

pub(super) fn materialize_projected_source(
    range: &LocalRange,
    revision: DocumentRevision,
) -> ProjectedTextSource {
    match range {
        LocalRange::Snapshot { text, bytes } => {
            ProjectedTextSource::Snapshot(SnapshotTextRange::new(revision, *text, bytes.clone()))
        }
        LocalRange::Composition { id, epoch, bytes } => ProjectedTextSource::Composition(
            crate::CompositionTextRange::new(*id, *epoch, bytes.clone()),
        ),
    }
}

pub(super) fn projected_position(
    position: LocalPosition,
    revision: DocumentRevision,
) -> ProjectedTextPosition {
    match position {
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
    ranges: &[LocalRange],
    revision: DocumentRevision,
) -> Option<SnapshotTextRange> {
    let [LocalRange::Snapshot { text, bytes }] = ranges else {
        return None;
    };
    Some(SnapshotTextRange::new(revision, *text, bytes.clone()))
}

pub(super) fn materialize_cursor_step(
    step: Option<&CachedCursorStep>,
    revision: DocumentRevision,
) -> Option<SceneCursorStep> {
    step.map(|step| SceneCursorStep {
        target: materialize_position(step.target, revision),
        source: step
            .source
            .as_ref()
            .map(|source| materialize_snapshot_unit(source, revision)),
    })
}

pub(super) fn materialize_range(
    range: &LocalRange,
    revision: DocumentRevision,
) -> SnapshotTextRange {
    let LocalRange::Snapshot { text, bytes } = range else {
        unreachable!("committed geometry cannot contain composition source")
    };
    SnapshotTextRange::new(revision, *text, bytes.clone())
}

pub(super) fn materialize_snapshot_unit(
    ranges: &[LocalRange],
    revision: DocumentRevision,
) -> SnapshotTextUnit {
    SnapshotTextUnit::new(
        ranges
            .iter()
            .map(|range| materialize_range(range, revision))
            .collect(),
    )
}

pub(super) fn materialize_position(
    position: LocalPosition,
    revision: DocumentRevision,
) -> SnapshotTextPosition {
    let LocalPosition::Snapshot {
        text,
        byte,
        affinity,
    } = position
    else {
        unreachable!("committed geometry cannot contain a composition position")
    };
    SnapshotTextPosition::new(revision, text, byte, affinity)
}
