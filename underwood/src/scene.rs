// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::ops::Range;

use crate::adapter::{
    FontSynthesis, FormationWork, InlineFlowRun, InlineFlowStyleId, LineBreakReason, PaintRun,
    ParagraphConstraints, ParagraphFormation, ParagraphInput, PreparedParagraph, ShapingRun,
    ShapingStyleId, TextAffinity,
};
use crate::document::Paragraph;
use crate::{
    Affine, CompositionError, CompositionErrorKind, CompositionId, CompositionSession,
    CompositionStart, DocumentRevision, DocumentSnapshot, FontData, InlineFlowStyle, InlineRole,
    PaintSlot, PaintTable, ParagraphId, ParagraphRole, Point, Rect, SceneError, SceneErrorKind,
    SceneRequest, SelectionError, SelectionErrorKind, SemanticId, ShapingStyle,
    SnapshotTextPosition, SnapshotTextRange, SnapshotTextSelection, SnapshotTextSelectionSet,
    TextId, TextMovement, TextSelectionMode, Vec2,
};

/// Mutable owner of one paragraph adapter and its retained stage caches.
pub struct LayoutEngine {
    paragraphs: Box<dyn ParagraphFormation>,
    cache: Vec<ParagraphCache>,
    composition_cache: Vec<ParagraphCache>,
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
    /// Creates an engine owning exactly one configured paragraph adapter.
    #[must_use]
    pub fn new(paragraphs: impl ParagraphFormation + 'static) -> Self {
        Self {
            paragraphs: Box::new(paragraphs),
            cache: Vec::new(),
            composition_cache: Vec::new(),
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

        for paragraph in snapshot.paragraphs() {
            let projection = Projection::new(paragraph, request)?;
            let cache_index = prepare_paragraph_geometry(
                self.paragraphs.as_mut(),
                &mut self.cache,
                paragraph,
                &projection,
                request.width.0,
                &mut work,
            )?;

            let geometry = &self.cache[cache_index].geometry;
            materialize_geometry(
                geometry,
                snapshot.revision(),
                y_offset,
                &mut lines,
                &mut fragments,
                &mut clusters,
                &mut carets,
                &mut movements,
                &mut texts,
                &mut semantics,
            );
            y_offset += geometry.height;
        }

        work.paint = StageWork {
            paragraphs: snapshot.paragraphs().len(),
            records: fragments.len(),
        };
        Ok(SceneOutput {
            scene: TextScene {
                document: snapshot.id(),
                revision: snapshot.revision(),
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
        })
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

        for paragraph in snapshot.paragraphs() {
            let transient = paragraph.id.index == target.paragraph;
            let projection = if transient {
                Projection::with_composition(paragraph, request, composition)?
            } else {
                Projection::new(paragraph, request)?
            };
            let cache = if transient {
                &mut self.composition_cache
            } else {
                &mut self.cache
            };
            let cache_index = prepare_paragraph_geometry(
                self.paragraphs.as_mut(),
                cache,
                paragraph,
                &projection,
                request.width.0,
                &mut work,
            )?;
            let geometry = &cache[cache_index].geometry;
            materialize_projected_geometry(
                geometry,
                snapshot.revision(),
                y_offset,
                &mut lines,
                &mut fragments,
                &mut clusters,
                &mut carets,
                &mut movements,
                &mut semantics,
            );
            y_offset += geometry.height;
        }

        work.paint = StageWork {
            paragraphs: snapshot.paragraphs().len(),
            records: fragments.len(),
        };
        Ok(CompositionSceneOutput {
            scene: CompositionScene {
                document: snapshot.id(),
                revision: snapshot.revision(),
                composition: composition.id(),
                epoch: composition.epoch(),
                lines,
                fragments,
                clusters,
                carets,
                movements,
                paint: request.paint.clone(),
                semantics,
            },
            work,
        })
    }
}

fn prepare_paragraph_geometry(
    paragraphs: &mut dyn ParagraphFormation,
    cache: &mut Vec<ParagraphCache>,
    paragraph: &Paragraph,
    projection: &Projection<'_>,
    width: f64,
    work: &mut WorkReport,
) -> Result<usize, SceneError> {
    let cache_index = cache
        .iter()
        .position(|entry| entry.paragraph == paragraph.id);
    let formation_matches = cache_index.is_some_and(|index| {
        cache[index]
            .formation_key
            .matches(paragraph.version, projection, width)
    });
    let paint_matches =
        cache_index.is_some_and(|index| cache[index].paint_runs == projection.paint_runs);
    if formation_matches && paint_matches {
        let cache_index = cache_index.expect("a reusable cache index must exist");
        if let Some((id, epoch)) = projection.composition_identity() {
            rebind_composition_geometry(&mut cache[cache_index].geometry, id, epoch);
        }
        work.reused_paragraphs += 1;
        return Ok(cache_index);
    }

    let shaping_styles: Vec<_> = projection
        .shaping_styles
        .iter()
        .map(|style| (*style).clone())
        .collect();
    let text_len = u32::try_from(projection.text.len())
        .map_err(|_| SceneError::for_paragraph(SceneErrorKind::SourceCoverage, paragraph.id))?;
    let constraints = ParagraphConstraints::try_new(width)
        .map_err(|error| SceneError::from_preparation(paragraph.id, error.kind()))?;
    let output = paragraphs
        .form(
            ParagraphInput::new(
                paragraph.id,
                &projection.text,
                &shaping_styles,
                &projection.shaping_runs,
                &projection.inline_flow_styles,
                &projection.inline_flow_runs,
                &projection.paint_runs,
            ),
            constraints,
        )
        .map_err(|error| SceneError::from_preparation(paragraph.id, error.kind()))?;
    if output.paragraph().paragraph() != paragraph.id || output.paragraph().text_len() != text_len {
        return Err(SceneError::for_paragraph(
            SceneErrorKind::SourceCoverage,
            paragraph.id,
        ));
    }
    validate_prepared(output.paragraph(), projection)?;
    record_formation_work(work, output.work());
    if projection.text.is_empty() && !formation_matches {
        work.flow.add_paragraph(1);
    }
    let geometry = build_geometry(output.paragraph(), projection)?;
    work.geometry.add_paragraph(geometry.fragments.len());
    let formation_key = FormationKey::new(
        paragraph.version,
        projection.text.clone(),
        shaping_styles,
        projection.shaping_runs.clone(),
        projection.inline_flow_styles.clone(),
        projection.inline_flow_runs.clone(),
        width,
        projection.empty_line_height_key(),
        projection,
    );
    if let Some(index) = cache_index {
        let entry = &mut cache[index];
        entry.formation_key = formation_key;
        entry.paint_runs = projection.paint_runs.clone();
        entry.geometry = geometry;
        Ok(index)
    } else {
        cache.push(ParagraphCache {
            paragraph: paragraph.id,
            formation_key,
            paint_runs: projection.paint_runs.clone(),
            geometry,
        });
        Ok(cache.len() - 1)
    }
}

#[derive(Clone, Debug, PartialEq)]
struct FormationKey {
    version: u64,
    text: alloc::string::String,
    source_map: Vec<ProjectionSourceKey>,
    shaping_styles: Vec<ShapingStyle>,
    shaping_runs: Vec<ShapingRun>,
    inline_flow_styles: Vec<InlineFlowStyle>,
    inline_flow_runs: Vec<InlineFlowRun>,
    width: u64,
    empty_line_height: u64,
}

impl FormationKey {
    fn new(
        version: u64,
        text: alloc::string::String,
        shaping_styles: Vec<ShapingStyle>,
        shaping_runs: Vec<ShapingRun>,
        inline_flow_styles: Vec<InlineFlowStyle>,
        inline_flow_runs: Vec<InlineFlowRun>,
        width: f64,
        empty_line_height: u64,
        projection: &Projection<'_>,
    ) -> Self {
        Self {
            version,
            text,
            source_map: ProjectionSourceKey::from_projection(projection),
            shaping_styles,
            shaping_runs,
            inline_flow_styles,
            inline_flow_runs,
            width: width.to_bits(),
            empty_line_height,
        }
    }

    fn matches(&self, version: u64, projection: &Projection<'_>, width: f64) -> bool {
        self.version == version
            && self.text == projection.text
            && self.source_map == ProjectionSourceKey::from_projection(projection)
            && self.shaping_styles.len() == projection.shaping_styles.len()
            && self
                .shaping_styles
                .iter()
                .zip(&projection.shaping_styles)
                .all(|(cached, projected)| cached == *projected)
            && self.shaping_runs == projection.shaping_runs
            && self.inline_flow_styles == projection.inline_flow_styles
            && self.inline_flow_runs == projection.inline_flow_runs
            && self.width == width.to_bits()
            && self.empty_line_height == projection.empty_line_height_key()
    }
}

#[derive(Clone, Debug)]
struct ParagraphCache {
    paragraph: ParagraphId,
    formation_key: FormationKey,
    paint_runs: Vec<PaintRun>,
    geometry: CachedGeometry,
}

#[derive(Clone, Debug)]
struct Projection<'a> {
    paragraph: ParagraphId,
    text: alloc::string::String,
    spans: Vec<LeafSpan>,
    shaping_styles: Vec<&'a ShapingStyle>,
    shaping_runs: Vec<ShapingRun>,
    inline_flow_styles: Vec<InlineFlowStyle>,
    inline_flow_runs: Vec<InlineFlowRun>,
    paint_runs: Vec<PaintRun>,
    default_font_size: f32,
    default_inline_flow: InlineFlowStyle,
    paragraph_semantic: SemanticId,
    paragraph_role: ParagraphRole,
}

impl<'a> Projection<'a> {
    fn new(paragraph: &Paragraph, request: &'a SceneRequest<'_>) -> Result<Self, SceneError> {
        let text = paragraph.projected_text();
        let mut spans = Vec::with_capacity(paragraph.leaves.len());
        let mut shaping_styles = Vec::new();
        let mut shaping_runs = Vec::with_capacity(paragraph.leaves.len());
        let mut inline_flow_styles = Vec::new();
        let mut inline_flow_runs = Vec::with_capacity(paragraph.leaves.len());
        let mut paint_runs = Vec::with_capacity(paragraph.leaves.len());
        let mut start = 0_u32;
        for leaf in &paragraph.leaves {
            let len = u32::try_from(leaf.text.len()).map_err(|_| {
                SceneError::for_paragraph(SceneErrorKind::SourceCoverage, paragraph.id)
            })?;
            let end = start.checked_add(len).ok_or_else(|| {
                SceneError::for_paragraph(SceneErrorKind::SourceCoverage, paragraph.id)
            })?;
            let style = request.styles.style_for(leaf.id);
            spans.push(LeafSpan {
                paragraph: start..end,
                text: leaf.id,
                source: LeafSpanSource::Snapshot { start: 0 },
                leaf_len: len,
                role: leaf.role,
                semantic: leaf.semantic_id(),
            });
            if start != end {
                append_shaping_run(
                    &mut shaping_styles,
                    &mut shaping_runs,
                    start..end,
                    style.shaping(),
                    paragraph.id,
                )?;
                append_inline_flow_run(
                    &mut inline_flow_styles,
                    &mut inline_flow_runs,
                    start..end,
                    style.inline_flow(),
                    paragraph.id,
                )?;
                append_paint_run(&mut paint_runs, start..end, style.paint());
            }
            start = end;
        }
        Ok(Self {
            paragraph: paragraph.id,
            text,
            spans,
            shaping_styles,
            shaping_runs,
            inline_flow_styles,
            inline_flow_runs,
            paint_runs,
            default_font_size: request.styles.default_style().shaping().font_size(),
            default_inline_flow: request.styles.default_style().inline_flow(),
            paragraph_semantic: paragraph.semantic_id(),
            paragraph_role: paragraph.role,
        })
    }

    fn with_composition(
        paragraph: &Paragraph,
        request: &'a SceneRequest<'_>,
        composition: &CompositionSession,
    ) -> Result<Self, SceneError> {
        let target = composition.target_text().ok_or_else(|| {
            SceneError::for_paragraph(SceneErrorKind::InvalidComposition, paragraph.id)
        })?;
        if target.paragraph != paragraph.id.index {
            return Err(SceneError::for_paragraph(
                SceneErrorKind::InvalidComposition,
                paragraph.id,
            ));
        }
        let ranges = composition.replacement_ranges();
        if ranges.is_empty()
            || ranges.iter().any(|range| {
                range.revision() != composition.base_revision() || range.text() != target
            })
        {
            return Err(SceneError::for_paragraph(
                SceneErrorKind::InvalidComposition,
                paragraph.id,
            ));
        }

        let mut text = alloc::string::String::new();
        let mut spans = Vec::with_capacity(paragraph.leaves.len() + ranges.len() + 1);
        let mut shaping_styles = Vec::new();
        let mut shaping_runs = Vec::with_capacity(paragraph.leaves.len() + ranges.len() + 1);
        let mut inline_flow_styles = Vec::new();
        let mut inline_flow_runs = Vec::with_capacity(paragraph.leaves.len() + ranges.len() + 1);
        let mut paint_runs = Vec::with_capacity(paragraph.leaves.len() + ranges.len() + 1);
        let mut target_found = false;

        for leaf in &paragraph.leaves {
            let style = request.styles.style_for(leaf.id);
            if leaf.id != target {
                append_projection_span(
                    paragraph.id,
                    &mut text,
                    &mut spans,
                    &mut shaping_styles,
                    &mut shaping_runs,
                    &mut inline_flow_styles,
                    &mut inline_flow_runs,
                    &mut paint_runs,
                    leaf,
                    leaf.text.as_ref(),
                    LeafSpanSource::Snapshot { start: 0 },
                    style,
                )?;
                continue;
            }
            target_found = true;

            let mut source = 0_u32;
            for (index, range) in ranges.iter().enumerate() {
                let bytes = range.bytes();
                if bytes.start < source
                    || leaf
                        .text
                        .get(bytes.start as usize..bytes.end as usize)
                        .is_none()
                {
                    return Err(SceneError::for_source(
                        SceneErrorKind::InvalidComposition,
                        paragraph.id,
                        bytes,
                    ));
                }
                if source < bytes.start {
                    let retained = leaf
                        .text
                        .get(source as usize..bytes.start as usize)
                        .ok_or_else(|| {
                            SceneError::for_source(
                                SceneErrorKind::InvalidComposition,
                                paragraph.id,
                                source..bytes.start,
                            )
                        })?;
                    append_projection_span(
                        paragraph.id,
                        &mut text,
                        &mut spans,
                        &mut shaping_styles,
                        &mut shaping_runs,
                        &mut inline_flow_styles,
                        &mut inline_flow_runs,
                        &mut paint_runs,
                        leaf,
                        retained,
                        LeafSpanSource::Snapshot { start: source },
                        style,
                    )?;
                }
                if index == 0 {
                    append_projection_span(
                        paragraph.id,
                        &mut text,
                        &mut spans,
                        &mut shaping_styles,
                        &mut shaping_runs,
                        &mut inline_flow_styles,
                        &mut inline_flow_runs,
                        &mut paint_runs,
                        leaf,
                        composition.text(),
                        LeafSpanSource::Composition {
                            id: composition.id(),
                            epoch: composition.epoch(),
                            start: 0,
                        },
                        style,
                    )?;
                }
                source = bytes.end;
            }
            let end = u32::try_from(leaf.text.len()).map_err(|_| {
                SceneError::for_paragraph(SceneErrorKind::SourceCoverage, paragraph.id)
            })?;
            if source < end {
                let retained = leaf.text.get(source as usize..).ok_or_else(|| {
                    SceneError::for_source(
                        SceneErrorKind::InvalidComposition,
                        paragraph.id,
                        source..end,
                    )
                })?;
                append_projection_span(
                    paragraph.id,
                    &mut text,
                    &mut spans,
                    &mut shaping_styles,
                    &mut shaping_runs,
                    &mut inline_flow_styles,
                    &mut inline_flow_runs,
                    &mut paint_runs,
                    leaf,
                    retained,
                    LeafSpanSource::Snapshot { start: source },
                    style,
                )?;
            }
        }
        if !target_found {
            return Err(SceneError::for_paragraph(
                SceneErrorKind::InvalidComposition,
                paragraph.id,
            ));
        }

        Ok(Self {
            paragraph: paragraph.id,
            text,
            spans,
            shaping_styles,
            shaping_runs,
            inline_flow_styles,
            inline_flow_runs,
            paint_runs,
            default_font_size: request.styles.default_style().shaping().font_size(),
            default_inline_flow: request.styles.default_style().inline_flow(),
            paragraph_semantic: paragraph.semantic_id(),
            paragraph_role: paragraph.role,
        })
    }

    fn local_ranges(&self, paragraph: Range<u32>) -> Result<Vec<LocalRange>, SceneError> {
        if self
            .text
            .get(paragraph.start as usize..paragraph.end as usize)
            .is_none()
        {
            return Err(SceneError::for_source(
                SceneErrorKind::SourceCoverage,
                self.paragraph,
                paragraph,
            ));
        }
        if paragraph.is_empty() {
            let span = span_for_position(&self.spans, paragraph.start, TextAffinity::Upstream)
                .ok_or_else(|| {
                    SceneError::for_source(
                        SceneErrorKind::SourceCoverage,
                        self.paragraph,
                        paragraph.clone(),
                    )
                })?;
            return Ok(alloc::vec![
                span.local_range(paragraph.start, paragraph.end)
            ]);
        }

        let mut covered = paragraph.start;
        let mut ranges = Vec::new();
        for span in &self.spans {
            let start = paragraph.start.max(span.paragraph.start);
            let end = paragraph.end.min(span.paragraph.end);
            if start >= end {
                continue;
            }
            if start != covered {
                return Err(SceneError::for_source(
                    SceneErrorKind::SourceCoverage,
                    self.paragraph,
                    paragraph,
                ));
            }
            ranges.push(span.local_range(start, end));
            covered = end;
        }
        if covered != paragraph.end {
            return Err(SceneError::for_source(
                SceneErrorKind::SourceCoverage,
                self.paragraph,
                paragraph,
            ));
        }
        Ok(ranges)
    }

    fn semantic_for_range(&self, paragraph: Range<u32>) -> Result<SemanticId, SceneError> {
        let mut semantics = self
            .spans
            .iter()
            .filter(|span| {
                if paragraph.is_empty() {
                    span.paragraph.start <= paragraph.start && paragraph.start <= span.paragraph.end
                } else {
                    span.paragraph.start < paragraph.end && paragraph.start < span.paragraph.end
                }
            })
            .map(|span| span.semantic);
        let Some(first) = semantics.next() else {
            return Err(SceneError::for_source(
                SceneErrorKind::SourceCoverage,
                self.paragraph,
                paragraph,
            ));
        };
        if semantics.any(|semantic| semantic != first) {
            return Err(SceneError::for_source(
                SceneErrorKind::SourceCoverage,
                self.paragraph,
                paragraph,
            ));
        }
        Ok(first)
    }

    fn position_at(
        &self,
        paragraph_offset: u32,
        affinity: TextAffinity,
    ) -> Result<LocalPosition, SceneError> {
        if !self.text.is_char_boundary(paragraph_offset as usize) {
            return Err(SceneError::for_source(
                SceneErrorKind::SourceCoverage,
                self.paragraph,
                paragraph_offset..paragraph_offset,
            ));
        }
        let span = span_for_position(&self.spans, paragraph_offset, affinity).ok_or_else(|| {
            SceneError::for_source(
                SceneErrorKind::SourceCoverage,
                self.paragraph,
                paragraph_offset..paragraph_offset,
            )
        })?;
        Ok(span.local_position(paragraph_offset, affinity))
    }

    fn empty_line_height_key(&self) -> u64 {
        if self.text.is_empty() {
            (f64::from(self.default_font_size)
                * f64::from(self.default_inline_flow.line_height().multiplier()))
            .to_bits()
        } else {
            0
        }
    }

    fn composition_identity(&self) -> Option<(CompositionId, crate::CompositionEpoch)> {
        self.spans.iter().find_map(|span| match span.source {
            LeafSpanSource::Composition { id, epoch, .. } => Some((id, epoch)),
            LeafSpanSource::Snapshot { .. } => None,
        })
    }
}

fn append_projection_span<'a>(
    paragraph: ParagraphId,
    text: &mut alloc::string::String,
    spans: &mut Vec<LeafSpan>,
    shaping_styles: &mut Vec<&'a ShapingStyle>,
    shaping_runs: &mut Vec<ShapingRun>,
    inline_flow_styles: &mut Vec<InlineFlowStyle>,
    inline_flow_runs: &mut Vec<InlineFlowRun>,
    paint_runs: &mut Vec<PaintRun>,
    leaf: &crate::document::TextLeaf,
    value: &str,
    source: LeafSpanSource,
    style: &'a crate::ComputedInlineStyle,
) -> Result<(), SceneError> {
    let start = u32::try_from(text.len())
        .map_err(|_| SceneError::for_paragraph(SceneErrorKind::SourceCoverage, paragraph))?;
    text.push_str(value);
    let end = u32::try_from(text.len())
        .map_err(|_| SceneError::for_paragraph(SceneErrorKind::SourceCoverage, paragraph))?;
    spans.push(LeafSpan {
        paragraph: start..end,
        text: leaf.id,
        source,
        leaf_len: u32::try_from(leaf.text.len())
            .map_err(|_| SceneError::for_paragraph(SceneErrorKind::SourceCoverage, paragraph))?,
        role: leaf.role,
        semantic: leaf.semantic_id(),
    });
    if start != end {
        append_shaping_run(
            shaping_styles,
            shaping_runs,
            start..end,
            style.shaping(),
            paragraph,
        )?;
        append_inline_flow_run(
            inline_flow_styles,
            inline_flow_runs,
            start..end,
            style.inline_flow(),
            paragraph,
        )?;
        append_paint_run(paint_runs, start..end, style.paint());
    }
    Ok(())
}

fn span_for_position(
    spans: &[LeafSpan],
    paragraph_offset: u32,
    affinity: TextAffinity,
) -> Option<&LeafSpan> {
    match affinity {
        TextAffinity::Upstream => spans.iter().rev().find(|span| {
            (span.paragraph.start < paragraph_offset && paragraph_offset <= span.paragraph.end)
                || (span.paragraph.is_empty() && span.paragraph.end == paragraph_offset)
        }),
        TextAffinity::Downstream => spans.iter().find(|span| {
            (span.paragraph.start <= paragraph_offset && paragraph_offset < span.paragraph.end)
                || (span.paragraph.is_empty() && span.paragraph.start == paragraph_offset)
        }),
    }
    .or_else(|| {
        spans.iter().find(|span| {
            span.paragraph.start <= paragraph_offset && paragraph_offset <= span.paragraph.end
        })
    })
}

#[derive(Clone, Debug)]
struct LeafSpan {
    paragraph: Range<u32>,
    text: TextId,
    source: LeafSpanSource,
    leaf_len: u32,
    role: InlineRole,
    semantic: SemanticId,
}

#[derive(Clone, Copy, Debug)]
enum LeafSpanSource {
    Snapshot {
        start: u32,
    },
    Composition {
        id: CompositionId,
        epoch: crate::CompositionEpoch,
        start: u32,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ProjectionSourceKey {
    paragraph: Range<u32>,
    text: TextId,
    source: ProjectionSourceKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProjectionSourceKind {
    Snapshot { start: u32 },
    Composition { start: u32 },
}

impl ProjectionSourceKey {
    fn from_projection(projection: &Projection<'_>) -> Vec<Self> {
        projection
            .spans
            .iter()
            .map(|span| Self {
                paragraph: span.paragraph.clone(),
                text: span.text,
                source: match span.source {
                    LeafSpanSource::Snapshot { start } => ProjectionSourceKind::Snapshot { start },
                    LeafSpanSource::Composition { start, .. } => {
                        ProjectionSourceKind::Composition { start }
                    }
                },
            })
            .collect()
    }
}

impl LeafSpan {
    fn local_range(&self, paragraph_start: u32, paragraph_end: u32) -> LocalRange {
        let relative_start = paragraph_start - self.paragraph.start;
        let relative_end = paragraph_end - self.paragraph.start;
        match self.source {
            LeafSpanSource::Snapshot { start } => LocalRange::Snapshot {
                text: self.text,
                bytes: (start + relative_start)..(start + relative_end),
            },
            LeafSpanSource::Composition { id, epoch, start } => LocalRange::Composition {
                id,
                epoch,
                bytes: (start + relative_start)..(start + relative_end),
            },
        }
    }

    fn local_position(&self, paragraph: u32, affinity: TextAffinity) -> LocalPosition {
        let relative = paragraph - self.paragraph.start;
        match self.source {
            LeafSpanSource::Snapshot { start } => LocalPosition::Snapshot {
                text: self.text,
                byte: start + relative,
                affinity,
            },
            LeafSpanSource::Composition { id, epoch, start } => LocalPosition::Composition {
                id,
                epoch,
                byte: start + relative,
                affinity,
            },
        }
    }
}

fn append_shaping_run<'a>(
    styles: &mut Vec<&'a ShapingStyle>,
    runs: &mut Vec<ShapingRun>,
    bytes: Range<u32>,
    style: &'a ShapingStyle,
    paragraph: ParagraphId,
) -> Result<(), SceneError> {
    let style = if let Some(index) = styles.iter().position(|candidate| *candidate == style) {
        ShapingStyleId::new(
            u16::try_from(index)
                .map_err(|_| SceneError::for_paragraph(SceneErrorKind::InvalidStyle, paragraph))?,
        )
    } else {
        let index = u16::try_from(styles.len())
            .map_err(|_| SceneError::for_paragraph(SceneErrorKind::InvalidStyle, paragraph))?;
        styles.push(style);
        ShapingStyleId::new(index)
    };
    if let Some(last) = runs.last_mut()
        && last.bytes().end == bytes.start
        && last.style() == style
    {
        let start = last.bytes().start;
        *last = ShapingRun::new(start..bytes.end, style);
    } else {
        runs.push(ShapingRun::new(bytes, style));
    }
    Ok(())
}

fn append_inline_flow_run(
    styles: &mut Vec<InlineFlowStyle>,
    runs: &mut Vec<InlineFlowRun>,
    bytes: Range<u32>,
    style: InlineFlowStyle,
    paragraph: ParagraphId,
) -> Result<(), SceneError> {
    let style = if let Some(index) = styles.iter().position(|candidate| *candidate == style) {
        InlineFlowStyleId::new(
            u16::try_from(index)
                .map_err(|_| SceneError::for_paragraph(SceneErrorKind::InvalidStyle, paragraph))?,
        )
    } else {
        let index = u16::try_from(styles.len())
            .map_err(|_| SceneError::for_paragraph(SceneErrorKind::InvalidStyle, paragraph))?;
        styles.push(style);
        InlineFlowStyleId::new(index)
    };
    if let Some(last) = runs.last_mut()
        && last.bytes().end == bytes.start
        && last.style() == style
    {
        let start = last.bytes().start;
        *last = InlineFlowRun::new(start..bytes.end, style);
    } else {
        runs.push(InlineFlowRun::new(bytes, style));
    }
    Ok(())
}

fn append_paint_run(runs: &mut Vec<PaintRun>, bytes: Range<u32>, slot: PaintSlot) {
    if let Some(last) = runs.last_mut()
        && last.bytes().end == bytes.start
        && last.slot() == slot
    {
        let start = last.bytes().start;
        *last = PaintRun::new(start..bytes.end, slot);
    } else {
        runs.push(PaintRun::new(bytes, slot));
    }
}

fn validate_styles(
    snapshot: &DocumentSnapshot,
    request: &SceneRequest<'_>,
) -> Result<(), SceneError> {
    if request
        .styles
        .overrides()
        .iter()
        .any(|(text, _)| snapshot.text(*text).is_none())
    {
        return Err(SceneError::for_document(
            SceneErrorKind::InvalidStyle,
            snapshot.id(),
        ));
    }
    for paragraph in snapshot.paragraphs() {
        for leaf in &paragraph.leaves {
            if request
                .paint
                .brush(request.styles.style_for(leaf.id).paint())
                .is_none()
            {
                return Err(SceneError::for_paragraph(
                    SceneErrorKind::InvalidStyle,
                    paragraph.id,
                ));
            }
        }
    }
    Ok(())
}

fn validate_prepared(
    prepared: &PreparedParagraph,
    projection: &Projection<'_>,
) -> Result<(), SceneError> {
    for line in prepared.lines() {
        let line_source = line.source();
        if projection
            .text
            .get(line_source.start as usize..line_source.end as usize)
            .is_none()
        {
            return Err(SceneError::for_source(
                SceneErrorKind::SourceCoverage,
                prepared.paragraph(),
                line_source,
            ));
        }
        for run in line.runs() {
            let source = run.source();
            let Some(source_text) = projection
                .text
                .get(source.start as usize..source.end as usize)
            else {
                return Err(SceneError::for_source(
                    SceneErrorKind::SourceCoverage,
                    prepared.paragraph(),
                    source,
                ));
            };
            for glyph in run.glyphs() {
                let source = glyph.source();
                if projection
                    .text
                    .get(source.start as usize..source.end as usize)
                    .is_none()
                {
                    return Err(SceneError::for_source(
                        SceneErrorKind::SourceCoverage,
                        prepared.paragraph(),
                        source,
                    ));
                }
                for segment in glyph.paint().segments() {
                    let source = segment.source();
                    if projection
                        .text
                        .get(source.start as usize..source.end as usize)
                        .is_none()
                    {
                        return Err(SceneError::for_source(
                            SceneErrorKind::SourceCoverage,
                            prepared.paragraph(),
                            source,
                        ));
                    }
                    projection.local_ranges(source)?;
                }
            }
            for range in run.unrendered_source() {
                if projection
                    .text
                    .get(range.start as usize..range.end as usize)
                    .is_none()
                {
                    return Err(SceneError::for_source(
                        SceneErrorKind::SourceCoverage,
                        prepared.paragraph(),
                        range.clone(),
                    ));
                }
            }
            for (offset, character) in source_text.char_indices() {
                let scalar_start = source.start
                    + u32::try_from(offset).map_err(|_| {
                        SceneError::for_source(
                            SceneErrorKind::SourceCoverage,
                            prepared.paragraph(),
                            source.clone(),
                        )
                    })?;
                let scalar_end = scalar_start
                    .checked_add(u32::try_from(character.len_utf8()).unwrap_or(u32::MAX))
                    .ok_or_else(|| {
                        SceneError::for_source(
                            SceneErrorKind::SourceCoverage,
                            prepared.paragraph(),
                            source.clone(),
                        )
                    })?;
                if !run.glyphs().iter().any(|glyph| {
                    let glyph_source = glyph.source();
                    glyph_source.start <= scalar_start && glyph_source.end >= scalar_end
                }) && !run
                    .unrendered_source()
                    .iter()
                    .any(|range| range.start <= scalar_start && range.end >= scalar_end)
                {
                    return Err(SceneError::for_source(
                        SceneErrorKind::SourceCoverage,
                        prepared.paragraph(),
                        scalar_start..scalar_end,
                    ));
                }
            }
        }
    }
    Ok(())
}

fn record_formation_work(report: &mut WorkReport, work: FormationWork) {
    if work.analyzed() {
        report.analysis.add_paragraph(1);
    }
    if work.itemized() {
        report.itemization.add_paragraph(1);
    }
    if work.selected_clusters() > 0 {
        report.font_selection.paragraphs += 1;
        report.font_selection.records += work.selected_clusters() as usize;
    }
    if work.shaped_runs() > 0 {
        report.shape.paragraphs += 1;
        report.shape.records += work.shaped_glyphs() as usize;
    }
    if work.formed_lines() > 0 {
        report.flow.paragraphs += 1;
        report.flow.records += work.formed_lines() as usize;
    }
    report.break_reshapes += work.break_reshapes() as usize;
}

#[derive(Clone, Debug)]
struct CachedGeometry {
    height: f64,
    lines: Vec<CachedLine>,
    fragments: Vec<CachedFragment>,
    clusters: Vec<CachedCluster>,
    carets: Vec<CachedCaret>,
    movements: Vec<CachedCursorMovement>,
    texts: Vec<LocalRange>,
    semantics: Vec<CachedSemantic>,
}

#[derive(Clone, Debug)]
struct CachedLine {
    bounds: Rect,
    sources: Vec<LocalRange>,
    break_reason: LineBreakReason,
    baseline: f64,
    content_ascent: f64,
    content_descent: f64,
}

#[derive(Clone, Debug)]
struct CachedFragment {
    id: SceneFragmentId,
    glyphs: Vec<CachedGlyph>,
    paint: PaintSlot,
    transform: Affine,
    sources: Vec<LocalRange>,
    bounds: Rect,
    clip: Rect,
    font: FontData,
    font_size: f32,
    synthesis: FontSynthesis,
    normalized_coords: Arc<[i16]>,
    bidi_level: u8,
    script: [u8; 4],
}

#[derive(Clone, Debug)]
struct CachedGlyph {
    id: u32,
    position: Point,
    advance: Vec2,
    sources: Vec<LocalRange>,
}

#[derive(Clone, Debug)]
struct CachedCluster {
    sources: Vec<LocalRange>,
    semantic_id: SemanticId,
    bounds: Rect,
    line: usize,
    left: LocalPosition,
    right: LocalPosition,
    bidi_level: u8,
}

#[derive(Clone, Debug)]
struct CachedCaret {
    position: LocalPosition,
    bounds: Rect,
}

#[derive(Clone, Debug)]
struct CachedCursorMovement {
    position: LocalPosition,
    previous_visual: Option<CachedCursorStep>,
    next_visual: Option<CachedCursorStep>,
    previous_logical: Option<CachedCursorStep>,
    next_logical: Option<CachedCursorStep>,
}

#[derive(Clone, Debug)]
struct CachedCursorStep {
    target: LocalPosition,
    source: Option<Vec<LocalRange>>,
}

#[derive(Clone, Debug)]
struct CachedSemantic {
    semantic_id: SemanticId,
    paragraph_role: Option<ParagraphRole>,
    inline_role: Option<InlineRole>,
    source: Option<Vec<LocalRange>>,
    bounds: Rect,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum LocalRange {
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
enum LocalPosition {
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

fn rebind_composition_geometry(
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

fn rebind_ranges(ranges: &mut [LocalRange], id: CompositionId, epoch: crate::CompositionEpoch) {
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

fn rebind_position(
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

fn build_geometry(
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

    for line in prepared.lines() {
        let line_index = lines.len();
        let baseline = line_top + line.baseline();
        let mut cluster_x = 0.0_f64;
        for cluster in line.clusters() {
            let paragraph_source = cluster.source();
            let sources = projection.local_ranges(paragraph_source.clone())?;
            let semantic_id = projection.semantic_for_range(paragraph_source.clone())?;
            let left =
                projection.position_at(cluster.left().offset(), cluster.left().affinity())?;
            let right =
                projection.position_at(cluster.right().offset(), cluster.right().affinity())?;
            let next_x = cluster_x + cluster.advance();
            let bounds = Rect::new(cluster_x, line_top, next_x, line_top + line.height());
            clusters.push(CachedCluster {
                sources,
                semantic_id,
                bounds,
                line: line_index,
                left,
                right,
                bidi_level: cluster.bidi_level(),
            });
            cluster_x = next_x;
        }
        if line.clusters().is_empty() && !projection.spans.is_empty() {
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
                bounds: Rect::new(0.0, line_top, 0.0, line_top + line.height()),
                line: line_index,
                left: position,
                right: position,
                bidi_level: 0,
            });
        }
        let mut x = 0.0_f64;
        let mut right = line.advance();
        for run in line.runs() {
            let normalized_coords: Arc<[i16]> = Arc::from(run.normalized_coords());
            for glyph in run.glyphs() {
                let position = Point::new(x + glyph.offset().x, baseline - glyph.offset().y);
                for segment in glyph.paint().segments() {
                    let sources = projection.local_ranges(segment.source())?;
                    let local_clip = segment.local_clip();
                    let clip = Rect::new(
                        position.x + local_clip.x0,
                        position.y + local_clip.y0,
                        position.x + local_clip.x1,
                        position.y + local_clip.y1,
                    );
                    right = right.max(clip.x1);
                    let id =
                        SceneFragmentId(fragment_identity(prepared.paragraph(), fragments.len()));
                    fragments.push(CachedFragment {
                        id,
                        glyphs: alloc::vec![CachedGlyph {
                            id: glyph.id(),
                            position,
                            advance: glyph.advance(),
                            sources: sources.clone(),
                        }],
                        paint: segment.slot(),
                        transform: Affine::IDENTITY,
                        sources,
                        bounds: clip,
                        clip,
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
            bounds: Rect::new(0.0, line_top, right.max(1.0), line_top + line.height()),
            sources: projection.local_ranges(line.source())?,
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
        for fragment in &fragments {
            if fragment.sources.iter().any(|source| {
                matches!(source, LocalRange::Snapshot { text, .. } if *text == span.text)
                    || matches!(span.source, LeafSpanSource::Composition { .. })
                        && matches!(source, LocalRange::Composition { .. })
            }) {
                bounds = Some(match bounds {
                    Some(current) => current.union(fragment.bounds),
                    None => fragment.bounds,
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

fn cached_cursor_step(
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

fn fragment_identity(paragraph: ParagraphId, fragment: usize) -> u64 {
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

fn materialize_geometry(
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
    lines.extend(geometry.lines.iter().map(|line| {
        SceneLine {
            bounds: line.bounds + translate,
            sources: line
                .sources
                .iter()
                .map(|source| materialize_range(source, revision))
                .collect(),
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
                    id: glyph.id,
                    position: glyph.position + translate,
                    advance: glyph.advance,
                    source: materialize_snapshot_range(&glyph.sources, revision),
                })
                .collect(),
            paint: fragment.paint,
            transform: fragment.transform,
            source: Some(materialize_snapshot_range(&fragment.sources, revision)),
            clip: fragment.clip + translate,
            font: fragment.font.clone(),
            font_size: fragment.font_size,
            synthesis: fragment.synthesis.clone(),
            normalized_coords: Arc::clone(&fragment.normalized_coords),
            bidi_level: fragment.bidi_level,
            script: fragment.script,
        }
    }));
    clusters.extend(geometry.clusters.iter().map(|cluster| SceneCluster {
        source: materialize_snapshot_range(&cluster.sources, revision),
        semantic_id: cluster.semantic_id,
        bounds: cluster.bounds + translate,
        line: line_base + cluster.line,
        left: materialize_position(cluster.left, revision),
        right: materialize_position(cluster.right, revision),
        bidi_level: cluster.bidi_level,
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

fn materialize_projected_geometry(
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
    lines.extend(geometry.lines.iter().map(|line| {
        SceneLine {
            bounds: line.bounds + translate,
            sources: line
                .sources
                .iter()
                .map(|source| projected_range(core::slice::from_ref(source), revision))
                .collect(),
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
                    id: glyph.id,
                    position: glyph.position + translate,
                    advance: glyph.advance,
                    source: projected_range(&glyph.sources, revision),
                })
                .collect(),
            paint: fragment.paint,
            transform: fragment.transform,
            source: Some(projected_range(&fragment.sources, revision)),
            clip: fragment.clip + translate,
            font: fragment.font.clone(),
            font_size: fragment.font_size,
            synthesis: fragment.synthesis.clone(),
            normalized_coords: Arc::clone(&fragment.normalized_coords),
            bidi_level: fragment.bidi_level,
            script: fragment.script,
        }
    }));
    clusters.extend(geometry.clusters.iter().map(|cluster| SceneCluster {
        source: projected_range(&cluster.sources, revision),
        semantic_id: cluster.semantic_id,
        bounds: cluster.bounds + translate,
        line: line_base + cluster.line,
        left: projected_position(cluster.left, revision),
        right: projected_position(cluster.right, revision),
        bidi_level: cluster.bidi_level,
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

fn projected_cursor_step(
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

fn projected_range(ranges: &[LocalRange], revision: DocumentRevision) -> ProjectedTextRange {
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

fn projected_position(
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

fn materialize_optional_snapshot_range(
    ranges: &[LocalRange],
    revision: DocumentRevision,
) -> Option<SnapshotTextRange> {
    let [LocalRange::Snapshot { text, bytes }] = ranges else {
        return None;
    };
    Some(SnapshotTextRange::new(revision, *text, bytes.clone()))
}

fn materialize_cursor_step(
    step: Option<&CachedCursorStep>,
    revision: DocumentRevision,
) -> Option<SceneCursorStep> {
    step.map(|step| SceneCursorStep {
        target: materialize_position(step.target, revision),
        source: step
            .source
            .as_ref()
            .map(|source| materialize_snapshot_range(source, revision)),
    })
}

fn materialize_range(range: &LocalRange, revision: DocumentRevision) -> SnapshotTextRange {
    let LocalRange::Snapshot { text, bytes } = range else {
        unreachable!("committed geometry cannot contain composition source")
    };
    SnapshotTextRange::new(revision, *text, bytes.clone())
}

fn materialize_snapshot_range(
    ranges: &[LocalRange],
    revision: DocumentRevision,
) -> SnapshotTextRange {
    let [range] = ranges else {
        unreachable!("committed geometry source must remain within one semantic text leaf")
    };
    materialize_range(range, revision)
}

fn materialize_position(
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

/// One provenance-preserving segment of transient projected scene text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProjectedTextSource {
    /// Bytes retained from the immutable committed snapshot.
    Snapshot(SnapshotTextRange),
    /// Bytes generated by the named composition epoch.
    Composition(crate::CompositionTextRange),
}

/// Source-complete range covered by one transient scene observation.
///
/// A shaped cluster or glyph can cover more than one segment when a generated
/// combining mark joins an authored base character. Keeping the ordered list
/// prevents either provenance from being fabricated or discarded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectedTextRange {
    sources: Arc<[ProjectedTextSource]>,
}

impl ProjectedTextRange {
    pub(crate) fn new(sources: Vec<ProjectedTextSource>) -> Self {
        Self {
            sources: sources.into(),
        }
    }

    /// Returns ordered, source-complete provenance segments.
    #[must_use]
    pub fn sources(&self) -> &[ProjectedTextSource] {
        &self.sources
    }
}

/// Exact caret position in either committed or generated projected text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ProjectedTextPosition {
    /// Position in the immutable document snapshot.
    Snapshot(SnapshotTextPosition),
    /// Position in the current generated composition epoch.
    Composition(crate::CompositionTextPosition),
}

/// Immutable prepared scene and exact work report.
#[derive(Clone, Debug)]
pub struct SceneOutput {
    scene: TextScene,
    work: WorkReport,
}

/// Immutable transient scene for one exact composition epoch.
#[derive(Clone, Debug)]
pub struct CompositionSceneOutput {
    scene: CompositionScene,
    work: WorkReport,
}

impl CompositionSceneOutput {
    /// Returns the prepared transient scene.
    #[must_use]
    pub const fn scene(&self) -> &CompositionScene {
        &self.scene
    }

    /// Returns actual work performed for this transient request.
    #[must_use]
    pub const fn work(&self) -> &WorkReport {
        &self.work
    }
}

impl SceneOutput {
    /// Returns the prepared scene.
    #[must_use]
    pub const fn scene(&self) -> &TextScene {
        &self.scene
    }

    /// Returns actual work performed for this request.
    #[must_use]
    pub const fn work(&self) -> &WorkReport {
        &self.work
    }
}

/// Count of paragraphs and records processed by one stage.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StageWork {
    paragraphs: usize,
    records: usize,
}

impl StageWork {
    fn add_paragraph(&mut self, records: usize) {
        self.paragraphs += 1;
        self.records += records;
    }

    /// Returns paragraphs processed rather than reused.
    #[must_use]
    pub const fn paragraphs(self) -> usize {
        self.paragraphs
    }

    /// Returns stage-specific records processed.
    #[must_use]
    pub const fn records(self) -> usize {
        self.records
    }
}

/// Exact stage work performed for one scene request.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WorkReport {
    analysis: StageWork,
    itemization: StageWork,
    font_selection: StageWork,
    shape: StageWork,
    flow: StageWork,
    geometry: StageWork,
    paint: StageWork,
    break_reshapes: usize,
    reused_paragraphs: usize,
}

impl WorkReport {
    /// Returns Unicode analysis work.
    #[must_use]
    pub const fn analysis(&self) -> StageWork {
        self.analysis
    }

    /// Returns shaping itemization work.
    #[must_use]
    pub const fn itemization(&self) -> StageWork {
        self.itemization
    }

    /// Returns font-selection work.
    #[must_use]
    pub const fn font_selection(&self) -> StageWork {
        self.font_selection
    }

    /// Returns shaping work.
    #[must_use]
    pub const fn shape(&self) -> StageWork {
        self.shape
    }

    /// Returns finite-width flow work.
    #[must_use]
    pub const fn flow(&self) -> StageWork {
        self.flow
    }

    /// Returns scene-geometry work.
    #[must_use]
    pub const fn geometry(&self) -> StageWork {
        self.geometry
    }

    /// Returns paint lowering work.
    #[must_use]
    pub const fn paint(&self) -> StageWork {
        self.paint
    }

    /// Returns committed line boundaries that required bounded reshaping.
    #[must_use]
    pub const fn break_reshapes(&self) -> usize {
        self.break_reshapes
    }

    /// Returns paragraphs reused without calling the adapter.
    #[must_use]
    pub const fn reused_paragraphs(&self) -> usize {
        self.reused_paragraphs
    }
}

/// Immutable renderer-neutral scene for one generated composition epoch.
#[derive(Clone, Debug)]
pub struct CompositionScene {
    document: crate::DocumentId,
    revision: DocumentRevision,
    composition: CompositionId,
    epoch: crate::CompositionEpoch,
    lines: Vec<SceneLine<ProjectedTextRange>>,
    fragments: Vec<SceneFragment<ProjectedTextRange>>,
    clusters: Vec<SceneCluster<ProjectedTextRange, ProjectedTextPosition>>,
    carets: Vec<SceneCaretStop<ProjectedTextPosition>>,
    movements: Vec<SceneCursorMovement<ProjectedTextRange, ProjectedTextPosition>>,
    paint: PaintTable,
    semantics: Vec<SemanticFragment>,
}

impl CompositionScene {
    /// Returns the immutable document identity below the transient projection.
    #[must_use]
    pub const fn document(&self) -> crate::DocumentId {
        self.document
    }

    /// Returns the immutable base revision below the transient projection.
    #[must_use]
    pub const fn revision(&self) -> DocumentRevision {
        self.revision
    }

    /// Returns the native composition identity.
    #[must_use]
    pub const fn composition(&self) -> CompositionId {
        self.composition
    }

    /// Returns the exact transient epoch represented by this scene.
    #[must_use]
    pub const fn epoch(&self) -> crate::CompositionEpoch {
        self.epoch
    }

    /// Returns visual lines in flow order.
    #[must_use]
    pub fn lines(&self) -> &[SceneLine<ProjectedTextRange>] {
        &self.lines
    }

    /// Returns paint-homogeneous projected glyph fragments.
    #[must_use]
    pub fn fragments(&self) -> &[SceneFragment<ProjectedTextRange>] {
        &self.fragments
    }

    /// Returns immutable paint values referenced by fragment slots.
    #[must_use]
    pub const fn paint(&self) -> &PaintTable {
        &self.paint
    }

    /// Iterates semantic fragments in document order.
    pub fn semantics(&self) -> impl Iterator<Item = &SemanticFragment> {
        self.semantics.iter()
    }

    /// Returns the exact projected cluster under a point.
    #[must_use]
    pub fn hit_test(
        &self,
        point: Point,
    ) -> Option<TextHit<ProjectedTextRange, ProjectedTextPosition>> {
        self.clusters
            .iter()
            .find(|cluster| cluster.bounds.contains(point))
            .map(|cluster| cluster.hit(point))
    }

    /// Returns the closest projected cluster side for native point queries.
    #[must_use]
    pub fn hit_test_closest(
        &self,
        point: Point,
    ) -> Option<TextHit<ProjectedTextRange, ProjectedTextPosition>> {
        let mut closest: Option<(
            &SceneCluster<ProjectedTextRange, ProjectedTextPosition>,
            f64,
            f64,
        )> = None;
        for cluster in &self.clusters {
            let (block_distance, inline_distance) = distance_to_rect_axes(point, cluster.bounds);
            if closest.is_none_or(|(_, current_block, current_inline)| {
                block_distance < current_block
                    || (block_distance == current_block && inline_distance < current_inline)
            }) {
                closest = Some((cluster, block_distance, inline_distance));
            }
        }
        closest.map(|(cluster, _, _)| cluster.hit(point))
    }

    /// Resolves exact scene geometry for one projected caret position.
    #[must_use]
    pub fn caret(
        &self,
        position: &ProjectedTextPosition,
    ) -> Option<SceneCaret<ProjectedTextPosition>> {
        self.carets
            .iter()
            .find(|caret| caret.position == *position)
            .map(|caret| SceneCaret {
                position: caret.position,
                bounds: caret.bounds,
            })
    }

    /// Moves one position through the adapter-produced cluster map.
    #[must_use]
    pub fn move_position(
        &self,
        position: &ProjectedTextPosition,
        movement: TextMovement,
    ) -> Option<ProjectedTextPosition> {
        let record = self
            .movements
            .iter()
            .find(|record| record.position == *position)?;
        let step = match movement {
            TextMovement::PreviousVisual => record.previous_visual.as_ref(),
            TextMovement::NextVisual => record.next_visual.as_ref(),
            TextMovement::PreviousLogical => record.previous_logical.as_ref(),
            TextMovement::NextLogical => record.next_logical.as_ref(),
        }?;
        Some(step.target)
    }

    /// Resolves highlight rectangles for the selected range inside preedit.
    pub fn composition_selection_geometry(
        &self,
        session: &CompositionSession,
    ) -> Result<Vec<SceneCompositionRect>, CompositionError> {
        if session.document() != self.document
            || session.base_revision() != self.revision
            || session.id() != self.composition
            || session.epoch() != self.epoch
        {
            return Err(CompositionError::new(CompositionErrorKind::WrongSnapshot));
        }
        let Some(selection) = session.selection() else {
            return Ok(Vec::new());
        };
        let mut geometry: Vec<SceneCompositionRect> = Vec::new();
        for cluster in &self.clusters {
            if !cluster.source.sources().iter().any(|source| {
                matches!(source, ProjectedTextSource::Composition(range)
                    if range.id() == self.composition
                        && range.epoch() == self.epoch
                        && range.bytes().start < selection.end
                        && selection.start < range.bytes().end)
            }) {
                continue;
            }
            if let Some(previous) = geometry.last_mut()
                && previous.line == cluster.line
                && previous.bidi_level == cluster.bidi_level
                && nearly_equal(previous.bounds.x1, cluster.bounds.x0)
            {
                previous.bounds.x1 = cluster.bounds.x1;
            } else {
                geometry.push(SceneCompositionRect {
                    line: cluster.line,
                    bounds: cluster.bounds,
                    bidi_level: cluster.bidi_level,
                });
            }
        }
        Ok(geometry)
    }

    pub(crate) fn range_geometry(&self, range: &ProjectedTextRange) -> Vec<(usize, Rect)> {
        self.clusters
            .iter()
            .filter(|cluster| projected_ranges_overlap(&cluster.source, range))
            .map(|cluster| (cluster.line, cluster.bounds))
            .collect()
    }
}

/// Immutable renderer-neutral text scene.
#[derive(Clone, Debug)]
pub struct TextScene {
    document: crate::DocumentId,
    revision: DocumentRevision,
    lines: Vec<SceneLine>,
    fragments: Vec<SceneFragment>,
    clusters: Vec<SceneCluster>,
    carets: Vec<SceneCaretStop>,
    movements: Vec<SceneCursorMovement>,
    texts: Vec<SnapshotTextRange>,
    paint: PaintTable,
    semantics: Vec<SemanticFragment>,
}

impl TextScene {
    /// Returns the document identity represented by this scene.
    #[must_use]
    pub const fn document(&self) -> crate::DocumentId {
        self.document
    }

    /// Returns the exact immutable snapshot revision represented by this scene.
    #[must_use]
    pub const fn revision(&self) -> DocumentRevision {
        self.revision
    }

    /// Starts one native composition over the current primary insertion point.
    ///
    /// Native composition protocols expose one marked region. A sole logical
    /// selection becomes that replacement target. If the scene has several
    /// independent selections, or the primary visual selection has several
    /// disjoint logical ranges, the host-visible set is explicitly normalized
    /// to one collapsed primary extent before composition starts. Callers can
    /// observe that normalization through [`CompositionStart::selection_changed`].
    pub fn begin_composition(
        &self,
        selections: &SnapshotTextSelectionSet,
        id: CompositionId,
    ) -> Result<CompositionStart, CompositionError> {
        if selections.document() != self.document || selections.revision() != self.revision {
            return Err(CompositionError::new(CompositionErrorKind::WrongSnapshot));
        }
        let Some(primary) = selections.primary() else {
            return Err(CompositionError::new(
                CompositionErrorKind::EmptySelectionSet,
            ));
        };
        let normalized = if selections.selections().len() == 1 && primary.ranges().len() == 1 {
            self.selection_set([primary.clone()])
        } else {
            self.collapsed_selection(primary.extent())
                .and_then(|selection| self.selection_set([selection]))
        }
        .map_err(|_| CompositionError::new(CompositionErrorKind::WrongSnapshot))?;
        let selection_changed = &normalized != selections;
        Ok(CompositionStart::new(
            CompositionSession::new(id, normalized.clone()),
            normalized,
            selection_changed,
        ))
    }

    /// Returns an empty selection set bound to this scene revision.
    #[must_use]
    pub fn empty_selection_set(&self) -> SnapshotTextSelectionSet {
        SnapshotTextSelectionSet::new(self.document, self.revision, Vec::new())
    }

    /// Creates one collapsed selection at an exact scene position.
    pub fn collapsed_selection(
        &self,
        position: &SnapshotTextPosition,
    ) -> Result<SnapshotTextSelection, SelectionError> {
        self.validate_position(position)?;
        Ok(SnapshotTextSelection::new(
            *position,
            *position,
            TextSelectionMode::Logical,
            alloc::vec![SnapshotTextRange::new(
                self.revision,
                position.text(),
                position.byte()..position.byte(),
            )],
        ))
    }

    /// Creates one logical or visual selection between two exact positions.
    ///
    /// A visual selection follows adapter-owned caret transitions and can
    /// expose several noncontiguous logical ranges across bidi boundaries.
    pub fn selection(
        &self,
        anchor: &SnapshotTextPosition,
        extent: &SnapshotTextPosition,
        mode: TextSelectionMode,
    ) -> Result<SnapshotTextSelection, SelectionError> {
        self.validate_position(anchor)?;
        self.validate_position(extent)?;
        let ranges = match mode {
            TextSelectionMode::Logical => self.logical_ranges(anchor, extent)?,
            TextSelectionMode::Visual => self.visual_ranges(anchor, extent)?,
        };
        Ok(SnapshotTextSelection::new(*anchor, *extent, mode, ranges))
    }

    /// Validates and collects independent selections for this scene.
    pub fn selection_set(
        &self,
        selections: impl IntoIterator<Item = SnapshotTextSelection>,
    ) -> Result<SnapshotTextSelectionSet, SelectionError> {
        let selections: Vec<_> = selections.into_iter().collect();
        for selection in &selections {
            let expected =
                self.selection(selection.anchor(), selection.extent(), selection.mode())?;
            if expected.ranges() != selection.ranges() {
                return Err(SelectionError::new(SelectionErrorKind::UnknownPosition));
            }
        }
        validate_independent_selections(&selections)?;
        Ok(SnapshotTextSelectionSet::new(
            self.document,
            self.revision,
            selections,
        ))
    }

    /// Moves every independent selection through the exact scene cursor map.
    ///
    /// When `extend` is true, each anchor is retained and the extent is moved.
    /// Otherwise a noncollapsed selection first collapses toward the requested
    /// direction and a collapsed selection advances by one cluster step.
    pub fn move_selections(
        &self,
        selections: &SnapshotTextSelectionSet,
        movement: TextMovement,
        extend: bool,
    ) -> Result<SnapshotTextSelectionSet, SelectionError> {
        if selections.document() != self.document || selections.revision() != self.revision {
            return Err(SelectionError::new(SelectionErrorKind::WrongSnapshot));
        }
        let mode = movement_mode(movement);
        let mut moved = Vec::with_capacity(selections.selections().len());
        for selection in selections.selections() {
            let next = if !extend && !selection.is_collapsed() {
                self.collapse_for_movement(selection, movement)?
            } else {
                let extent = self
                    .cursor_step(selection.extent(), movement)?
                    .map_or(*selection.extent(), |step| step.target);
                if extend {
                    self.selection(selection.anchor(), &extent, mode)?
                } else {
                    self.collapsed_selection(&extent)?
                }
            };
            moved.push(next);
        }
        self.selection_set(moved)
    }

    /// Resolves visual highlight rectangles for a complete selection set.
    pub fn selection_geometry(
        &self,
        selections: &SnapshotTextSelectionSet,
    ) -> Result<Vec<SceneSelectionRect>, SelectionError> {
        if selections.document() != self.document || selections.revision() != self.revision {
            return Err(SelectionError::new(SelectionErrorKind::WrongSnapshot));
        }
        let mut geometry: Vec<SceneSelectionRect> = Vec::new();
        for (selection_index, selection) in selections.selections().iter().enumerate() {
            for cluster in &self.clusters {
                let Some((range_index, _)) = selection
                    .ranges()
                    .iter()
                    .enumerate()
                    .find(|(_, range)| ranges_overlap(range, &cluster.source))
                else {
                    continue;
                };
                if let Some(previous) = geometry.last_mut()
                    && previous.selection == selection_index
                    && previous.range == range_index
                    && previous.line == cluster.line
                    && previous.bidi_level == cluster.bidi_level
                    && nearly_equal(previous.bounds.x1, cluster.bounds.x0)
                {
                    previous.bounds.x1 = cluster.bounds.x1;
                } else {
                    geometry.push(SceneSelectionRect {
                        selection: selection_index,
                        range: range_index,
                        line: cluster.line,
                        bounds: cluster.bounds,
                        bidi_level: cluster.bidi_level,
                    });
                }
            }
        }
        Ok(geometry)
    }

    /// Returns visual lines in flow order.
    #[must_use]
    pub fn lines(&self) -> &[SceneLine] {
        &self.lines
    }

    /// Returns paint-homogeneous glyph fragments in visual order.
    #[must_use]
    pub fn fragments(&self) -> &[SceneFragment] {
        &self.fragments
    }

    /// Returns immutable paint values referenced by fragment slots.
    #[must_use]
    pub const fn paint(&self) -> &PaintTable {
        &self.paint
    }

    /// Iterates semantic fragments in document order.
    pub fn semantics(&self) -> impl Iterator<Item = &SemanticFragment> {
        self.semantics.iter()
    }

    /// Returns an exact shaped-cluster hit under a scene-space point.
    ///
    /// Unlike selection hit testing, this does not clamp points outside cluster
    /// geometry to the nearest line edge.
    #[must_use]
    pub fn hit_test(&self, point: Point) -> Option<TextHit> {
        self.clusters
            .iter()
            .find(|cluster| cluster.bounds.contains(point))
            .map(|cluster| cluster.hit(point))
    }

    /// Returns the closest shaped-cluster side for pointer selection.
    ///
    /// This includes whitespace and empty editable text which may have no
    /// painted glyph fragment.
    #[must_use]
    pub fn hit_test_closest(&self, point: Point) -> Option<TextHit> {
        let mut closest: Option<(&SceneCluster, f64, f64)> = None;
        for cluster in &self.clusters {
            let (block_distance, inline_distance) = distance_to_rect_axes(point, cluster.bounds);
            if closest.is_none_or(|(_, current_block, current_inline)| {
                block_distance < current_block
                    || (block_distance == current_block && inline_distance < current_inline)
            }) {
                closest = Some((cluster, block_distance, inline_distance));
            }
        }
        closest.map(|(cluster, _, _)| cluster.hit(point))
    }

    /// Resolves exact scene-space caret geometry for a snapshot position.
    ///
    /// Returns `None` for a stale revision, foreign text leaf, invalid
    /// affinity, or a valid snapshot position not represented by this scene.
    #[must_use]
    pub fn caret(&self, position: &SnapshotTextPosition) -> Option<SceneCaret> {
        self.carets
            .iter()
            .find(|caret| caret.position == *position)
            .map(|caret| SceneCaret {
                position: caret.position,
                bounds: caret.bounds,
            })
    }

    fn validate_position(&self, position: &SnapshotTextPosition) -> Result<(), SelectionError> {
        if position.revision() != self.revision || position.text().document != self.document {
            return Err(SelectionError::new(SelectionErrorKind::WrongSnapshot));
        }
        if self
            .movements
            .iter()
            .any(|movement| movement.position == *position)
        {
            Ok(())
        } else {
            Err(SelectionError::new(SelectionErrorKind::UnknownPosition))
        }
    }

    fn logical_ranges(
        &self,
        anchor: &SnapshotTextPosition,
        extent: &SnapshotTextPosition,
    ) -> Result<Vec<SnapshotTextRange>, SelectionError> {
        let anchor_text = self.text_rank(anchor.text())?;
        let extent_text = self.text_rank(extent.text())?;
        let ordering = (anchor_text, anchor.byte()).cmp(&(extent_text, extent.byte()));
        let (start, start_text, end, end_text) = if ordering.is_gt() {
            (extent, extent_text, anchor, anchor_text)
        } else {
            (anchor, anchor_text, extent, extent_text)
        };
        if start_text == end_text && start.byte() == end.byte() {
            return Ok(alloc::vec![SnapshotTextRange::new(
                self.revision,
                extent.text(),
                extent.byte()..extent.byte(),
            )]);
        }
        let mut ranges = Vec::new();
        for index in start_text..=end_text {
            let text = &self.texts[index];
            let bytes = if start_text == end_text {
                start.byte()..end.byte()
            } else if index == start_text {
                start.byte()..text.bytes().end
            } else if index == end_text {
                0..end.byte()
            } else {
                text.bytes()
            };
            if !bytes.is_empty() {
                ranges.push(SnapshotTextRange::new(self.revision, text.text(), bytes));
            }
        }
        if ranges.is_empty() {
            ranges.push(SnapshotTextRange::new(
                self.revision,
                extent.text(),
                extent.byte()..extent.byte(),
            ));
        }
        Ok(ranges)
    }

    fn visual_ranges(
        &self,
        anchor: &SnapshotTextPosition,
        extent: &SnapshotTextPosition,
    ) -> Result<Vec<SnapshotTextRange>, SelectionError> {
        if anchor == extent {
            return Ok(alloc::vec![SnapshotTextRange::new(
                self.revision,
                extent.text(),
                extent.byte()..extent.byte(),
            )]);
        }
        let ranges = self
            .walk_visual_ranges(anchor, extent, TextMovement::NextVisual)?
            .or(self.walk_visual_ranges(anchor, extent, TextMovement::PreviousVisual)?);
        let Some(mut ranges) = ranges else {
            return Err(SelectionError::new(
                SelectionErrorKind::DisconnectedMovement,
            ));
        };
        canonicalize_ranges(&mut ranges, &self.texts);
        if ranges.is_empty() {
            ranges.push(SnapshotTextRange::new(
                self.revision,
                extent.text(),
                extent.byte()..extent.byte(),
            ));
        }
        Ok(ranges)
    }

    fn walk_visual_ranges(
        &self,
        start: &SnapshotTextPosition,
        end: &SnapshotTextPosition,
        movement: TextMovement,
    ) -> Result<Option<Vec<SnapshotTextRange>>, SelectionError> {
        let mut position = *start;
        let mut ranges = Vec::new();
        for _ in 0..=self.movements.len() {
            if position == *end {
                return Ok(Some(ranges));
            }
            let Some(step) = self.cursor_step(&position, movement)? else {
                return Ok(None);
            };
            if let Some(source) = step.source {
                ranges.push(source);
            }
            position = step.target;
        }
        Ok(None)
    }

    fn cursor_step(
        &self,
        position: &SnapshotTextPosition,
        movement: TextMovement,
    ) -> Result<Option<SceneCursorStep>, SelectionError> {
        self.validate_position(position)?;
        let record = self
            .movements
            .iter()
            .find(|record| record.position == *position)
            .ok_or_else(|| SelectionError::new(SelectionErrorKind::UnknownPosition))?;
        let step = match movement {
            TextMovement::PreviousVisual => record.previous_visual.clone(),
            TextMovement::NextVisual => record.next_visual.clone(),
            TextMovement::PreviousLogical => record.previous_logical.clone(),
            TextMovement::NextLogical => record.next_logical.clone(),
        };
        Ok(step.or_else(|| self.adjacent_paragraph_step(position, movement)))
    }

    fn adjacent_paragraph_step(
        &self,
        position: &SnapshotTextPosition,
        movement: TextMovement,
    ) -> Option<SceneCursorStep> {
        let current = position.text().paragraph;
        let previous = matches!(
            movement,
            TextMovement::PreviousVisual | TextMovement::PreviousLogical
        );
        let paragraph = self
            .movements
            .iter()
            .map(|movement| movement.position.text().paragraph)
            .filter(|paragraph| {
                if previous {
                    *paragraph < current
                } else {
                    *paragraph > current
                }
            })
            .reduce(|candidate, paragraph| {
                if previous {
                    candidate.max(paragraph)
                } else {
                    candidate.min(paragraph)
                }
            })?;
        let mut candidates = self.movements.iter().filter(|record| {
            record.position.text().paragraph == paragraph
                && match movement {
                    TextMovement::PreviousVisual => record.next_visual.is_none(),
                    TextMovement::NextVisual => record.previous_visual.is_none(),
                    TextMovement::PreviousLogical => record.next_logical.is_none(),
                    TextMovement::NextLogical => record.previous_logical.is_none(),
                }
        });
        let target = candidates.next()?.position;
        if candidates.next().is_some() {
            return None;
        }
        Some(SceneCursorStep {
            target,
            source: None,
        })
    }

    fn collapse_for_movement(
        &self,
        selection: &SnapshotTextSelection,
        movement: TextMovement,
    ) -> Result<SnapshotTextSelection, SelectionError> {
        let anchor = *selection.anchor();
        let extent = *selection.extent();
        let choose_anchor = match movement {
            TextMovement::PreviousVisual | TextMovement::NextVisual => {
                let anchor_before = self.visual_ordering(&anchor, &extent)?.is_lt();
                matches!(movement, TextMovement::PreviousVisual) == anchor_before
            }
            TextMovement::PreviousLogical | TextMovement::NextLogical => {
                let anchor_before = self.compare_positions(&anchor, &extent)?.is_lt();
                matches!(movement, TextMovement::PreviousLogical) == anchor_before
            }
        };
        self.collapsed_selection(if choose_anchor { &anchor } else { &extent })
    }

    fn visual_ordering(
        &self,
        first: &SnapshotTextPosition,
        second: &SnapshotTextPosition,
    ) -> Result<core::cmp::Ordering, SelectionError> {
        if first == second {
            return Ok(core::cmp::Ordering::Equal);
        }
        if self.can_reach_visual(first, second, TextMovement::NextVisual)? {
            return Ok(core::cmp::Ordering::Less);
        }
        if self.can_reach_visual(first, second, TextMovement::PreviousVisual)? {
            return Ok(core::cmp::Ordering::Greater);
        }
        Err(SelectionError::new(
            SelectionErrorKind::DisconnectedMovement,
        ))
    }

    fn can_reach_visual(
        &self,
        start: &SnapshotTextPosition,
        end: &SnapshotTextPosition,
        movement: TextMovement,
    ) -> Result<bool, SelectionError> {
        let mut position = *start;
        for _ in 0..=self.movements.len() {
            if position == *end {
                return Ok(true);
            }
            let Some(step) = self.cursor_step(&position, movement)? else {
                return Ok(false);
            };
            position = step.target;
        }
        Ok(false)
    }

    fn compare_positions(
        &self,
        first: &SnapshotTextPosition,
        second: &SnapshotTextPosition,
    ) -> Result<core::cmp::Ordering, SelectionError> {
        Ok((self.text_rank(first.text())?, first.byte())
            .cmp(&(self.text_rank(second.text())?, second.byte())))
    }

    fn text_rank(&self, text: TextId) -> Result<usize, SelectionError> {
        self.texts
            .iter()
            .position(|range| range.text() == text)
            .ok_or_else(|| SelectionError::new(SelectionErrorKind::UnknownPosition))
    }

    pub(crate) fn range_geometry(&self, range: &SnapshotTextRange) -> Vec<(usize, Rect)> {
        self.clusters
            .iter()
            .filter(|cluster| ranges_overlap(range, &cluster.source))
            .map(|cluster| (cluster.line, cluster.bounds))
            .collect()
    }
}

fn projected_ranges_overlap(first: &ProjectedTextRange, second: &ProjectedTextRange) -> bool {
    first.sources().iter().any(|first| {
        second.sources().iter().any(|second| match (first, second) {
            (ProjectedTextSource::Snapshot(first), ProjectedTextSource::Snapshot(second)) => {
                ranges_overlap(first, second)
            }
            (ProjectedTextSource::Composition(first), ProjectedTextSource::Composition(second))
                if first.id() == second.id() && first.epoch() == second.epoch() =>
            {
                let first = first.bytes();
                let second = second.bytes();
                first.start < second.end && second.start < first.end
            }
            _ => false,
        })
    })
}

fn movement_mode(movement: TextMovement) -> TextSelectionMode {
    match movement {
        TextMovement::PreviousVisual | TextMovement::NextVisual => TextSelectionMode::Visual,
        TextMovement::PreviousLogical | TextMovement::NextLogical => TextSelectionMode::Logical,
    }
}

fn canonicalize_ranges(ranges: &mut Vec<SnapshotTextRange>, texts: &[SnapshotTextRange]) {
    ranges.sort_by_key(|range| {
        (
            texts
                .iter()
                .position(|text| text.text() == range.text())
                .unwrap_or(usize::MAX),
            range.bytes().start,
        )
    });
    let mut canonical: Vec<SnapshotTextRange> = Vec::with_capacity(ranges.len());
    for range in ranges.drain(..) {
        if let Some(previous) = canonical.last_mut()
            && previous.text() == range.text()
            && previous.bytes().end >= range.bytes().start
        {
            let start = previous.bytes().start;
            let end = previous.bytes().end.max(range.bytes().end);
            *previous = SnapshotTextRange::new(previous.revision(), previous.text(), start..end);
        } else {
            canonical.push(range);
        }
    }
    *ranges = canonical;
}

fn validate_independent_selections(
    selections: &[SnapshotTextSelection],
) -> Result<(), SelectionError> {
    for (index, selection) in selections.iter().enumerate() {
        for other in &selections[..index] {
            for range in selection.ranges() {
                for other_range in other.ranges() {
                    if ranges_conflict(range, other_range) {
                        return Err(SelectionError::new(
                            SelectionErrorKind::OverlappingSelections,
                        ));
                    }
                }
            }
        }
    }
    Ok(())
}

fn ranges_conflict(first: &SnapshotTextRange, second: &SnapshotTextRange) -> bool {
    if first.text() != second.text() {
        return false;
    }
    let first = first.bytes();
    let second = second.bytes();
    if first.is_empty() && second.is_empty() {
        first.start == second.start
    } else if first.is_empty() {
        second.start <= first.start && first.start <= second.end
    } else if second.is_empty() {
        first.start <= second.start && second.start <= first.end
    } else {
        first.start < second.end && second.start < first.end
    }
}

fn ranges_overlap(first: &SnapshotTextRange, second: &SnapshotTextRange) -> bool {
    if first.text() != second.text() {
        return false;
    }
    let first = first.bytes();
    let second = second.bytes();
    first.start < second.end && second.start < first.end
}

fn nearly_equal(first: f64, second: f64) -> bool {
    (first - second).abs() <= f64::max(1.0, first.abs().max(second.abs())) * 1.0e-9
}

#[derive(Clone, Debug)]
struct SceneCluster<Source = SnapshotTextRange, Position = SnapshotTextPosition> {
    source: Source,
    semantic_id: SemanticId,
    bounds: Rect,
    line: usize,
    left: Position,
    right: Position,
    bidi_level: u8,
}

impl<Source: Clone, Position: Copy> SceneCluster<Source, Position> {
    fn hit(&self, point: Point) -> TextHit<Source, Position> {
        let midpoint = self.bounds.x0 + self.bounds.width() * 0.5;
        TextHit {
            source: self.source.clone(),
            position: if point.x <= midpoint {
                self.left
            } else {
                self.right
            },
            semantic_id: self.semantic_id,
            bidi_level: self.bidi_level,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct SceneCaretStop<Position = SnapshotTextPosition> {
    position: Position,
    bounds: Rect,
}

#[derive(Clone, Debug)]
struct SceneCursorMovement<Source = SnapshotTextRange, Position = SnapshotTextPosition> {
    position: Position,
    previous_visual: Option<SceneCursorStep<Source, Position>>,
    next_visual: Option<SceneCursorStep<Source, Position>>,
    previous_logical: Option<SceneCursorStep<Source, Position>>,
    next_logical: Option<SceneCursorStep<Source, Position>>,
}

#[derive(Clone, Debug)]
struct SceneCursorStep<Source = SnapshotTextRange, Position = SnapshotTextPosition> {
    target: Position,
    source: Option<Source>,
}

fn distance_to_rect_axes(point: Point, bounds: Rect) -> (f64, f64) {
    let inline = if point.x < bounds.x0 {
        bounds.x0 - point.x
    } else if point.x > bounds.x1 {
        point.x - bounds.x1
    } else {
        0.0
    };
    let block = if point.y < bounds.y0 {
        bounds.y0 - point.y
    } else if point.y > bounds.y1 {
        point.y - bounds.y1
    } else {
        0.0
    };
    (block, inline)
}

/// One visual line.
#[derive(Clone, Debug)]
pub struct SceneLine<Source = SnapshotTextRange> {
    bounds: Rect,
    sources: Vec<Source>,
    break_reason: LineBreakReason,
    baseline: f64,
    content_ascent: f64,
    content_descent: f64,
}

impl<Source> SceneLine<Source> {
    /// Returns scene-space line bounds.
    #[must_use]
    pub const fn bounds(&self) -> Rect {
        self.bounds
    }

    /// Returns the source-complete slices represented by the line.
    ///
    /// A line has multiple slices when it crosses semantic text leaves.
    #[must_use]
    pub fn sources(&self) -> &[Source] {
        &self.sources
    }

    /// Returns why this line ended.
    #[must_use]
    pub const fn break_reason(&self) -> LineBreakReason {
        self.break_reason
    }

    /// Returns the scene-space baseline.
    #[must_use]
    pub const fn baseline(&self) -> f64 {
        self.baseline
    }

    /// Returns the maximum font ascent contributing to this line.
    #[must_use]
    pub const fn content_ascent(&self) -> f64 {
        self.content_ascent
    }

    /// Returns the maximum font descent contributing to this line.
    #[must_use]
    pub const fn content_descent(&self) -> f64 {
        self.content_descent
    }
}

/// Opaque identity of a fragment within the current retained engine context.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SceneFragmentId(u64);

/// Paint-homogeneous shaped glyph fragment.
#[derive(Clone, Debug)]
pub struct SceneFragment<Source = SnapshotTextRange> {
    id: SceneFragmentId,
    glyphs: Vec<SceneGlyph<Source>>,
    paint: PaintSlot,
    transform: Affine,
    source: Option<Source>,
    clip: Rect,
    font: FontData,
    font_size: f32,
    synthesis: FontSynthesis,
    normalized_coords: Arc<[i16]>,
    bidi_level: u8,
    script: [u8; 4],
}

impl<Source> SceneFragment<Source> {
    /// Returns the retained fragment identity.
    #[must_use]
    pub const fn id(&self) -> SceneFragmentId {
        self.id
    }

    /// Returns shaped glyph observations.
    #[must_use]
    pub fn glyphs(&self) -> &[SceneGlyph<Source>] {
        &self.glyphs
    }

    /// Returns the paint slot.
    #[must_use]
    pub const fn paint(&self) -> PaintSlot {
        self.paint
    }

    /// Returns the fragment transform.
    #[must_use]
    pub const fn transform(&self) -> Affine {
        self.transform
    }

    /// Returns the source covered by this fragment when one is present.
    #[must_use]
    pub const fn source(&self) -> Option<&Source> {
        self.source.as_ref()
    }

    /// Returns the exact font bytes and face index for these glyphs.
    #[must_use]
    pub const fn font(&self) -> &FontData {
        &self.font
    }

    /// Returns the scene-unit font size used to shape and position these glyphs.
    #[must_use]
    pub const fn font_size(&self) -> f32 {
        self.font_size
    }

    /// Returns synthesis suggestions selected for this font instance.
    #[must_use]
    pub const fn synthesis(&self) -> &FontSynthesis {
        &self.synthesis
    }

    /// Returns normalized variation coordinates for the exact font instance.
    #[must_use]
    pub fn normalized_coords(&self) -> &[i16] {
        &self.normalized_coords
    }

    /// Returns the scene-space clip preserving this fragment's paint coverage.
    #[must_use]
    pub const fn clip(&self) -> Rect {
        self.clip
    }

    /// Returns the resolved Unicode bidi level for the shaped run.
    #[must_use]
    pub const fn bidi_level(&self) -> u8 {
        self.bidi_level
    }

    /// Returns the resolved ISO 15924 script tag for the shaped run.
    #[must_use]
    pub const fn script(&self) -> [u8; 4] {
        self.script
    }
}

/// One shaped glyph observation.
#[derive(Clone, Debug)]
pub struct SceneGlyph<Source = SnapshotTextRange> {
    id: u32,
    position: Point,
    advance: Vec2,
    source: Source,
}

impl<Source> SceneGlyph<Source> {
    /// Returns the backend glyph identifier.
    #[must_use]
    pub const fn id(&self) -> u32 {
        self.id
    }

    /// Returns the glyph origin in scene coordinates.
    #[must_use]
    pub const fn position(&self) -> Point {
        self.position
    }

    /// Returns the shaped advance.
    #[must_use]
    pub const fn advance(&self) -> Vec2 {
        self.advance
    }

    /// Returns the source covered by this painted glyph observation.
    #[must_use]
    pub const fn source(&self) -> &Source {
        &self.source
    }
}

/// Semantic observation with scene geometry.
#[derive(Clone, Debug)]
pub struct SemanticFragment {
    semantic_id: SemanticId,
    paragraph_role: Option<ParagraphRole>,
    inline_role: Option<InlineRole>,
    source: Option<SnapshotTextRange>,
    bounds: Rect,
}

impl SemanticFragment {
    /// Returns the source semantic identity.
    #[must_use]
    pub const fn semantic_id(&self) -> SemanticId {
        self.semantic_id
    }

    /// Returns the paragraph role, or `None` for an inline semantic fragment.
    #[must_use]
    pub const fn paragraph_role(&self) -> Option<ParagraphRole> {
        self.paragraph_role
    }

    /// Returns the inline role, or `None` for a block-level semantic fragment.
    #[must_use]
    pub const fn inline_role(&self) -> Option<InlineRole> {
        self.inline_role
    }

    /// Returns snapshot-local source when present.
    #[must_use]
    pub const fn source(&self) -> Option<&SnapshotTextRange> {
        self.source.as_ref()
    }

    /// Returns scene-space semantic bounds.
    #[must_use]
    pub const fn bounds(&self) -> Rect {
        self.bounds
    }
}

/// Result of scene-space hit testing.
#[derive(Clone, Debug)]
pub struct TextHit<Source = SnapshotTextRange, Position = SnapshotTextPosition> {
    source: Source,
    position: Position,
    semantic_id: SemanticId,
    bidi_level: u8,
}

impl<Source, Position> TextHit<Source, Position> {
    /// Returns the exact source-complete cluster under the point.
    #[must_use]
    pub const fn source(&self) -> &Source {
        &self.source
    }

    /// Returns the collapsed position selected by the cluster side.
    #[must_use]
    pub const fn position(&self) -> &Position {
        &self.position
    }

    /// Returns the semantic text-node identity under the point.
    #[must_use]
    pub const fn semantic_id(&self) -> SemanticId {
        self.semantic_id
    }

    /// Returns the resolved bidi level of the hit cluster.
    #[must_use]
    pub const fn bidi_level(&self) -> u8 {
        self.bidi_level
    }
}

/// Exact scene-space caret for one snapshot position.
#[derive(Clone, Copy, Debug)]
pub struct SceneCaret<Position = SnapshotTextPosition> {
    position: Position,
    bounds: Rect,
}

impl<Position> SceneCaret<Position> {
    /// Returns the revision- or epoch-bound position represented by the caret.
    #[must_use]
    pub const fn position(&self) -> &Position {
        &self.position
    }

    /// Returns scene-space caret bounds.
    #[must_use]
    pub const fn bounds(&self) -> Rect {
        self.bounds
    }
}

/// One visual highlight rectangle owned by a selection and logical range.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneSelectionRect {
    selection: usize,
    range: usize,
    line: usize,
    bounds: Rect,
    bidi_level: u8,
}

/// One visual highlight rectangle for selected generated preedit text.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneCompositionRect {
    line: usize,
    bounds: Rect,
    bidi_level: u8,
}

impl SceneCompositionRect {
    /// Returns the visual line index within the transient scene.
    #[must_use]
    pub const fn line(self) -> usize {
        self.line
    }

    /// Returns scene-space highlight bounds.
    #[must_use]
    pub const fn bounds(self) -> Rect {
        self.bounds
    }

    /// Returns the bidi level of the covered visual run.
    #[must_use]
    pub const fn bidi_level(self) -> u8 {
        self.bidi_level
    }
}

impl SceneSelectionRect {
    /// Returns the selection index within the requested selection set.
    #[must_use]
    pub const fn selection(self) -> usize {
        self.selection
    }

    /// Returns the logical-range index within the owning selection.
    #[must_use]
    pub const fn range(self) -> usize {
        self.range
    }

    /// Returns the visual line index within the scene.
    #[must_use]
    pub const fn line(self) -> usize {
        self.line
    }

    /// Returns the scene-space highlight bounds.
    #[must_use]
    pub const fn bounds(self) -> Rect {
        self.bounds
    }

    /// Returns the bidi level of the covered visual run.
    #[must_use]
    pub const fn bidi_level(self) -> u8 {
        self.bidi_level
    }
}

#[cfg(test)]
mod tests {
    use alloc::{vec, vec::Vec};

    use peniko::Blob;

    use super::{LayoutEngine, append_inline_flow_run, append_shaping_run};
    use crate::adapter::{
        ClusterBoundary, ClusterWhitespace, FontSynthesis, FormationWork, GlyphPaintCoverage,
        GlyphPaintSegment, LineBreakReason, ParagraphConstraints, ParagraphFormation,
        ParagraphFormationOutput, ParagraphInput, PreparationError, PreparedCaret, PreparedCluster,
        PreparedClusterSide, PreparedCursorMovement, PreparedCursorStep, PreparedGlyph,
        PreparedLine, PreparedParagraph, PreparedRun, TextAffinity,
    };
    use crate::{
        Brush, Color, CompositionClause, CompositionClauseKind, CompositionErrorKind,
        CompositionId, CompositionSession, CompositionUpdate, ComputedInlineStyle, Document,
        DocumentId, EditableSurface, EditableSurfaceElement, FiniteWidth, FontData, FontFamily,
        InlineFlowStyle, InlineRole, PaintSlot, PaintTable, ParagraphRole, Point,
        ProjectedTextSource, Rect, SceneErrorKind, SceneRequest, ShapingStyle,
        SnapshotTextPosition, SnapshotTextRange, SnapshotTextSelection, SnapshotTextSelectionSet,
        StyleMap, SurfaceErrorKind, SurfaceTextEncoding, TextId, TextSelectionMode, Vec2,
    };

    #[derive(Debug)]
    struct EchoAdapter {
        split_utf8: bool,
        glyphless: bool,
        interior_cursor: bool,
    }

    impl ParagraphFormation for EchoAdapter {
        fn form(
            &mut self,
            input: ParagraphInput<'_>,
            _constraints: ParagraphConstraints,
        ) -> Result<ParagraphFormationOutput, PreparationError> {
            let text_len = u32::try_from(input.text().len())
                .map_err(|_| PreparationError::invalid_output())?;
            if text_len == 0 {
                let position = PreparedClusterSide::new(0, TextAffinity::Downstream);
                let movements = [PreparedCursorMovement::new(
                    position,
                    PreparedCaret::try_new(0, 0.0)?,
                    None,
                    None,
                    None,
                    None,
                )];
                let paragraph =
                    PreparedParagraph::try_new(input.paragraph(), text_len, [], movements)?;
                return Ok(ParagraphFormationOutput::new(
                    paragraph,
                    FormationWork::new(true, true, 0, 0, 0, 0, 0),
                ));
            }
            let glyph_source = if self.split_utf8 {
                1..text_len
            } else {
                0..text_len
            };
            let glyphs = if self.glyphless {
                Vec::new()
            } else {
                let segment = GlyphPaintSegment::new(
                    glyph_source.clone(),
                    input.paint_runs()[0].slot(),
                    Rect::new(0., -16., 10., 4.),
                )?;
                let coverage = GlyphPaintCoverage::try_from_segments([segment])?;
                vec![PreparedGlyph::try_new(
                    7,
                    glyph_source,
                    Vec2::new(10., 0.),
                    Vec2::ZERO,
                    coverage,
                )?]
            };
            let run = PreparedRun::try_new(
                0..text_len,
                0,
                *b"Latn",
                FontData::new(Blob::from(vec![0_u8]), 0),
                input.shaping_styles()[input.shaping_runs()[0].style().index()].font_size(),
                FontSynthesis::default(),
                [],
                [],
                glyphs,
            )?;
            let font_size =
                input.shaping_styles()[input.shaping_runs()[0].style().index()].font_size();
            let line_height = f64::from(font_size)
                * f64::from(
                    input.inline_flow_styles()[input.inline_flow_runs()[0].style().index()]
                        .line_height()
                        .multiplier(),
                );
            let line = PreparedLine::try_new(
                0..text_len,
                LineBreakReason::End,
                10.0,
                line_height / 2.0,
                line_height,
                f64::from(font_size) * 0.75,
                f64::from(font_size) * 0.25,
                [PreparedCluster::try_new(
                    0..text_len,
                    10.0,
                    0,
                    ClusterBoundary::None,
                    ClusterWhitespace::None,
                    PreparedClusterSide::new(0, TextAffinity::Downstream),
                    PreparedClusterSide::new(text_len, TextAffinity::Upstream),
                )?],
                [run],
            )?;
            let start = PreparedClusterSide::new(0, TextAffinity::Downstream);
            let end = PreparedClusterSide::new(text_len, TextAffinity::Upstream);
            let mut movements = vec![
                PreparedCursorMovement::new(
                    start,
                    PreparedCaret::try_new(0, 0.0)?,
                    None,
                    Some(PreparedCursorStep::new(end, Some(0..text_len))),
                    None,
                    Some(PreparedCursorStep::new(end, Some(0..text_len))),
                ),
                PreparedCursorMovement::new(
                    end,
                    PreparedCaret::try_new(0, 10.0)?,
                    Some(PreparedCursorStep::new(start, Some(0..text_len))),
                    None,
                    Some(PreparedCursorStep::new(start, Some(0..text_len))),
                    None,
                ),
            ];
            if self.interior_cursor {
                movements.push(PreparedCursorMovement::new(
                    PreparedClusterSide::new(1, TextAffinity::Downstream),
                    PreparedCaret::try_new(0, 5.0)?,
                    None,
                    None,
                    None,
                    None,
                ));
            }
            let paragraph =
                PreparedParagraph::try_new(input.paragraph(), text_len, [line], movements)?;
            Ok(ParagraphFormationOutput::new(
                paragraph,
                FormationWork::new(true, true, 1, 1, 1, 1, 2),
            ))
        }
    }

    #[test]
    fn layout_rejects_adapter_ranges_inside_a_utf8_scalar() {
        let (document, styles, paint) = one_leaf_document(*b"scene-test-doc01", "é");
        let mut layout = LayoutEngine::new(EchoAdapter {
            split_utf8: true,
            glyphless: false,
            interior_cursor: false,
        });
        let request = SceneRequest::new(
            FiniteWidth::new(100.).expect("test width is valid"),
            &styles,
            &paint,
        );
        let error = layout
            .prepare(&document.snapshot(), &request)
            .expect_err("mid-scalar adapter source must be rejected");
        assert_eq!(
            error.kind(),
            SceneErrorKind::SourceCoverage,
            "invalid UTF-8 coverage must be a source-coverage error"
        );
        assert!(
            error.paragraph().is_some(),
            "source-coverage diagnostics must identify the paragraph"
        );
        assert_eq!(
            error.source(),
            Some(1..2),
            "source-coverage diagnostics must retain the invalid range"
        );
    }

    #[test]
    fn layout_rejects_a_cursor_inside_a_utf8_scalar() {
        let (document, styles, paint) = one_leaf_document(*b"scene-test-doc08", "é");
        let mut layout = LayoutEngine::new(EchoAdapter {
            split_utf8: false,
            glyphless: false,
            interior_cursor: true,
        });
        let request = SceneRequest::new(
            FiniteWidth::new(100.).expect("test width is valid"),
            &styles,
            &paint,
        );
        let error = layout
            .prepare(&document.snapshot(), &request)
            .expect_err("mid-scalar cursor output must be rejected");
        assert_eq!(error.kind(), SceneErrorKind::SourceCoverage);
        assert_eq!(error.source(), Some(1..1));
    }

    #[test]
    fn layout_rejects_glyphless_non_control_source() {
        let (document, styles, paint) = one_leaf_document(*b"scene-test-doc06", "a");
        let mut layout = LayoutEngine::new(EchoAdapter {
            split_utf8: false,
            glyphless: true,
            interior_cursor: false,
        });
        let request = SceneRequest::new(
            FiniteWidth::new(100.).expect("test width is valid"),
            &styles,
            &paint,
        );
        let error = layout
            .prepare(&document.snapshot(), &request)
            .expect_err("glyphless non-control source must be rejected");
        assert_eq!(error.kind(), SceneErrorKind::SourceCoverage);
        assert_eq!(error.source(), Some(0..1));
    }

    #[test]
    fn layout_rejects_partially_unmapped_run_source() {
        let (document, styles, paint) = one_leaf_document(*b"scene-test-doc07", "ab");
        let mut layout = LayoutEngine::new(EchoAdapter {
            split_utf8: true,
            glyphless: false,
            interior_cursor: false,
        });
        let request = SceneRequest::new(
            FiniteWidth::new(100.).expect("test width is valid"),
            &styles,
            &paint,
        );
        let error = layout
            .prepare(&document.snapshot(), &request)
            .expect_err("every ordinary source scalar must map to a glyph");
        assert_eq!(error.kind(), SceneErrorKind::SourceCoverage);
        assert_eq!(error.source(), Some(0..1));
    }

    #[test]
    fn fragment_identity_is_distinct_across_documents() {
        let (first, first_styles, first_paint) = one_leaf_document(*b"scene-test-doc02", "a");
        let (second, second_styles, second_paint) = one_leaf_document(*b"scene-test-doc03", "b");
        let mut layout = LayoutEngine::new(EchoAdapter {
            split_utf8: false,
            glyphless: false,
            interior_cursor: false,
        });
        let width = FiniteWidth::new(100.).expect("test width is valid");
        let first_request = SceneRequest::new(width, &first_styles, &first_paint);
        let first_scene = layout
            .prepare(&first.snapshot(), &first_request)
            .expect("first scene must prepare");
        assert_eq!(
            first_scene.work().break_reshapes(),
            2,
            "adapter break-reshape work must survive scene reporting"
        );
        let second_request = SceneRequest::new(width, &second_styles, &second_paint);
        let second_scene = layout
            .prepare(&second.snapshot(), &second_request)
            .expect("second scene must prepare");
        assert_ne!(
            first_scene.scene().fragments()[0].id(),
            second_scene.scene().fragments()[0].id(),
            "document identity must participate in retained fragment identity"
        );
    }

    #[test]
    fn paragraph_projection_interns_repeated_style_partitions() {
        let (document, _, _) = one_leaf_document(*b"scene-test-doc04", "abc");
        let paragraph = document.snapshot().paragraphs()[0].id;
        let first = ShapingStyle::new(FontFamily::named("Test"), 16.).expect("test style is valid");
        let second =
            ShapingStyle::new(FontFamily::named("Test"), 24.).expect("test style is valid");
        let mut shaping_styles = Vec::new();
        let mut shaping_runs = Vec::new();
        append_shaping_run(
            &mut shaping_styles,
            &mut shaping_runs,
            0..1,
            &first,
            paragraph,
        )
        .expect("first style must intern");
        append_shaping_run(
            &mut shaping_styles,
            &mut shaping_runs,
            1..2,
            &second,
            paragraph,
        )
        .expect("second style must intern");
        append_shaping_run(
            &mut shaping_styles,
            &mut shaping_runs,
            2..3,
            &first,
            paragraph,
        )
        .expect("repeated style must intern");
        assert_eq!(shaping_styles, [&first, &second]);
        assert_eq!(shaping_runs[0].style().index(), 0);
        assert_eq!(shaping_runs[1].style().index(), 1);
        assert_eq!(shaping_runs[2].style().index(), 0);

        let compact = InlineFlowStyle::new(
            crate::LineHeight::from_multiplier(1.0).expect("line height is valid"),
        );
        let spacious = InlineFlowStyle::new(
            crate::LineHeight::from_multiplier(2.0).expect("line height is valid"),
        );
        let mut flow_styles = Vec::new();
        let mut flow_runs = Vec::new();
        append_inline_flow_run(&mut flow_styles, &mut flow_runs, 0..1, compact, paragraph)
            .expect("first flow style must intern");
        append_inline_flow_run(&mut flow_styles, &mut flow_runs, 1..2, spacious, paragraph)
            .expect("second flow style must intern");
        append_inline_flow_run(&mut flow_styles, &mut flow_runs, 2..3, compact, paragraph)
            .expect("repeated flow style must intern");
        assert_eq!(flow_styles, [compact, spacious]);
        assert_eq!(flow_runs[0].style().index(), 0);
        assert_eq!(flow_runs[1].style().index(), 1);
        assert_eq!(flow_runs[2].style().index(), 0);
    }

    #[test]
    fn empty_paragraph_line_height_has_a_flow_identity() {
        let mut document = Document::new(DocumentId::from_bytes(*b"scene-test-doc05"));
        let mut edit = document.edit();
        edit.append_paragraph(ParagraphRole::BODY)
            .expect("empty paragraph must append");
        let second = edit
            .append_paragraph(ParagraphRole::BODY)
            .expect("second paragraph must append");
        let text = edit
            .append_text(second, InlineRole::TEXT, "a")
            .expect("second paragraph text must append");
        edit.commit().expect("test document must commit");

        let shaping =
            ShapingStyle::new(FontFamily::named("Test"), 10.).expect("test style is valid");
        let compact = ComputedInlineStyle::new(
            shaping.clone(),
            InlineFlowStyle::new(
                crate::LineHeight::from_multiplier(1.0).expect("line height is valid"),
            ),
            PaintSlot::new(0),
        );
        let spacious = ComputedInlineStyle::new(
            shaping,
            InlineFlowStyle::new(
                crate::LineHeight::from_multiplier(2.0).expect("line height is valid"),
            ),
            PaintSlot::new(0),
        );
        let mut compact_styles = StyleMap::new(compact.clone());
        compact_styles.set(text, compact.clone());
        let mut spacious_styles = StyleMap::new(spacious);
        spacious_styles.set(text, compact);
        let paint = PaintTable::from_brushes([Brush::Solid(Color::BLACK)]);
        let width = FiniteWidth::new(100.).expect("test width is valid");
        let mut layout = LayoutEngine::new(EchoAdapter {
            split_utf8: false,
            glyphless: false,
            interior_cursor: false,
        });

        let compact_request = SceneRequest::new(width, &compact_styles, &paint);
        let compact_scene = layout
            .prepare(&document.snapshot(), &compact_request)
            .expect("compact scene must prepare");
        let spacious_request = SceneRequest::new(width, &spacious_styles, &paint);
        let spacious_scene = layout
            .prepare(&document.snapshot(), &spacious_request)
            .expect("spacious scene must prepare");
        assert_eq!(spacious_scene.work().shape().paragraphs(), 0);
        assert_eq!(spacious_scene.work().flow().paragraphs(), 1);
        assert_eq!(compact_scene.scene().lines()[0].bounds().y0, 10.0);
        assert_eq!(spacious_scene.scene().lines()[0].bounds().y0, 20.0);
    }

    #[test]
    fn composition_epochs_preserve_generated_provenance_and_committed_cache() {
        let (mut document, styles, paint) = one_leaf_document(*b"scene-test-doc09", "office");
        let snapshot = document.snapshot();
        let mut layout = LayoutEngine::new(EchoAdapter {
            split_utf8: false,
            glyphless: false,
            interior_cursor: false,
        });
        let request = SceneRequest::new(
            FiniteWidth::new(100.).expect("test width is valid"),
            &styles,
            &paint,
        );
        let committed = layout
            .prepare(&snapshot, &request)
            .expect("committed scene must prepare");
        let left = committed
            .scene()
            .hit_test(Point::new(0.0, 1.0))
            .expect("left cluster side must hit");
        let right = committed
            .scene()
            .hit_test(Point::new(9.9, 1.0))
            .expect("right cluster side must hit");
        let selection = committed
            .scene()
            .selection(
                left.position(),
                right.position(),
                TextSelectionMode::Logical,
            )
            .expect("whole-leaf selection must form");
        let selections = committed
            .scene()
            .selection_set([selection])
            .expect("selection set must validate");
        let start = committed
            .scene()
            .begin_composition(&selections, CompositionId::from_bytes(*b"composition-0001"))
            .expect("composition must start");
        assert!(!start.selection_changed());
        let mut session = start.into_session();
        let initial_epoch = session.epoch();
        let first_epoch = session
            .update(
                initial_epoch,
                CompositionUpdate::new("a\u{301}")
                    .with_selection(0..3)
                    .with_clauses([CompositionClause::new(
                        0..3,
                        CompositionClauseKind::Selected,
                    )]),
            )
            .expect("combining preedit must validate");
        let invalid_selection = session
            .update(
                first_epoch,
                CompositionUpdate::new("é").with_selection(1..1),
            )
            .expect_err("a selection inside one UTF-8 scalar must fail atomically");
        assert_eq!(
            invalid_selection.kind(),
            CompositionErrorKind::InvalidPreeditRange,
            "the error must identify the preedit selection rather than mutate it"
        );
        assert_eq!(
            session.epoch(),
            first_epoch,
            "a rejected preedit update must not advance the epoch"
        );
        assert_eq!(
            session.text(),
            "a\u{301}",
            "a rejected preedit update must retain the preceding text"
        );
        let invalid_clauses = session
            .update(
                first_epoch,
                CompositionUpdate::new("abcd").with_clauses([
                    CompositionClause::new(0..3, CompositionClauseKind::Raw),
                    CompositionClause::new(2..4, CompositionClauseKind::Selected),
                ]),
            )
            .expect_err("overlapping native clauses must fail atomically");
        assert_eq!(
            invalid_clauses.kind(),
            CompositionErrorKind::InvalidClauseRange,
            "the error must distinguish clause topology from the preedit selection"
        );
        assert_eq!(
            session.epoch(),
            first_epoch,
            "a rejected clause update must not advance the epoch"
        );

        let first = layout
            .prepare_composition(&snapshot, &request, &session)
            .expect("first transient epoch must prepare");
        assert_eq!(first.work().shape().paragraphs(), 1);
        assert_eq!(first.scene().epoch(), first_epoch);
        assert!(first.scene().fragments().iter().all(|fragment| {
            fragment.source().is_some_and(|source| {
                source.sources().iter().all(|segment| {
                    matches!(segment, ProjectedTextSource::Composition(range)
                        if range.id() == session.id() && range.epoch() == first_epoch)
                })
            })
        }));
        assert!(
            !first
                .scene()
                .composition_selection_geometry(&session)
                .expect("preedit selection geometry must resolve")
                .is_empty()
        );

        let repeated = layout
            .prepare_composition(&snapshot, &request, &session)
            .expect("same epoch must reuse transient work");
        assert_eq!(repeated.work().shape().paragraphs(), 0);
        assert_eq!(repeated.work().reused_paragraphs(), 1);

        let selection_epoch = session
            .update(
                first_epoch,
                CompositionUpdate::new("a\u{301}").with_selection(3..3),
            )
            .expect("selection-only preedit change must advance the epoch");
        let selection_only = layout
            .prepare_composition(&snapshot, &request, &session)
            .expect("selection-only epoch must rebind retained geometry");
        assert_eq!(selection_only.work().shape().paragraphs(), 0);
        assert_eq!(selection_only.work().geometry().paragraphs(), 0);
        assert_eq!(selection_only.work().reused_paragraphs(), 1);
        assert!(selection_only.scene().fragments().iter().all(|fragment| {
            fragment.source().is_some_and(|source| {
                source.sources().iter().all(|segment| {
                    matches!(segment, ProjectedTextSource::Composition(range)
                        if range.epoch() == selection_epoch)
                })
            })
        }));
        assert!(
            selection_only
                .scene()
                .composition_selection_geometry(&session)
                .expect("rebound selected range must resolve")
                .is_empty()
        );

        let second_epoch = session
            .update(
                selection_epoch,
                CompositionUpdate::new("مرحبا").with_selection(10..10),
            )
            .expect("Arabic preedit must validate");
        assert_eq!(second_epoch.get(), selection_epoch.get() + 1);
        let stale = session
            .update(first_epoch, CompositionUpdate::new("stale"))
            .expect_err("delayed epoch must fail");
        assert_eq!(stale.kind(), CompositionErrorKind::StaleEpoch);
        let second = layout
            .prepare_composition(&snapshot, &request, &session)
            .expect("updated transient epoch must prepare");
        assert_eq!(second.work().shape().paragraphs(), 1);
        assert_eq!(snapshot.text(left.position().text()), Some("office"));
        let surface = EditableSurface::new(
            &snapshot,
            [EditableSurfaceElement::text(left.position().text())],
        )
        .expect("focused surface must flatten the selected semantic leaf");
        let host = surface
            .bind_composition(second.scene(), &session)
            .expect("host queries must bind to the exact composition epoch");
        assert_eq!(
            surface
                .bind_composition(first.scene(), &session)
                .expect_err("host queries must reject geometry from an older epoch")
                .kind(),
            SurfaceErrorKind::WrongSnapshot,
            "text and geometry from different composition epochs must never be combined"
        );
        assert_eq!(host.text(), "مرحبا");
        assert_eq!(host.marked_range(), Some(0..10));
        assert_eq!(host.host_selection(), Some(10..10));
        assert_eq!(
            host.range_in_encoding(0..10, SurfaceTextEncoding::Utf16)
                .expect("Arabic range must convert to UTF-16"),
            0..5
        );
        assert_eq!(
            host.range_from_encoding(0..5, SurfaceTextEncoding::Utf16)
                .expect("UTF-16 range must round trip"),
            0..10
        );
        assert_eq!(
            host.text_for_range(0..10)
                .expect("synchronous text query must resolve"),
            "مرحبا"
        );
        assert!(host.caret_rect().is_some());
        assert!(
            host.first_rect_for_range(0..10)
                .expect("synchronous geometry query must resolve")
                .is_some()
        );
        assert!(
            host.offset_for_point(Point::new(0.0, 1.0)).is_some(),
            "point queries must map through the same transient scene"
        );

        let cancelled = layout
            .prepare(&snapshot, &request)
            .expect("cancelling must reveal committed geometry");
        assert_eq!(cancelled.work().shape().paragraphs(), 0);
        assert_eq!(cancelled.work().geometry().paragraphs(), 0);
        assert_eq!(cancelled.work().reused_paragraphs(), 1);

        let stale_session = session.clone();
        let replacement = session
            .commit(&mut document, "مرحبا")
            .expect("commit must publish one replacement");
        assert_eq!(replacement.publication().changes().paragraphs().len(), 1);
        assert_eq!(snapshot.text(left.position().text()), Some("office"));
        assert_eq!(
            replacement
                .publication()
                .snapshot()
                .text(left.position().text()),
            Some("مرحبا")
        );
        assert_eq!(
            layout
                .prepare_composition(
                    replacement.publication().snapshot(),
                    &request,
                    &stale_session,
                )
                .expect_err("a committed document revision must reject its stale preedit")
                .kind(),
            SceneErrorKind::InvalidComposition,
            "composition base revisions are exact rather than relocatable"
        );
    }

    #[test]
    fn composition_projection_rejects_a_missing_semantic_target() {
        let (document, styles, paint) = one_leaf_document(*b"scene-test-doc10", "office");
        let snapshot = document.snapshot();
        let missing = TextId {
            document: snapshot.id(),
            paragraph: 0,
            index: 99,
        };
        let position =
            SnapshotTextPosition::new(snapshot.revision(), missing, 0, TextAffinity::Downstream);
        let selection = SnapshotTextSelection::new(
            position,
            position,
            TextSelectionMode::Logical,
            vec![SnapshotTextRange::new(snapshot.revision(), missing, 0..0)],
        );
        let selections =
            SnapshotTextSelectionSet::new(snapshot.id(), snapshot.revision(), vec![selection]);
        let session =
            CompositionSession::new(CompositionId::from_bytes(*b"missing-target01"), selections);
        let request = SceneRequest::new(
            FiniteWidth::new(100.).expect("test width is valid"),
            &styles,
            &paint,
        );
        let mut layout = LayoutEngine::new(EchoAdapter {
            split_utf8: false,
            glyphless: false,
            interior_cursor: false,
        });
        assert_eq!(
            layout
                .prepare_composition(&snapshot, &request, &session)
                .expect_err("generated text must not be projected into a missing leaf")
                .kind(),
            SceneErrorKind::InvalidComposition,
            "a matching paragraph index is insufficient without the semantic text leaf"
        );
    }

    fn one_leaf_document(identity: [u8; 16], text: &str) -> (Document, StyleMap, PaintTable) {
        let mut document = Document::new(DocumentId::from_bytes(identity));
        let mut edit = document.edit();
        let paragraph = edit
            .append_paragraph(ParagraphRole::BODY)
            .expect("test paragraph must append");
        edit.append_text(paragraph, InlineRole::TEXT, text)
            .expect("test text must append");
        edit.commit().expect("test document must commit");
        let styles = StyleMap::new(ComputedInlineStyle::new(
            ShapingStyle::new(FontFamily::named("Test"), 16.).expect("test style must be valid"),
            InlineFlowStyle::default(),
            PaintSlot::new(0),
        ));
        let paint = PaintTable::from_brushes([Brush::Solid(Color::BLACK)]);
        (document, styles, paint)
    }
}
