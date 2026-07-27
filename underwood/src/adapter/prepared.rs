// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Validated prepared fonts, glyphs, runs, lines, and paragraphs.
//!
//! This module owns renderer-neutral shaped records; it explicitly does not
//! own paragraph formation policy or scene-space materialization.

use super::*;

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
/// One source-complete line with backend-derived metrics and visual runs.
#[derive(Clone, Debug)]
pub struct PreparedLine {
    slot: Option<crate::LineSlot>,
    source: Range<u32>,
    break_reason: LineBreakReason,
    advance: f64,
    trailing_whitespace_start: u32,
    trailing_whitespace_advance: f64,
    baseline: f64,
    height: f64,
    content_ascent: f64,
    content_descent: f64,
    interaction: Arc<PreparedLineInteraction>,
    runs: Vec<PreparedRun>,
}

#[derive(Clone, Debug)]
struct PreparedLineInteraction {
    slices: Vec<PreparedInteractionSlice>,
    units: Vec<PreparedInteractionUnit>,
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
        slices: impl IntoIterator<Item = PreparedInteractionSlice>,
        units: impl IntoIterator<Item = PreparedInteractionUnit>,
        runs: impl IntoIterator<Item = PreparedRun>,
    ) -> Result<Self, PreparationError> {
        Self::try_new_in_slot(
            None,
            source,
            break_reason,
            advance,
            baseline,
            height,
            content_ascent,
            content_descent,
            slices,
            units,
            runs,
        )
    }

    /// Validates and owns one formed line accepted into an exact region slot.
    #[expect(
        clippy::too_many_arguments,
        reason = "mirrors complete portable line data"
    )]
    pub fn try_new_in_slot(
        slot: Option<crate::LineSlot>,
        source: Range<u32>,
        break_reason: LineBreakReason,
        advance: f64,
        baseline: f64,
        height: f64,
        content_ascent: f64,
        content_descent: f64,
        slices: impl IntoIterator<Item = PreparedInteractionSlice>,
        units: impl IntoIterator<Item = PreparedInteractionUnit>,
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
            || slot.is_some_and(|slot| height > slot.block_size())
        {
            return Err(PreparationError::invalid_output());
        }
        let slices: Vec<_> = slices.into_iter().collect();
        let units: Vec<_> = units.into_iter().collect();
        let mut next_slice = 0;
        for unit in &units {
            let range = unit.slice_range();
            if range.start != next_slice {
                return Err(PreparationError::invalid_output());
            }
            unit.validate_slices(&slices)?;
            next_slice = range.end;
        }
        if next_slice != slices.len() {
            return Err(PreparationError::invalid_output());
        }
        let runs: Vec<_> = runs.into_iter().collect();
        let mut coverage: Vec<_> = runs.iter().map(|run| run.source.clone()).collect();
        coverage.sort_unstable_by_key(|range| range.start);
        let mut unit_coverage: Vec<_> = units.iter().map(PreparedInteractionUnit::source).collect();
        unit_coverage.sort_unstable_by_key(|range| range.start);
        let source_is_valid = if source.is_empty() {
            break_reason == LineBreakReason::End
                && advance == 0.0
                && runs.is_empty()
                && slices.is_empty()
                && units.is_empty()
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
            for range in &unit_coverage {
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
        let mut trailing_whitespace_advance = 0.0;
        let mut trailing_start = source.end;
        while let Some(unit) = units
            .iter()
            .find(|unit| unit.source().end == trailing_start)
        {
            if unit.whitespace() == ClusterWhitespace::None {
                break;
            }
            trailing_whitespace_advance += unit.advance();
            trailing_start = unit.source().start;
        }
        let unit_advance = units
            .iter()
            .map(PreparedInteractionUnit::advance)
            .sum::<f64>();
        let tolerance = f64::max(1.0, advance.abs()) * 1.0e-6;
        if (unit_advance - advance).abs() > tolerance {
            return Err(PreparationError::invalid_output());
        }
        Ok(Self {
            slot,
            source,
            break_reason,
            advance,
            trailing_whitespace_start: trailing_start,
            trailing_whitespace_advance,
            baseline,
            height,
            content_ascent,
            content_descent,
            interaction: Arc::new(PreparedLineInteraction { slices, units }),
            runs,
        })
    }

    /// Returns the exact accepted region slot, when region flow was requested.
    #[must_use]
    pub const fn slot(&self) -> Option<crate::LineSlot> {
        self.slot
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

    /// Returns the logical trailing-whitespace advance excluded from alignment.
    ///
    /// Trailing whitespace remains source-complete and interactive, but hangs
    /// from the visual line edge instead of changing the aligned content box.
    #[must_use]
    pub const fn trailing_whitespace_advance(&self) -> f64 {
        self.trailing_whitespace_advance
    }

    pub(crate) const fn trailing_whitespace_start(&self) -> u32 {
        self.trailing_whitespace_start
    }

    /// Returns the number of explicit Western inter-word opportunities.
    #[must_use]
    pub fn western_justification_opportunities(&self) -> usize {
        self.western_justification_opportunity_sources().count()
    }

    /// Returns source ranges for non-trailing eligible Western spaces.
    pub fn western_justification_opportunity_sources(
        &self,
    ) -> impl Iterator<Item = Range<u32>> + '_ {
        self.interaction
            .units
            .iter()
            .filter(|unit| {
                unit.is_western_justification_opportunity()
                    && unit.source().end <= self.trailing_whitespace_start
            })
            .map(PreparedInteractionUnit::source)
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

    /// Returns extended-grapheme interaction units in line-local visual order.
    #[must_use]
    pub fn units(&self) -> PreparedInteractionUnits<'_> {
        PreparedInteractionUnits::new(&self.interaction.units, &self.interaction.slices)
    }

    /// Returns shaped runs in line-local visual order.
    #[must_use]
    pub fn runs(&self) -> &[PreparedRun] {
        &self.runs
    }
}

const NO_CURSOR_INDEX: u32 = u32::MAX;

#[derive(Clone, Copy, Debug)]
struct CompactCursorStep {
    target: u32,
    source: u32,
}

impl CompactCursorStep {
    const NONE: Self = Self {
        target: NO_CURSOR_INDEX,
        source: NO_CURSOR_INDEX,
    };
}

#[derive(Clone, Copy, Debug)]
struct CompactCursorMovement {
    position: PreparedClusterSide,
    caret: PreparedCaret,
    steps: [CompactCursorStep; 4],
}

#[derive(Debug)]
pub(crate) struct PreparedCursorTopology {
    movements: Vec<CompactCursorMovement>,
    unit_sources: Vec<Range<u32>>,
}

impl PreparedCursorTopology {
    fn try_new(
        mut movements: Vec<PreparedCursorMovement>,
        unit_sources: Vec<Range<u32>>,
        lines: &[PreparedLine],
        text_len: u32,
        features: SceneFeatures,
    ) -> Result<Self, PreparationError> {
        if !features.has_selection() {
            if !movements.is_empty() {
                return Err(PreparationError::invalid_output());
            }
            return Ok(Self {
                movements: Vec::new(),
                unit_sources: Vec::new(),
            });
        }
        movements.sort_by_key(|movement| cluster_side_key(movement.position()));
        if movements
            .windows(2)
            .any(|pair| pair[0].position() == pair[1].position())
        {
            return Err(PreparationError::invalid_output());
        }
        let mut compact = Vec::with_capacity(movements.len());
        for movement in &movements {
            if movement.position().offset() > text_len {
                return Err(PreparationError::invalid_output());
            }
            let line = usize::try_from(movement.caret().line())
                .map_err(|_| PreparationError::invalid_output())?;
            if if lines.is_empty() {
                line != 0 || movement.caret().inline() != 0.0
            } else {
                lines
                    .get(line)
                    .is_none_or(|line| movement.caret().inline() > line.advance)
            } {
                return Err(PreparationError::invalid_output());
            }
            compact.push(CompactCursorMovement {
                position: movement.position(),
                caret: movement.caret(),
                steps: [
                    compact_cursor_step(movement.previous_visual(), &movements, &unit_sources)?,
                    compact_cursor_step(movement.next_visual(), &movements, &unit_sources)?,
                    compact_cursor_step(movement.previous_logical(), &movements, &unit_sources)?,
                    compact_cursor_step(movement.next_logical(), &movements, &unit_sources)?,
                ],
            });
        }
        Ok(Self {
            movements: compact,
            unit_sources,
        })
    }

    pub(crate) fn movements(&self) -> PreparedCursorMovements<'_> {
        PreparedCursorMovements { topology: self }
    }

    fn contains(&self, position: PreparedClusterSide) -> bool {
        self.movements
            .binary_search_by_key(&cluster_side_key(position), |movement| {
                cluster_side_key(movement.position)
            })
            .is_ok()
    }

    pub(crate) fn accounted_owned_bytes(&self) -> usize {
        vec_bytes::<CompactCursorMovement>(self.movements.capacity()).saturating_add(vec_bytes::<
            Range<u32>,
        >(
            self.unit_sources.capacity(),
        ))
    }

    fn selection_owned_bytes(&self) -> usize {
        self.movements.capacity().saturating_mul(
            size_of::<PreparedClusterSide>().saturating_add(size_of::<PreparedCaret>()),
        )
    }

    fn navigation_owned_bytes(&self) -> usize {
        self.accounted_owned_bytes()
            .saturating_sub(self.selection_owned_bytes())
    }

    #[cfg(test)]
    pub(crate) fn from_movements(
        movements: Vec<PreparedCursorMovement>,
        unit_sources: impl IntoIterator<Item = Range<u32>>,
        text_len: u32,
    ) -> Arc<Self> {
        Arc::new(
            Self::try_new(
                movements,
                unit_sources.into_iter().collect(),
                &[],
                text_len,
                SceneFeatures::EDITABLE,
            )
            .expect("test cursor topology must be valid"),
        )
    }
}

fn compact_cursor_step(
    step: Option<&PreparedCursorStep>,
    movements: &[PreparedCursorMovement],
    unit_sources: &[Range<u32>],
) -> Result<CompactCursorStep, PreparationError> {
    let Some(step) = step else {
        return Ok(CompactCursorStep::NONE);
    };
    let target = movements
        .binary_search_by_key(&cluster_side_key(step.target()), |movement| {
            cluster_side_key(movement.position())
        })
        .map_err(|_| PreparationError::invalid_output())?;
    let source = step
        .source()
        .map(|source| {
            unit_sources
                .iter()
                .position(|candidate| *candidate == source)
                .ok_or_else(PreparationError::invalid_output)
        })
        .transpose()?;
    Ok(CompactCursorStep {
        target: u32::try_from(target).map_err(|_| PreparationError::invalid_output())?,
        source: source
            .map(u32::try_from)
            .transpose()
            .map_err(|_| PreparationError::invalid_output())?
            .unwrap_or(NO_CURSOR_INDEX),
    })
}

const fn cluster_side_key(position: PreparedClusterSide) -> (u32, u8) {
    (
        position.offset(),
        match position.affinity() {
            TextAffinity::Upstream => 0,
            TextAffinity::Downstream => 1,
        },
    )
}

/// Borrowed complete cursor topology for one prepared paragraph.
#[derive(Clone, Copy, Debug)]
pub struct PreparedCursorMovements<'a> {
    topology: &'a PreparedCursorTopology,
}

impl<'a> PreparedCursorMovements<'a> {
    /// Returns the number of represented cursor positions.
    #[must_use]
    pub fn len(self) -> usize {
        self.topology.movements.len()
    }

    /// Returns whether no cursor positions are represented.
    #[must_use]
    pub fn is_empty(self) -> bool {
        self.topology.movements.is_empty()
    }

    /// Iterates represented positions in paragraph-local source order.
    pub fn iter(self) -> impl ExactSizeIterator<Item = PreparedCursorMovementView<'a>> {
        (0..self.topology.movements.len()).map(|index| PreparedCursorMovementView {
            topology: self.topology,
            index,
        })
    }

    pub(crate) fn get(
        self,
        position: PreparedClusterSide,
    ) -> Option<PreparedCursorMovementView<'a>> {
        let index = self
            .topology
            .movements
            .binary_search_by_key(&cluster_side_key(position), |movement| {
                cluster_side_key(movement.position)
            })
            .ok()?;
        Some(PreparedCursorMovementView {
            topology: self.topology,
            index,
        })
    }

    pub(crate) fn selection_owned_bytes(self) -> usize {
        self.topology.selection_owned_bytes()
    }

    pub(crate) fn navigation_owned_bytes(self) -> usize {
        self.topology.navigation_owned_bytes()
    }
}

/// Borrowed cursor transitions for one paragraph-local position.
#[derive(Clone, Copy, Debug)]
pub struct PreparedCursorMovementView<'a> {
    topology: &'a PreparedCursorTopology,
    index: usize,
}

impl<'a> PreparedCursorMovementView<'a> {
    fn record(self) -> CompactCursorMovement {
        self.topology.movements[self.index]
    }

    /// Returns the source position for these transitions.
    #[must_use]
    pub fn position(self) -> PreparedClusterSide {
        self.record().position
    }

    /// Returns the exact paragraph-local caret placement.
    #[must_use]
    pub fn caret(self) -> PreparedCaret {
        self.record().caret
    }

    /// Returns the preceding position in visual order.
    #[must_use]
    pub fn previous_visual(self) -> Option<PreparedCursorStepView<'a>> {
        self.step(0)
    }

    /// Returns the following position in visual order.
    #[must_use]
    pub fn next_visual(self) -> Option<PreparedCursorStepView<'a>> {
        self.step(1)
    }

    /// Returns the preceding interaction-unit boundary in logical order.
    #[must_use]
    pub fn previous_logical(self) -> Option<PreparedCursorStepView<'a>> {
        self.step(2)
    }

    /// Returns the following interaction-unit boundary in logical order.
    #[must_use]
    pub fn next_logical(self) -> Option<PreparedCursorStepView<'a>> {
        self.step(3)
    }

    fn step(self, index: usize) -> Option<PreparedCursorStepView<'a>> {
        let step = self.record().steps[index];
        (step.target != NO_CURSOR_INDEX).then_some(PreparedCursorStepView {
            topology: self.topology,
            step,
        })
    }
}

/// Borrowed destination and crossed unit for one cursor transition.
#[derive(Clone, Copy, Debug)]
pub struct PreparedCursorStepView<'a> {
    topology: &'a PreparedCursorTopology,
    step: CompactCursorStep,
}

impl PreparedCursorStepView<'_> {
    /// Returns the destination position.
    #[must_use]
    pub fn target(self) -> PreparedClusterSide {
        self.topology.movements[self.step.target as usize].position
    }

    /// Returns the complete interaction unit crossed by this step.
    #[must_use]
    pub fn source(self) -> Option<Range<u32>> {
        (self.step.source != NO_CURSOR_INDEX)
            .then(|| self.topology.unit_sources[self.step.source as usize].clone())
    }
}

#[derive(Debug)]
pub(crate) struct PreparedParagraphFacts {
    text_len: u32,
    resolved_direction: ResolvedDirection,
    features: SceneFeatures,
    lines: Vec<PreparedLine>,
    movements: Arc<PreparedCursorTopology>,
}

/// Validated owned formed lines for one paragraph.
///
/// The paragraph identity is a fresh envelope around immutable paragraph-local
/// facts. This lets an eligible retained cache share exact preparation without
/// sharing document or paragraph identity.
#[derive(Clone, Debug)]
pub struct PreparedParagraph {
    paragraph: ParagraphId,
    facts: Arc<PreparedParagraphFacts>,
}

impl PreparedParagraph {
    /// Validates and collects formed lines plus complete cursor transitions.
    pub fn try_new(
        paragraph: ParagraphId,
        text_len: u32,
        resolved_direction: ResolvedDirection,
        lines: impl IntoIterator<Item = PreparedLine>,
        movements: impl IntoIterator<Item = PreparedCursorMovement>,
    ) -> Result<Self, PreparationError> {
        Self::try_new_with_features(
            paragraph,
            text_len,
            resolved_direction,
            SceneFeatures::EDITABLE,
            lines,
            movements,
        )
    }

    /// Validates formed lines and only the interaction facts requested by `features`.
    ///
    /// A profile below selection must supply no cursor-movement graph. This
    /// keeps a display or accessibility adapter output from merely hiding
    /// maximal lowering behind an empty scene sidecar.
    pub fn try_new_with_features(
        paragraph: ParagraphId,
        text_len: u32,
        resolved_direction: ResolvedDirection,
        features: SceneFeatures,
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
            if line.interaction.units.is_empty() {
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
                for unit in &line.interaction.units {
                    push_unique_position(&mut positions, unit.left());
                    push_unique_position(&mut positions, unit.right());
                }
            }
        }
        if positions.is_empty() && text_len == 0 {
            positions.push(PreparedClusterSide::new(0, TextAffinity::Downstream));
        }
        let unit_sources: Vec<_> = lines
            .iter()
            .flat_map(|line| {
                line.interaction
                    .units
                    .iter()
                    .map(PreparedInteractionUnit::source)
            })
            .collect();
        let movements = PreparedCursorTopology::try_new(
            movements.into_iter().collect(),
            unit_sources,
            &lines,
            text_len,
            features,
        )?;
        if features.has_selection()
            && positions
                .iter()
                .any(|position| !movements.contains(*position))
        {
            return Err(PreparationError::invalid_output());
        }
        Ok(Self {
            paragraph,
            facts: Arc::new(PreparedParagraphFacts {
                text_len,
                resolved_direction,
                features,
                lines,
                movements: Arc::new(movements),
            }),
        })
    }

    pub(crate) fn from_shared_facts(
        paragraph: ParagraphId,
        facts: Arc<PreparedParagraphFacts>,
    ) -> Self {
        Self { paragraph, facts }
    }

    pub(crate) fn shared_facts(&self) -> Arc<PreparedParagraphFacts> {
        self.facts.clone()
    }

    /// Returns the paragraph identity.
    #[must_use]
    pub const fn paragraph(&self) -> ParagraphId {
        self.paragraph
    }

    /// Returns the projected paragraph length in UTF-8 bytes.
    #[must_use]
    pub fn text_len(&self) -> u32 {
        self.facts.text_len
    }

    /// Returns the base direction resolved by the backend's Unicode analysis.
    #[must_use]
    pub fn resolved_direction(&self) -> ResolvedDirection {
        self.facts.resolved_direction
    }

    /// Returns the normalized capabilities represented by this prepared output.
    #[must_use]
    pub fn features(&self) -> SceneFeatures {
        self.facts.features
    }

    /// Returns the source-ordered formed lines.
    #[must_use]
    pub fn lines(&self) -> &[PreparedLine] {
        &self.facts.lines
    }

    /// Returns complete paragraph-local cursor transitions.
    #[must_use]
    pub fn movements(&self) -> PreparedCursorMovements<'_> {
        self.facts.movements.movements()
    }

    /// Returns the deterministic byte charge for this prepared paragraph's
    /// owned portable records.
    ///
    /// Shared font blobs and allocator overhead are excluded. An adapter
    /// retaining another handle to the same facts may use this charge for its
    /// own conservative cache budgeting.
    #[must_use]
    pub fn accounted_owned_bytes(&self) -> usize {
        self.facts.estimated_owned_bytes()
    }
}

impl PreparedParagraphFacts {
    pub(crate) const fn features(&self) -> SceneFeatures {
        self.features
    }

    pub(crate) fn movements(&self) -> PreparedCursorMovements<'_> {
        self.movements.movements()
    }

    pub(crate) fn lines(&self) -> &[PreparedLine] {
        &self.lines
    }

    #[cfg(test)]
    pub(crate) fn for_test(
        text_len: u32,
        features: SceneFeatures,
        movements: Arc<PreparedCursorTopology>,
    ) -> Arc<Self> {
        Arc::new(Self {
            text_len,
            resolved_direction: ResolvedDirection::Ltr,
            features,
            lines: Vec::new(),
            movements,
        })
    }

    pub(crate) fn estimated_owned_bytes(&self) -> usize {
        let mut bytes = size_of::<Self>()
            .saturating_add(vec_bytes::<PreparedLine>(self.lines.capacity()))
            .saturating_add(self.movements.accounted_owned_bytes());
        for line in &self.lines {
            bytes = bytes
                .saturating_add(vec_bytes::<PreparedInteractionUnit>(
                    line.interaction.units.capacity(),
                ))
                .saturating_add(vec_bytes::<PreparedInteractionSlice>(
                    line.interaction.slices.capacity(),
                ))
                .saturating_add(vec_bytes::<PreparedRun>(line.runs.capacity()));
            for run in &line.runs {
                bytes = bytes
                    .saturating_add(vec_bytes::<i16>(run.normalized_coords.capacity()))
                    .saturating_add(vec_bytes::<Range<u32>>(run.unrendered_source.capacity()))
                    .saturating_add(vec_bytes::<PreparedGlyph>(run.glyphs.capacity()));
                if let Some(evidence) = &run.synthesis.evidence {
                    bytes = bytes
                        .saturating_add(size_of::<FontSynthesisEvidence>())
                        .saturating_add(vec_bytes::<FontVariation>(evidence.variations.capacity()));
                }
                for glyph in &run.glyphs {
                    bytes = bytes.saturating_add(
                        size_of::<GlyphPaintSegment>()
                            .saturating_mul(glyph.paint.segment_capacity()),
                    );
                }
            }
        }
        bytes
    }
}

const fn vec_bytes<T>(capacity: usize) -> usize {
    size_of::<T>().saturating_mul(capacity)
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
    normalized_coords: Arc<Vec<i16>>,
    unrendered_source: Arc<Vec<Range<u32>>>,
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
            normalized_coords: Arc::new(normalized_coords.into_iter().collect()),
            unrendered_source: Arc::new(unrendered_source),
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
            || paint.segments().first().is_none_or(|segment| {
                segment.source().start != source.start
                    || paint
                        .segments()
                        .last()
                        .is_none_or(|last| last.source().end != source.end)
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

    /// Returns complete source-to-paint coverage.
    #[must_use]
    pub const fn paint(&self) -> &GlyphPaintCoverage {
        &self.paint
    }
}
