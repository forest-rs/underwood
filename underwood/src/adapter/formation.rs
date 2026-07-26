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

    /// Declares eligibility for exact cross-identity prepared-fact reuse.
    ///
    /// `None` is the safe default and disables sharing. `Some(epoch)` promises
    /// that prepared facts depend only on non-identity formation inputs,
    /// constraints, and the returned epoch. The epoch must change before any
    /// hidden font, text-data, or backend resource can change prepared output.
    #[must_use]
    fn shared_preparation_epoch(&self) -> Option<u64> {
        None
    }

    /// Releases retained preparation for one paragraph identity.
    ///
    /// Stateless implementations may keep the default no-op behavior.
    fn release(&mut self, _preparation: ParagraphPreparationId) {}

    /// Releases all retained paragraph preparation.
    ///
    /// Stateless implementations may keep the default no-op behavior.
    fn clear(&mut self) {}

    /// Returns the number of retained backend paragraph entries, when observable.
    #[must_use]
    fn retained_entries(&self) -> Option<usize> {
        None
    }
}

/// Opaque identity of one retained paragraph-preparation lane.
///
/// A committed paragraph and a transient composition over that paragraph have
/// distinct identities even though they share one semantic [`ParagraphId`].
/// [`LayoutEngine`](crate::LayoutEngine) creates and releases these identities;
/// backends use them only as exact retained-cache keys.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ParagraphPreparationId {
    paragraph: ParagraphId,
    lane: u8,
}

impl ParagraphPreparationId {
    pub(crate) const fn new(paragraph: ParagraphId, lane: u8) -> Self {
        Self { paragraph, lane }
    }

    /// Returns the semantic paragraph whose projected preparation this key owns.
    #[must_use]
    pub const fn paragraph(self) -> ParagraphId {
        self.paragraph
    }
}

/// Facets proven changed since one retained backend preparation.
///
/// This record is constructed only by `LayoutEngine` after comparing its
/// validated retained inputs. A backend may therefore use unchanged facets
/// without rebuilding or deep-comparing them. A missing backend cache entry
/// still requires an ordinary cold preparation regardless of these values.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ParagraphFormationChange {
    analysis: bool,
    font_selection: bool,
    ligature_policy: bool,
    inline_flow_projection: bool,
    spacing: bool,
    line_metrics: bool,
    break_policy: bool,
    constraints: bool,
    paint: bool,
}

impl ParagraphFormationChange {
    #[expect(
        clippy::too_many_arguments,
        reason = "the fields are the complete portable formation invalidation contract"
    )]
    pub(crate) const fn new(
        analysis: bool,
        font_selection: bool,
        ligature_policy: bool,
        inline_flow_projection: bool,
        spacing: bool,
        line_metrics: bool,
        break_policy: bool,
        constraints: bool,
        paint: bool,
    ) -> Self {
        Self {
            analysis,
            font_selection,
            ligature_policy,
            inline_flow_projection,
            spacing,
            line_metrics,
            break_policy,
            constraints,
            paint,
        }
    }

    pub(crate) const fn all() -> Self {
        Self::new(true, true, true, true, true, true, true, true, true)
    }

    /// Returns whether projected text or Unicode-analysis inputs changed.
    #[must_use]
    pub const fn analysis_changed(self) -> bool {
        self.analysis
    }

    /// Returns whether font-selection or shaping-style inputs changed.
    #[must_use]
    pub const fn font_selection_changed(self) -> bool {
        self.font_selection
    }

    /// Returns whether spacing changed ligature-formation policy.
    #[must_use]
    pub const fn ligature_policy_changed(self) -> bool {
        self.ligature_policy
    }

    /// Returns whether inline-flow run partitioning or values changed.
    #[must_use]
    pub const fn inline_flow_projection_changed(self) -> bool {
        self.inline_flow_projection
    }

    /// Returns whether authored letter or word spacing changed.
    #[must_use]
    pub const fn spacing_changed(self) -> bool {
        self.spacing
    }

    /// Returns whether line-height inputs changed.
    #[must_use]
    pub const fn line_metrics_changed(self) -> bool {
        self.line_metrics
    }

    /// Returns whether wrapping or overflow policy changed.
    #[must_use]
    pub const fn break_policy_changed(self) -> bool {
        self.break_policy
    }

    /// Returns whether width, intrinsic, empty-line, or region constraints changed.
    #[must_use]
    pub const fn constraints_changed(self) -> bool {
        self.constraints
    }

    /// Returns whether projected paint-slot coverage changed.
    #[must_use]
    pub const fn paint_changed(self) -> bool {
        self.paint
    }

    /// Returns whether the exact validated prepared output remains reusable.
    #[must_use]
    pub const fn output_retained(self) -> bool {
        !self.analysis
            && !self.font_selection
            && !self.ligature_policy
            && !self.inline_flow_projection
            && !self.spacing
            && !self.line_metrics
            && !self.break_policy
            && !self.constraints
            && !self.paint
    }
}

/// Borrowed projection of one semantic paragraph.
#[derive(Clone, Copy, Debug)]
pub struct ParagraphInput<'a> {
    preparation: ParagraphPreparationId,
    reuse_preparation: Option<ParagraphPreparationId>,
    change: ParagraphFormationChange,
    paragraph: ParagraphId,
    paragraph_style: ParagraphStyle,
    text: &'a str,
    analysis_styles: &'a [AnalysisStyle],
    analysis_runs: &'a [AnalysisRun],
    shaping_styles: &'a [ShapingStyle],
    shaping_runs: &'a [ShapingRun],
    inline_flow_styles: &'a [InlineFlowStyle],
    inline_flow_runs: &'a [InlineFlowRun],
    paint_runs: &'a [PaintRun],
}

impl<'a> ParagraphInput<'a> {
    pub(crate) const fn new(
        preparation: ParagraphPreparationId,
        reuse_preparation: Option<ParagraphPreparationId>,
        change: ParagraphFormationChange,
        paragraph: ParagraphId,
        paragraph_style: ParagraphStyle,
        text: &'a str,
        analysis_styles: &'a [AnalysisStyle],
        analysis_runs: &'a [AnalysisRun],
        shaping_styles: &'a [ShapingStyle],
        shaping_runs: &'a [ShapingRun],
        inline_flow_styles: &'a [InlineFlowStyle],
        inline_flow_runs: &'a [InlineFlowRun],
        paint_runs: &'a [PaintRun],
    ) -> Self {
        Self {
            preparation,
            reuse_preparation,
            change,
            paragraph,
            paragraph_style,
            text,
            analysis_styles,
            analysis_runs,
            shaping_styles,
            shaping_runs,
            inline_flow_styles,
            inline_flow_runs,
            paint_runs,
        }
    }

    /// Returns the exact retained-cache identity for this preparation lane.
    #[must_use]
    pub const fn preparation(&self) -> ParagraphPreparationId {
        self.preparation
    }

    /// Returns another lane whose exact prepared output may be rebound here.
    ///
    /// `LayoutEngine` supplies this only after proving every backend-visible
    /// formation and paint facet equal. Backends may ignore the opportunity.
    #[must_use]
    pub const fn reusable_preparation(&self) -> Option<ParagraphPreparationId> {
        self.reuse_preparation
    }

    /// Returns facets proven changed since this preparation identity last ran.
    #[must_use]
    pub const fn change(&self) -> ParagraphFormationChange {
        self.change
    }

    /// Returns the paragraph-local table of unique Unicode-analysis values.
    #[must_use]
    pub const fn analysis_styles(&self) -> &[AnalysisStyle] {
        self.analysis_styles
    }

    /// Returns source-ordered Unicode-analysis metadata covering the paragraph.
    #[must_use]
    pub const fn analysis_runs(&self) -> &[AnalysisRun] {
        self.analysis_runs
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

    /// Returns the complete computed paragraph-level values.
    #[must_use]
    pub const fn paragraph_style(&self) -> ParagraphStyle {
        self.paragraph_style
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
#[derive(Clone, Debug, PartialEq)]
pub struct ParagraphConstraints {
    text: TextConstraint,
    empty_line_height: f64,
    region_flow: Option<RegionFlow>,
    region_cursor: Option<RegionCursor>,
}

impl ParagraphConstraints {
    pub(crate) const fn new(text: TextConstraint, empty_line_height: f64) -> Self {
        Self {
            text,
            empty_line_height,
            region_flow: None,
            region_cursor: None,
        }
    }

    pub(crate) fn in_regions(
        text: TextConstraint,
        empty_line_height: f64,
        region_flow: RegionFlow,
        region_cursor: RegionCursor,
    ) -> Self {
        Self {
            text,
            empty_line_height,
            region_flow: Some(region_flow),
            region_cursor: Some(region_cursor),
        }
    }

    /// Returns the requested intrinsic or constrained formation mode.
    #[must_use]
    pub const fn text(&self) -> TextConstraint {
        self.text
    }

    /// Returns the deterministic line-box height for a paragraph with no text.
    #[must_use]
    pub const fn empty_line_height(&self) -> f64 {
        self.empty_line_height
    }

    /// Returns immutable region policy when exact line slots replace one width.
    #[must_use]
    pub const fn region_flow(&self) -> Option<&RegionFlow> {
        self.region_flow.as_ref()
    }

    /// Returns the cursor from which this paragraph must resume region flow.
    #[must_use]
    pub const fn region_cursor(&self) -> Option<RegionCursor> {
        self.region_cursor
    }
}

/// Dense paragraph-local identity for one entry in the analysis-style table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AnalysisStyleId(u16);

impl AnalysisStyleId {
    pub(crate) const fn new(index: u16) -> Self {
        Self(index)
    }

    /// Returns the paragraph-local table index.
    #[must_use]
    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

/// Complete Unicode-analysis values over a paragraph-local UTF-8 byte range.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnalysisRun {
    bytes: Range<u32>,
    style: AnalysisStyleId,
}

impl AnalysisRun {
    pub(crate) const fn new(bytes: Range<u32>, style: AnalysisStyleId) -> Self {
        Self { bytes, style }
    }

    /// Returns the paragraph-local UTF-8 byte range.
    #[must_use]
    pub fn bytes(&self) -> Range<u32> {
        self.bytes.clone()
    }

    /// Returns the paragraph-local analysis-style identity for this range.
    #[must_use]
    pub const fn style(&self) -> AnalysisStyleId {
        self.style
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
    region_transcript: Option<RegionTranscript>,
}

impl ParagraphFormationOutput {
    /// Pairs validated prepared data with actual backend work.
    #[must_use]
    pub const fn new(paragraph: PreparedParagraph, work: FormationWork) -> Self {
        Self {
            paragraph,
            work,
            region_transcript: None,
        }
    }

    /// Pairs prepared data with a replayable exact-region transcript.
    #[must_use]
    pub const fn in_regions(
        paragraph: PreparedParagraph,
        work: FormationWork,
        region_transcript: RegionTranscript,
    ) -> Self {
        Self {
            paragraph,
            work,
            region_transcript: Some(region_transcript),
        }
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

    /// Returns exact region attempts and cursor transitions, when requested.
    #[must_use]
    pub const fn region_transcript(&self) -> Option<&RegionTranscript> {
        self.region_transcript.as_ref()
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
    line_shaping: LineShapingWork,
}

/// Exact work performed while forming and shaping line candidates.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LineShapingWork {
    attempts: u32,
    resolved_clusters: u32,
    shaped_runs: u32,
    shaped_glyphs: u32,
    candidates: u32,
    rejected_candidates: u32,
    checkpoint_restores: u32,
}

impl LineShapingWork {
    /// Creates line-final work from backend observations.
    #[must_use]
    pub const fn new(
        attempts: u32,
        resolved_clusters: u32,
        shaped_runs: u32,
        shaped_glyphs: u32,
    ) -> Self {
        Self {
            attempts,
            resolved_clusters,
            shaped_runs,
            shaped_glyphs,
            candidates: 0,
            rejected_candidates: 0,
            checkpoint_restores: 0,
        }
    }

    /// Adds state-machine observations for line formation.
    ///
    /// A proposed candidate can use retained canonical shaping, so `candidates`
    /// is intentionally independent of [`Self::attempts`].
    #[must_use]
    pub const fn with_formation(
        mut self,
        candidates: u32,
        rejected_candidates: u32,
        checkpoint_restores: u32,
    ) -> Self {
        self.candidates = candidates;
        self.rejected_candidates = rejected_candidates;
        self.checkpoint_restores = checkpoint_restores;
        self
    }

    /// Returns the number of line-final shaping attempts, including rejected
    /// candidates whose shaped advance did not fit.
    #[must_use]
    pub const fn attempts(self) -> u32 {
        self.attempts
    }

    /// Returns clusters mapped back to their retained canonical font.
    #[must_use]
    pub const fn resolved_clusters(self) -> u32 {
        self.resolved_clusters
    }

    /// Returns shaped runs produced across all line-final shaping attempts.
    #[must_use]
    pub const fn shaped_runs(self) -> u32 {
        self.shaped_runs
    }

    /// Returns glyphs produced across all line-final shaping attempts.
    #[must_use]
    pub const fn shaped_glyphs(self) -> u32 {
        self.shaped_glyphs
    }

    /// Returns the number of proposed line candidates, including retries.
    #[must_use]
    pub const fn candidates(self) -> u32 {
        self.candidates
    }

    /// Returns candidates rejected after line-final width or height checks.
    #[must_use]
    pub const fn rejected_candidates(self) -> u32 {
        self.rejected_candidates
    }

    /// Returns candidates committed to the current line sequence.
    #[must_use]
    pub const fn accepted_candidates(self) -> u32 {
        self.candidates.saturating_sub(self.rejected_candidates)
    }

    /// Returns restorations of traversal and provisional line output.
    #[must_use]
    pub const fn checkpoint_restores(self) -> u32 {
        self.checkpoint_restores
    }
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
        line_shaping: LineShapingWork,
    ) -> Self {
        Self {
            analyzed,
            itemized,
            selected_clusters,
            shaped_runs,
            shaped_glyphs,
            formed_lines,
            line_shaping,
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

    /// Returns the number of line-final shaping attempts, including rejected
    /// candidates whose shaped advance did not fit.
    #[must_use]
    pub const fn line_reshapes(self) -> u32 {
        self.line_shaping.attempts()
    }

    /// Returns clusters mapped back to their retained canonical font.
    #[must_use]
    pub const fn line_resolved_clusters(self) -> u32 {
        self.line_shaping.resolved_clusters()
    }

    /// Returns shaped runs produced across all line-final shaping attempts.
    #[must_use]
    pub const fn line_shaped_runs(self) -> u32 {
        self.line_shaping.shaped_runs()
    }

    /// Returns glyphs produced across all line-final shaping attempts.
    #[must_use]
    pub const fn line_shaped_glyphs(self) -> u32 {
        self.line_shaping.shaped_glyphs()
    }

    /// Returns proposed line candidates, including retry candidates.
    #[must_use]
    pub const fn line_candidates(self) -> u32 {
        self.line_shaping.candidates()
    }

    /// Returns line candidates rejected by line-final fit checks.
    #[must_use]
    pub const fn rejected_line_candidates(self) -> u32 {
        self.line_shaping.rejected_candidates()
    }

    /// Returns line candidates committed to the current line sequence.
    #[must_use]
    pub const fn accepted_line_candidates(self) -> u32 {
        self.line_shaping.accepted_candidates()
    }

    /// Returns restorations of line traversal and provisional output.
    #[must_use]
    pub const fn line_checkpoint_restores(self) -> u32 {
        self.line_shaping.checkpoint_restores()
    }

    /// Returns the complete line-final shaping work record.
    #[must_use]
    pub const fn line_shaping(self) -> LineShapingWork {
        self.line_shaping
    }
}
