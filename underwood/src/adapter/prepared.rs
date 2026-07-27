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
/// Checked metadata for one source-complete formed line.
///
/// A [`PreparedParagraphBuilder`] accepts this metadata, then streams the
/// line's interaction units and visual runs directly into the canonical
/// paragraph tables.
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
}

impl PreparedLine {
    /// Validates metadata for one formed line.
    pub fn try_new(
        source: Range<u32>,
        break_reason: LineBreakReason,
        advance: f64,
        baseline: f64,
        height: f64,
        content_ascent: f64,
        content_descent: f64,
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
        )
    }

    /// Validates metadata for one line accepted into an exact region slot.
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
        let source_end = source.end;
        Ok(Self {
            slot,
            source,
            break_reason,
            advance,
            trailing_whitespace_start: source_end,
            trailing_whitespace_advance: 0.0,
            baseline,
            height,
            content_ascent,
            content_descent,
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
    glyph_placements: Vec<PreparedGlyphPlacement>,
    split_glyph_paints: Vec<PreparedSplitGlyphPaint>,
    interaction_slices: Vec<PreparedInteractionSlice>,
    interaction_slice_spills: Vec<PreparedInteractionSliceSpill>,
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
    inline_advance: f32,
}

#[derive(Debug)]
struct PreparedGlyphPlacement {
    glyph: u32,
    block_advance: f32,
    offset: [f32; 2],
}

impl PreparedGlyphRecord {
    fn try_from_glyph(
        glyph: PreparedGlyph,
    ) -> Result<(Self, Option<PreparedGlyphPlacement>, GlyphPaintCoverage), PreparationError> {
        let inline_advance = compact_shaping_coordinate(glyph.advance.x)
            .ok_or_else(PreparationError::invalid_output)?;
        let block_advance = compact_shaping_coordinate(glyph.advance.y)
            .ok_or_else(PreparationError::invalid_output)?;
        let offset = [
            compact_shaping_coordinate(glyph.offset.x)
                .ok_or_else(PreparationError::invalid_output)?,
            compact_shaping_coordinate(glyph.offset.y)
                .ok_or_else(PreparationError::invalid_output)?,
        ];
        let placement =
            (block_advance != 0.0 || offset != [0.0, 0.0]).then_some(PreparedGlyphPlacement {
                glyph: 0,
                block_advance,
                offset,
            });
        Ok((
            Self {
                id: glyph.id,
                source: glyph.source,
                inline_advance,
            },
            placement,
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

/// Checked streaming construction of one canonical prepared paragraph.
///
/// Adapters write each line, interaction unit, run, and glyph directly into
/// the final flat tables. Construction does not create a nested line/run/glyph
/// graph and does not flatten or deep-validate that graph a second time.
#[derive(Debug)]
pub struct PreparedParagraphBuilder {
    paragraph: ParagraphId,
    text_len: u32,
    previous_end: u32,
    failed: bool,
    facts: PreparedParagraphFacts,
}

/// Exact common-table capacity hint for a prepared paragraph.
///
/// The hint changes allocation shape only; builders still validate the number
/// and content of every record actually written. Exceptional split-paint,
/// multi-slice, source-order, placement, and unrendered-source tables allocate
/// only when such records occur.
#[derive(Clone, Copy, Debug, Default)]
pub struct PreparedParagraphCapacity {
    lines: usize,
    runs: usize,
    glyphs: usize,
    interaction_units: usize,
    normalized_coords: usize,
}

impl PreparedParagraphCapacity {
    /// Describes the common line, run, glyph, and interaction-unit tables.
    #[must_use]
    pub const fn new(lines: usize, runs: usize, glyphs: usize, interaction_units: usize) -> Self {
        Self {
            lines,
            runs,
            glyphs,
            interaction_units,
            normalized_coords: 0,
        }
    }

    /// Adds the exact normalized-coordinate table capacity.
    #[must_use]
    pub const fn with_normalized_coords(mut self, normalized_coords: usize) -> Self {
        self.normalized_coords = normalized_coords;
        self
    }
}

impl PreparedParagraphBuilder {
    /// Starts a builder for the default editable capability set.
    #[must_use]
    pub fn new(
        paragraph: ParagraphId,
        text_len: u32,
        resolved_direction: ResolvedDirection,
    ) -> Self {
        Self::with_features(
            paragraph,
            text_len,
            resolved_direction,
            SceneFeatures::EDITABLE,
        )
    }

    /// Starts a builder for an exact normalized capability set.
    #[must_use]
    pub fn with_features(
        paragraph: ParagraphId,
        text_len: u32,
        resolved_direction: ResolvedDirection,
        features: SceneFeatures,
    ) -> Self {
        Self {
            paragraph,
            text_len,
            previous_end: 0,
            failed: false,
            facts: PreparedParagraphFacts::empty(text_len, resolved_direction, features),
        }
    }

    /// Reserves the common final tables without constructing temporary values.
    pub fn reserve_exact(&mut self, capacity: PreparedParagraphCapacity) {
        self.facts.lines.reserve_exact(capacity.lines);
        self.facts.runs.reserve_exact(capacity.runs);
        self.facts.glyphs.reserve_exact(capacity.glyphs);
        self.facts
            .interaction_units
            .reserve_exact(capacity.interaction_units);
        self.facts
            .normalized_coords
            .reserve_exact(capacity.normalized_coords);
    }

    /// Begins one source-contiguous formed line.
    ///
    /// Dropping the returned builder without finishing it poisons this
    /// paragraph builder, so partial public adapter output cannot be frozen.
    pub fn begin_line(
        &mut self,
        line: PreparedLine,
    ) -> Result<PreparedLineBuilder<'_>, PreparationError> {
        if self.failed || line.source.start != self.previous_end || line.source.end > self.text_len
        {
            self.failed = true;
            return Err(PreparationError::invalid_output());
        }
        let units_start = self.facts.interaction_units.len();
        let runs_start = self.facts.runs.len();
        Ok(PreparedLineBuilder {
            paragraph: self,
            line: Some(line),
            units_start,
            runs_start,
            finished: false,
        })
    }

    /// Freezes the validated canonical tables.
    pub fn finish(self) -> Result<PreparedParagraph, PreparationError> {
        if self.failed || self.previous_end != self.text_len {
            return Err(PreparationError::invalid_output());
        }
        Ok(PreparedParagraph {
            paragraph: self.paragraph,
            facts: Arc::new(self.facts),
        })
    }
}

/// Streaming construction of one line inside a [`PreparedParagraphBuilder`].
#[derive(Debug)]
pub struct PreparedLineBuilder<'a> {
    paragraph: &'a mut PreparedParagraphBuilder,
    line: Option<PreparedLine>,
    units_start: usize,
    runs_start: usize,
    finished: bool,
}

impl PreparedLineBuilder<'_> {
    /// Appends one visual interaction unit and its shaping slices.
    ///
    /// The slices must be nonempty, source-complete, non-overlapping, and have
    /// the same summed advance as `unit`. The common one-slice case is encoded
    /// directly in the unit and retains no slice-table entry.
    pub fn push_unit(
        &mut self,
        unit: PreparedInteractionUnit,
        slices: impl IntoIterator<Item = PreparedInteractionSlice>,
    ) -> Result<(), PreparationError> {
        let result = self.try_push_unit(unit, slices.into_iter().map(Ok));
        if result.is_err() {
            self.paragraph.failed = true;
        }
        result
    }

    /// Validates and appends one interaction unit from raw source/advance
    /// slice pairs.
    ///
    /// This is the zero-temporary path for preparation backends whose shaping
    /// records have not already been converted to [`PreparedInteractionSlice`].
    pub fn push_unit_parts(
        &mut self,
        unit: PreparedInteractionUnit,
        slices: impl IntoIterator<Item = (Range<u32>, f64)>,
    ) -> Result<(), PreparationError> {
        let result = self.try_push_unit(
            unit,
            slices
                .into_iter()
                .map(|(source, advance)| PreparedInteractionSlice::try_new(source, advance)),
        );
        if result.is_err() {
            self.paragraph.failed = true;
        }
        result
    }

    fn try_push_unit(
        &mut self,
        unit: PreparedInteractionUnit,
        slices: impl IntoIterator<Item = Result<PreparedInteractionSlice, PreparationError>>,
    ) -> Result<(), PreparationError> {
        let source = unit.source();
        let expected_advance = unit.advance();
        let mut slices = slices.into_iter();
        let first = slices
            .next()
            .ok_or_else(PreparationError::invalid_output)??;
        let second = slices.next().transpose()?;
        if second.is_none() {
            let slice_source = first.source();
            let tolerance = f64::max(1.0, expected_advance.abs()) * 1.0e-6;
            if slice_source != source || (first.advance() - expected_advance).abs() > tolerance {
                return Err(PreparationError::invalid_output());
            }
            self.paragraph
                .facts
                .interaction_units
                .push(PreparedInteractionUnitRecord::try_from_unit(unit)?);
            return Ok(());
        }
        let unit_index = u32::try_from(self.paragraph.facts.interaction_units.len())
            .map_err(|_| PreparationError::invalid_output())?;

        let mut covered = 0_u32;
        let mut advance = 0.0;
        let spill_start = self.paragraph.facts.interaction_slices.len();
        let mut count = 0_usize;
        for slice in core::iter::once(Ok(first))
            .chain(second.map(Ok))
            .chain(slices)
        {
            let slice = slice?;
            let slice_source = slice.source();
            if slice_source.start < source.start || slice_source.end > source.end {
                return Err(PreparationError::invalid_output());
            }
            for previous in &self.paragraph.facts.interaction_slices[spill_start..] {
                let previous = previous.source();
                if slice_source.start < previous.end && previous.start < slice_source.end {
                    return Err(PreparationError::invalid_output());
                }
            }
            covered = covered
                .checked_add(slice_source.end - slice_source.start)
                .ok_or_else(PreparationError::invalid_output)?;
            advance += slice.advance();
            if !advance.is_finite() {
                return Err(PreparationError::invalid_output());
            }
            self.paragraph.facts.interaction_slices.push(slice);
            count += 1;
        }
        let tolerance = f64::max(1.0, expected_advance.abs()) * 1.0e-6;
        if covered != source.end - source.start || (advance - expected_advance).abs() > tolerance {
            return Err(PreparationError::invalid_output());
        }
        debug_assert!(count > 1, "the direct case returned before spill encoding");
        let spill_end = self.paragraph.facts.interaction_slices.len();
        let spill_range = TableRange::try_from_usize(spill_start..spill_end)?;
        self.paragraph
            .facts
            .interaction_slice_spills
            .push(PreparedInteractionSliceSpill {
                unit: unit_index,
                slices: spill_range.start..spill_range.end,
            });
        self.paragraph
            .facts
            .interaction_units
            .push(PreparedInteractionUnitRecord::try_from_unit(unit)?);
        Ok(())
    }

    /// Begins one visual run and streams its variable-size facts directly into
    /// the paragraph tables.
    pub fn begin_run(&mut self, run: PreparedRun) -> PreparedRunBuilder<'_> {
        let normalized_coords_start = self.paragraph.facts.normalized_coords.len();
        let unrendered_source_start = self.paragraph.facts.unrendered_source.len();
        let glyphs_start = self.paragraph.facts.glyphs.len();
        PreparedRunBuilder {
            paragraph: self.paragraph,
            run: Some(run),
            normalized_coords_start,
            unrendered_source_start,
            glyphs_start,
            finished: false,
        }
    }

    /// Finishes the line after proving exact run and interaction coverage.
    pub fn finish(mut self) -> Result<(), PreparationError> {
        let result = self.try_finish();
        self.finished = true;
        if result.is_err() {
            self.paragraph.failed = true;
        }
        result
    }

    fn try_finish(&mut self) -> Result<(), PreparationError> {
        let mut line = self
            .line
            .take()
            .ok_or_else(PreparationError::invalid_output)?;
        let unit_end = self.paragraph.facts.interaction_units.len();
        let run_end = self.paragraph.facts.runs.len();
        let units = &self.paragraph.facts.interaction_units[self.units_start..unit_end];
        let runs = &self.paragraph.facts.runs[self.runs_start..run_end];

        if line.source.is_empty() {
            if line.break_reason != LineBreakReason::End
                || line.advance != 0.0
                || !units.is_empty()
                || !runs.is_empty()
            {
                return Err(PreparationError::invalid_output());
            }
        } else {
            validate_run_coverage(line.source.clone(), runs)?;
        }

        let source_order_start = self.paragraph.facts.source_order.len();
        if !units
            .windows(2)
            .all(|pair| pair[0].source().end <= pair[1].source().start)
        {
            for index in 0..units.len() {
                self.paragraph
                    .facts
                    .source_order
                    .push(u32::try_from(index).map_err(|_| PreparationError::invalid_output())?);
            }
            self.paragraph.facts.source_order[source_order_start..]
                .sort_unstable_by_key(|&index| units[index as usize].source().start);
        }
        let source_order_end = self.paragraph.facts.source_order.len();
        let source_order = &self.paragraph.facts.source_order[source_order_start..source_order_end];
        let mut covered = line.source.start;
        for rank in 0..units.len() {
            let index = source_order.get(rank).map_or(rank, |&index| index as usize);
            let range = units[index].source();
            if range.start != covered || range.end > line.source.end {
                return Err(PreparationError::invalid_output());
            }
            covered = range.end;
        }
        if covered != line.source.end {
            return Err(PreparationError::invalid_output());
        }

        let mut trailing_start = line.source.end;
        let mut trailing_advance = 0.0;
        for rank in (0..units.len()).rev() {
            let index = source_order.get(rank).map_or(rank, |&index| index as usize);
            let unit = &units[index];
            if unit.source().end != trailing_start {
                return Err(PreparationError::invalid_output());
            }
            if unit.whitespace() == ClusterWhitespace::None {
                break;
            }
            trailing_advance += unit.advance();
            trailing_start = unit.source().start;
        }
        let unit_advance = units
            .iter()
            .map(PreparedInteractionUnitRecord::advance)
            .sum::<f64>();
        let tolerance = f64::max(1.0, line.advance.abs()) * 1.0e-6;
        if (unit_advance - line.advance).abs() > tolerance {
            return Err(PreparationError::invalid_output());
        }
        line.trailing_whitespace_start = trailing_start;
        line.trailing_whitespace_advance = trailing_advance;
        self.paragraph.facts.lines.push(PreparedLineRecord {
            slot: line.slot,
            source: line.source.clone(),
            break_reason: line.break_reason,
            advance: line.advance,
            trailing_whitespace_start: line.trailing_whitespace_start,
            trailing_whitespace_advance: line.trailing_whitespace_advance,
            baseline: line.baseline,
            height: line.height,
            content_ascent: line.content_ascent,
            content_descent: line.content_descent,
            units: TableRange::try_from_usize(self.units_start..unit_end)?,
            source_order: TableRange::try_from_usize(source_order_start..source_order_end)?,
            runs: TableRange::try_from_usize(self.runs_start..run_end)?,
        });
        self.paragraph.previous_end = line.source.end;
        Ok(())
    }
}

impl Drop for PreparedLineBuilder<'_> {
    fn drop(&mut self) {
        if !self.finished {
            self.paragraph.failed = true;
        }
    }
}

/// Streaming construction of one run inside a [`PreparedLineBuilder`].
#[derive(Debug)]
pub struct PreparedRunBuilder<'a> {
    paragraph: &'a mut PreparedParagraphBuilder,
    run: Option<PreparedRun>,
    normalized_coords_start: usize,
    unrendered_source_start: usize,
    glyphs_start: usize,
    finished: bool,
}

impl PreparedRunBuilder<'_> {
    /// Appends normalized variation coordinates in font-axis order.
    pub fn extend_normalized_coords(&mut self, coords: impl IntoIterator<Item = i16>) {
        self.paragraph.facts.normalized_coords.extend(coords);
    }

    /// Appends one source range that intentionally produces no glyph.
    pub fn push_unrendered_source(&mut self, source: Range<u32>) -> Result<(), PreparationError> {
        let run = self
            .run
            .as_ref()
            .ok_or_else(PreparationError::invalid_output)?;
        let previous = self.paragraph.facts.unrendered_source.last().filter(|_| {
            self.paragraph.facts.unrendered_source.len() > self.unrendered_source_start
        });
        if source.start < run.source.start
            || source.start >= source.end
            || source.end > run.source.end
            || previous.is_some_and(|previous| previous.end >= source.start)
        {
            self.paragraph.failed = true;
            return Err(PreparationError::invalid_output());
        }
        self.paragraph.facts.unrendered_source.push(source);
        Ok(())
    }

    /// Returns whether a glyph already written to this run covers `source`.
    #[must_use]
    pub fn renders(&self, source: Range<u32>) -> bool {
        self.paragraph.facts.glyphs[self.glyphs_start..]
            .iter()
            .any(|glyph| glyph.source.start <= source.start && glyph.source.end >= source.end)
    }

    /// Appends one checked shaped glyph.
    pub fn push_glyph(&mut self, glyph: PreparedGlyph) -> Result<(), PreparationError> {
        let result = self.try_push_glyph(glyph);
        if result.is_err() {
            self.paragraph.failed = true;
        }
        result
    }

    fn try_push_glyph(&mut self, glyph: PreparedGlyph) -> Result<(), PreparationError> {
        let run = self
            .run
            .as_ref()
            .ok_or_else(PreparationError::invalid_output)?;
        if glyph.source.start < run.source.start || glyph.source.end > run.source.end {
            self.paragraph.failed = true;
            return Err(PreparationError::invalid_output());
        }
        let glyph_index = u32::try_from(self.paragraph.facts.glyphs.len())
            .map_err(|_| PreparationError::invalid_output())?;
        let (glyph, placement, paint) = PreparedGlyphRecord::try_from_glyph(glyph)?;
        self.paragraph.facts.glyphs.push(glyph);
        if let Some(mut placement) = placement {
            placement.glyph = glyph_index;
            self.paragraph.facts.glyph_placements.push(placement);
        }
        if !paint.is_whole() {
            self.paragraph
                .facts
                .split_glyph_paints
                .push(PreparedSplitGlyphPaint {
                    glyph: glyph_index,
                    coverage: paint,
                });
        }
        Ok(())
    }

    /// Finishes the run after proving exceptional unrendered source does not
    /// overlap any emitted glyph.
    pub fn finish(mut self) -> Result<(), PreparationError> {
        let result = self.try_finish();
        self.finished = true;
        if result.is_err() {
            self.paragraph.failed = true;
        }
        result
    }

    fn try_finish(&mut self) -> Result<(), PreparationError> {
        let run = self
            .run
            .take()
            .ok_or_else(PreparationError::invalid_output)?;
        let unrendered_end = self.paragraph.facts.unrendered_source.len();
        let glyphs_end = self.paragraph.facts.glyphs.len();
        let unrendered =
            &self.paragraph.facts.unrendered_source[self.unrendered_source_start..unrendered_end];
        let glyphs = &self.paragraph.facts.glyphs[self.glyphs_start..glyphs_end];
        if unrendered.iter().any(|source| {
            glyphs
                .iter()
                .any(|glyph| glyph.source.start < source.end && glyph.source.end > source.start)
        }) {
            return Err(PreparationError::invalid_output());
        }
        self.paragraph.facts.runs.push(PreparedRunRecord {
            source: run.source,
            bidi_level: run.bidi_level,
            script: run.script,
            font: run.font,
            font_size: run.font_size,
            synthesis: run.synthesis,
            normalized_coords: TableRange::try_from_usize(
                self.normalized_coords_start..self.paragraph.facts.normalized_coords.len(),
            )?,
            unrendered_source: TableRange::try_from_usize(
                self.unrendered_source_start..unrendered_end,
            )?,
            glyphs: TableRange::try_from_usize(self.glyphs_start..glyphs_end)?,
        });
        Ok(())
    }
}

impl Drop for PreparedRunBuilder<'_> {
    fn drop(&mut self) {
        if !self.finished {
            self.paragraph.failed = true;
        }
    }
}

fn validate_run_coverage(
    source: Range<u32>,
    runs: &[PreparedRunRecord],
) -> Result<(), PreparationError> {
    let mut covered = source.start;
    let mut consumed = 0_usize;
    while covered < source.end {
        let mut matching = runs.iter().filter(|run| run.source.start == covered);
        let run = matching
            .next()
            .ok_or_else(PreparationError::invalid_output)?;
        if matching.next().is_some() || run.source.end > source.end {
            return Err(PreparationError::invalid_output());
        }
        covered = run.source.end;
        consumed = consumed.saturating_add(1);
    }
    (covered == source.end && consumed == runs.len())
        .then_some(())
        .ok_or_else(PreparationError::invalid_output)
}

impl PreparedParagraphFacts {
    fn empty(
        text_len: u32,
        resolved_direction: ResolvedDirection,
        features: SceneFeatures,
    ) -> Self {
        Self {
            text_len,
            resolved_direction,
            features,
            lines: Vec::new(),
            runs: Vec::new(),
            glyphs: Vec::new(),
            glyph_placements: Vec::new(),
            split_glyph_paints: Vec::new(),
            interaction_slices: Vec::new(),
            interaction_slice_spills: Vec::new(),
            interaction_units: Vec::new(),
            source_order: Vec::new(),
            normalized_coords: Vec::new(),
            unrendered_source: Vec::new(),
        }
    }

    pub(crate) const fn text_len(&self) -> u32 {
        self.text_len
    }

    pub(crate) const fn features(&self) -> SceneFeatures {
        self.features
    }

    pub(crate) fn lines(&self) -> PreparedLines<'_> {
        PreparedLines::new(self)
    }

    pub(crate) fn line_unit_table_range(&self, line: usize) -> Option<Range<usize>> {
        self.lines.get(line).map(|line| line.units.as_usize())
    }

    pub(crate) fn estimated_owned_bytes(&self) -> usize {
        let mut bytes = size_of::<Self>()
            .saturating_add(vec_bytes::<PreparedLineRecord>(self.lines.capacity()))
            .saturating_add(vec_bytes::<PreparedRunRecord>(self.runs.capacity()))
            .saturating_add(vec_bytes::<PreparedGlyphRecord>(self.glyphs.capacity()))
            .saturating_add(vec_bytes::<PreparedGlyphPlacement>(
                self.glyph_placements.capacity(),
            ))
            .saturating_add(vec_bytes::<PreparedSplitGlyphPaint>(
                self.split_glyph_paints.capacity(),
            ))
            .saturating_add(vec_bytes::<PreparedInteractionSlice>(
                self.interaction_slices.capacity(),
            ))
            .saturating_add(vec_bytes::<PreparedInteractionSliceSpill>(
                self.interaction_slice_spills.capacity(),
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
        let unit_base = record.units.start;
        let units = record.units.as_usize();
        PreparedInteractionUnits::new(
            &self.facts.interaction_units[units.clone()],
            unit_base,
            &self.facts.interaction_slices,
            &self.facts.interaction_slice_spills,
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

    fn placement(self) -> Option<&'a PreparedGlyphPlacement> {
        let index =
            u32::try_from(self.index).expect("canonical glyph indexes were validated as u32");
        self.facts
            .glyph_placements
            .binary_search_by_key(&index, |placement| placement.glyph)
            .ok()
            .map(|index| &self.facts.glyph_placements[index])
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
        Vec2::new(
            f64::from(self.record().inline_advance),
            self.placement()
                .map_or(0.0, |placement| f64::from(placement.block_advance)),
        )
    }

    /// Returns the shaped glyph offset.
    #[must_use]
    pub fn offset(self) -> Vec2 {
        self.placement().map_or(Vec2::ZERO, |placement| {
            Vec2::new(
                f64::from(placement.offset[0]),
                f64::from(placement.offset[1]),
            )
        })
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

/// Checked metadata for one shaped run with a single font instance and bidi level.
///
/// A [`PreparedRunBuilder`] streams coordinates, unrendered source, and glyphs
/// directly into the canonical paragraph tables.
#[derive(Clone, Debug)]
pub struct PreparedRun {
    source: Range<u32>,
    bidi_level: u8,
    script: [u8; 4],
    font: FontData,
    font_size: f32,
    synthesis: FontSynthesis,
}

impl PreparedRun {
    /// Validates and owns one shaped run.
    ///
    /// A run may later receive no glyphs when its source consists only of
    /// controls such as a mandatory line break. Its source range remains
    /// significant.
    pub fn try_new(
        source: Range<u32>,
        bidi_level: u8,
        script: [u8; 4],
        font: FontData,
        font_size: f32,
        synthesis: FontSynthesis,
    ) -> Result<Self, PreparationError> {
        if source.start >= source.end || !font_size.is_finite() || font_size <= 0.0 {
            return Err(PreparationError::invalid_output());
        }
        Ok(Self {
            source,
            bidi_level,
            script,
            font,
            font_size,
            synthesis,
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
