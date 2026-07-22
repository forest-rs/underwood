// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Pre-stable, backend-facing paragraph preparation contract.
//!
//! Successful outputs own every retained font, coordinate, and glyph record.
//! No backend-specific type crosses this boundary.

use alloc::sync::Arc;
use alloc::vec::Vec;
use core::fmt;
use core::ops::Range;

use crate::{
    Affine, FontData, FontVariation, InlineFlowStyle, PaintSlot, ParagraphId, Rect, ShapingStyle,
    Vec2,
};

/// Logical attachment of a snapshot-local text position.
///
/// Affinity distinguishes the two visual caret locations that can share one
/// logical UTF-8 boundary at a soft wrap or bidi discontinuity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TextAffinity {
    /// The position is attached to source immediately before the boundary.
    Upstream,
    /// The position is attached to source immediately after the boundary.
    Downstream,
}

/// Forms portable lines for one paragraph through a retained text backend.
pub trait ParagraphFormation {
    /// Produces validated, owned formed lines for `input` and `constraints`.
    fn form(
        &mut self,
        input: ParagraphInput<'_>,
        constraints: ParagraphConstraints,
    ) -> Result<ParagraphFormationOutput, PreparationError>;
}

/// Borrowed projection of one semantic paragraph.
#[derive(Clone, Copy, Debug)]
pub struct ParagraphInput<'a> {
    paragraph: ParagraphId,
    text: &'a str,
    shaping_styles: &'a [ShapingStyle],
    shaping_runs: &'a [ShapingRun],
    inline_flow_styles: &'a [InlineFlowStyle],
    inline_flow_runs: &'a [InlineFlowRun],
    paint_runs: &'a [PaintRun],
}

impl<'a> ParagraphInput<'a> {
    pub(crate) const fn new(
        paragraph: ParagraphId,
        text: &'a str,
        shaping_styles: &'a [ShapingStyle],
        shaping_runs: &'a [ShapingRun],
        inline_flow_styles: &'a [InlineFlowStyle],
        inline_flow_runs: &'a [InlineFlowRun],
        paint_runs: &'a [PaintRun],
    ) -> Self {
        Self {
            paragraph,
            text,
            shaping_styles,
            shaping_runs,
            inline_flow_styles,
            inline_flow_runs,
            paint_runs,
        }
    }

    /// Returns the paragraph-local table of unique shaping values.
    #[must_use]
    pub const fn shaping_styles(&self) -> &[ShapingStyle] {
        self.shaping_styles
    }

    /// Returns the paragraph identity.
    #[must_use]
    pub const fn paragraph(&self) -> ParagraphId {
        self.paragraph
    }

    /// Returns the complete projected UTF-8 paragraph.
    #[must_use]
    pub const fn text(&self) -> &str {
        self.text
    }

    /// Returns source-ordered shaping metadata covering the paragraph.
    #[must_use]
    pub const fn shaping_runs(&self) -> &[ShapingRun] {
        self.shaping_runs
    }

    /// Returns the paragraph-local table of unique inline-flow values.
    #[must_use]
    pub const fn inline_flow_styles(&self) -> &[InlineFlowStyle] {
        self.inline_flow_styles
    }

    /// Returns source-ordered inline-flow metadata covering the paragraph.
    #[must_use]
    pub const fn inline_flow_runs(&self) -> &[InlineFlowRun] {
        self.inline_flow_runs
    }

    /// Returns source-ordered paint metadata covering the paragraph.
    #[must_use]
    pub const fn paint_runs(&self) -> &[PaintRun] {
        self.paint_runs
    }
}

/// Validated paragraph formation constraints.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ParagraphConstraints {
    max_inline_advance: f64,
}

impl ParagraphConstraints {
    pub(crate) fn try_new(max_inline_advance: f64) -> Result<Self, PreparationError> {
        if !max_inline_advance.is_finite() || max_inline_advance <= 0.0 {
            return Err(PreparationError::invalid_output());
        }
        Ok(Self { max_inline_advance })
    }

    /// Returns the finite positive maximum inline advance.
    #[must_use]
    pub const fn max_inline_advance(self) -> f64 {
        self.max_inline_advance
    }
}

/// Dense paragraph-local identity for one entry in the shaping-style table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShapingStyleId(u16);

impl ShapingStyleId {
    pub(crate) const fn new(index: u16) -> Self {
        Self(index)
    }

    /// Returns the paragraph-local table index.
    #[must_use]
    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

/// Complete shaping values over a paragraph-local UTF-8 byte range.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShapingRun {
    bytes: Range<u32>,
    style: ShapingStyleId,
}

impl ShapingRun {
    pub(crate) const fn new(bytes: Range<u32>, style: ShapingStyleId) -> Self {
        Self { bytes, style }
    }

    /// Returns the paragraph-local UTF-8 byte range.
    #[must_use]
    pub fn bytes(&self) -> Range<u32> {
        self.bytes.clone()
    }

    /// Returns the paragraph-local shaping-style identity for this range.
    #[must_use]
    pub const fn style(&self) -> ShapingStyleId {
        self.style
    }
}

/// Dense paragraph-local identity for one entry in the inline-flow table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InlineFlowStyleId(u16);

impl InlineFlowStyleId {
    pub(crate) const fn new(index: u16) -> Self {
        Self(index)
    }

    /// Returns the paragraph-local table index.
    #[must_use]
    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

/// Complete inline-flow values over a paragraph-local UTF-8 byte range.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InlineFlowRun {
    bytes: Range<u32>,
    style: InlineFlowStyleId,
}

impl InlineFlowRun {
    pub(crate) const fn new(bytes: Range<u32>, style: InlineFlowStyleId) -> Self {
        Self { bytes, style }
    }

    /// Returns the paragraph-local UTF-8 byte range.
    #[must_use]
    pub fn bytes(&self) -> Range<u32> {
        self.bytes.clone()
    }

    /// Returns the paragraph-local inline-flow-style identity for this range.
    #[must_use]
    pub const fn style(&self) -> InlineFlowStyleId {
        self.style
    }
}

/// Paint slot over a paragraph-local UTF-8 byte range.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PaintRun {
    bytes: Range<u32>,
    slot: PaintSlot,
}

impl PaintRun {
    pub(crate) const fn new(bytes: Range<u32>, slot: PaintSlot) -> Self {
        Self { bytes, slot }
    }

    /// Returns the paragraph-local UTF-8 byte range.
    #[must_use]
    pub fn bytes(&self) -> Range<u32> {
        self.bytes.clone()
    }

    /// Returns the paint slot for this range.
    #[must_use]
    pub const fn slot(&self) -> PaintSlot {
        self.slot
    }
}

/// Owned paragraph data and exact work performed to produce it.
#[derive(Clone, Debug)]
pub struct ParagraphFormationOutput {
    paragraph: PreparedParagraph,
    work: FormationWork,
}

impl ParagraphFormationOutput {
    /// Pairs validated prepared data with actual backend work.
    #[must_use]
    pub const fn new(paragraph: PreparedParagraph, work: FormationWork) -> Self {
        Self { paragraph, work }
    }

    /// Returns the prepared paragraph.
    #[must_use]
    pub const fn paragraph(&self) -> &PreparedParagraph {
        &self.paragraph
    }

    /// Returns the work performed by the adapter.
    #[must_use]
    pub const fn work(&self) -> FormationWork {
        self.work
    }
}

/// Actual adapter work performed during one preparation call.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FormationWork {
    analyzed: bool,
    itemized: bool,
    selected_clusters: u32,
    shaped_runs: u32,
    shaped_glyphs: u32,
    formed_lines: u32,
    break_reshapes: u32,
}

impl FormationWork {
    /// Creates a work record from backend observations.
    #[must_use]
    pub const fn new(
        analyzed: bool,
        itemized: bool,
        selected_clusters: u32,
        shaped_runs: u32,
        shaped_glyphs: u32,
        formed_lines: u32,
        break_reshapes: u32,
    ) -> Self {
        Self {
            analyzed,
            itemized,
            selected_clusters,
            shaped_runs,
            shaped_glyphs,
            formed_lines,
            break_reshapes,
        }
    }

    /// Returns whether Unicode analysis ran.
    #[must_use]
    pub const fn analyzed(self) -> bool {
        self.analyzed
    }

    /// Returns whether itemization ran.
    #[must_use]
    pub const fn itemized(self) -> bool {
        self.itemized
    }

    /// Returns the number of clusters for which the adapter selected a font.
    #[must_use]
    pub const fn selected_clusters(self) -> u32 {
        self.selected_clusters
    }

    /// Returns the number of shaped runs.
    #[must_use]
    pub const fn shaped_runs(self) -> u32 {
        self.shaped_runs
    }

    /// Returns the number of shaped glyphs.
    #[must_use]
    pub const fn shaped_glyphs(self) -> u32 {
        self.shaped_glyphs
    }

    /// Returns the number of lines formed for new constraints or flow values.
    #[must_use]
    pub const fn formed_lines(self) -> u32 {
        self.formed_lines
    }

    /// Returns the number of committed boundaries that required bounded reshaping.
    #[must_use]
    pub const fn break_reshapes(self) -> u32 {
        self.break_reshapes
    }
}

/// Portable synthesis suggestions retained with an exact selected font.
///
/// Variation settings are shaping inputs. Embolden and skew are renderer-facing
/// suggestions whose execution depends on renderer capabilities.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FontSynthesis {
    evidence: Option<Arc<FontSynthesisEvidence>>,
}

#[derive(Debug, PartialEq)]
struct FontSynthesisEvidence {
    variations: Vec<FontVariation>,
    embolden: bool,
    skew_degrees: f32,
}

impl FontSynthesis {
    /// Validates and owns synthesis evidence from a preparation backend.
    pub fn try_new(
        variations: impl IntoIterator<Item = FontVariation>,
        embolden: bool,
        skew_degrees: Option<f32>,
    ) -> Result<Self, PreparationError> {
        let mut input: Vec<_> = variations.into_iter().collect();
        if input.iter().any(|variation| !variation.value.is_finite())
            || skew_degrees.is_some_and(|angle| !angle.is_finite())
        {
            return Err(PreparationError::invalid_output());
        }
        let mut variations = Vec::with_capacity(input.len());
        while let Some(mut variation) = input.pop() {
            if variation.value == 0.0 {
                variation.value = 0.0;
            }
            if !variations
                .iter()
                .any(|candidate: &FontVariation| candidate.tag == variation.tag)
            {
                variations.push(variation);
            }
        }
        variations.sort_by_key(|variation| variation.tag);
        let skew_degrees = skew_degrees.filter(|angle| *angle != 0.0);
        let evidence = (!variations.is_empty() || embolden || skew_degrees.is_some()).then(|| {
            Arc::new(FontSynthesisEvidence {
                variations,
                embolden,
                skew_degrees: skew_degrees.unwrap_or(0.0),
            })
        });
        Ok(Self { evidence })
    }

    /// Returns variation settings suggested by the font resolver.
    #[must_use]
    pub fn variations(&self) -> &[FontVariation] {
        self.evidence
            .as_ref()
            .map_or(&[], |evidence| evidence.variations.as_slice())
    }

    /// Returns whether the renderer should apply synthetic emboldening.
    #[must_use]
    pub fn embolden(&self) -> bool {
        match &self.evidence {
            Some(evidence) => evidence.embolden,
            None => false,
        }
    }

    /// Returns a synthetic skew angle in degrees, when requested.
    #[must_use]
    pub fn skew_degrees(&self) -> Option<f32> {
        match &self.evidence {
            Some(evidence) if evidence.skew_degrees != 0.0 => Some(evidence.skew_degrees),
            Some(_) | None => None,
        }
    }

    /// Returns the renderer-facing affine transform for synthetic skew.
    ///
    /// Coverage adapters and renderers should use this shared transform so
    /// their `no_std` math and glyph geometry remain identical.
    #[must_use]
    pub fn skew_transform(&self) -> Option<Affine> {
        self.skew_degrees()
            .map(|degrees| Affine::skew(f64::from(libm::tanf(degrees.to_radians())), 0.0))
    }
}

/// Why a formed line ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineBreakReason {
    /// The paragraph ended without another break.
    End,
    /// The line ended at a legal soft-wrap opportunity.
    Regular,
    /// The line ended at an explicit mandatory break.
    Mandatory,
}

/// Unicode boundary fact attached to one prepared cluster.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClusterBoundary {
    /// The cluster does not begin a word or line-break opportunity.
    None,
    /// The cluster begins a word.
    Word,
    /// The cluster begins a possible line break.
    Line,
    /// The cluster carries a mandatory line break.
    Mandatory,
}

/// Whitespace classification attached to one prepared cluster.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClusterWhitespace {
    /// The cluster is not whitespace with special cursor behavior.
    None,
    /// The cluster represents U+0020 SPACE.
    Space,
    /// The cluster represents U+00A0 NO-BREAK SPACE.
    NoBreakSpace,
    /// The cluster represents a horizontal tab.
    Tab,
    /// The cluster represents a mandatory line-break scalar.
    Newline,
}

/// One logical position reached from a visual side of a prepared cluster.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PreparedClusterSide {
    offset: u32,
    affinity: TextAffinity,
}

impl PreparedClusterSide {
    /// Creates a paragraph-local cluster-side position.
    #[must_use]
    pub const fn new(offset: u32, affinity: TextAffinity) -> Self {
        Self { offset, affinity }
    }

    /// Returns the paragraph-local UTF-8 boundary.
    #[must_use]
    pub const fn offset(self) -> u32 {
        self.offset
    }

    /// Returns which logical side owns the position.
    #[must_use]
    pub const fn affinity(self) -> TextAffinity {
        self.affinity
    }
}

/// One paragraph-local cursor step supplied by a formation backend.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreparedCursorStep {
    target: PreparedClusterSide,
    source: Option<Range<u32>>,
}

impl PreparedCursorStep {
    /// Creates a step and the source cluster crossed by it, when any.
    #[must_use]
    pub const fn new(target: PreparedClusterSide, source: Option<Range<u32>>) -> Self {
        Self { target, source }
    }

    /// Returns the destination position.
    #[must_use]
    pub const fn target(&self) -> PreparedClusterSide {
        self.target
    }

    /// Returns the source cluster crossed by this step.
    ///
    /// A transition across a soft wrap carries no source cluster.
    #[must_use]
    pub fn source(&self) -> Option<Range<u32>> {
        self.source.clone()
    }
}

/// Paragraph-local caret placement for one cursor position.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PreparedCaret {
    line: u32,
    inline: f64,
}

impl PreparedCaret {
    /// Creates a caret placement in one prepared line.
    pub fn try_new(line: u32, inline: f64) -> Result<Self, PreparationError> {
        if !inline.is_finite() || inline < 0.0 {
            return Err(PreparationError::invalid_output());
        }
        Ok(Self { line, inline })
    }

    /// Returns the prepared line index.
    #[must_use]
    pub const fn line(self) -> u32 {
        self.line
    }

    /// Returns the inline-axis caret coordinate within the line.
    #[must_use]
    pub const fn inline(self) -> f64 {
        self.inline
    }
}

/// Paragraph-local cursor transitions supplied by a formation backend.
///
/// Underwood maps these positions into semantic snapshot positions without
/// reconstructing bidi or soft-wrap cursor rules.
#[derive(Clone, Debug, PartialEq)]
pub struct PreparedCursorMovement {
    position: PreparedClusterSide,
    caret: PreparedCaret,
    previous_visual: Option<PreparedCursorStep>,
    next_visual: Option<PreparedCursorStep>,
    previous_logical: Option<PreparedCursorStep>,
    next_logical: Option<PreparedCursorStep>,
}

impl PreparedCursorMovement {
    /// Creates the movement facts for one paragraph-local position.
    #[must_use]
    pub const fn new(
        position: PreparedClusterSide,
        caret: PreparedCaret,
        previous_visual: Option<PreparedCursorStep>,
        next_visual: Option<PreparedCursorStep>,
        previous_logical: Option<PreparedCursorStep>,
        next_logical: Option<PreparedCursorStep>,
    ) -> Self {
        Self {
            position,
            caret,
            previous_visual,
            next_visual,
            previous_logical,
            next_logical,
        }
    }

    /// Returns the source position for these transitions.
    #[must_use]
    pub const fn position(&self) -> PreparedClusterSide {
        self.position
    }

    /// Returns the exact paragraph-local caret placement.
    #[must_use]
    pub const fn caret(&self) -> PreparedCaret {
        self.caret
    }

    /// Returns the preceding position in visual order.
    #[must_use]
    pub const fn previous_visual(&self) -> Option<&PreparedCursorStep> {
        self.previous_visual.as_ref()
    }

    /// Returns the following position in visual order.
    #[must_use]
    pub const fn next_visual(&self) -> Option<&PreparedCursorStep> {
        self.next_visual.as_ref()
    }

    /// Returns the preceding source-cluster boundary in logical order.
    #[must_use]
    pub const fn previous_logical(&self) -> Option<&PreparedCursorStep> {
        self.previous_logical.as_ref()
    }

    /// Returns the following source-cluster boundary in logical order.
    #[must_use]
    pub const fn next_logical(&self) -> Option<&PreparedCursorStep> {
        self.next_logical.as_ref()
    }
}

/// One source cluster in line-local visual order.
///
/// The paragraph adapter supplies both visual sides so the scene layer never
/// reconstructs cursor direction from glyph order.
#[derive(Clone, Debug)]
pub struct PreparedCluster {
    source: Range<u32>,
    advance: f64,
    bidi_level: u8,
    boundary: ClusterBoundary,
    whitespace: ClusterWhitespace,
    left: PreparedClusterSide,
    right: PreparedClusterSide,
}

impl PreparedCluster {
    /// Validates one portable cluster and its two visual sides.
    pub fn try_new(
        source: Range<u32>,
        advance: f64,
        bidi_level: u8,
        boundary: ClusterBoundary,
        whitespace: ClusterWhitespace,
        left: PreparedClusterSide,
        right: PreparedClusterSide,
    ) -> Result<Self, PreparationError> {
        if source.start >= source.end
            || !advance.is_finite()
            || advance < 0.0
            || !matches!(left.offset, offset if offset == source.start || offset == source.end)
            || !matches!(right.offset, offset if offset == source.start || offset == source.end)
        {
            return Err(PreparationError::invalid_output());
        }
        Ok(Self {
            source,
            advance,
            bidi_level,
            boundary,
            whitespace,
            left,
            right,
        })
    }

    /// Returns the paragraph-local UTF-8 source range.
    #[must_use]
    pub fn source(&self) -> Range<u32> {
        self.source.clone()
    }

    /// Returns the visual inline advance.
    #[must_use]
    pub const fn advance(&self) -> f64 {
        self.advance
    }

    /// Returns the resolved bidi level.
    #[must_use]
    pub const fn bidi_level(&self) -> u8 {
        self.bidi_level
    }

    /// Returns the Unicode boundary fact.
    #[must_use]
    pub const fn boundary(&self) -> ClusterBoundary {
        self.boundary
    }

    /// Returns the whitespace classification.
    #[must_use]
    pub const fn whitespace(&self) -> ClusterWhitespace {
        self.whitespace
    }

    /// Returns the position reached from the visual left side.
    #[must_use]
    pub const fn left(&self) -> PreparedClusterSide {
        self.left
    }

    /// Returns the position reached from the visual right side.
    #[must_use]
    pub const fn right(&self) -> PreparedClusterSide {
        self.right
    }
}

/// One source-complete line with backend-derived metrics and visual runs.
#[derive(Clone, Debug)]
pub struct PreparedLine {
    source: Range<u32>,
    break_reason: LineBreakReason,
    advance: f64,
    baseline: f64,
    height: f64,
    content_ascent: f64,
    content_descent: f64,
    clusters: Vec<PreparedCluster>,
    runs: Vec<PreparedRun>,
}

impl PreparedLine {
    /// Validates and owns one formed line.
    pub fn try_new(
        source: Range<u32>,
        break_reason: LineBreakReason,
        advance: f64,
        baseline: f64,
        height: f64,
        content_ascent: f64,
        content_descent: f64,
        clusters: impl IntoIterator<Item = PreparedCluster>,
        runs: impl IntoIterator<Item = PreparedRun>,
    ) -> Result<Self, PreparationError> {
        if source.start > source.end
            || !advance.is_finite()
            || advance < 0.0
            || !baseline.is_finite()
            || baseline < 0.0
            || !height.is_finite()
            || height <= 0.0
            || baseline > height
            || !content_ascent.is_finite()
            || content_ascent < 0.0
            || !content_descent.is_finite()
            || content_descent < 0.0
        {
            return Err(PreparationError::invalid_output());
        }
        let clusters: Vec<_> = clusters.into_iter().collect();
        let runs: Vec<_> = runs.into_iter().collect();
        let mut coverage: Vec<_> = runs.iter().map(|run| run.source.clone()).collect();
        coverage.sort_unstable_by_key(|range| range.start);
        let mut cluster_coverage: Vec<_> = clusters
            .iter()
            .map(|cluster| cluster.source.clone())
            .collect();
        cluster_coverage.sort_unstable_by_key(|range| range.start);
        let source_is_valid = if source.is_empty() {
            break_reason == LineBreakReason::End
                && advance == 0.0
                && runs.is_empty()
                && clusters.is_empty()
        } else {
            let mut covered = source.start;
            for range in &coverage {
                if range.start != covered || range.end > source.end {
                    return Err(PreparationError::invalid_output());
                }
                covered = range.end;
            }
            if covered != source.end {
                return Err(PreparationError::invalid_output());
            }
            covered = source.start;
            for range in &cluster_coverage {
                if range.start != covered || range.end > source.end {
                    return Err(PreparationError::invalid_output());
                }
                covered = range.end;
            }
            covered == source.end
        };
        if !source_is_valid {
            return Err(PreparationError::invalid_output());
        }
        let cluster_advance = clusters.iter().map(PreparedCluster::advance).sum::<f64>();
        let tolerance = f64::max(1.0, advance.abs()) * 1.0e-6;
        if (cluster_advance - advance).abs() > tolerance {
            return Err(PreparationError::invalid_output());
        }
        Ok(Self {
            source,
            break_reason,
            advance,
            baseline,
            height,
            content_ascent,
            content_descent,
            clusters,
            runs,
        })
    }

    /// Returns the paragraph-local source range, including a terminating control.
    #[must_use]
    pub fn source(&self) -> Range<u32> {
        self.source.clone()
    }

    /// Returns why the line ended.
    #[must_use]
    pub const fn break_reason(&self) -> LineBreakReason {
        self.break_reason
    }

    /// Returns the full inline advance, including trailing whitespace.
    #[must_use]
    pub const fn advance(&self) -> f64 {
        self.advance
    }

    /// Returns the baseline offset from the top of the line box.
    #[must_use]
    pub const fn baseline(&self) -> f64 {
        self.baseline
    }

    /// Returns the block-axis line-box extent.
    #[must_use]
    pub const fn height(&self) -> f64 {
        self.height
    }

    /// Returns the maximum font ascent contributing to the line.
    #[must_use]
    pub const fn content_ascent(&self) -> f64 {
        self.content_ascent
    }

    /// Returns the maximum font descent contributing to the line.
    #[must_use]
    pub const fn content_descent(&self) -> f64 {
        self.content_descent
    }

    /// Returns source clusters in line-local visual order.
    #[must_use]
    pub fn clusters(&self) -> &[PreparedCluster] {
        &self.clusters
    }

    /// Returns shaped runs in line-local visual order.
    #[must_use]
    pub fn runs(&self) -> &[PreparedRun] {
        &self.runs
    }
}

/// Validated owned formed lines for one paragraph.
#[derive(Clone, Debug)]
pub struct PreparedParagraph {
    paragraph: ParagraphId,
    text_len: u32,
    lines: Vec<PreparedLine>,
    movements: Vec<PreparedCursorMovement>,
}

impl PreparedParagraph {
    /// Validates and collects formed lines plus complete cursor transitions.
    pub fn try_new(
        paragraph: ParagraphId,
        text_len: u32,
        lines: impl IntoIterator<Item = PreparedLine>,
        movements: impl IntoIterator<Item = PreparedCursorMovement>,
    ) -> Result<Self, PreparationError> {
        let lines: Vec<_> = lines.into_iter().collect();
        let mut previous_end = 0;
        for line in &lines {
            if line.source.start != previous_end || line.source.end > text_len {
                return Err(PreparationError::invalid_output());
            }
            previous_end = line.source.end;
        }
        if previous_end != text_len {
            return Err(PreparationError::invalid_output());
        }
        let mut positions = Vec::new();
        for line in &lines {
            if line.clusters.is_empty() {
                let affinity = if line.source.start == 0 {
                    TextAffinity::Downstream
                } else {
                    TextAffinity::Upstream
                };
                push_unique_position(
                    &mut positions,
                    PreparedClusterSide::new(line.source.start, affinity),
                );
            } else {
                for cluster in &line.clusters {
                    push_unique_position(&mut positions, cluster.left);
                    push_unique_position(&mut positions, cluster.right);
                }
            }
        }
        if positions.is_empty() && text_len == 0 {
            positions.push(PreparedClusterSide::new(0, TextAffinity::Downstream));
        }
        let movements: Vec<_> = movements.into_iter().collect();
        let movement_positions: Vec<_> = movements
            .iter()
            .map(PreparedCursorMovement::position)
            .collect();
        let cluster_sources: Vec<_> = lines
            .iter()
            .flat_map(|line| line.clusters.iter().map(|cluster| cluster.source.clone()))
            .collect();
        if positions
            .iter()
            .any(|position| !movement_positions.contains(position))
            || movements.iter().enumerate().any(|(index, movement)| {
                movements[..index]
                    .iter()
                    .any(|previous| previous.position == movement.position)
                    || movement.position.offset > text_len
                    || usize::try_from(movement.caret.line).map_or(true, |line| {
                        if lines.is_empty() {
                            line != 0 || movement.caret.inline != 0.0
                        } else {
                            lines
                                .get(line)
                                .is_none_or(|line| movement.caret.inline > line.advance)
                        }
                    })
                    || movement.previous_visual.as_ref().is_some_and(|step| {
                        !movement_positions.contains(&step.target)
                            || !valid_step_source(step, &cluster_sources)
                    })
                    || movement.next_visual.as_ref().is_some_and(|step| {
                        !movement_positions.contains(&step.target)
                            || !valid_step_source(step, &cluster_sources)
                    })
                    || movement.previous_logical.as_ref().is_some_and(|step| {
                        !movement_positions.contains(&step.target)
                            || !valid_step_source(step, &cluster_sources)
                    })
                    || movement.next_logical.as_ref().is_some_and(|step| {
                        !movement_positions.contains(&step.target)
                            || !valid_step_source(step, &cluster_sources)
                    })
            })
        {
            return Err(PreparationError::invalid_output());
        }
        Ok(Self {
            paragraph,
            text_len,
            lines,
            movements,
        })
    }

    /// Returns the paragraph identity.
    #[must_use]
    pub const fn paragraph(&self) -> ParagraphId {
        self.paragraph
    }

    /// Returns the projected paragraph length in UTF-8 bytes.
    #[must_use]
    pub const fn text_len(&self) -> u32 {
        self.text_len
    }

    /// Returns the source-ordered formed lines.
    #[must_use]
    pub fn lines(&self) -> &[PreparedLine] {
        &self.lines
    }

    /// Returns complete paragraph-local cursor transitions.
    #[must_use]
    pub fn movements(&self) -> &[PreparedCursorMovement] {
        &self.movements
    }
}

fn valid_step_source(step: &PreparedCursorStep, cluster_sources: &[Range<u32>]) -> bool {
    step.source
        .as_ref()
        .is_none_or(|source| cluster_sources.contains(source))
}

fn push_unique_position(positions: &mut Vec<PreparedClusterSide>, position: PreparedClusterSide) {
    if !positions.contains(&position) {
        positions.push(position);
    }
}

/// One shaped run with a single font instance and bidi level.
#[derive(Clone, Debug)]
pub struct PreparedRun {
    source: Range<u32>,
    bidi_level: u8,
    script: [u8; 4],
    font: FontData,
    font_size: f32,
    synthesis: FontSynthesis,
    normalized_coords: Vec<i16>,
    unrendered_source: Vec<Range<u32>>,
    glyphs: Vec<PreparedGlyph>,
}

impl PreparedRun {
    /// Validates and owns one shaped run.
    ///
    /// A run may contain no glyphs when its source consists only of controls
    /// such as a mandatory line break. Its source range remains significant.
    pub fn try_new(
        source: Range<u32>,
        bidi_level: u8,
        script: [u8; 4],
        font: FontData,
        font_size: f32,
        synthesis: FontSynthesis,
        normalized_coords: impl IntoIterator<Item = i16>,
        unrendered_source: impl IntoIterator<Item = Range<u32>>,
        glyphs: impl IntoIterator<Item = PreparedGlyph>,
    ) -> Result<Self, PreparationError> {
        if source.start >= source.end || !font_size.is_finite() || font_size <= 0.0 {
            return Err(PreparationError::invalid_output());
        }
        let unrendered_source: Vec<_> = unrendered_source.into_iter().collect();
        let glyphs: Vec<_> = glyphs.into_iter().collect();
        if unrendered_source.iter().any(|range| {
            range.start < source.start
                || range.start >= range.end
                || range.end > source.end
                || glyphs
                    .iter()
                    .any(|glyph| glyph.source.start < range.end && glyph.source.end > range.start)
        }) || unrendered_source
            .windows(2)
            .any(|pair| pair[0].end >= pair[1].start)
            || glyphs
                .iter()
                .any(|glyph| glyph.source.start < source.start || glyph.source.end > source.end)
        {
            return Err(PreparationError::invalid_output());
        }
        Ok(Self {
            source,
            bidi_level,
            script,
            font,
            font_size,
            synthesis,
            normalized_coords: normalized_coords.into_iter().collect(),
            unrendered_source,
            glyphs,
        })
    }

    /// Returns the paragraph-local source range.
    #[must_use]
    pub fn source(&self) -> Range<u32> {
        self.source.clone()
    }

    /// Returns the resolved bidi level.
    #[must_use]
    pub const fn bidi_level(&self) -> u8 {
        self.bidi_level
    }

    /// Returns the ISO 15924 script tag.
    #[must_use]
    pub const fn script(&self) -> [u8; 4] {
        self.script
    }

    /// Returns the exact font resource and face index.
    #[must_use]
    pub const fn font(&self) -> &FontData {
        &self.font
    }

    /// Returns the font size used for shaping.
    #[must_use]
    pub const fn font_size(&self) -> f32 {
        self.font_size
    }

    /// Returns synthesis suggestions selected for this font instance.
    #[must_use]
    pub const fn synthesis(&self) -> &FontSynthesis {
        &self.synthesis
    }

    /// Returns normalized variation coordinates.
    #[must_use]
    pub fn normalized_coords(&self) -> &[i16] {
        &self.normalized_coords
    }

    /// Returns source-ordered ranges which intentionally produce no glyphs.
    ///
    /// Paragraph adapters use this for controls and format characters which
    /// participate in text semantics but not font shaping.
    #[must_use]
    pub fn unrendered_source(&self) -> &[Range<u32>] {
        &self.unrendered_source
    }

    /// Returns glyphs in backend-provided visual order.
    ///
    /// This is empty for a control-only shaped run, whose source remains
    /// explicit in [`Self::unrendered_source`].
    #[must_use]
    pub fn glyphs(&self) -> &[PreparedGlyph] {
        &self.glyphs
    }
}

/// One shaped glyph with paragraph source and paint coverage.
#[derive(Clone, Debug)]
pub struct PreparedGlyph {
    id: u32,
    source: Range<u32>,
    advance: Vec2,
    offset: Vec2,
    paint: GlyphPaintCoverage,
}

impl PreparedGlyph {
    /// Validates one shaped glyph.
    pub fn try_new(
        id: u32,
        source: Range<u32>,
        advance: Vec2,
        offset: Vec2,
        paint: GlyphPaintCoverage,
    ) -> Result<Self, PreparationError> {
        if source.start >= source.end
            || !advance.x.is_finite()
            || !advance.y.is_finite()
            || !offset.x.is_finite()
            || !offset.y.is_finite()
            || paint.segments.first().is_none_or(|segment| {
                segment.source.start != source.start
                    || paint
                        .segments
                        .last()
                        .is_none_or(|last| last.source.end != source.end)
            })
        {
            return Err(PreparationError::invalid_output());
        }
        Ok(Self {
            id,
            source,
            advance,
            offset,
            paint,
        })
    }

    /// Returns the backend glyph identifier.
    #[must_use]
    pub const fn id(&self) -> u32 {
        self.id
    }

    /// Returns the paragraph-local source range.
    #[must_use]
    pub fn source(&self) -> Range<u32> {
        self.source.clone()
    }

    /// Returns the shaped advance.
    #[must_use]
    pub const fn advance(&self) -> Vec2 {
        self.advance
    }

    /// Returns the shaped glyph offset.
    #[must_use]
    pub const fn offset(&self) -> Vec2 {
        self.offset
    }

    /// Returns source and local-clip paint coverage.
    #[must_use]
    pub const fn paint(&self) -> &GlyphPaintCoverage {
        &self.paint
    }
}

/// Complete source-ordered paint coverage for one glyph.
#[derive(Clone, Debug)]
pub struct GlyphPaintCoverage {
    segments: Vec<GlyphPaintSegment>,
}

impl GlyphPaintCoverage {
    /// Validates non-empty, contiguous, source-ordered segments.
    pub fn try_from_segments(
        segments: impl IntoIterator<Item = GlyphPaintSegment>,
    ) -> Result<Self, PreparationError> {
        let segments: Vec<_> = segments.into_iter().collect();
        if segments.is_empty()
            || segments
                .windows(2)
                .any(|pair| pair[0].source.end != pair[1].source.start)
        {
            return Err(PreparationError::unsupported_paint_coverage());
        }
        Ok(Self { segments })
    }

    /// Returns source-ordered coverage segments.
    #[must_use]
    pub fn segments(&self) -> &[GlyphPaintSegment] {
        &self.segments
    }
}

/// Paint and local clip for one source portion of a shaped glyph.
#[derive(Clone, Debug)]
pub struct GlyphPaintSegment {
    source: Range<u32>,
    slot: PaintSlot,
    local_clip: Rect,
}

impl GlyphPaintSegment {
    /// Creates a finite, non-empty coverage segment.
    pub fn new(
        source: Range<u32>,
        slot: PaintSlot,
        local_clip: Rect,
    ) -> Result<Self, PreparationError> {
        if source.start >= source.end
            || !local_clip.x0.is_finite()
            || !local_clip.y0.is_finite()
            || !local_clip.x1.is_finite()
            || !local_clip.y1.is_finite()
            || local_clip.width() < 0.0
            || local_clip.height() < 0.0
        {
            return Err(PreparationError::unsupported_paint_coverage());
        }
        Ok(Self {
            source,
            slot,
            local_clip,
        })
    }

    /// Returns the paragraph-local UTF-8 source range.
    #[must_use]
    pub fn source(&self) -> Range<u32> {
        self.source.clone()
    }

    /// Returns the segment paint slot.
    #[must_use]
    pub const fn slot(&self) -> PaintSlot {
        self.slot
    }

    /// Returns glyph-local clip geometry.
    #[must_use]
    pub const fn local_clip(&self) -> Rect {
        self.local_clip
    }
}

/// Stable category for adapter and prepared-output failures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum PreparationErrorKind {
    /// Required Unicode data or another capability is unavailable.
    MissingCapability,
    /// No usable font is available for the source.
    MissingFont,
    /// Faithful source-to-paint coverage cannot be represented.
    UnsupportedPaintCoverage,
    /// Adapter output violates the owned preparation contract.
    InvalidOutput,
    /// Work was cancelled before publication.
    Cancelled,
}

/// Concrete paragraph-preparation error.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct PreparationError {
    kind: PreparationErrorKind,
}

impl PreparationError {
    /// Creates an error for unavailable Unicode or shaping capabilities.
    #[must_use]
    pub const fn missing_capability() -> Self {
        Self {
            kind: PreparationErrorKind::MissingCapability,
        }
    }

    /// Creates an error for missing usable fonts.
    #[must_use]
    pub const fn missing_font() -> Self {
        Self {
            kind: PreparationErrorKind::MissingFont,
        }
    }

    /// Creates an error for paint coverage that cannot be represented faithfully.
    #[must_use]
    pub const fn unsupported_paint_coverage() -> Self {
        Self {
            kind: PreparationErrorKind::UnsupportedPaintCoverage,
        }
    }

    /// Creates an error for invalid backend output.
    #[must_use]
    pub const fn invalid_output() -> Self {
        Self {
            kind: PreparationErrorKind::InvalidOutput,
        }
    }

    /// Creates an error for cancelled work.
    #[must_use]
    pub const fn cancelled() -> Self {
        Self {
            kind: PreparationErrorKind::Cancelled,
        }
    }

    /// Returns the stable error category.
    #[must_use]
    pub const fn kind(&self) -> PreparationErrorKind {
        self.kind
    }
}

impl fmt::Display for PreparationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "paragraph preparation failed: {:?}", self.kind)
    }
}

impl core::error::Error for PreparationError {}

#[cfg(test)]
mod tests {
    use alloc::vec;

    use peniko::Blob;

    use super::{
        ClusterBoundary, ClusterWhitespace, FontSynthesis, GlyphPaintCoverage, GlyphPaintSegment,
        LineBreakReason, PreparationErrorKind, PreparedCaret, PreparedCluster, PreparedClusterSide,
        PreparedCursorMovement, PreparedCursorStep, PreparedGlyph, PreparedLine, PreparedParagraph,
        PreparedRun, TextAffinity,
    };
    use crate::{DocumentId, FontData, FontVariation, PaintSlot, ParagraphId, Rect, Tag, Vec2};

    #[test]
    fn synthesis_evidence_is_validated_canonical_and_last_wins() {
        let wght = Tag::new(b"wght");
        let wdth = Tag::new(b"wdth");
        let synthesis = FontSynthesis::try_new(
            [
                FontVariation::new(wght, 400.0),
                FontVariation::new(wdth, 75.0),
                FontVariation::new(wght, 700.0),
            ],
            true,
            Some(0.0),
        )
        .expect("finite synthesis evidence is valid");
        assert_eq!(
            synthesis.variations(),
            &[
                FontVariation::new(wdth, 75.0),
                FontVariation::new(wght, 700.0),
            ],
            "synthesis axes must be tag ordered with duplicate-last-wins semantics"
        );
        assert!(synthesis.embolden(), "embolden evidence must be retained");
        assert_eq!(
            synthesis.skew_degrees(),
            None,
            "zero skew must have the canonical absent representation"
        );
        let oblique =
            FontSynthesis::try_new([], false, Some(14.0)).expect("a finite non-zero skew is valid");
        let transform = oblique
            .skew_transform()
            .expect("a non-zero skew must produce a transform");
        assert!(
            transform.as_coeffs()[2].is_finite() && transform.as_coeffs()[2] > 0.0,
            "the shared skew transform must contain a finite horizontal shear"
        );
        assert!(
            FontSynthesis::try_new([FontVariation::new(wght, f32::NAN)], false, None).is_err(),
            "non-finite synthesis evidence must fail at the adapter boundary"
        );
    }

    #[test]
    fn prepared_paragraph_rejects_a_gap_between_lines() {
        let paragraph = ParagraphId {
            document: DocumentId::from_bytes(*b"adapter-test-001"),
            index: 0,
        };
        let first = line(0..1);
        let second = line(2..3);
        let error = PreparedParagraph::try_new(paragraph, 3, [first, second], [])
            .expect_err("source gaps must be rejected at the adapter boundary");
        assert_eq!(
            error.kind(),
            PreparationErrorKind::InvalidOutput,
            "a source gap is invalid adapter output"
        );
    }

    #[test]
    fn prepared_paragraph_rejects_incomplete_cursor_facts() {
        let paragraph = ParagraphId {
            document: DocumentId::from_bytes(*b"adapter-test-002"),
            index: 0,
        };
        let start = PreparedClusterSide::new(0, TextAffinity::Downstream);
        let end = PreparedClusterSide::new(1, TextAffinity::Upstream);
        let unknown = PreparedClusterSide::new(0, TextAffinity::Upstream);
        let caret = PreparedCaret::try_new(0, 0.0).expect("test caret is valid");
        let start_movement = PreparedCursorMovement::new(
            start,
            caret,
            None,
            Some(PreparedCursorStep::new(unknown, Some(0..1))),
            None,
            Some(PreparedCursorStep::new(end, Some(0..1))),
        );
        let end_movement = PreparedCursorMovement::new(
            end,
            caret,
            Some(PreparedCursorStep::new(start, Some(0..1))),
            None,
            Some(PreparedCursorStep::new(start, Some(0..1))),
            None,
        );
        let error =
            PreparedParagraph::try_new(paragraph, 1, [line(0..1)], [start_movement, end_movement])
                .expect_err("every cursor target must have its own movement record");
        assert_eq!(error.kind(), PreparationErrorKind::InvalidOutput);
    }

    #[test]
    fn prepared_paragraph_rejects_a_caret_on_an_unknown_line() {
        let paragraph = ParagraphId {
            document: DocumentId::from_bytes(*b"adapter-test-003"),
            index: 0,
        };
        let start = PreparedClusterSide::new(0, TextAffinity::Downstream);
        let end = PreparedClusterSide::new(1, TextAffinity::Upstream);
        let invalid_caret = PreparedCaret::try_new(1, 0.0).expect("coordinates are finite");
        let start_movement = PreparedCursorMovement::new(
            start,
            invalid_caret,
            None,
            Some(PreparedCursorStep::new(end, Some(0..1))),
            None,
            Some(PreparedCursorStep::new(end, Some(0..1))),
        );
        let end_movement = PreparedCursorMovement::new(
            end,
            invalid_caret,
            Some(PreparedCursorStep::new(start, Some(0..1))),
            None,
            Some(PreparedCursorStep::new(start, Some(0..1))),
            None,
        );
        let error =
            PreparedParagraph::try_new(paragraph, 1, [line(0..1)], [start_movement, end_movement])
                .expect_err("caret line identities must resolve inside the paragraph");
        assert_eq!(error.kind(), PreparationErrorKind::InvalidOutput);
    }

    #[test]
    fn prepared_paragraph_rejects_a_step_source_that_is_not_a_cluster() {
        let paragraph = ParagraphId {
            document: DocumentId::from_bytes(*b"adapter-test-004"),
            index: 0,
        };
        let start = PreparedClusterSide::new(0, TextAffinity::Downstream);
        let end = PreparedClusterSide::new(2, TextAffinity::Upstream);
        let start_movement = PreparedCursorMovement::new(
            start,
            PreparedCaret::try_new(0, 0.0).expect("test caret is valid"),
            None,
            Some(PreparedCursorStep::new(end, Some(0..1))),
            None,
            Some(PreparedCursorStep::new(end, Some(0..1))),
        );
        let end_movement = PreparedCursorMovement::new(
            end,
            PreparedCaret::try_new(0, 1.0).expect("test caret is valid"),
            Some(PreparedCursorStep::new(start, Some(0..1))),
            None,
            Some(PreparedCursorStep::new(start, Some(0..1))),
            None,
        );
        let error =
            PreparedParagraph::try_new(paragraph, 2, [line(0..2)], [start_movement, end_movement])
                .expect_err("a cursor step must cross one actual prepared cluster");
        assert_eq!(error.kind(), PreparationErrorKind::InvalidOutput);
    }

    #[test]
    fn prepared_line_rejects_missing_run_source() {
        let error = PreparedLine::try_new(
            0..2,
            LineBreakReason::End,
            1.0,
            0.8,
            1.0,
            0.8,
            0.2,
            [cluster(0..2, 1.0)],
            [run(0..1)],
        )
        .expect_err("visual runs must cover the complete non-empty line source");
        assert_eq!(error.kind(), PreparationErrorKind::InvalidOutput);
    }

    #[test]
    fn prepared_line_rejects_missing_cluster_source() {
        let error = PreparedLine::try_new(
            0..2,
            LineBreakReason::End,
            1.0,
            0.8,
            1.0,
            0.8,
            0.2,
            [cluster(0..1, 1.0)],
            [run(0..2)],
        )
        .expect_err("visual clusters must cover the complete line source");
        assert_eq!(error.kind(), PreparationErrorKind::InvalidOutput);
    }

    #[test]
    fn prepared_cluster_rejects_a_side_outside_its_source() {
        let error = PreparedCluster::try_new(
            1..2,
            1.0,
            0,
            ClusterBoundary::None,
            ClusterWhitespace::None,
            PreparedClusterSide::new(0, TextAffinity::Downstream),
            PreparedClusterSide::new(2, TextAffinity::Upstream),
        )
        .expect_err("cluster sides must name one of the cluster boundaries");
        assert_eq!(error.kind(), PreparationErrorKind::InvalidOutput);
    }

    fn line(source: core::ops::Range<u32>) -> PreparedLine {
        PreparedLine::try_new(
            source.clone(),
            LineBreakReason::End,
            1.0,
            0.8,
            1.0,
            0.8,
            0.2,
            [cluster(source.clone(), 1.0)],
            [run(source)],
        )
        .expect("test line is valid")
    }

    fn cluster(source: core::ops::Range<u32>, advance: f64) -> PreparedCluster {
        PreparedCluster::try_new(
            source.clone(),
            advance,
            0,
            ClusterBoundary::None,
            ClusterWhitespace::None,
            PreparedClusterSide::new(source.start, TextAffinity::Downstream),
            PreparedClusterSide::new(source.end, TextAffinity::Upstream),
        )
        .expect("test cluster is valid")
    }

    #[test]
    fn prepared_run_accepts_control_only_source_without_a_phantom_glyph() {
        let run = PreparedRun::try_new(
            0..1,
            0,
            *b"Zyyy",
            FontData::new(Blob::from(vec![0_u8]), 0),
            16.,
            FontSynthesis::default(),
            [],
            core::iter::once(0..1),
            [],
        )
        .expect("control-only source does not require a fabricated glyph");
        assert!(
            run.glyphs().is_empty(),
            "control-only runs must retain an honest empty glyph sequence"
        );
    }

    fn run(source: core::ops::Range<u32>) -> PreparedRun {
        let coverage = GlyphPaintCoverage::try_from_segments([GlyphPaintSegment::new(
            source.clone(),
            PaintSlot::new(0),
            Rect::new(0., 0., 1., 1.),
        )
        .expect("test coverage is finite")])
        .expect("test coverage is contiguous");
        let glyph =
            PreparedGlyph::try_new(1, source.clone(), Vec2::new(1., 0.), Vec2::ZERO, coverage)
                .expect("test glyph is valid");
        PreparedRun::try_new(
            source,
            0,
            *b"Latn",
            FontData::new(Blob::from(vec![0_u8]), 0),
            16.,
            FontSynthesis::default(),
            [],
            [],
            [glyph],
        )
        .expect("test run is internally valid")
    }
}
