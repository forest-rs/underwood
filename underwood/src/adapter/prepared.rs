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
    interaction: PreparedLineInteraction,
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
            interaction: PreparedLineInteraction {
                slices,
                units,
                source_order,
            },
            runs,
        })
    }
}

#[derive(Debug)]
pub(crate) struct PreparedParagraphFacts {
    text_len: u32,
    resolved_direction: ResolvedDirection,
    features: SceneFeatures,
    lines: Vec<PreparedLineRecord>,
    runs: Vec<PreparedRunRecord>,
    glyphs: Vec<PreparedGlyphRecord>,
    split_glyph_paints: Vec<PreparedSplitGlyphPaint>,
    interaction_slices: Vec<PreparedInteractionSlice>,
    interaction_units: Vec<PreparedInteractionUnitRecord>,
    source_order: Vec<u32>,
    normalized_coords: Vec<i16>,
    unrendered_source: Vec<Range<u32>>,
}

#[derive(Debug)]
struct PreparedLineRecord {
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
    slices: TableRange,
    units: TableRange,
    source_order: TableRange,
    runs: TableRange,
}

#[derive(Debug)]
struct PreparedRunRecord {
    source: Range<u32>,
    bidi_level: u8,
    script: [u8; 4],
    font: FontData,
    font_size: f32,
    synthesis: FontSynthesis,
    normalized_coords: TableRange,
    unrendered_source: TableRange,
    glyphs: TableRange,
}

#[derive(Debug)]
struct PreparedGlyphRecord {
    id: u32,
    source: Range<u32>,
    advance: [f32; 2],
    offset: [f32; 2],
}

impl PreparedGlyphRecord {
    fn try_from_glyph(
        glyph: PreparedGlyph,
    ) -> Result<(Self, GlyphPaintCoverage), PreparationError> {
        let advance = [
            compact_shaping_coordinate(glyph.advance.x)
                .ok_or_else(PreparationError::invalid_output)?,
            compact_shaping_coordinate(glyph.advance.y)
                .ok_or_else(PreparationError::invalid_output)?,
        ];
        let offset = [
            compact_shaping_coordinate(glyph.offset.x)
                .ok_or_else(PreparationError::invalid_output)?,
            compact_shaping_coordinate(glyph.offset.y)
                .ok_or_else(PreparationError::invalid_output)?,
        ];
        Ok((
            Self {
                id: glyph.id,
                source: glyph.source,
                advance,
                offset,
            },
            glyph.paint,
        ))
    }
}

#[derive(Debug)]
struct PreparedSplitGlyphPaint {
    glyph: u32,
    coverage: GlyphPaintCoverage,
}

#[derive(Clone, Copy, Debug)]
struct TableRange {
    start: u32,
    end: u32,
}

impl TableRange {
    fn try_from_usize(range: Range<usize>) -> Result<Self, PreparationError> {
        Ok(Self {
            start: u32::try_from(range.start).map_err(|_| PreparationError::invalid_output())?,
            end: u32::try_from(range.end).map_err(|_| PreparationError::invalid_output())?,
        })
    }

    fn as_usize(self) -> Range<usize> {
        self.start as usize..self.end as usize
    }
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
        let facts = PreparedParagraphFacts::flatten(text_len, resolved_direction, features, lines)?;
        Ok(Self {
            paragraph,
            facts: Arc::new(facts),
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
    pub fn lines(&self) -> PreparedLines<'_> {
        self.facts.lines()
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

    /// Returns whether both paragraph envelopes share one canonical artifact.
    #[must_use]
    pub fn shares_facts_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.facts, &other.facts)
    }
}

impl PreparedParagraphFacts {
    pub(crate) const fn text_len(&self) -> u32 {
        self.text_len
    }

    pub(crate) const fn features(&self) -> SceneFeatures {
        self.features
    }

    pub(crate) fn lines(&self) -> PreparedLines<'_> {
        PreparedLines::new(self)
    }

    #[cfg(test)]
    pub(crate) fn for_test(
        text_len: u32,
        features: SceneFeatures,
        lines: Vec<PreparedLine>,
    ) -> Arc<Self> {
        Arc::new(
            Self::flatten(text_len, ResolvedDirection::Ltr, features, lines)
                .expect("test prepared lines must fit canonical table indexes"),
        )
    }

    pub(crate) fn estimated_owned_bytes(&self) -> usize {
        let mut bytes = size_of::<Self>()
            .saturating_add(vec_bytes::<PreparedLineRecord>(self.lines.capacity()))
            .saturating_add(vec_bytes::<PreparedRunRecord>(self.runs.capacity()))
            .saturating_add(vec_bytes::<PreparedGlyphRecord>(self.glyphs.capacity()))
            .saturating_add(vec_bytes::<PreparedSplitGlyphPaint>(
                self.split_glyph_paints.capacity(),
            ))
            .saturating_add(vec_bytes::<PreparedInteractionSlice>(
                self.interaction_slices.capacity(),
            ))
            .saturating_add(vec_bytes::<PreparedInteractionUnitRecord>(
                self.interaction_units.capacity(),
            ))
            .saturating_add(vec_bytes::<u32>(self.source_order.capacity()))
            .saturating_add(vec_bytes::<i16>(self.normalized_coords.capacity()))
            .saturating_add(vec_bytes::<Range<u32>>(self.unrendered_source.capacity()));
        for run in &self.runs {
            if let Some(evidence) = &run.synthesis.evidence {
                bytes = bytes
                    .saturating_add(size_of::<FontSynthesisEvidence>())
                    .saturating_add(vec_bytes::<FontVariation>(evidence.variations.capacity()));
            }
        }
        for split in &self.split_glyph_paints {
            bytes = bytes.saturating_add(
                size_of::<GlyphPaintSegment>().saturating_mul(
                    split
                        .coverage
                        .split_segments()
                        .map_or(0, <[GlyphPaintSegment]>::len),
                ),
            );
        }
        bytes
    }

    fn flatten(
        text_len: u32,
        resolved_direction: ResolvedDirection,
        features: SceneFeatures,
        lines: Vec<PreparedLine>,
    ) -> Result<Self, PreparationError> {
        let run_capacity = lines.iter().map(|line| line.runs.len()).sum();
        let glyph_capacity = lines
            .iter()
            .flat_map(|line| &line.runs)
            .map(|run| run.glyphs.len())
            .sum();
        let slice_capacity = lines.iter().map(|line| line.interaction.slices.len()).sum();
        let unit_capacity = lines.iter().map(|line| line.interaction.units.len()).sum();
        let source_order_capacity = lines
            .iter()
            .map(|line| line.interaction.source_order.len())
            .sum();
        let normalized_coord_capacity = lines
            .iter()
            .flat_map(|line| &line.runs)
            .map(|run| run.normalized_coords.len())
            .sum();
        let unrendered_capacity = lines
            .iter()
            .flat_map(|line| &line.runs)
            .map(|run| run.unrendered_source.len())
            .sum();

        let mut line_records = Vec::with_capacity(lines.len());
        let mut run_records = Vec::with_capacity(run_capacity);
        let mut glyphs = Vec::with_capacity(glyph_capacity);
        let mut split_glyph_paints = Vec::new();
        let mut interaction_slices = Vec::with_capacity(slice_capacity);
        let mut interaction_units = Vec::with_capacity(unit_capacity);
        let mut source_order = Vec::with_capacity(source_order_capacity);
        let mut normalized_coords = Vec::with_capacity(normalized_coord_capacity);
        let mut unrendered_source = Vec::with_capacity(unrendered_capacity);

        for line in lines {
            let PreparedLine {
                slot,
                source,
                break_reason,
                advance,
                trailing_whitespace_start,
                trailing_whitespace_advance,
                baseline,
                height,
                content_ascent,
                content_descent,
                interaction,
                runs,
            } = line;
            let PreparedLineInteraction {
                slices,
                units,
                source_order: line_source_order,
            } = interaction;

            let slices_start = interaction_slices.len();
            interaction_slices.extend(slices);
            let units_start = interaction_units.len();
            for unit in units {
                interaction_units.push(PreparedInteractionUnitRecord::try_from_unit(unit)?);
            }
            let source_order_start = source_order.len();
            source_order.extend(line_source_order);
            let runs_start = run_records.len();

            for run in runs {
                let PreparedRun {
                    source,
                    bidi_level,
                    script,
                    font,
                    font_size,
                    synthesis,
                    normalized_coords: run_normalized_coords,
                    unrendered_source: run_unrendered_source,
                    glyphs: run_glyphs,
                } = run;
                let normalized_coords_start = normalized_coords.len();
                normalized_coords.extend(run_normalized_coords);
                let unrendered_source_start = unrendered_source.len();
                unrendered_source.extend(run_unrendered_source);
                let glyphs_start = glyphs.len();
                for glyph in run_glyphs {
                    let glyph_index = u32::try_from(glyphs.len())
                        .map_err(|_| PreparationError::invalid_output())?;
                    let (glyph, paint) = PreparedGlyphRecord::try_from_glyph(glyph)?;
                    glyphs.push(glyph);
                    if !paint.is_whole() {
                        split_glyph_paints.push(PreparedSplitGlyphPaint {
                            glyph: glyph_index,
                            coverage: paint,
                        });
                    }
                }
                run_records.push(PreparedRunRecord {
                    source,
                    bidi_level,
                    script,
                    font,
                    font_size,
                    synthesis,
                    normalized_coords: TableRange::try_from_usize(
                        normalized_coords_start..normalized_coords.len(),
                    )?,
                    unrendered_source: TableRange::try_from_usize(
                        unrendered_source_start..unrendered_source.len(),
                    )?,
                    glyphs: TableRange::try_from_usize(glyphs_start..glyphs.len())?,
                });
            }

            line_records.push(PreparedLineRecord {
                slot,
                source,
                break_reason,
                advance,
                trailing_whitespace_start,
                trailing_whitespace_advance,
                baseline,
                height,
                content_ascent,
                content_descent,
                slices: TableRange::try_from_usize(slices_start..interaction_slices.len())?,
                units: TableRange::try_from_usize(units_start..interaction_units.len())?,
                source_order: TableRange::try_from_usize(source_order_start..source_order.len())?,
                runs: TableRange::try_from_usize(runs_start..run_records.len())?,
            });
        }

        Ok(Self {
            text_len,
            resolved_direction,
            features,
            lines: line_records,
            runs: run_records,
            glyphs,
            split_glyph_paints,
            interaction_slices,
            interaction_units,
            source_order,
            normalized_coords,
            unrendered_source,
        })
    }
}

/// Borrowed source-ordered formed lines from one canonical paragraph artifact.
#[derive(Clone, Copy, Debug)]
pub struct PreparedLines<'a> {
    facts: &'a PreparedParagraphFacts,
    front: usize,
    back: usize,
}

impl<'a> PreparedLines<'a> {
    fn new(facts: &'a PreparedParagraphFacts) -> Self {
        Self {
            facts,
            front: 0,
            back: facts.lines.len(),
        }
    }

    /// Returns a fresh traversal over every line.
    #[must_use]
    pub fn iter(&self) -> Self {
        Self::new(self.facts)
    }

    /// Returns the number of lines.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.back - self.front
    }

    /// Returns whether the artifact has no formed lines.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns a line by source-order index.
    #[must_use]
    pub fn get(&self, index: usize) -> Option<PreparedLineView<'a>> {
        (index < self.facts.lines.len()).then_some(PreparedLineView {
            facts: self.facts,
            index,
        })
    }

    /// Returns the first line.
    #[must_use]
    pub fn first(&self) -> Option<PreparedLineView<'a>> {
        self.get(0)
    }

    /// Returns the final line.
    #[must_use]
    pub fn last(self) -> Option<PreparedLineView<'a>> {
        self.facts
            .lines
            .len()
            .checked_sub(1)
            .and_then(|index| self.get(index))
    }

    pub(crate) fn partition_point(
        &self,
        mut predicate: impl FnMut(PreparedLineView<'a>) -> bool,
    ) -> usize {
        let mut left = 0;
        let mut right = self.facts.lines.len();
        while left < right {
            let middle = left + (right - left) / 2;
            if predicate(
                self.get(middle)
                    .expect("binary-search midpoint remains in the line table"),
            ) {
                left = middle + 1;
            } else {
                right = middle;
            }
        }
        left
    }
}

impl<'a> Iterator for PreparedLines<'a> {
    type Item = PreparedLineView<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let index = self.front;
        if index == self.back {
            return None;
        }
        self.front += 1;
        Some(PreparedLineView {
            facts: self.facts,
            index,
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}

impl DoubleEndedIterator for PreparedLines<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.front == self.back {
            return None;
        }
        self.back -= 1;
        Some(PreparedLineView {
            facts: self.facts,
            index: self.back,
        })
    }
}

impl ExactSizeIterator for PreparedLines<'_> {}

/// Borrowed view of one formed line in a canonical paragraph artifact.
#[derive(Clone, Copy, Debug)]
pub struct PreparedLineView<'a> {
    facts: &'a PreparedParagraphFacts,
    index: usize,
}

impl<'a> PreparedLineView<'a> {
    fn record(self) -> &'a PreparedLineRecord {
        &self.facts.lines[self.index]
    }

    /// Returns the exact accepted region slot, when region flow was requested.
    #[must_use]
    pub fn slot(self) -> Option<crate::LineSlot> {
        self.record().slot
    }

    /// Returns the paragraph-local source range.
    #[must_use]
    pub fn source(self) -> Range<u32> {
        self.record().source.clone()
    }

    /// Returns why the line ended.
    #[must_use]
    pub fn break_reason(self) -> LineBreakReason {
        self.record().break_reason
    }

    /// Returns the full inline advance, including trailing whitespace.
    #[must_use]
    pub fn advance(self) -> f64 {
        self.record().advance
    }

    /// Returns the logical trailing-whitespace advance excluded from alignment.
    #[must_use]
    pub fn trailing_whitespace_advance(self) -> f64 {
        self.record().trailing_whitespace_advance
    }

    /// Returns the number of explicit Western inter-word opportunities.
    #[must_use]
    pub fn western_justification_opportunities(self) -> usize {
        self.western_justification_opportunity_sources().count()
    }

    /// Returns source ranges for non-trailing eligible Western spaces.
    pub fn western_justification_opportunity_sources(
        self,
    ) -> impl Iterator<Item = Range<u32>> + 'a {
        let trailing_whitespace_start = self.record().trailing_whitespace_start;
        self.units()
            .filter(move |unit| {
                unit.is_western_justification_opportunity()
                    && unit.source().end <= trailing_whitespace_start
            })
            .map(|unit| unit.source())
    }

    /// Returns the baseline offset from the top of the line box.
    #[must_use]
    pub fn baseline(self) -> f64 {
        self.record().baseline
    }

    /// Returns the block-axis line-box extent.
    #[must_use]
    pub fn height(self) -> f64 {
        self.record().height
    }

    /// Returns the maximum font ascent contributing to the line.
    #[must_use]
    pub fn content_ascent(self) -> f64 {
        self.record().content_ascent
    }

    /// Returns the maximum font descent contributing to the line.
    #[must_use]
    pub fn content_descent(self) -> f64 {
        self.record().content_descent
    }

    /// Returns extended-grapheme units in line-local visual order.
    #[must_use]
    pub fn units(self) -> PreparedInteractionUnits<'a> {
        let record = self.record();
        PreparedInteractionUnits::new(
            &self.facts.interaction_units[record.units.as_usize()],
            &self.facts.interaction_slices[record.slices.as_usize()],
        )
    }

    pub(crate) fn unit_at_source_rank(
        self,
        rank: usize,
    ) -> Option<(usize, PreparedInteractionUnitView<'a>)> {
        let record = self.record();
        let source_order = &self.facts.source_order[record.source_order.as_usize()];
        let index = source_order.get(rank).map_or(rank, |&index| index as usize);
        self.units().nth(index).map(|unit| (index, unit))
    }

    /// Returns shaped runs in line-local visual order.
    #[must_use]
    pub fn runs(self) -> PreparedRuns<'a> {
        PreparedRuns {
            facts: self.facts,
            front: self.record().runs.start as usize,
            back: self.record().runs.end as usize,
        }
    }
}

/// Borrowed visual shaped runs from one formed line.
#[derive(Clone, Copy, Debug)]
pub struct PreparedRuns<'a> {
    facts: &'a PreparedParagraphFacts,
    front: usize,
    back: usize,
}

impl<'a> PreparedRuns<'a> {
    /// Returns a fresh traversal over the same line's runs.
    #[must_use]
    pub fn iter(&self) -> Self {
        *self
    }

    /// Returns the number of runs.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.back - self.front
    }

    /// Returns whether the line has no shaped runs.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns a run by line-local visual index.
    #[must_use]
    pub fn get(&self, index: usize) -> Option<PreparedRunView<'a>> {
        let index = self.front.checked_add(index)?;
        (index < self.back).then_some(PreparedRunView {
            facts: self.facts,
            index,
        })
    }
}

impl<'a> Iterator for PreparedRuns<'a> {
    type Item = PreparedRunView<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.front == self.back {
            return None;
        }
        let index = self.front;
        self.front += 1;
        Some(PreparedRunView {
            facts: self.facts,
            index,
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}

impl DoubleEndedIterator for PreparedRuns<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.front == self.back {
            return None;
        }
        self.back -= 1;
        Some(PreparedRunView {
            facts: self.facts,
            index: self.back,
        })
    }
}

impl ExactSizeIterator for PreparedRuns<'_> {}

/// Borrowed view of one shaped run in the canonical paragraph artifact.
#[derive(Clone, Copy, Debug)]
pub struct PreparedRunView<'a> {
    facts: &'a PreparedParagraphFacts,
    index: usize,
}

impl<'a> PreparedRunView<'a> {
    fn record(self) -> &'a PreparedRunRecord {
        &self.facts.runs[self.index]
    }

    /// Returns the paragraph-local source range.
    #[must_use]
    pub fn source(self) -> Range<u32> {
        self.record().source.clone()
    }

    /// Returns the resolved bidi level.
    #[must_use]
    pub fn bidi_level(self) -> u8 {
        self.record().bidi_level
    }

    /// Returns the ISO 15924 script tag.
    #[must_use]
    pub fn script(self) -> [u8; 4] {
        self.record().script
    }

    /// Returns the exact font resource and face index.
    #[must_use]
    pub fn font(self) -> &'a FontData {
        &self.record().font
    }

    /// Returns the font size used for shaping.
    #[must_use]
    pub fn font_size(self) -> f32 {
        self.record().font_size
    }

    /// Returns synthesis suggestions selected for this font instance.
    #[must_use]
    pub fn synthesis(self) -> &'a FontSynthesis {
        &self.record().synthesis
    }

    /// Returns normalized variation coordinates.
    #[must_use]
    pub fn normalized_coords(self) -> &'a [i16] {
        &self.facts.normalized_coords[self.record().normalized_coords.as_usize()]
    }

    /// Returns source ranges which intentionally produce no glyphs.
    #[must_use]
    pub fn unrendered_source(self) -> &'a [Range<u32>] {
        &self.facts.unrendered_source[self.record().unrendered_source.as_usize()]
    }

    /// Returns glyphs in backend-provided visual order.
    #[must_use]
    pub fn glyphs(self) -> PreparedGlyphs<'a> {
        PreparedGlyphs::new(self.facts, self.record().glyphs.as_usize())
    }
}

/// Allocation-free traversal of shaped glyphs in canonical visual order.
#[derive(Clone, Copy, Debug)]
pub struct PreparedGlyphs<'a> {
    facts: &'a PreparedParagraphFacts,
    front: usize,
    back: usize,
}

impl<'a> PreparedGlyphs<'a> {
    fn new(facts: &'a PreparedParagraphFacts, range: Range<usize>) -> Self {
        Self {
            facts,
            front: range.start,
            back: range.end,
        }
    }

    /// Returns a fresh traversal over the same glyphs.
    #[must_use]
    pub fn iter(&self) -> Self {
        *self
    }

    /// Returns the number of remaining glyphs.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.back - self.front
    }

    /// Returns whether no glyphs remain.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns one glyph by traversal-local index.
    #[must_use]
    pub fn get(&self, index: usize) -> Option<PreparedGlyphView<'a>> {
        let index = self.front.checked_add(index)?;
        (index < self.back).then_some(PreparedGlyphView {
            facts: self.facts,
            index,
        })
    }

    /// Returns the first glyph.
    #[must_use]
    pub fn first(&self) -> Option<PreparedGlyphView<'a>> {
        self.get(0)
    }

    /// Returns the final glyph.
    #[must_use]
    pub fn last(&self) -> Option<PreparedGlyphView<'a>> {
        self.len().checked_sub(1).and_then(|index| self.get(index))
    }

    pub(crate) fn slice(self, range: Range<usize>) -> Option<Self> {
        if range.start > range.end || range.end > self.len() {
            return None;
        }
        Some(Self {
            facts: self.facts,
            front: self.front + range.start,
            back: self.front + range.end,
        })
    }
}

impl<'a> Iterator for PreparedGlyphs<'a> {
    type Item = PreparedGlyphView<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.front == self.back {
            return None;
        }
        let index = self.front;
        self.front += 1;
        Some(PreparedGlyphView {
            facts: self.facts,
            index,
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}

impl DoubleEndedIterator for PreparedGlyphs<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.front == self.back {
            return None;
        }
        self.back -= 1;
        Some(PreparedGlyphView {
            facts: self.facts,
            index: self.back,
        })
    }
}

impl ExactSizeIterator for PreparedGlyphs<'_> {}
impl core::iter::FusedIterator for PreparedGlyphs<'_> {}

/// Borrowed shaped glyph from one canonical paragraph artifact.
#[derive(Clone, Copy, Debug)]
pub struct PreparedGlyphView<'a> {
    facts: &'a PreparedParagraphFacts,
    index: usize,
}

impl<'a> PreparedGlyphView<'a> {
    fn record(self) -> &'a PreparedGlyphRecord {
        &self.facts.glyphs[self.index]
    }

    /// Returns the backend glyph identifier.
    #[must_use]
    pub fn id(self) -> u32 {
        self.record().id
    }

    /// Returns the paragraph-local source range.
    #[must_use]
    pub fn source(self) -> Range<u32> {
        self.record().source.clone()
    }

    /// Returns the shaped advance.
    #[must_use]
    pub fn advance(self) -> Vec2 {
        let [x, y] = self.record().advance;
        Vec2::new(f64::from(x), f64::from(y))
    }

    /// Returns the shaped glyph offset.
    #[must_use]
    pub fn offset(self) -> Vec2 {
        let [x, y] = self.record().offset;
        Vec2::new(f64::from(x), f64::from(y))
    }

    /// Returns exceptional split-paint coverage.
    ///
    /// Whole glyphs share one zero-payload marker. Only exceptional split
    /// glyphs retain out-of-line clipped coverage.
    #[must_use]
    pub fn paint(self) -> &'a GlyphPaintCoverage {
        let index =
            u32::try_from(self.index).expect("canonical glyph indexes were validated as u32");
        self.facts
            .split_glyph_paints
            .binary_search_by_key(&index, |split| split.glyph)
            .ok()
            .map_or_else(
                || whole_glyph_paint(),
                |index| &self.facts.split_glyph_paints[index].coverage,
            )
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
