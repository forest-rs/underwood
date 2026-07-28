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

    /// Configures the deterministic byte budget for reusable adapter facts.
    ///
    /// The budget is independent from published scene residency. A zero
    /// budget permits the adapter to discard analysis, shaping, and line facts
    /// after a successful output has been lowered into a scene.
    fn set_retained_facts_budget(&mut self, _bytes: usize) {}

    /// Commits one successfully consumed preparation to adapter retention.
    ///
    /// [`LayoutEngine`](crate::LayoutEngine) calls this only after validating
    /// and lowering the returned output. Adapters use the call to update
    /// recency and enforce the configured retained-fact budget.
    fn commit_preparation(&mut self, _preparation: ParagraphPreparationId) {}

    /// Drops reusable adapter facts without invalidating published scenes.
    fn trim_retained_facts(&mut self) {}

    /// Returns retained adapter-fact accounting, when observable.
    #[must_use]
    fn retained_facts(&self) -> Option<ParagraphFormationCacheDiagnostics> {
        None
    }
}

/// Deterministic retained-fact accounting reported by a paragraph adapter.
///
/// The byte charge covers adapter-owned reusable analysis, shaping, line, and
/// lowered-output storage. Shared font blobs and caller-owned published scenes
/// are excluded.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ParagraphFormationCacheDiagnostics {
    /// Configured deterministic byte budget.
    pub budget_bytes: usize,
    /// Retained preparation identities.
    pub entries: usize,
    /// Current deterministic adapter-fact byte charge.
    pub resident_bytes: usize,
    /// Current reusable scratch byte charge.
    pub scratch_bytes: usize,
    /// Formation calls that found reusable adapter facts.
    pub hits: usize,
    /// Formation calls that began without reusable facts.
    pub misses: usize,
    /// Entries removed to enforce the byte budget.
    pub evictions: usize,
    /// Entries removed by explicit lifecycle operations.
    pub releases: usize,
}

/// Retained adapter state used to produce one formation output.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ParagraphFormationReuse {
    /// No reusable adapter facts were available.
    #[default]
    Cold,
    /// Reusable analysis, shaping, or formed-line facts were available.
    RetainedFacts,
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
}

/// Facets proven changed since one retained backend preparation.
///
/// This record is constructed only by `LayoutEngine` after comparing its
/// validated retained inputs. A backend may therefore use unchanged facets
/// without rebuilding or deep-comparing them. A missing backend cache entry
/// still requires an ordinary cold preparation regardless of these values.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ParagraphFormationChange {
    /// Projected text or Unicode-analysis inputs changed.
    pub analysis: bool,
    /// Font-selection or shaping-style inputs changed.
    pub font_selection: bool,
    /// Spacing changed ligature-formation policy.
    pub ligature_policy: bool,
    /// Inline-flow run partitioning or values changed.
    pub inline_flow_projection: bool,
    /// Authored letter or word spacing changed.
    pub spacing: bool,
    /// Line-height inputs changed.
    pub line_metrics: bool,
    /// Wrapping or overflow policy changed.
    pub break_policy: bool,
    /// Width, intrinsic, empty-line, or region constraints changed.
    pub constraints: bool,
}

impl ParagraphFormationChange {
    pub(crate) const fn all() -> Self {
        Self {
            analysis: true,
            font_selection: true,
            ligature_policy: true,
            inline_flow_projection: true,
            spacing: true,
            line_metrics: true,
            break_policy: true,
            constraints: true,
        }
    }
}

/// Borrowed, validated projection of one semantic paragraph.
///
/// [`LayoutEngine`](crate::LayoutEngine) constructs this record after proving
/// that each run table exactly covers `text` with ordered UTF-8 boundaries and
/// valid style-table indices. Paragraph backends may rely on those invariants
/// instead of repeating source validation on every formation call.
#[non_exhaustive]
#[derive(Clone, Copy, Debug)]
pub struct ParagraphInput<'a> {
    /// Exact retained-cache identity for this preparation lane.
    pub preparation: ParagraphPreparationId,
    /// Facets proven changed since this preparation identity last ran.
    pub change: ParagraphFormationChange,
    /// Normalized prepared-scene capabilities required downstream.
    pub features: SceneFeatures,
    /// Paragraph identity.
    pub paragraph: ParagraphId,
    /// Complete computed paragraph-level values.
    pub paragraph_style: ParagraphStyle,
    /// Complete projected UTF-8 paragraph.
    pub text: &'a str,
    /// Paragraph-local table of unique Unicode-analysis values.
    pub analysis_styles: &'a [AnalysisStyle],
    /// Source-ordered Unicode-analysis metadata.
    pub analysis_runs: &'a [AnalysisRun],
    /// Paragraph-local table of unique shaping values.
    pub shaping_styles: &'a [ShapingStyle],
    /// Source-ordered shaping metadata.
    pub shaping_runs: &'a [ShapingRun],
    /// Paragraph-local table of unique inline-flow values.
    pub inline_flow_styles: &'a [InlineFlowStyle],
    /// Source-ordered inline-flow metadata.
    pub inline_flow_runs: &'a [InlineFlowRun],
}

impl<'a> ParagraphInput<'a> {
    // Keep construction crate-private: paragraph backends rely on LayoutEngine
    // having proved the run-table invariants documented on this type.
    pub(crate) const fn new(
        preparation: ParagraphPreparationId,
        change: ParagraphFormationChange,
        features: SceneFeatures,
        paragraph: ParagraphId,
        paragraph_style: ParagraphStyle,
        text: &'a str,
        analysis_styles: &'a [AnalysisStyle],
        analysis_runs: &'a [AnalysisRun],
        shaping_styles: &'a [ShapingStyle],
        shaping_runs: &'a [ShapingRun],
        inline_flow_styles: &'a [InlineFlowStyle],
        inline_flow_runs: &'a [InlineFlowRun],
    ) -> Self {
        Self {
            preparation,
            change,
            features,
            paragraph,
            paragraph_style,
            text,
            analysis_styles,
            analysis_runs,
            shaping_styles,
            shaping_runs,
            inline_flow_styles,
            inline_flow_runs,
        }
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

/// Owned paragraph data and exact work performed to produce it.
#[derive(Clone, Debug)]
pub struct ParagraphFormationOutput {
    paragraph: PreparedParagraph,
    work: FormationWork,
    region_transcript: Option<RegionTranscript>,
    reuse: ParagraphFormationReuse,
}

impl ParagraphFormationOutput {
    /// Pairs validated prepared data with actual backend work.
    #[must_use]
    pub const fn new(paragraph: PreparedParagraph, work: FormationWork) -> Self {
        Self {
            paragraph,
            work,
            region_transcript: None,
            reuse: ParagraphFormationReuse::Cold,
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
            reuse: ParagraphFormationReuse::Cold,
        }
    }

    /// Returns a copy reporting which retained adapter state produced it.
    #[must_use]
    pub const fn with_reuse(mut self, reuse: ParagraphFormationReuse) -> Self {
        self.reuse = reuse;
        self
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

    /// Returns the retained adapter state used by this formation call.
    #[must_use]
    pub const fn reuse(&self) -> ParagraphFormationReuse {
        self.reuse
    }
}

/// Actual adapter work performed during one preparation call.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FormationWork {
    /// Whether Unicode analysis ran.
    pub analyzed: bool,
    /// Whether shaping itemization ran.
    pub itemized: bool,
    /// Clusters for which the adapter selected a font.
    pub selected_clusters: u32,
    /// Canonical paragraph runs shaped.
    pub shaped_runs: u32,
    /// Canonical paragraph glyphs shaped.
    pub shaped_glyphs: u32,
    /// Lines formed for new constraints or flow values.
    pub formed_lines: u32,
    /// Work performed while shaping and fitting line candidates.
    pub line_shaping: LineShapingWork,
}

/// Exact work performed while forming and shaping line candidates.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LineShapingWork {
    /// Line-final shaping attempts, including rejected candidates.
    pub attempts: u32,
    /// Clusters mapped back to their retained canonical font.
    pub resolved_clusters: u32,
    /// Runs produced across line-final shaping attempts.
    pub shaped_runs: u32,
    /// Glyphs produced across line-final shaping attempts.
    pub shaped_glyphs: u32,
    /// Proposed line candidates, including retries.
    pub candidates: u32,
    /// Candidates rejected by line-final fit checks.
    pub rejected_candidates: u32,
}
