// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Snapshot-independent cached geometry and source-aware scene lowering.
//!
//! This module owns conversion from prepared paragraphs to reusable geometry;
//! it explicitly does not own shaping or public scene interaction policy.

use super::*;
use crate::adapter::{ClusterBoundary, ClusterWhitespace};
use core::mem::size_of;

#[derive(Clone, Debug)]
pub(super) struct CachedGeometry {
    pub(super) height: f64,
    pub(super) lines: Vec<CachedLine>,
    pub(super) fragments: Vec<CachedFragment>,
    pub(super) clusters: Vec<CachedCluster>,
    pub(super) carets: Vec<CachedCaret>,
    pub(super) movements: Vec<CachedCursorMovement>,
    pub(super) texts: Vec<LocalRange>,
    pub(super) semantics: Vec<CachedSemantic>,
}

#[derive(Clone, Debug)]
pub(super) struct CachedLine {
    bounds: Rect,
    advance: f64,
    sources: Vec<LocalRange>,
    fragments: Range<usize>,
    break_reason: LineBreakReason,
    baseline: f64,
    content_ascent: f64,
    content_descent: f64,
    adjustment: LineAdjustment,
}

#[derive(Clone, Debug)]
pub(super) struct CachedFragment {
    id: SceneFragmentId,
    glyphs: Vec<CachedGlyph>,
    paint: PaintSlot,
    transform: Affine,
    sources: Vec<LocalRange>,
    paint_clip: Option<Rect>,
    font: FontData,
    font_size: f32,
    synthesis: FontSynthesis,
    normalized_coords: Arc<[i16]>,
    bidi_level: u8,
    script: [u8; 4],
}

#[derive(Clone, Debug)]
pub(super) struct CachedGlyph {
    instance: usize,
    id: u32,
    position: Point,
    advance: Vec2,
    sources: Vec<LocalRange>,
}

#[derive(Clone, Debug)]
pub(super) struct CachedCluster {
    sources: Vec<LocalRange>,
    semantic_id: SemanticId,
    boundary: ClusterBoundary,
    whitespace: ClusterWhitespace,
    hit_slices: Vec<CachedHitSlice>,
    bounds: Rect,
    line: usize,
    left: LocalPosition,
    right: LocalPosition,
    bidi_level: u8,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct CachedHitSlice {
    semantic_id: SemanticId,
    x0: f64,
    x1: f64,
}

#[derive(Clone, Debug)]
pub(super) struct CachedCaret {
    position: LocalPosition,
    bounds: Rect,
}

#[derive(Clone, Debug)]
pub(super) struct CachedCursorMovement {
    position: LocalPosition,
    previous_visual: Option<CachedCursorStep>,
    next_visual: Option<CachedCursorStep>,
    previous_logical: Option<CachedCursorStep>,
    next_logical: Option<CachedCursorStep>,
}

#[derive(Clone, Debug)]
pub(super) struct CachedCursorStep {
    target: LocalPosition,
    source: Option<Vec<LocalRange>>,
}

#[derive(Clone, Debug)]
pub(super) struct CachedSemantic {
    semantic_id: SemanticId,
    paragraph_role: Option<ParagraphRole>,
    inline_role: Option<InlineRole>,
    source: Option<Vec<LocalRange>>,
    bounds: Rect,
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
    pub(super) fn accounted_owned_bytes(&self) -> usize {
        let mut bytes = vec_bytes::<CachedLine>(self.lines.capacity())
            .saturating_add(vec_bytes::<CachedFragment>(self.fragments.capacity()))
            .saturating_add(vec_bytes::<CachedCluster>(self.clusters.capacity()))
            .saturating_add(vec_bytes::<CachedCaret>(self.carets.capacity()))
            .saturating_add(vec_bytes::<CachedCursorMovement>(self.movements.capacity()))
            .saturating_add(vec_bytes::<LocalRange>(self.texts.capacity()))
            .saturating_add(vec_bytes::<CachedSemantic>(self.semantics.capacity()));
        for line in &self.lines {
            bytes = bytes.saturating_add(vec_bytes::<LocalRange>(line.sources.capacity()));
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
        for cluster in &self.clusters {
            bytes = bytes
                .saturating_add(vec_bytes::<LocalRange>(cluster.sources.capacity()))
                .saturating_add(vec_bytes::<CachedHitSlice>(cluster.hit_slices.capacity()));
        }
        for movement in &self.movements {
            bytes = bytes
                .saturating_add(cursor_step_bytes(movement.previous_visual.as_ref()))
                .saturating_add(cursor_step_bytes(movement.next_visual.as_ref()))
                .saturating_add(cursor_step_bytes(movement.previous_logical.as_ref()))
                .saturating_add(cursor_step_bytes(movement.next_logical.as_ref()));
        }
        for semantic in &self.semantics {
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
    for line in &mut geometry.lines {
        rebind_ranges(&mut line.sources, id, epoch);
    }
    for fragment in &mut geometry.fragments {
        rebind_ranges(&mut fragment.sources, id, epoch);
        for glyph in &mut fragment.glyphs {
            rebind_ranges(&mut glyph.sources, id, epoch);
        }
    }
    for cluster in &mut geometry.clusters {
        rebind_ranges(&mut cluster.sources, id, epoch);
        rebind_position(&mut cluster.left, id, epoch);
        rebind_position(&mut cluster.right, id, epoch);
    }
    for caret in &mut geometry.carets {
        rebind_position(&mut caret.position, id, epoch);
    }
    for movement in &mut geometry.movements {
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
    rebind_ranges(&mut geometry.texts, id, epoch);
    for semantic in &mut geometry.semantics {
        if let Some(source) = &mut semantic.source {
            rebind_ranges(source, id, epoch);
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
    let mut fragments = Vec::new();
    let mut clusters = Vec::new();
    let mut carets = Vec::new();
    let mut glyph_index = 0;
    let mut caret_maps = Vec::new();

    for line in prepared.lines() {
        let line_index = lines.len();
        let fragment_start = fragments.len();
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
        let mut caret_map = CaretAdjustmentMap::with_capacity(line.units().len());
        for unit in line.units() {
            let paragraph_source = unit.source();
            let unit_expansion = if opportunity_sources.contains(&paragraph_source) {
                expansion
            } else {
                0.0
            };
            let sources = projection.local_ranges(paragraph_source.clone())?;
            let left = projection.position_at(unit.left().offset(), unit.left().affinity())?;
            let right = projection.position_at(unit.right().offset(), unit.right().affinity())?;
            let mut slice_x = unit_x;
            let mut hit_slices = Vec::with_capacity(unit.slices().len());
            for slice in unit.slices() {
                let next_x = slice_x + slice.advance();
                let source = slice.source();
                projection.local_ranges(source.clone())?;
                hit_slices.push(CachedHitSlice {
                    semantic_id: projection.semantic_for_range(source)?,
                    x0: slice_x,
                    x1: next_x,
                });
                slice_x = next_x;
            }
            if unit_expansion > 0.0
                && let Some(last) = hit_slices.last_mut()
            {
                last.x1 += unit_expansion;
            }
            let semantic_id = hit_slices.first().map_or_else(
                || projection.semantic_for_range(paragraph_source),
                |slice| Ok(slice.semantic_id),
            )?;
            let adjusted_unit_advance = unit.advance() + unit_expansion;
            let next_x = unit_x + adjusted_unit_advance;
            let bounds = Rect::new(
                unit_x,
                current_line_top,
                next_x,
                current_line_top + line.height(),
            );
            clusters.push(CachedCluster {
                sources,
                semantic_id,
                boundary: unit.boundary(),
                whitespace: unit.whitespace(),
                hit_slices,
                bounds,
                line: line_index,
                left,
                right,
                bidi_level: unit.bidi_level(),
            });
            caret_map.push(
                original_unit_x,
                original_unit_x + unit.advance(),
                adjusted_unit_x,
                adjusted_unit_x + adjusted_unit_advance,
            );
            original_unit_x += unit.advance();
            adjusted_unit_x += adjusted_unit_advance;
            unit_x = next_x;
        }
        caret_map.finish_empty();
        caret_maps.push(caret_map);
        if line.units().is_empty() && !projection.spans.is_empty() {
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
        let mut x = inline_start;
        line_glyph_index = 0;
        for run in line.runs() {
            let normalized_coords: Arc<[i16]> = Arc::from(run.normalized_coords());
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
                let position = Point::new(x + glyph.offset().x, baseline - glyph.offset().y);
                for segment in glyph.paint().segments() {
                    let sources = projection.local_ranges(segment.source())?;
                    let paint_clip = segment.clip().map(|clip| {
                        Rect::new(
                            position.x + clip.x0,
                            position.y + clip.y0,
                            position.x + clip.x1,
                            position.y + clip.y1,
                        )
                    });
                    let id =
                        SceneFragmentId(fragment_identity(prepared.paragraph(), fragments.len()));
                    fragments.push(CachedFragment {
                        id,
                        glyphs: alloc::vec![CachedGlyph {
                            instance,
                            id: glyph.id(),
                            position,
                            advance: glyph_advance,
                            sources: sources.clone(),
                        }],
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
                x += glyph_advance.x;
                line_glyph_index += 1;
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
            advance: adjusted_advance,
            sources: projection.local_ranges(line.source())?,
            fragments: fragment_start..fragments.len(),
            break_reason: line.break_reason(),
            baseline,
            content_ascent: line.content_ascent(),
            content_descent: line.content_descent(),
            adjustment,
        });
        line_top = line_top.max(current_line_top + line.height());
    }

    if prepared.lines().is_empty()
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
    if !projection.spans.is_empty()
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

    let movements = if projection.spans.is_empty() {
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
    for (prepared_movement, movement) in prepared.movements().iter().zip(&movements) {
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
            position: movement.position,
            bounds: Rect::new(
                line_bounds.x0 + adjusted_inline,
                line_bounds.y0,
                line_bounds.x0 + adjusted_inline + 1.0,
                line_bounds.y1,
            ),
        });
    }
    let texts = projection
        .spans
        .iter()
        .map(|span| span.local_range(span.paragraph.start, span.paragraph.end))
        .collect();

    Ok(CachedGeometry {
        height: if prepared.lines().is_empty() {
            empty_bounds.y1
        } else {
            line_top
        },
        lines,
        fragments,
        clusters,
        carets,
        movements,
        texts,
        semantics,
    })
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

pub(super) fn materialize_geometry(
    geometry: &CachedGeometry,
    revision: DocumentRevision,
    y_offset: f64,
    lines: &mut Vec<SceneLine>,
    fragments: &mut Vec<SceneFragment>,
    clusters: &mut Vec<SceneCluster>,
    carets: &mut Vec<SceneCaretStop>,
    movements: &mut Vec<SceneCursorMovement>,
    texts: &mut Vec<SnapshotTextRange>,
    semantics: &mut Vec<SemanticFragment>,
) {
    let translate = Vec2::new(0.0, y_offset);
    let line_base = lines.len();
    let fragment_base = fragments.len();
    lines.extend(geometry.lines.iter().map(|line| {
        SceneLine {
            bounds: line.bounds + translate,
            advance: line.advance,
            sources: line
                .sources
                .iter()
                .map(|source| materialize_range(source, revision))
                .collect(),
            fragments: fragment_base + line.fragments.start..fragment_base + line.fragments.end,
            break_reason: line.break_reason,
            baseline: line.baseline + y_offset,
            content_ascent: line.content_ascent,
            content_descent: line.content_descent,
            adjustment: line.adjustment,
        }
    }));
    fragments.extend(geometry.fragments.iter().map(|fragment| {
        let (source, additional_sources) = materialize_sources(&fragment.sources, revision);
        SceneFragment {
            id: fragment.id,
            glyphs: fragment
                .glyphs
                .iter()
                .map(|glyph| {
                    let (source, additional_sources) =
                        materialize_sources(&glyph.sources, revision);
                    SceneGlyph {
                        instance: SceneGlyphInstanceId(fragment_base + glyph.instance),
                        id: glyph.id,
                        position: glyph.position + translate,
                        advance: glyph.advance,
                        source,
                        additional_sources,
                    }
                })
                .collect(),
            paint: fragment.paint,
            transform: fragment.transform,
            source,
            additional_sources,
            paint_clip: fragment.paint_clip.map(|clip| clip + translate),
            font: fragment.font.clone(),
            font_size: fragment.font_size,
            synthesis: fragment.synthesis.clone(),
            normalized_coords: Arc::clone(&fragment.normalized_coords),
            bidi_level: fragment.bidi_level,
            script: fragment.script,
        }
    }));
    clusters.extend(geometry.clusters.iter().map(|cluster| {
        SceneCluster {
            source: materialize_snapshot_unit(&cluster.sources, revision),
            semantic_id: cluster.semantic_id,
            boundary: cluster.boundary,
            whitespace: cluster.whitespace,
            hit_slices: cluster
                .hit_slices
                .iter()
                .map(|slice| SceneHitSlice {
                    semantic_id: slice.semantic_id,
                    x0: slice.x0,
                    x1: slice.x1,
                })
                .collect(),
            bounds: cluster.bounds + translate,
            line: line_base + cluster.line,
            left: materialize_position(cluster.left, revision),
            right: materialize_position(cluster.right, revision),
            bidi_level: cluster.bidi_level,
        }
    }));
    carets.extend(geometry.carets.iter().map(|caret| SceneCaretStop {
        position: materialize_position(caret.position, revision),
        bounds: caret.bounds + translate,
    }));
    movements.extend(
        geometry
            .movements
            .iter()
            .map(|movement| SceneCursorMovement {
                position: materialize_position(movement.position, revision),
                previous_visual: materialize_cursor_step(
                    movement.previous_visual.as_ref(),
                    revision,
                ),
                next_visual: materialize_cursor_step(movement.next_visual.as_ref(), revision),
                previous_logical: materialize_cursor_step(
                    movement.previous_logical.as_ref(),
                    revision,
                ),
                next_logical: materialize_cursor_step(movement.next_logical.as_ref(), revision),
            }),
    );
    texts.extend(
        geometry
            .texts
            .iter()
            .map(|range| materialize_range(range, revision)),
    );
    semantics.extend(geometry.semantics.iter().map(|semantic| {
        SemanticFragment {
            semantic_id: semantic.semantic_id,
            paragraph_role: semantic.paragraph_role,
            inline_role: semantic.inline_role,
            source: semantic
                .source
                .as_ref()
                .map(|source| materialize_snapshot_range(source, revision)),
            bounds: semantic.bounds + translate,
        }
    }));
}

pub(super) fn materialize_projected_geometry(
    geometry: &CachedGeometry,
    revision: DocumentRevision,
    y_offset: f64,
    lines: &mut Vec<SceneLine<ProjectedTextRange>>,
    fragments: &mut Vec<SceneFragment<ProjectedTextRange>>,
    clusters: &mut Vec<SceneCluster<ProjectedTextRange, ProjectedTextPosition>>,
    carets: &mut Vec<SceneCaretStop<ProjectedTextPosition>>,
    movements: &mut Vec<SceneCursorMovement<ProjectedTextRange, ProjectedTextPosition>>,
    semantics: &mut Vec<SemanticFragment>,
) {
    let translate = Vec2::new(0.0, y_offset);
    let line_base = lines.len();
    let fragment_base = fragments.len();
    lines.extend(geometry.lines.iter().map(|line| {
        SceneLine {
            bounds: line.bounds + translate,
            advance: line.advance,
            sources: line
                .sources
                .iter()
                .map(|source| projected_range(core::slice::from_ref(source), revision))
                .collect(),
            fragments: fragment_base + line.fragments.start..fragment_base + line.fragments.end,
            break_reason: line.break_reason,
            baseline: line.baseline + y_offset,
            content_ascent: line.content_ascent,
            content_descent: line.content_descent,
            adjustment: line.adjustment,
        }
    }));
    fragments.extend(geometry.fragments.iter().map(|fragment| {
        SceneFragment {
            id: fragment.id,
            glyphs: fragment
                .glyphs
                .iter()
                .map(|glyph| SceneGlyph {
                    instance: SceneGlyphInstanceId(fragment_base + glyph.instance),
                    id: glyph.id,
                    position: glyph.position + translate,
                    advance: glyph.advance,
                    source: projected_range(&glyph.sources, revision),
                    additional_sources: Vec::new(),
                })
                .collect(),
            paint: fragment.paint,
            transform: fragment.transform,
            source: projected_range(&fragment.sources, revision),
            additional_sources: Vec::new(),
            paint_clip: fragment.paint_clip.map(|clip| clip + translate),
            font: fragment.font.clone(),
            font_size: fragment.font_size,
            synthesis: fragment.synthesis.clone(),
            normalized_coords: Arc::clone(&fragment.normalized_coords),
            bidi_level: fragment.bidi_level,
            script: fragment.script,
        }
    }));
    clusters.extend(geometry.clusters.iter().map(|cluster| {
        SceneCluster {
            source: projected_range(&cluster.sources, revision),
            semantic_id: cluster.semantic_id,
            boundary: cluster.boundary,
            whitespace: cluster.whitespace,
            hit_slices: cluster
                .hit_slices
                .iter()
                .map(|slice| SceneHitSlice {
                    semantic_id: slice.semantic_id,
                    x0: slice.x0,
                    x1: slice.x1,
                })
                .collect(),
            bounds: cluster.bounds + translate,
            line: line_base + cluster.line,
            left: projected_position(cluster.left, revision),
            right: projected_position(cluster.right, revision),
            bidi_level: cluster.bidi_level,
        }
    }));
    carets.extend(geometry.carets.iter().map(|caret| SceneCaretStop {
        position: projected_position(caret.position, revision),
        bounds: caret.bounds + translate,
    }));
    movements.extend(
        geometry
            .movements
            .iter()
            .map(|movement| SceneCursorMovement {
                position: projected_position(movement.position, revision),
                previous_visual: projected_cursor_step(movement.previous_visual.as_ref(), revision),
                next_visual: projected_cursor_step(movement.next_visual.as_ref(), revision),
                previous_logical: projected_cursor_step(
                    movement.previous_logical.as_ref(),
                    revision,
                ),
                next_logical: projected_cursor_step(movement.next_logical.as_ref(), revision),
            }),
    );
    semantics.extend(geometry.semantics.iter().map(|semantic| {
        SemanticFragment {
            semantic_id: semantic.semantic_id,
            paragraph_role: semantic.paragraph_role,
            inline_role: semantic.inline_role,
            source: semantic
                .source
                .as_ref()
                .and_then(|sources| materialize_optional_snapshot_range(sources, revision)),
            bounds: semantic.bounds + translate,
        }
    }));
}

pub(super) fn projected_cursor_step(
    step: Option<&CachedCursorStep>,
    revision: DocumentRevision,
) -> Option<SceneCursorStep<ProjectedTextRange, ProjectedTextPosition>> {
    step.map(|step| SceneCursorStep {
        target: projected_position(step.target, revision),
        source: step
            .source
            .as_ref()
            .map(|source| projected_range(source, revision)),
    })
}

pub(super) fn projected_range(
    ranges: &[LocalRange],
    revision: DocumentRevision,
) -> ProjectedTextRange {
    ProjectedTextRange::new(
        ranges
            .iter()
            .map(|range| match range {
                LocalRange::Snapshot { text, bytes } => ProjectedTextSource::Snapshot(
                    SnapshotTextRange::new(revision, *text, bytes.clone()),
                ),
                LocalRange::Composition { id, epoch, bytes } => ProjectedTextSource::Composition(
                    crate::CompositionTextRange::new(*id, *epoch, bytes.clone()),
                ),
            })
            .collect(),
    )
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

pub(super) fn materialize_snapshot_range(
    ranges: &[LocalRange],
    revision: DocumentRevision,
) -> SnapshotTextRange {
    let [range] = ranges else {
        unreachable!("committed geometry source must remain within one semantic text leaf")
    };
    materialize_range(range, revision)
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

pub(super) fn materialize_sources(
    ranges: &[LocalRange],
    revision: DocumentRevision,
) -> (SnapshotTextRange, Vec<SnapshotTextRange>) {
    let mut sources = ranges
        .iter()
        .map(|source| materialize_range(source, revision));
    let source = sources
        .next()
        .expect("validated scene observations always retain source");
    (source, sources.collect())
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
