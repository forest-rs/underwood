// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Paragraph formation inputs, outputs, work, and backend contract.
//!
//! This module owns the portable formation call boundary; it explicitly does
//! not own prepared-record validation or scene lowering.

use super::*;

/// Forms portable lines for one paragraph through a retained text backend.
pub trait ParagraphFormation {
    /// Produces validated, owned formed lines for `input` and `constraints`.
    fn form(
        &mut self,
        input: ParagraphInput<'_>,
        constraints: ParagraphConstraints,
    ) -> Result<ParagraphFormationOutput, PreparationError>;

    /// Releases retained physics for one paragraph identity.
    ///
    /// Stateless implementations may keep the default no-op behavior.
    fn release(&mut self, _paragraph: ParagraphId) {}

    /// Releases all retained paragraph physics.
    ///
    /// Stateless implementations may keep the default no-op behavior.
    fn clear(&mut self) {}

    /// Returns the number of retained backend paragraph entries, when observable.
    #[must_use]
    fn retained_entries(&self) -> Option<usize> {
        None
    }
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
    text: TextConstraint,
}

impl ParagraphConstraints {
    pub(crate) const fn new(text: TextConstraint) -> Self {
        Self { text }
    }

    /// Returns the requested intrinsic or constrained formation mode.
    #[must_use]
    pub const fn text(self) -> TextConstraint {
        self.text
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
