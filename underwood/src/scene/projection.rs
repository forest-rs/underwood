// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Projection of semantic leaves into complete paragraph adapter inputs.
//!
//! This module owns style/source runs and composition projection; it explicitly
//! does not own retained cache policy or scene-space geometry.

use super::*;
use alloc::string::String;
use core::mem::{self, size_of};
use core::slice;

mod styles;

use styles::project_style_runs;

#[derive(Clone, Copy, Debug)]
pub(super) enum ParagraphSource<'a> {
    Document(&'a Paragraph),
    Block(&'a TextBlockSnapshot),
}

impl<'a> ParagraphSource<'a> {
    pub(super) const fn document(paragraph: &'a Paragraph) -> Self {
        Self::Document(paragraph)
    }

    pub(super) const fn block(block: &'a TextBlockSnapshot) -> Self {
        Self::Block(block)
    }

    pub(super) fn id(self) -> ParagraphId {
        match self {
            Self::Document(paragraph) => paragraph.id,
            Self::Block(block) => block.paragraph_id(),
        }
    }

    pub(super) fn version(self) -> u64 {
        match self {
            Self::Document(paragraph) => paragraph.version,
            Self::Block(block) => block.revision().0,
        }
    }

    pub(super) fn role(self) -> ParagraphRole {
        match self {
            Self::Document(paragraph) => paragraph.role,
            Self::Block(_) => ParagraphRole::BODY,
        }
    }

    pub(super) fn semantic_id(self) -> SemanticId {
        match self {
            Self::Document(paragraph) => paragraph.semantic_id(),
            Self::Block(block) => SemanticId::for_paragraph(block.paragraph_id()),
        }
    }

    pub(super) fn leaves(self) -> ParagraphLeaves<'a> {
        match self {
            Self::Document(paragraph) => ParagraphLeaves::Document(paragraph.leaves.iter()),
            Self::Block(block) => ParagraphLeaves::Block(Some(block)),
        }
    }

    pub(super) fn leaf_count(self) -> usize {
        match self {
            Self::Document(paragraph) => paragraph.leaves.len(),
            Self::Block(_) => 1,
        }
    }

    pub(super) fn is_empty(self) -> bool {
        self.leaves().all(|leaf| leaf.text().is_empty())
    }

    pub(super) fn project_text_into(self, text: &mut String) {
        let capacity = self.leaves().fold(0_usize, |total, leaf| {
            total.saturating_add(leaf.text().len())
        });
        text.clear();
        text.reserve(capacity);
        for leaf in self.leaves() {
            text.push_str(leaf.text());
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) enum LeafSource<'a> {
    Document(&'a crate::document::TextLeaf),
    Block(&'a TextBlockSnapshot),
}

impl<'a> LeafSource<'a> {
    pub(super) fn id(self) -> TextId {
        match self {
            Self::Document(leaf) => leaf.id,
            Self::Block(block) => block.text_id(),
        }
    }

    pub(super) fn role(self) -> InlineRole {
        match self {
            Self::Document(leaf) => leaf.role,
            Self::Block(_) => InlineRole::TEXT,
        }
    }

    pub(super) fn text(self) -> &'a str {
        match self {
            Self::Document(leaf) => leaf.text(),
            Self::Block(block) => block.text(),
        }
    }

    pub(super) fn semantic_id(self) -> SemanticId {
        match self {
            Self::Document(leaf) => leaf.semantic_id(),
            Self::Block(block) => SemanticId::for_text(block.text_id()),
        }
    }
}

#[derive(Clone, Debug)]
pub(super) enum ParagraphLeaves<'a> {
    Document(slice::Iter<'a, crate::document::TextLeaf>),
    Block(Option<&'a TextBlockSnapshot>),
}

impl<'a> Iterator for ParagraphLeaves<'a> {
    type Item = LeafSource<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Document(leaves) => leaves.next().map(LeafSource::Document),
            Self::Block(block) => block.take().map(LeafSource::Block),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}

impl ExactSizeIterator for ParagraphLeaves<'_> {
    fn len(&self) -> usize {
        match self {
            Self::Document(leaves) => leaves.len(),
            Self::Block(block) => usize::from(block.is_some()),
        }
    }
}

impl core::iter::FusedIterator for ParagraphLeaves<'_> {}

#[derive(Debug, Default)]
pub(super) struct ProjectionScratch {
    mapping_source: String,
    mapping_projected: String,
    mapping_segments: Vec<ProjectionSegment>,
    spans: Vec<LeafSpan>,
    analysis_styles: Vec<AnalysisStyle>,
    analysis_runs: Vec<AnalysisRun>,
    shaping_styles: Vec<ShapingStyle>,
    shaping_runs: Vec<ShapingRun>,
    inline_flow_styles: Vec<InlineFlowStyle>,
    inline_flow_runs: Vec<InlineFlowRun>,
    paint_runs: Vec<PaintRun>,
}

impl ProjectionScratch {
    pub(super) fn accounted_capacity_bytes(&self) -> usize {
        self.mapping_source
            .capacity()
            .saturating_add(self.mapping_projected.capacity())
            .saturating_add(
                size_of::<ProjectionSegment>().saturating_mul(self.mapping_segments.capacity()),
            )
            .saturating_add(size_of::<LeafSpan>().saturating_mul(self.spans.capacity()))
            .saturating_add(
                size_of::<AnalysisStyle>().saturating_mul(self.analysis_styles.capacity()),
            )
            .saturating_add(size_of::<AnalysisRun>().saturating_mul(self.analysis_runs.capacity()))
            .saturating_add(
                size_of::<ShapingStyle>().saturating_mul(self.shaping_styles.capacity()),
            )
            .saturating_add(size_of::<ShapingRun>().saturating_mul(self.shaping_runs.capacity()))
            .saturating_add(
                size_of::<InlineFlowStyle>().saturating_mul(self.inline_flow_styles.capacity()),
            )
            .saturating_add(
                size_of::<InlineFlowRun>().saturating_mul(self.inline_flow_runs.capacity()),
            )
            .saturating_add(size_of::<PaintRun>().saturating_mul(self.paint_runs.capacity()))
    }
}

#[derive(Clone, Debug)]
pub(super) struct Projection {
    pub(super) paragraph: ParagraphId,
    pub(super) mapping: TextProjection,
    pub(super) spans: Vec<LeafSpan>,
    pub(super) analysis_styles: Vec<AnalysisStyle>,
    pub(super) analysis_runs: Vec<AnalysisRun>,
    pub(super) shaping_styles: Vec<ShapingStyle>,
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

impl Projection {
    pub(super) fn new_in(
        paragraph: ParagraphSource<'_>,
        request: &SceneRequest<'_>,
        scratch: &mut ProjectionScratch,
    ) -> Result<Self, SceneError> {
        paragraph.project_text_into(&mut scratch.mapping_source);
        let paragraph_id = paragraph.id();
        let mut spans = mem::take(&mut scratch.spans);
        let mut analysis_styles = mem::take(&mut scratch.analysis_styles);
        let mut analysis_runs = mem::take(&mut scratch.analysis_runs);
        let mut shaping_styles = mem::take(&mut scratch.shaping_styles);
        let mut shaping_runs = mem::take(&mut scratch.shaping_runs);
        let mut inline_flow_styles = mem::take(&mut scratch.inline_flow_styles);
        let mut inline_flow_runs = mem::take(&mut scratch.inline_flow_runs);
        let mut paint_runs = mem::take(&mut scratch.paint_runs);
        spans.clear();
        analysis_styles.clear();
        analysis_runs.clear();
        shaping_styles.clear();
        shaping_runs.clear();
        inline_flow_styles.clear();
        inline_flow_runs.clear();
        paint_runs.clear();
        spans.reserve(paragraph.leaf_count());
        analysis_runs.reserve(paragraph.leaf_count());
        shaping_runs.reserve(paragraph.leaf_count());
        inline_flow_runs.reserve(paragraph.leaf_count());
        paint_runs.reserve(paragraph.leaf_count());
        let mut start = 0_u32;
        for leaf in paragraph.leaves() {
            let len = u32::try_from(leaf.text().len()).map_err(|_| {
                SceneError::for_paragraph(SceneErrorKind::SourceCoverage, paragraph_id)
            })?;
            let end = start.checked_add(len).ok_or_else(|| {
                SceneError::for_paragraph(SceneErrorKind::SourceCoverage, paragraph_id)
            })?;
            let style = request.styles.style_for(leaf.id());
            spans.push(LeafSpan {
                paragraph: start..end,
                text: leaf.id(),
                source: LeafSpanSource::Snapshot { start: 0 },
                leaf_len: len,
                role: leaf.role(),
                semantic: leaf.semantic_id(),
            });
            if start != end {
                append_analysis_run(
                    &mut analysis_styles,
                    &mut analysis_runs,
                    start..end,
                    style.analysis(),
                    paragraph_id,
                )?;
                append_shaping_run(
                    &mut shaping_styles,
                    &mut shaping_runs,
                    start..end,
                    style.shaping(),
                    paragraph_id,
                )?;
                append_inline_flow_run(
                    &mut inline_flow_styles,
                    &mut inline_flow_runs,
                    start..end,
                    style.inline_flow(),
                    paragraph_id,
                )?;
                append_paint_run(&mut paint_runs, start..end, style.paint());
            }
            start = end;
        }
        let paragraph_style = request.styles.paragraph_style_for(paragraph_id);
        let mapping = TextProjection::from_whitespace_reusing(
            mem::take(&mut scratch.mapping_source),
            paragraph_style.whitespace_collapse(),
            &mut scratch.mapping_projected,
            &mut scratch.mapping_segments,
        )
        .map_err(|_| SceneError::for_paragraph(SceneErrorKind::SourceCoverage, paragraph_id))?;
        project_style_runs(
            paragraph_id,
            &mapping,
            &mut analysis_runs,
            &mut shaping_runs,
            &mut inline_flow_runs,
            &mut paint_runs,
        )?;
        Ok(Self {
            paragraph: paragraph_id,
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
            paragraph_role: paragraph.role(),
        })
    }

    pub(super) fn with_composition_in(
        paragraph: ParagraphSource<'_>,
        request: &SceneRequest<'_>,
        composition: &CompositionSession,
        scratch: &mut ProjectionScratch,
    ) -> Result<Self, SceneError> {
        let paragraph_id = paragraph.id();
        let target = composition.target_text().ok_or_else(|| {
            SceneError::for_paragraph(SceneErrorKind::InvalidComposition, paragraph_id)
        })?;
        if target.paragraph != paragraph_id.index {
            return Err(SceneError::for_paragraph(
                SceneErrorKind::InvalidComposition,
                paragraph_id,
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
                paragraph_id,
            ));
        }

        let mut text = mem::take(&mut scratch.mapping_source);
        text.clear();
        let capacity = paragraph.leaf_count() + ranges.len() + 1;
        let mut spans = mem::take(&mut scratch.spans);
        let mut analysis_styles = mem::take(&mut scratch.analysis_styles);
        let mut analysis_runs = mem::take(&mut scratch.analysis_runs);
        let mut shaping_styles = mem::take(&mut scratch.shaping_styles);
        let mut shaping_runs = mem::take(&mut scratch.shaping_runs);
        let mut inline_flow_styles = mem::take(&mut scratch.inline_flow_styles);
        let mut inline_flow_runs = mem::take(&mut scratch.inline_flow_runs);
        let mut paint_runs = mem::take(&mut scratch.paint_runs);
        spans.clear();
        analysis_styles.clear();
        analysis_runs.clear();
        shaping_styles.clear();
        shaping_runs.clear();
        inline_flow_styles.clear();
        inline_flow_runs.clear();
        paint_runs.clear();
        spans.reserve(capacity);
        analysis_runs.reserve(capacity);
        shaping_runs.reserve(capacity);
        inline_flow_runs.reserve(capacity);
        paint_runs.reserve(capacity);
        let mut target_found = false;

        for leaf in paragraph.leaves() {
            let style = request.styles.style_for(leaf.id());
            if leaf.id() != target {
                append_projection_span(
                    paragraph_id,
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
                    leaf.text(),
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
                        .text()
                        .get(bytes.start as usize..bytes.end as usize)
                        .is_none()
                {
                    return Err(SceneError::for_source(
                        SceneErrorKind::InvalidComposition,
                        paragraph_id,
                        bytes,
                    ));
                }
                if source < bytes.start {
                    let retained = leaf
                        .text()
                        .get(source as usize..bytes.start as usize)
                        .ok_or_else(|| {
                            SceneError::for_source(
                                SceneErrorKind::InvalidComposition,
                                paragraph_id,
                                source..bytes.start,
                            )
                        })?;
                    append_projection_span(
                        paragraph_id,
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
                        paragraph_id,
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
            let end = u32::try_from(leaf.text().len()).map_err(|_| {
                SceneError::for_paragraph(SceneErrorKind::SourceCoverage, paragraph_id)
            })?;
            if source < end {
                let retained = leaf.text().get(source as usize..).ok_or_else(|| {
                    SceneError::for_source(
                        SceneErrorKind::InvalidComposition,
                        paragraph_id,
                        source..end,
                    )
                })?;
                append_projection_span(
                    paragraph_id,
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
                paragraph_id,
            ));
        }

        let paragraph_style = request.styles.paragraph_style_for(paragraph_id);
        let mapping = TextProjection::from_whitespace_reusing(
            text,
            paragraph_style.whitespace_collapse(),
            &mut scratch.mapping_projected,
            &mut scratch.mapping_segments,
        )
        .map_err(|_| SceneError::for_paragraph(SceneErrorKind::SourceCoverage, paragraph_id))?;
        project_style_runs(
            paragraph_id,
            &mapping,
            &mut analysis_runs,
            &mut shaping_runs,
            &mut inline_flow_runs,
            &mut paint_runs,
        )?;
        Ok(Self {
            paragraph: paragraph_id,
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
            paragraph_role: paragraph.role(),
        })
    }

    pub(super) fn recycle_into(self, scratch: &mut ProjectionScratch) {
        let Self {
            mapping,
            spans,
            analysis_styles,
            analysis_runs,
            shaping_styles,
            shaping_runs,
            inline_flow_styles,
            inline_flow_runs,
            paint_runs,
            ..
        } = self;
        mapping.recycle_into(
            &mut scratch.mapping_source,
            &mut scratch.mapping_projected,
            &mut scratch.mapping_segments,
        );
        scratch.spans = spans;
        scratch.analysis_styles = analysis_styles;
        scratch.analysis_runs = analysis_runs;
        scratch.shaping_styles = shaping_styles;
        scratch.shaping_runs = shaping_runs;
        scratch.inline_flow_styles = inline_flow_styles;
        scratch.inline_flow_runs = inline_flow_runs;
        scratch.paint_runs = paint_runs;
    }

    pub(super) fn whole_paint_slot(&self, source: Range<u32>) -> Option<PaintSlot> {
        let mut matching = self.paint_runs.iter().filter(|paint| {
            let paint_source = paint.bytes();
            paint_source.start < source.end && source.start < paint_source.end
        });
        let paint = matching.next()?;
        (matching.next().is_none()
            && paint.bytes().start <= source.start
            && paint.bytes().end >= source.end)
            .then(|| paint.slot())
    }

    pub(super) fn validate_source_range(&self, paragraph: Range<u32>) -> Result<(), SceneError> {
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
            let _ = span;
            return Ok(());
        }

        let mut covered = source.start;
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
            covered = end;
        }
        if covered != source.end {
            return Err(SceneError::for_source(
                SceneErrorKind::SourceCoverage,
                self.paragraph,
                paragraph,
            ));
        }
        Ok(())
    }

    pub(super) fn source_position(
        &self,
        paragraph_offset: u32,
        affinity: TextAffinity,
    ) -> Result<SourcePosition, SceneError> {
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
        span_for_position(&self.spans, source, affinity).ok_or_else(|| {
            SceneError::for_source(
                SceneErrorKind::SourceCoverage,
                self.paragraph,
                paragraph_offset..paragraph_offset,
            )
        })?;
        Ok(SourcePosition::new(paragraph_offset, affinity))
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

pub(super) fn append_projection_span(
    paragraph: ParagraphId,
    text: &mut String,
    spans: &mut Vec<LeafSpan>,
    analysis_styles: &mut Vec<AnalysisStyle>,
    analysis_runs: &mut Vec<AnalysisRun>,
    shaping_styles: &mut Vec<ShapingStyle>,
    shaping_runs: &mut Vec<ShapingRun>,
    inline_flow_styles: &mut Vec<InlineFlowStyle>,
    inline_flow_runs: &mut Vec<InlineFlowRun>,
    paint_runs: &mut Vec<PaintRun>,
    leaf: LeafSource<'_>,
    value: &str,
    source: LeafSpanSource,
    style: &ComputedInlineStyle,
) -> Result<(), SceneError> {
    let start = u32::try_from(text.len())
        .map_err(|_| SceneError::for_paragraph(SceneErrorKind::SourceCoverage, paragraph))?;
    text.push_str(value);
    let end = u32::try_from(text.len())
        .map_err(|_| SceneError::for_paragraph(SceneErrorKind::SourceCoverage, paragraph))?;
    spans.push(LeafSpan {
        paragraph: start..end,
        text: leaf.id(),
        source,
        leaf_len: u32::try_from(leaf.text().len())
            .map_err(|_| SceneError::for_paragraph(SceneErrorKind::SourceCoverage, paragraph))?,
        role: leaf.role(),
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

pub(super) fn append_shaping_run(
    styles: &mut Vec<ShapingStyle>,
    runs: &mut Vec<ShapingRun>,
    bytes: Range<u32>,
    style: &ShapingStyle,
    paragraph: ParagraphId,
) -> Result<(), SceneError> {
    let style = if let Some(index) = styles.iter().position(|candidate| candidate == style) {
        ShapingStyleId::new(
            u16::try_from(index)
                .map_err(|_| SceneError::for_paragraph(SceneErrorKind::InvalidStyle, paragraph))?,
        )
    } else {
        let index = u16::try_from(styles.len())
            .map_err(|_| SceneError::for_paragraph(SceneErrorKind::InvalidStyle, paragraph))?;
        styles.push(style.clone());
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
) -> Result<usize, SceneError> {
    for paragraph in request.features.overridden_paragraphs() {
        if paragraph.document != snapshot.id()
            || snapshot
                .paragraphs()
                .get(paragraph.index as usize)
                .is_none_or(|represented| represented.id != paragraph)
        {
            return Err(SceneError::for_paragraph(
                SceneErrorKind::InvalidFeatures,
                paragraph,
            ));
        }
    }
    let mut required_paint_slots = 0;
    let mut inline_overrides = 0_usize;
    let mut paragraph_overrides = 0_usize;
    for paragraph in snapshot.paragraphs() {
        if request
            .styles
            .paragraph_style_override(paragraph.id)
            .is_some()
        {
            paragraph_overrides = paragraph_overrides.saturating_add(1);
        }
        for leaf in &paragraph.leaves {
            if request.styles.style_override(leaf.id).is_some() {
                inline_overrides = inline_overrides.saturating_add(1);
            }
            let slot = request.styles.style_for(leaf.id).paint();
            if request.paint.brush(slot).is_none() {
                return Err(SceneError::for_paragraph(
                    SceneErrorKind::InvalidStyle,
                    paragraph.id,
                ));
            }
            required_paint_slots = required_paint_slots.max(slot.index() as usize + 1);
        }
    }
    if inline_overrides != request.styles.inline_override_count()
        || paragraph_overrides != request.styles.paragraph_override_count()
    {
        return Err(SceneError::for_document(
            SceneErrorKind::InvalidStyle,
            snapshot.id(),
        ));
    }
    Ok(required_paint_slots)
}

pub(super) fn validate_resolved_direction(
    prepared: &PreparedParagraph,
    projection: &Projection,
) -> Result<(), SceneError> {
    if matches!(
        (
            projection.paragraph_style.base_direction(),
            prepared.resolved_direction(),
        ),
        (BaseDirection::Ltr, ResolvedDirection::Rtl) | (BaseDirection::Rtl, ResolvedDirection::Ltr)
    ) {
        return Err(SceneError::from_preparation(
            prepared.paragraph(),
            PreparationErrorKind::InvalidOutput,
        ));
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
