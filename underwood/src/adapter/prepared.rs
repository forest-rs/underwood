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
/// Checked metadata and table ranges for one source-complete formed line.
#[derive(Debug)]
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
    units: TableRange,
    source_order: TableRange,
    runs: TableRange,
}

impl PreparedLine {
    /// Validates metadata for one formed line.
    pub fn try_new(
        source: Range<u32>,
        break_reason: LineBreakReason,
        baseline: f64,
        height: f64,
        content_ascent: f64,
        content_descent: f64,
    ) -> Result<Self, PreparationError> {
        Self::try_new_in_slot(
            None,
            source,
            break_reason,
            baseline,
            height,
            content_ascent,
            content_descent,
        )
    }

    /// Validates metadata for one line accepted into an exact region slot.
    pub fn try_new_in_slot(
        slot: Option<crate::LineSlot>,
        source: Range<u32>,
        break_reason: LineBreakReason,
        baseline: f64,
        height: f64,
        content_ascent: f64,
        content_descent: f64,
    ) -> Result<Self, PreparationError> {
        if source.start > source.end
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
        let source_end = source.end;
        Ok(Self {
            slot,
            source,
            break_reason,
            advance: 0.0,
            trailing_whitespace_start: source_end,
            trailing_whitespace_advance: 0.0,
            baseline,
            height,
            content_ascent,
            content_descent,
            units: TableRange::EMPTY,
            source_order: TableRange::EMPTY,
            runs: TableRange::EMPTY,
        })
    }
}

#[derive(Debug)]
pub(crate) struct PreparedParagraphFacts {
    text_len: u32,
    resolved_direction: ResolvedDirection,
    features: SceneFeatures,
    lines: Vec<PreparedLine>,
    runs: Vec<PreparedRun>,
    glyphs: Vec<PreparedGlyphRecord>,
    glyph_placements: Vec<PreparedGlyphPlacement>,
    interaction_slices: Vec<PreparedInteractionSlice>,
    interaction_slice_spills: Vec<PreparedInteractionSliceSpill>,
    interaction_units: Vec<PreparedInteractionUnit>,
    source_order: Vec<u32>,
    normalized_coords: Vec<i16>,
    unrendered_source: Vec<Range<u32>>,
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
    ) -> Result<(Self, Option<PreparedGlyphPlacement>), PreparationError> {
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
        ))
    }
}

#[derive(Clone, Copy, Debug)]
struct TableRange {
    start: u32,
    end: u32,
}

impl TableRange {
    const EMPTY: Self = Self { start: 0, end: 0 };

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

    /// Checks flat adapter data and publishes one canonical paragraph artifact.
    pub fn try_from_data(
        paragraph: ParagraphId,
        text_len: u32,
        resolved_direction: ResolvedDirection,
        features: SceneFeatures,
        data: PreparedParagraphData,
    ) -> Result<Self, PreparationError> {
        data.validate_topology(text_len)?;
        let PreparedParagraphData {
            lines,
            runs,
            glyphs,
            glyph_placements,
            interaction_slices,
            interaction_slice_spills,
            interaction_units,
            source_order,
            normalized_coords,
            unrendered_source,
        } = data;
        Ok(Self {
            paragraph,
            facts: Arc::new(PreparedParagraphFacts {
                text_len,
                resolved_direction,
                features,
                lines,
                runs,
                glyphs,
                glyph_placements,
                interaction_slices,
                interaction_slice_spills,
                interaction_units,
                source_order,
                normalized_coords,
                unrendered_source,
            }),
        })
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
    pub fn lines(
        &self,
    ) -> impl DoubleEndedIterator<Item = PreparedLineView<'_>> + ExactSizeIterator + Clone + '_
    {
        self.facts.lines()
    }

    /// Returns one formed line by source-order index.
    #[must_use]
    pub fn line(&self, index: usize) -> Option<PreparedLineView<'_>> {
        self.facts.line(index)
    }

    /// Returns the number of formed lines.
    #[must_use]
    pub fn line_count(&self) -> usize {
        self.facts.lines.len()
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

/// Flat checked input tables for one prepared paragraph.
///
/// Adapters append scalar records to these tables, describe the contiguous
/// ranges belonging to each run and line, then cross the checked boundary at
/// [`PreparedParagraph::try_from_data`]. There is no nested builder state to
/// finish or poison.
#[derive(Debug, Default)]
pub struct PreparedParagraphData {
    lines: Vec<PreparedLine>,
    runs: Vec<PreparedRun>,
    glyphs: Vec<PreparedGlyphRecord>,
    glyph_placements: Vec<PreparedGlyphPlacement>,
    interaction_slices: Vec<PreparedInteractionSlice>,
    interaction_slice_spills: Vec<PreparedInteractionSliceSpill>,
    interaction_units: Vec<PreparedInteractionUnit>,
    source_order: Vec<u32>,
    normalized_coords: Vec<i16>,
    unrendered_source: Vec<Range<u32>>,
}

impl PreparedParagraphData {
    /// Creates empty prepared paragraph tables.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            lines: Vec::new(),
            runs: Vec::new(),
            glyphs: Vec::new(),
            glyph_placements: Vec::new(),
            interaction_slices: Vec::new(),
            interaction_slice_spills: Vec::new(),
            interaction_units: Vec::new(),
            source_order: Vec::new(),
            normalized_coords: Vec::new(),
            unrendered_source: Vec::new(),
        }
    }

    /// Creates tables with cheap estimates for their common records.
    #[must_use]
    pub fn with_capacity(
        lines: usize,
        runs: usize,
        glyphs: usize,
        units: usize,
        normalized_coords: usize,
    ) -> Self {
        Self {
            lines: Vec::with_capacity(lines),
            runs: Vec::with_capacity(runs),
            glyphs: Vec::with_capacity(glyphs),
            interaction_units: Vec::with_capacity(units),
            normalized_coords: Vec::with_capacity(normalized_coords),
            ..Self::new()
        }
    }

    /// Returns the number of appended lines.
    #[must_use]
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// Returns the number of appended runs.
    #[must_use]
    pub fn run_count(&self) -> usize {
        self.runs.len()
    }

    /// Returns the number of appended glyphs.
    #[must_use]
    pub fn glyph_count(&self) -> usize {
        self.glyphs.len()
    }

    /// Returns the number of appended interaction units.
    #[must_use]
    pub fn unit_count(&self) -> usize {
        self.interaction_units.len()
    }

    /// Returns the number of appended normalized coordinates.
    #[must_use]
    pub fn normalized_coord_count(&self) -> usize {
        self.normalized_coords.len()
    }

    /// Returns the number of appended unrendered source ranges.
    #[must_use]
    pub fn unrendered_source_count(&self) -> usize {
        self.unrendered_source.len()
    }

    /// Appends normalized variation coordinates in font-axis order.
    pub fn extend_normalized_coords(&mut self, coords: impl IntoIterator<Item = i16>) {
        self.normalized_coords.extend(coords);
    }

    /// Appends one nonempty source range that intentionally emits no glyph.
    pub fn push_unrendered_source(&mut self, source: Range<u32>) -> Result<(), PreparationError> {
        if source.start >= source.end {
            return Err(PreparationError::invalid_output());
        }
        self.unrendered_source.push(source);
        Ok(())
    }

    /// Returns whether one appended glyph range covers `source`.
    pub fn renders(
        &self,
        glyphs: Range<usize>,
        source: Range<u32>,
    ) -> Result<bool, PreparationError> {
        Ok(self
            .glyphs
            .get(glyphs)
            .ok_or_else(PreparationError::invalid_output)?
            .iter()
            .any(|glyph| glyph.source.start <= source.start && glyph.source.end >= source.end))
    }

    /// Compacts and appends one checked shaped glyph.
    pub fn push_glyph(&mut self, glyph: PreparedGlyph) -> Result<(), PreparationError> {
        let glyph_index =
            u32::try_from(self.glyphs.len()).map_err(|_| PreparationError::invalid_output())?;
        let (glyph, placement) = PreparedGlyphRecord::try_from_glyph(glyph)?;
        self.glyphs.push(glyph);
        if let Some(mut placement) = placement {
            placement.glyph = glyph_index;
            self.glyph_placements.push(placement);
        }
        Ok(())
    }

    /// Appends one visual interaction unit and its checked shaping slices.
    pub fn push_unit(
        &mut self,
        unit: PreparedInteractionUnit,
        slices: impl IntoIterator<Item = PreparedInteractionSlice>,
    ) -> Result<(), PreparationError> {
        self.push_unit_results(unit, slices.into_iter().map(Ok))
    }

    /// Appends one visual interaction unit from raw source/advance pairs.
    pub fn push_unit_parts(
        &mut self,
        unit: PreparedInteractionUnit,
        slices: impl IntoIterator<Item = (Range<u32>, f64)>,
    ) -> Result<(), PreparationError> {
        self.push_unit_results(
            unit,
            slices
                .into_iter()
                .map(|(source, advance)| PreparedInteractionSlice::try_new(source, advance)),
        )
    }

    fn push_unit_results(
        &mut self,
        unit: PreparedInteractionUnit,
        slices: impl IntoIterator<Item = Result<PreparedInteractionSlice, PreparationError>>,
    ) -> Result<(), PreparationError> {
        let slice_checkpoint = self.interaction_slices.len();
        let result = self.try_push_unit(unit, slices);
        if result.is_err() {
            self.interaction_slices.truncate(slice_checkpoint);
        }
        result
    }

    fn try_push_unit(
        &mut self,
        mut unit: PreparedInteractionUnit,
        slices: impl IntoIterator<Item = Result<PreparedInteractionSlice, PreparationError>>,
    ) -> Result<(), PreparationError> {
        let source = unit.source();
        let mut slices = slices.into_iter();
        let first = slices
            .next()
            .ok_or_else(PreparationError::invalid_output)??;
        let second = slices.next().transpose()?;
        if second.is_none() {
            if first.source() != source {
                return Err(PreparationError::invalid_output());
            }
            unit.bind_advance(first.advance())?;
            self.interaction_units.push(unit);
            return Ok(());
        }
        let unit_index = u32::try_from(self.interaction_units.len())
            .map_err(|_| PreparationError::invalid_output())?;
        let spill_start = self.interaction_slices.len();
        let mut covered = 0_u32;
        let mut advance = 0.0;
        for slice in core::iter::once(Ok(first))
            .chain(second.map(Ok))
            .chain(slices)
        {
            let slice = slice?;
            let slice_source = slice.source();
            if slice_source.start < source.start
                || slice_source.end > source.end
                || self.interaction_slices[spill_start..]
                    .iter()
                    .any(|previous| {
                        let previous = previous.source();
                        slice_source.start < previous.end && previous.start < slice_source.end
                    })
            {
                return Err(PreparationError::invalid_output());
            }
            covered = covered
                .checked_add(slice_source.end - slice_source.start)
                .ok_or_else(PreparationError::invalid_output)?;
            advance += slice.advance();
            if !advance.is_finite() {
                return Err(PreparationError::invalid_output());
            }
            self.interaction_slices.push(slice);
        }
        if covered != source.end - source.start {
            return Err(PreparationError::invalid_output());
        }
        unit.bind_advance(advance)?;
        let slices = TableRange::try_from_usize(spill_start..self.interaction_slices.len())?;
        self.interaction_slice_spills
            .push(PreparedInteractionSliceSpill {
                unit: unit_index,
                slices: slices.start..slices.end,
            });
        self.interaction_units.push(unit);
        Ok(())
    }

    /// Finishes one run by binding its previously appended table ranges.
    pub fn push_run(
        &mut self,
        mut run: PreparedRun,
        normalized_coords: Range<usize>,
        unrendered_source: Range<usize>,
        glyphs: Range<usize>,
    ) -> Result<(), PreparationError> {
        let normalized_coords =
            checked_table_range(normalized_coords, self.normalized_coords.len())?;
        let unrendered_source =
            checked_table_range(unrendered_source, self.unrendered_source.len())?;
        let glyphs = checked_table_range(glyphs, self.glyphs.len())?;
        let unrendered = &self.unrendered_source[unrendered_source.as_usize()];
        let glyph_records = &self.glyphs[glyphs.as_usize()];
        if glyph_records
            .iter()
            .any(|glyph| glyph.source.start < run.source.start || glyph.source.end > run.source.end)
            || unrendered.iter().enumerate().any(|(index, source)| {
                source.start < run.source.start
                    || source.end > run.source.end
                    || index > 0 && unrendered[index - 1].end >= source.start
                    || glyph_records.iter().any(|glyph| {
                        glyph.source.start < source.end && glyph.source.end > source.start
                    })
            })
        {
            return Err(PreparationError::invalid_output());
        }
        run.normalized_coords = normalized_coords;
        run.unrendered_source = unrendered_source;
        run.glyphs = glyphs;
        self.runs.push(run);
        Ok(())
    }

    /// Finishes one line by binding its previously appended unit and run ranges.
    ///
    /// Interaction units cover the complete source. Shaping runs may leave
    /// gaps for source that intentionally contributes no shaping record.
    pub fn push_line(
        &mut self,
        mut line: PreparedLine,
        units: Range<usize>,
        runs: Range<usize>,
    ) -> Result<(), PreparationError> {
        let units = checked_table_range(units, self.interaction_units.len())?;
        let runs = checked_table_range(runs, self.runs.len())?;
        let unit_records = &self.interaction_units[units.as_usize()];
        let run_records = &self.runs[runs.as_usize()];
        if line.source.is_empty() {
            if line.break_reason != LineBreakReason::End
                || line.advance != 0.0
                || !unit_records.is_empty()
                || !run_records.is_empty()
            {
                return Err(PreparationError::invalid_output());
            }
        } else {
            validate_run_sources(line.source.clone(), run_records)?;
        }

        let source_order_start = self.source_order.len();
        if !unit_records
            .windows(2)
            .all(|pair| pair[0].source().end <= pair[1].source().start)
        {
            let unit_count = u32::try_from(unit_records.len())
                .map_err(|_| PreparationError::invalid_output())?;
            self.source_order.extend(0..unit_count);
            self.source_order[source_order_start..]
                .sort_unstable_by_key(|&index| unit_records[index as usize].source().start);
        }
        let source_order =
            match TableRange::try_from_usize(source_order_start..self.source_order.len()) {
                Ok(source_order) => source_order,
                Err(error) => {
                    self.source_order.truncate(source_order_start);
                    return Err(error);
                }
            };
        let ordered = &self.source_order[source_order.as_usize()];
        let mut covered = line.source.start;
        for rank in 0..unit_records.len() {
            let index = ordered.get(rank).map_or(rank, |&index| index as usize);
            let range = unit_records[index].source();
            if range.start != covered || range.end > line.source.end {
                self.source_order.truncate(source_order_start);
                return Err(PreparationError::invalid_output());
            }
            covered = range.end;
        }
        if covered != line.source.end {
            self.source_order.truncate(source_order_start);
            return Err(PreparationError::invalid_output());
        }

        let mut trailing_start = line.source.end;
        let mut trailing_advance = 0.0;
        for rank in (0..unit_records.len()).rev() {
            let index = ordered.get(rank).map_or(rank, |&index| index as usize);
            let unit = &unit_records[index];
            if unit.source().end != trailing_start {
                self.source_order.truncate(source_order_start);
                return Err(PreparationError::invalid_output());
            }
            if unit.whitespace() == ClusterWhitespace::None {
                break;
            }
            trailing_advance += unit.retained_advance();
            trailing_start = unit.source().start;
        }
        line.advance = unit_records
            .iter()
            .map(PreparedInteractionUnit::retained_advance)
            .sum::<f64>();
        line.trailing_whitespace_start = trailing_start;
        line.trailing_whitespace_advance = trailing_advance;
        line.units = units;
        line.source_order = source_order;
        line.runs = runs;
        self.lines.push(line);
        Ok(())
    }

    fn validate_topology(&self, text_len: u32) -> Result<(), PreparationError> {
        let contiguous_lines = self.lines.iter().try_fold(0_u32, |previous_end, line| {
            (line.source.start == previous_end && line.source.end <= text_len)
                .then_some(line.source.end)
        }) == Some(text_len);
        let partitions = [
            table_partition(
                self.lines.iter().map(|line| line.units),
                self.interaction_units.len(),
            ),
            table_partition(self.lines.iter().map(|line| line.runs), self.runs.len()),
            table_partition(
                self.lines.iter().map(|line| line.source_order),
                self.source_order.len(),
            ),
            table_partition(
                self.runs.iter().map(|run| run.normalized_coords),
                self.normalized_coords.len(),
            ),
            table_partition(
                self.runs.iter().map(|run| run.unrendered_source),
                self.unrendered_source.len(),
            ),
            table_partition(self.runs.iter().map(|run| run.glyphs), self.glyphs.len()),
            table_partition(
                self.interaction_slice_spills
                    .iter()
                    .map(|spill| TableRange {
                        start: spill.slices.start,
                        end: spill.slices.end,
                    }),
                self.interaction_slices.len(),
            ),
        ];
        let spills_valid =
            self.interaction_slice_spills
                .iter()
                .enumerate()
                .all(|(index, spill)| {
                    usize::try_from(spill.unit)
                        .is_ok_and(|unit| unit < self.interaction_units.len())
                        && spill.slices.end - spill.slices.start > 1
                        && (index == 0
                            || self.interaction_slice_spills[index - 1].unit < spill.unit)
                });
        if !contiguous_lines || partitions.contains(&false) || !spills_valid {
            return Err(PreparationError::invalid_output());
        }
        Ok(())
    }
}

fn checked_table_range(range: Range<usize>, len: usize) -> Result<TableRange, PreparationError> {
    if range.start > range.end || range.end > len {
        return Err(PreparationError::invalid_output());
    }
    TableRange::try_from_usize(range)
}

fn table_partition(ranges: impl IntoIterator<Item = TableRange>, len: usize) -> bool {
    let mut end = 0;
    for range in ranges {
        if range.start as usize != end || range.end < range.start || range.end as usize > len {
            return false;
        }
        end = range.end as usize;
    }
    end == len
}

fn validate_run_sources(source: Range<u32>, runs: &[PreparedRun]) -> Result<(), PreparationError> {
    for (index, run) in runs.iter().enumerate() {
        if run.source.start < source.start
            || run.source.end > source.end
            || runs[..index].iter().any(|previous| {
                run.source.start < previous.source.end && previous.source.start < run.source.end
            })
        {
            return Err(PreparationError::invalid_output());
        }
    }
    Ok(())
}

impl PreparedParagraphFacts {
    pub(crate) const fn text_len(&self) -> u32 {
        self.text_len
    }

    pub(crate) const fn features(&self) -> SceneFeatures {
        self.features
    }

    pub(crate) fn lines(
        &self,
    ) -> impl DoubleEndedIterator<Item = PreparedLineView<'_>> + ExactSizeIterator + Clone + '_
    {
        (0..self.lines.len()).map(|index| PreparedLineView { facts: self, index })
    }

    pub(crate) fn line(&self, index: usize) -> Option<PreparedLineView<'_>> {
        (index < self.lines.len()).then_some(PreparedLineView { facts: self, index })
    }

    pub(crate) fn line_partition_point(
        &self,
        mut predicate: impl FnMut(PreparedLineView<'_>) -> bool,
    ) -> usize {
        let mut left = 0;
        let mut right = self.lines.len();
        while left < right {
            let middle = left + (right - left) / 2;
            if predicate(
                self.line(middle)
                    .expect("binary-search midpoint remains in the line table"),
            ) {
                left = middle + 1;
            } else {
                right = middle;
            }
        }
        left
    }

    pub(crate) fn line_unit_table_range(&self, line: usize) -> Option<Range<usize>> {
        self.lines.get(line).map(|line| line.units.as_usize())
    }

    pub(crate) fn estimated_owned_bytes(&self) -> usize {
        let mut bytes = size_of::<Self>()
            .saturating_add(vec_bytes::<PreparedLine>(self.lines.capacity()))
            .saturating_add(vec_bytes::<PreparedRun>(self.runs.capacity()))
            .saturating_add(vec_bytes::<PreparedGlyphRecord>(self.glyphs.capacity()))
            .saturating_add(vec_bytes::<PreparedGlyphPlacement>(
                self.glyph_placements.capacity(),
            ))
            .saturating_add(vec_bytes::<PreparedInteractionSlice>(
                self.interaction_slices.capacity(),
            ))
            .saturating_add(vec_bytes::<PreparedInteractionSliceSpill>(
                self.interaction_slice_spills.capacity(),
            ))
            .saturating_add(vec_bytes::<PreparedInteractionUnit>(
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
        bytes
    }
}

/// Borrowed view of one formed line in a canonical paragraph artifact.
#[derive(Clone, Copy, Debug)]
pub struct PreparedLineView<'a> {
    facts: &'a PreparedParagraphFacts,
    index: usize,
}

impl<'a> PreparedLineView<'a> {
    fn record(self) -> &'a PreparedLine {
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
    pub fn units(
        self,
    ) -> impl DoubleEndedIterator<Item = PreparedInteractionUnitView<'a>> + ExactSizeIterator + Clone + 'a
    {
        self.record().units.as_usize().map(move |index| {
            prepared_interaction_unit_view(
                &self.facts.interaction_units,
                &self.facts.interaction_slices,
                &self.facts.interaction_slice_spills,
                index,
            )
            .expect("validated line unit range indexes the interaction table")
        })
    }

    /// Returns one interaction unit by line-local visual index.
    #[must_use]
    pub fn unit(self, index: usize) -> Option<PreparedInteractionUnitView<'a>> {
        let global = self.record().units.as_usize().nth(index)?;
        prepared_interaction_unit_view(
            &self.facts.interaction_units,
            &self.facts.interaction_slices,
            &self.facts.interaction_slice_spills,
            global,
        )
    }

    /// Returns the number of interaction units in this line.
    #[must_use]
    pub fn unit_count(self) -> usize {
        self.record().units.as_usize().len()
    }

    pub(crate) fn unit_at_source_rank(
        self,
        rank: usize,
    ) -> Option<(usize, PreparedInteractionUnitView<'a>)> {
        let record = self.record();
        let source_order = &self.facts.source_order[record.source_order.as_usize()];
        let index = source_order.get(rank).map_or(rank, |&index| index as usize);
        self.unit(index).map(|unit| (index, unit))
    }

    /// Returns shaped runs in line-local visual order.
    #[must_use]
    pub fn runs(
        self,
    ) -> impl DoubleEndedIterator<Item = PreparedRunView<'a>> + ExactSizeIterator + Clone + 'a {
        self.record()
            .runs
            .as_usize()
            .map(move |index| PreparedRunView {
                facts: self.facts,
                index,
            })
    }

    /// Returns one shaped run by line-local visual index.
    #[must_use]
    pub fn run(self, index: usize) -> Option<PreparedRunView<'a>> {
        let index = self.record().runs.as_usize().nth(index)?;
        Some(PreparedRunView {
            facts: self.facts,
            index,
        })
    }

    /// Returns the number of runs.
    #[must_use]
    pub fn run_count(self) -> usize {
        self.record().runs.as_usize().len()
    }
}

/// Borrowed view of one shaped run in the canonical paragraph artifact.
#[derive(Clone, Copy, Debug)]
pub struct PreparedRunView<'a> {
    facts: &'a PreparedParagraphFacts,
    index: usize,
}

impl<'a> PreparedRunView<'a> {
    fn record(self) -> &'a PreparedRun {
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
    pub fn glyphs(
        self,
    ) -> impl DoubleEndedIterator<Item = PreparedGlyphView<'a>> + ExactSizeIterator + Clone + 'a
    {
        self.record()
            .glyphs
            .as_usize()
            .map(move |index| PreparedGlyphView {
                facts: self.facts,
                index,
            })
    }

    /// Returns one glyph by run-local visual index.
    #[must_use]
    pub fn glyph(self, index: usize) -> Option<PreparedGlyphView<'a>> {
        let index = self.record().glyphs.as_usize().nth(index)?;
        Some(PreparedGlyphView {
            facts: self.facts,
            index,
        })
    }

    /// Returns the number of glyphs in this run.
    #[must_use]
    pub fn glyph_count(self) -> usize {
        self.record().glyphs.as_usize().len()
    }
}

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
}

const fn vec_bytes<T>(capacity: usize) -> usize {
    size_of::<T>().saturating_mul(capacity)
}

/// Checked metadata and table ranges for one shaped font and bidi run.
#[derive(Debug)]
pub struct PreparedRun {
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
            normalized_coords: TableRange::EMPTY,
            unrendered_source: TableRange::EMPTY,
            glyphs: TableRange::EMPTY,
        })
    }
}

/// One shaped glyph with paragraph source.
#[derive(Clone, Debug)]
pub struct PreparedGlyph {
    id: u32,
    source: Range<u32>,
    advance: Vec2,
    offset: Vec2,
}

impl PreparedGlyph {
    /// Validates one shaped glyph.
    pub fn try_new(
        id: u32,
        source: Range<u32>,
        advance: Vec2,
        offset: Vec2,
    ) -> Result<Self, PreparationError> {
        if source.start >= source.end
            || !advance.x.is_finite()
            || !advance.y.is_finite()
            || !offset.x.is_finite()
            || !offset.y.is_finite()
        {
            return Err(PreparationError::invalid_output());
        }
        Ok(Self {
            id,
            source,
            advance,
            offset,
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
}
