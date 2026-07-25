// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Projection of semantic leaves into complete paragraph adapter inputs.
//!
//! This module owns style/source runs and composition projection; it explicitly
//! does not own retained cache policy or scene-space geometry.

use super::*;

mod styles;

use styles::project_style_runs;

#[derive(Clone, Debug)]
pub(super) struct Projection<'a> {
    pub(super) paragraph: ParagraphId,
    pub(super) mapping: TextProjection,
    pub(super) spans: Vec<LeafSpan>,
    pub(super) analysis_styles: Vec<AnalysisStyle>,
    pub(super) analysis_runs: Vec<AnalysisRun>,
    pub(super) shaping_styles: Vec<&'a ShapingStyle>,
    pub(super) shaping_runs: Vec<ShapingRun>,
    pub(super) inline_flow_styles: Vec<InlineFlowStyle>,
    pub(super) inline_flow_runs: Vec<InlineFlowRun>,
    pub(super) paint_runs: Vec<PaintRun>,
    pub(super) default_font_size: f32,
    pub(super) default_inline_flow: InlineFlowStyle,
    pub(super) paragraph_style: ParagraphStyle,
    pub(super) paragraph_semantic: SemanticId,
    pub(super) paragraph_role: ParagraphRole,
}

impl<'a> Projection<'a> {
    pub(super) fn new(
        paragraph: &Paragraph,
        request: &'a SceneRequest<'_>,
    ) -> Result<Self, SceneError> {
        let text = paragraph.projected_text();
        let mut spans = Vec::with_capacity(paragraph.leaves.len());
        let mut analysis_styles = Vec::new();
        let mut analysis_runs = Vec::with_capacity(paragraph.leaves.len());
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
                append_analysis_run(
                    &mut analysis_styles,
                    &mut analysis_runs,
                    start..end,
                    style.analysis(),
                    paragraph.id,
                )?;
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
        let paragraph_style = request.styles.paragraph_style_for(paragraph.id);
        let mapping = TextProjection::from_whitespace(text, paragraph_style.whitespace_collapse())
            .map_err(|_| SceneError::for_paragraph(SceneErrorKind::SourceCoverage, paragraph.id))?;
        project_style_runs(
            paragraph.id,
            &mapping,
            &mut analysis_runs,
            &mut shaping_runs,
            &mut inline_flow_runs,
            &mut paint_runs,
        )?;
        Ok(Self {
            paragraph: paragraph.id,
            mapping,
            spans,
            analysis_styles,
            analysis_runs,
            shaping_styles,
            shaping_runs,
            inline_flow_styles,
            inline_flow_runs,
            paint_runs,
            default_font_size: request.styles.default_style().shaping().font_size(),
            default_inline_flow: request.styles.default_style().inline_flow(),
            paragraph_style,
            paragraph_semantic: paragraph.semantic_id(),
            paragraph_role: paragraph.role,
        })
    }

    pub(super) fn with_composition(
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
        let mut analysis_styles = Vec::new();
        let mut analysis_runs = Vec::with_capacity(paragraph.leaves.len() + ranges.len() + 1);
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
                    &mut analysis_styles,
                    &mut analysis_runs,
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
                        &mut analysis_styles,
                        &mut analysis_runs,
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
                        &mut analysis_styles,
                        &mut analysis_runs,
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
                    &mut analysis_styles,
                    &mut analysis_runs,
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

        let paragraph_style = request.styles.paragraph_style_for(paragraph.id);
        let mapping = TextProjection::from_whitespace(text, paragraph_style.whitespace_collapse())
            .map_err(|_| SceneError::for_paragraph(SceneErrorKind::SourceCoverage, paragraph.id))?;
        project_style_runs(
            paragraph.id,
            &mapping,
            &mut analysis_runs,
            &mut shaping_runs,
            &mut inline_flow_runs,
            &mut paint_runs,
        )?;
        Ok(Self {
            paragraph: paragraph.id,
            mapping,
            spans,
            analysis_styles,
            analysis_runs,
            shaping_styles,
            shaping_runs,
            inline_flow_styles,
            inline_flow_runs,
            paint_runs,
            default_font_size: request.styles.default_style().shaping().font_size(),
            default_inline_flow: request.styles.default_style().inline_flow(),
            paragraph_style,
            paragraph_semantic: paragraph.semantic_id(),
            paragraph_role: paragraph.role,
        })
    }

    pub(super) fn local_ranges(
        &self,
        paragraph: Range<u32>,
    ) -> Result<Vec<LocalRange>, SceneError> {
        let source = self.mapping.source_range(paragraph.clone()).map_err(|_| {
            SceneError::for_source(
                SceneErrorKind::SourceCoverage,
                self.paragraph,
                paragraph.clone(),
            )
        })?;
        if source.is_empty() {
            let span = span_for_position(&self.spans, source.start, TextAffinity::Upstream)
                .ok_or_else(|| {
                    SceneError::for_source(
                        SceneErrorKind::SourceCoverage,
                        self.paragraph,
                        paragraph.clone(),
                    )
                })?;
            return Ok(alloc::vec![span.local_range(source.start, source.end)]);
        }

        let mut covered = source.start;
        let mut ranges = Vec::new();
        for span in &self.spans {
            let start = source.start.max(span.paragraph.start);
            let end = source.end.min(span.paragraph.end);
            if start >= end {
                continue;
            }
            if start != covered {
                return Err(SceneError::for_source(
                    SceneErrorKind::SourceCoverage,
                    self.paragraph,
                    paragraph.clone(),
                ));
            }
            ranges.push(span.local_range(start, end));
            covered = end;
        }
        if covered != source.end {
            return Err(SceneError::for_source(
                SceneErrorKind::SourceCoverage,
                self.paragraph,
                paragraph,
            ));
        }
        Ok(ranges)
    }

    pub(super) fn semantic_for_range(
        &self,
        paragraph: Range<u32>,
    ) -> Result<SemanticId, SceneError> {
        let owner = self.mapping.source_owner(paragraph.clone()).map_err(|_| {
            SceneError::for_source(
                SceneErrorKind::SourceCoverage,
                self.paragraph,
                paragraph.clone(),
            )
        })?;
        let transformed_unit = !paragraph.is_empty()
            && self.mapping.segments().any(|segment| {
                segment.kind() != ProjectionKind::Identity
                    && !segment.projected().is_empty()
                    && segment.projected().start <= paragraph.start
                    && paragraph.end <= segment.projected().end
            });
        if paragraph.is_empty() || transformed_unit {
            return span_for_position(&self.spans, owner, TextAffinity::Downstream)
                .or_else(|| span_for_position(&self.spans, owner, TextAffinity::Upstream))
                .map(|span| span.semantic)
                .ok_or_else(|| {
                    SceneError::for_source(
                        SceneErrorKind::SourceCoverage,
                        self.paragraph,
                        paragraph,
                    )
                });
        }

        let source = self.mapping.source_range(paragraph.clone()).map_err(|_| {
            SceneError::for_source(
                SceneErrorKind::SourceCoverage,
                self.paragraph,
                paragraph.clone(),
            )
        })?;
        let mut semantics = self
            .spans
            .iter()
            .filter(|span| span.paragraph.start < source.end && source.start < span.paragraph.end)
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

    pub(super) fn position_at(
        &self,
        paragraph_offset: u32,
        affinity: TextAffinity,
    ) -> Result<LocalPosition, SceneError> {
        let source = self
            .mapping
            .source_position(paragraph_offset, affinity)
            .map_err(|_| {
                SceneError::for_source(
                    SceneErrorKind::SourceCoverage,
                    self.paragraph,
                    paragraph_offset..paragraph_offset,
                )
            })?;
        let span = span_for_position(&self.spans, source, affinity).ok_or_else(|| {
            SceneError::for_source(
                SceneErrorKind::SourceCoverage,
                self.paragraph,
                paragraph_offset..paragraph_offset,
            )
        })?;
        Ok(span.local_position(source, affinity))
    }

    pub(super) fn empty_line_height_key(&self) -> u64 {
        if self.mapping.text().is_empty() {
            self.empty_line_height().to_bits()
        } else {
            0
        }
    }

    pub(super) fn empty_line_height(&self) -> f64 {
        let font_size = self.default_font_size;
        // Empty text selects no font. Until a paragraph strut carries selected
        // font metrics, metrics-relative height uses the computed font size as
        // its explicit deterministic fallback.
        f64::from(
            self.default_inline_flow
                .line_height()
                .resolve(font_size, font_size),
        )
    }

    pub(super) fn composition_identity(&self) -> Option<(CompositionId, crate::CompositionEpoch)> {
        self.spans.iter().find_map(|span| match span.source {
            LeafSpanSource::Composition { id, epoch, .. } => Some((id, epoch)),
            LeafSpanSource::Snapshot { .. } => None,
        })
    }
}

pub(super) fn append_projection_span<'a>(
    paragraph: ParagraphId,
    text: &mut alloc::string::String,
    spans: &mut Vec<LeafSpan>,
    analysis_styles: &mut Vec<AnalysisStyle>,
    analysis_runs: &mut Vec<AnalysisRun>,
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
        append_analysis_run(
            analysis_styles,
            analysis_runs,
            start..end,
            style.analysis(),
            paragraph,
        )?;
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

pub(super) fn span_for_position(
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
pub(super) struct LeafSpan {
    pub(super) paragraph: Range<u32>,
    pub(super) text: TextId,
    pub(super) source: LeafSpanSource,
    pub(super) leaf_len: u32,
    pub(super) role: InlineRole,
    pub(super) semantic: SemanticId,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum LeafSpanSource {
    Snapshot {
        start: u32,
    },
    Composition {
        id: CompositionId,
        epoch: crate::CompositionEpoch,
        start: u32,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ProjectionSourceKind {
    Snapshot { start: u32 },
    Composition { start: u32 },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum ProjectionSourceKey {
    Relation(ProjectionSegment),
    Text {
        paragraph: Range<u32>,
        text: TextId,
        source: ProjectionSourceKind,
    },
}

impl ProjectionSourceKey {
    pub(super) fn from_projection(projection: &Projection<'_>) -> Vec<Self> {
        let mut keys = Vec::with_capacity(
            projection.spans.len()
                + if projection.mapping.is_identity() {
                    0
                } else {
                    projection.mapping.segments().len()
                },
        );
        if !projection.mapping.is_identity() {
            keys.extend(projection.mapping.segments().map(Self::Relation));
        }
        keys.extend(projection.spans.iter().map(|span| Self::Text {
            paragraph: span.paragraph.clone(),
            text: span.text,
            source: match span.source {
                LeafSpanSource::Snapshot { start } => ProjectionSourceKind::Snapshot { start },
                LeafSpanSource::Composition { start, .. } => {
                    ProjectionSourceKind::Composition { start }
                }
            },
        }));
        keys
    }
}

impl LeafSpan {
    pub(super) fn local_range(&self, paragraph_start: u32, paragraph_end: u32) -> LocalRange {
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

    pub(super) fn local_position(&self, paragraph: u32, affinity: TextAffinity) -> LocalPosition {
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

pub(super) fn append_analysis_run(
    styles: &mut Vec<AnalysisStyle>,
    runs: &mut Vec<AnalysisRun>,
    bytes: Range<u32>,
    style: AnalysisStyle,
    paragraph: ParagraphId,
) -> Result<(), SceneError> {
    let style = if let Some(index) = styles.iter().position(|candidate| *candidate == style) {
        AnalysisStyleId::new(
            u16::try_from(index)
                .map_err(|_| SceneError::for_paragraph(SceneErrorKind::InvalidStyle, paragraph))?,
        )
    } else {
        let index = u16::try_from(styles.len())
            .map_err(|_| SceneError::for_paragraph(SceneErrorKind::InvalidStyle, paragraph))?;
        styles.push(style);
        AnalysisStyleId::new(index)
    };
    if let Some(last) = runs.last_mut()
        && last.bytes().end == bytes.start
        && last.style() == style
    {
        let start = last.bytes().start;
        *last = AnalysisRun::new(start..bytes.end, style);
    } else {
        runs.push(AnalysisRun::new(bytes, style));
    }
    Ok(())
}

pub(super) fn append_shaping_run<'a>(
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

pub(super) fn append_inline_flow_run(
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

pub(super) fn append_paint_run(runs: &mut Vec<PaintRun>, bytes: Range<u32>, slot: PaintSlot) {
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

pub(super) fn validate_styles(
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
    if request.styles.paragraph_overrides().iter().any(|(id, _)| {
        !snapshot
            .paragraphs()
            .iter()
            .any(|paragraph| paragraph.id == *id)
    }) {
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

pub(super) fn validate_prepared(
    prepared: &PreparedParagraph,
    projection: &Projection<'_>,
) -> Result<(), SceneError> {
    for line in prepared.lines() {
        let line_source = line.source();
        if projection
            .mapping
            .text()
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
                .mapping
                .text()
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
                    .mapping
                    .text()
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
                        .mapping
                        .text()
                        .get(source.start as usize..source.end as usize)
                        .is_none()
                    {
                        return Err(SceneError::for_source(
                            SceneErrorKind::SourceCoverage,
                            prepared.paragraph(),
                            source,
                        ));
                    }
                    let mut matching = projection.paint_runs.iter().filter(|paint| {
                        let paint_source = paint.bytes();
                        paint_source.start < source.end && source.start < paint_source.end
                    });
                    let Some(paint) = matching.next() else {
                        return Err(SceneError::from_preparation_source(
                            prepared.paragraph(),
                            source,
                            PreparationErrorKind::InvalidOutput,
                        ));
                    };
                    if matching.next().is_some()
                        || paint.bytes().start > source.start
                        || paint.bytes().end < source.end
                        || paint.slot() != segment.slot()
                    {
                        return Err(SceneError::from_preparation_source(
                            prepared.paragraph(),
                            source,
                            PreparationErrorKind::InvalidOutput,
                        ));
                    }
                    projection.local_ranges(source)?;
                }
            }
            for range in run.unrendered_source() {
                if projection
                    .mapping
                    .text()
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

pub(super) fn record_formation_work(report: &mut WorkReport, work: FormationWork) {
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
    if work.line_resolved_clusters() > 0 {
        report.line_font_resolution.paragraphs += 1;
        report.line_font_resolution.records += work.line_resolved_clusters() as usize;
    }
    if work.line_shaped_runs() > 0 {
        report.line_shape.paragraphs += 1;
        report.line_shape.records += work.line_shaped_glyphs() as usize;
    }
    if work.formed_lines() > 0 {
        report.flow.paragraphs += 1;
        report.flow.records += work.formed_lines() as usize;
    }
    report.line_reshapes += work.line_reshapes() as usize;
    report.line_candidates = report
        .line_candidates
        .saturating_add(work.line_candidates());
    report.rejected_line_candidates = report
        .rejected_line_candidates
        .saturating_add(work.rejected_line_candidates());
    report.line_checkpoint_restores = report
        .line_checkpoint_restores
        .saturating_add(work.line_checkpoint_restores());
}
