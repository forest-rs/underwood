// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Snapshot-independent cached geometry and source-aware scene lowering.
//!
//! This module owns conversion from prepared paragraphs to reusable geometry;
//! it explicitly does not own shaping or public scene interaction policy.

use super::*;

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
) -> Result<CachedGeometry, SceneError> {
    let empty_line_height = f64::from(projection.default_font_size)
        * f64::from(projection.default_inline_flow.line_height().multiplier());
    let mut line_top = 0.0;
    let mut lines = Vec::new();
    let mut fragments = Vec::new();
    let mut clusters = Vec::new();
    let mut carets = Vec::new();
    let mut glyph_index = 0;

    for line in prepared.lines() {
        let line_index = lines.len();
        let fragment_start = fragments.len();
        let baseline = line_top + line.baseline();
        let mut unit_x = 0.0_f64;
        for unit in line.units() {
            let paragraph_source = unit.source();
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
            let semantic_id = hit_slices.first().map_or_else(
                || projection.semantic_for_range(paragraph_source),
                |slice| Ok(slice.semantic_id),
            )?;
            let next_x = unit_x + unit.advance();
            let bounds = Rect::new(unit_x, line_top, next_x, line_top + line.height());
            clusters.push(CachedCluster {
                sources,
                semantic_id,
                hit_slices,
                bounds,
                line: line_index,
                left,
                right,
                bidi_level: unit.bidi_level(),
            });
            unit_x = next_x;
        }
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
                hit_slices: Vec::new(),
                bounds: Rect::new(0.0, line_top, 0.0, line_top + line.height()),
                line: line_index,
                left: position,
                right: position,
                bidi_level: 0,
            });
        }
        let mut x = 0.0_f64;
        for run in line.runs() {
            let normalized_coords: Arc<[i16]> = Arc::from(run.normalized_coords());
            for glyph in run.glyphs() {
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
                            advance: glyph.advance(),
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
                x += glyph.advance().x;
            }
        }
        lines.push(CachedLine {
            bounds: Rect::new(
                0.0,
                line_top,
                line.advance().max(1.0),
                line_top + line.height(),
            ),
            advance: line.advance(),
            sources: projection.local_ranges(line.source())?,
            fragments: fragment_start..fragments.len(),
            break_reason: line.break_reason(),
            baseline,
            content_ascent: line.content_ascent(),
            content_descent: line.content_descent(),
        });
        line_top += line.height();
    }

    if prepared.lines().is_empty() && projection.text.is_empty() && !projection.spans.is_empty() {
        let position = projection.position_at(0, TextAffinity::Downstream)?;
        let sources = projection.local_ranges(0..0)?;
        clusters.push(CachedCluster {
            semantic_id: projection.semantic_for_range(0..0)?,
            sources,
            hit_slices: Vec::new(),
            bounds: Rect::new(0.0, 0.0, 0.0, empty_line_height),
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
            bounds: bounds.unwrap_or(Rect::new(0.0, 0.0, 0.0, empty_line_height)),
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
            0.0,
            0.0,
            1.0,
            empty_line_height,
        ));
        carets.push(CachedCaret {
            position: movement.position,
            bounds: Rect::new(
                caret.inline(),
                line_bounds.y0,
                caret.inline() + 1.0,
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
            empty_line_height
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
