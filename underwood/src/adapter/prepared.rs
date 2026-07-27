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
    source_order: Vec<u32>,
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
        let source_ordered = units
            .windows(2)
            .all(|pair| pair[0].source().end <= pair[1].source().start);
        let mut source_order = Vec::new();
        if !source_ordered {
            source_order = (0..units.len())
                .map(u32::try_from)
                .collect::<Result<_, _>>()
                .map_err(|_| PreparationError::invalid_output())?;
            source_order.sort_unstable_by_key(|&index| units[index as usize].source().start);
        }
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
            for rank in 0..units.len() {
                let index = source_order.get(rank).map_or(rank, |&index| index as usize);
                let range = units[index].source();
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
        for rank in (0..units.len()).rev() {
            let index = source_order.get(rank).map_or(rank, |&index| index as usize);
            let unit = &units[index];
            debug_assert_eq!(
                unit.source().end,
                trailing_start,
                "validated source-order units remain contiguous"
            );
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
            interaction: Arc::new(PreparedLineInteraction {
                slices,
                units,
                source_order,
            }),
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

    pub(crate) fn unit_at_source_rank(
        &self,
        rank: usize,
    ) -> Option<(usize, PreparedInteractionUnitView<'_>)> {
        let index = self
            .interaction
            .source_order
            .get(rank)
            .map_or(rank, |&index| index as usize);
        self.units().nth(index).map(|unit| (index, unit))
    }

    /// Returns shaped runs in line-local visual order.
    #[must_use]
    pub fn runs(&self) -> &[PreparedRun] {
        &self.runs
    }
}

#[derive(Debug)]
pub(crate) struct PreparedParagraphFacts {
    text_len: u32,
    resolved_direction: ResolvedDirection,
    features: SceneFeatures,
    lines: Vec<PreparedLine>,
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
    /// Validates and collects source-complete formed lines.
    pub fn try_new(
        paragraph: ParagraphId,
        text_len: u32,
        resolved_direction: ResolvedDirection,
        lines: impl IntoIterator<Item = PreparedLine>,
    ) -> Result<Self, PreparationError> {
        Self::try_new_with_features(
            paragraph,
            text_len,
            resolved_direction,
            SceneFeatures::EDITABLE,
            lines,
        )
    }

    /// Validates formed lines and only the interaction facts requested by `features`.
    ///
    /// Cursor movement and caret geometry are derived from line-local
    /// interaction units, so adapters do not build or retain a parallel graph.
    pub fn try_new_with_features(
        paragraph: ParagraphId,
        text_len: u32,
        resolved_direction: ResolvedDirection,
        features: SceneFeatures,
        lines: impl IntoIterator<Item = PreparedLine>,
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
        Ok(Self {
            paragraph,
            facts: Arc::new(PreparedParagraphFacts {
                text_len,
                resolved_direction,
                features,
                lines,
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
    pub(crate) const fn text_len(&self) -> u32 {
        self.text_len
    }

    pub(crate) const fn features(&self) -> SceneFeatures {
        self.features
    }

    pub(crate) fn lines(&self) -> &[PreparedLine] {
        &self.lines
    }

    #[cfg(test)]
    pub(crate) fn for_test(
        text_len: u32,
        features: SceneFeatures,
        lines: Vec<PreparedLine>,
    ) -> Arc<Self> {
        Arc::new(Self {
            text_len,
            resolved_direction: ResolvedDirection::Ltr,
            features,
            lines,
        })
    }

    pub(crate) fn estimated_owned_bytes(&self) -> usize {
        let mut bytes =
            size_of::<Self>().saturating_add(vec_bytes::<PreparedLine>(self.lines.capacity()));
        for line in &self.lines {
            bytes = bytes
                .saturating_add(vec_bytes::<PreparedInteractionUnit>(
                    line.interaction.units.capacity(),
                ))
                .saturating_add(vec_bytes::<PreparedInteractionSlice>(
                    line.interaction.slices.capacity(),
                ))
                .saturating_add(vec_bytes::<u32>(line.interaction.source_order.capacity()))
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
                        size_of::<GlyphPaintSegment>().saturating_mul(
                            glyph
                                .paint
                                .split_segments()
                                .map_or(0, <[GlyphPaintSegment]>::len),
                        ),
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

/// One shaped glyph with paragraph source and exceptional split-paint coverage.
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
            || paint.split_segments().is_some_and(|segments| {
                segments.first().is_none_or(|segment| {
                    segment.source().start != source.start
                        || segments
                            .last()
                            .is_none_or(|last| last.source().end != source.end)
                })
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

    /// Returns exceptional split-paint coverage.
    ///
    /// Whole glyphs carry no slot or source copy; core binds their existing
    /// source range to the authoritative paragraph paint runs.
    #[must_use]
    pub const fn paint(&self) -> &GlyphPaintCoverage {
        &self.paint
    }
}
