// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Prepared cursor, caret, cluster, and line-boundary facts.
//!
//! This module owns backend-produced interaction records; it explicitly does
//! not own document selections or scene-space hit testing.

use super::*;

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

/// Unicode boundary fact attached to one prepared interaction unit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClusterBoundary {
    /// The unit does not begin a word or line-break opportunity.
    None,
    /// The unit begins a word.
    Word,
    /// The unit begins a possible line break.
    Line,
    /// The unit carries a mandatory line break.
    Mandatory,
}

/// Whitespace classification attached to one prepared interaction unit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClusterWhitespace {
    /// The unit is not whitespace with special cursor behavior.
    None,
    /// The unit represents U+0020 SPACE.
    Space,
    /// The unit represents U+00A0 NO-BREAK SPACE.
    NoBreakSpace,
    /// The unit represents a horizontal tab.
    Tab,
    /// The unit represents a mandatory line break.
    Newline,
}

/// One logical position reached from a visual side of an interaction unit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PreparedClusterSide {
    offset: u32,
    affinity: TextAffinity,
}

impl PreparedClusterSide {
    /// Creates a paragraph-local interaction-side position.
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

/// One shaping-record contribution within a prepared interaction unit.
///
/// Slices remain in line-local visual order. Their canonical source union is
/// validated by [`PreparedInteractionUnit`], so zero-advance marks and
/// unrendered controls remain source-complete without becoming caret stops.
#[derive(Clone, Copy, Debug)]
pub struct PreparedInteractionSlice {
    source_start: u32,
    source_end: u32,
    advance: f32,
}

impl PreparedInteractionSlice {
    /// Validates one nonempty shaping-record source and its visual advance.
    pub fn try_new(source: Range<u32>, advance: f64) -> Result<Self, PreparationError> {
        let compact_advance = compact_shaping_coordinate(advance);
        if source.start >= source.end
            || !advance.is_finite()
            || advance < 0.0
            || compact_advance.is_none()
        {
            return Err(PreparationError::invalid_output());
        }
        Ok(Self {
            source_start: source.start,
            source_end: source.end,
            advance: compact_advance.expect("the compact advance was validated"),
        })
    }

    /// Returns the paragraph-local UTF-8 source range.
    #[must_use]
    pub const fn source(self) -> Range<u32> {
        self.source_start..self.source_end
    }

    /// Returns this slice's contribution to the unit's inline advance.
    #[must_use]
    pub fn advance(self) -> f64 {
        f64::from(self.advance)
    }
}

/// One analysis-derived extended grapheme in line-local visual order.
///
/// The paragraph adapter supplies every shaping slice and both endpoint sides
/// so the scene layer never reconstructs Unicode or bidi behavior from glyph
/// order. Internal shaping-record and semantic-leaf boundaries are not caret
/// positions.
#[derive(Clone, Debug)]
pub struct PreparedInteractionUnit {
    source: Range<u32>,
    slices: Range<usize>,
    advance: f64,
    bidi_level: u8,
    boundary: ClusterBoundary,
    whitespace: ClusterWhitespace,
    western_justification_opportunity: bool,
    left: PreparedClusterSide,
    right: PreparedClusterSide,
}

impl PreparedInteractionUnit {
    /// Validates one interaction-unit record over a line-local slice table.
    ///
    /// `slices` must name a nonempty contiguous range in the slice table later
    /// supplied to [`crate::adapter::PreparedLine`]. Line construction checks
    /// exact canonical source coverage and computes the visual advance.
    pub fn try_new(
        source: Range<u32>,
        slices: Range<usize>,
        advance: f64,
        bidi_level: u8,
        boundary: ClusterBoundary,
        whitespace: ClusterWhitespace,
        left: PreparedClusterSide,
        right: PreparedClusterSide,
    ) -> Result<Self, PreparationError> {
        Self::try_new_with_justification(
            source, slices, advance, bidi_level, boundary, whitespace, false, left, right,
        )
    }

    /// Validates one interaction unit and its Western justification eligibility.
    ///
    /// `western_justification_opportunity` must only be set for an ordinary
    /// space whose script context is supported by the backend's explicit
    /// Western inter-word strategy.
    #[expect(
        clippy::too_many_arguments,
        reason = "mirrors complete portable interaction data"
    )]
    pub fn try_new_with_justification(
        source: Range<u32>,
        slices: Range<usize>,
        advance: f64,
        bidi_level: u8,
        boundary: ClusterBoundary,
        whitespace: ClusterWhitespace,
        western_justification_opportunity: bool,
        left: PreparedClusterSide,
        right: PreparedClusterSide,
    ) -> Result<Self, PreparationError> {
        if source.start >= source.end
            || slices.start >= slices.end
            || !advance.is_finite()
            || advance < 0.0
            || !matches!(left.offset, offset if offset == source.start || offset == source.end)
            || !matches!(right.offset, offset if offset == source.start || offset == source.end)
            || left.offset == right.offset
            || western_justification_opportunity && whitespace != ClusterWhitespace::Space
        {
            return Err(PreparationError::invalid_output());
        }
        Ok(Self {
            source,
            slices,
            advance,
            bidi_level,
            boundary,
            whitespace,
            western_justification_opportunity,
            left,
            right,
        })
    }

    pub(crate) fn validate_slices(
        &self,
        table: &[PreparedInteractionSlice],
    ) -> Result<(), PreparationError> {
        let slices = table
            .get(self.slices.clone())
            .ok_or_else(PreparationError::invalid_output)?;
        let mut covered = 0_u32;
        let mut advance = 0.0;
        for (index, slice) in slices.iter().enumerate() {
            let source = slice.source();
            if source.start < self.source.start || source.end > self.source.end {
                return Err(PreparationError::invalid_output());
            }
            for previous in &slices[..index] {
                let previous = previous.source();
                if source.start < previous.end && previous.start < source.end {
                    return Err(PreparationError::invalid_output());
                }
            }
            covered = covered
                .checked_add(source.end - source.start)
                .ok_or_else(PreparationError::invalid_output)?;
            advance += slice.advance();
            if !advance.is_finite() {
                return Err(PreparationError::invalid_output());
            }
        }
        if covered != self.source.end - self.source.start {
            return Err(PreparationError::invalid_output());
        }
        let tolerance = f64::max(1.0, self.advance.abs()) * 1.0e-6;
        ((advance - self.advance).abs() <= tolerance)
            .then_some(())
            .ok_or_else(PreparationError::invalid_output)
    }

    pub(crate) fn slice_range(&self) -> Range<usize> {
        self.slices.clone()
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

    /// Returns whether this ordinary space is an eligible Western inter-word
    /// justification opportunity.
    #[must_use]
    pub const fn is_western_justification_opportunity(&self) -> bool {
        self.western_justification_opportunity
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

const LEFT_IS_SOURCE_START: u8 = 1 << 0;
const LEFT_IS_UPSTREAM: u8 = 1 << 1;
const RIGHT_IS_UPSTREAM: u8 = 1 << 2;
const WESTERN_JUSTIFICATION: u8 = 1 << 3;
const DIRECT_SLICE: u8 = 1 << 4;

/// Compact canonical form of one validated interaction unit.
#[derive(Debug)]
pub(crate) struct PreparedInteractionUnitRecord {
    source: Range<u32>,
    slices: Range<u32>,
    advance: f32,
    bidi_level: u8,
    boundary: ClusterBoundary,
    whitespace: ClusterWhitespace,
    flags: u8,
}

impl PreparedInteractionUnitRecord {
    pub(crate) fn try_from_unit(
        unit: PreparedInteractionUnit,
        slices: Range<u32>,
        direct_slice: bool,
    ) -> Result<Self, PreparationError> {
        let advance = compact_shaping_coordinate(unit.advance)
            .ok_or_else(PreparationError::invalid_output)?;
        let mut flags = 0;
        if unit.left.offset == unit.source.start {
            flags |= LEFT_IS_SOURCE_START;
        }
        if unit.left.affinity == TextAffinity::Upstream {
            flags |= LEFT_IS_UPSTREAM;
        }
        if unit.right.affinity == TextAffinity::Upstream {
            flags |= RIGHT_IS_UPSTREAM;
        }
        if unit.western_justification_opportunity {
            flags |= WESTERN_JUSTIFICATION;
        }
        if direct_slice {
            flags |= DIRECT_SLICE;
        }
        Ok(Self {
            source: unit.source,
            slices,
            advance,
            bidi_level: unit.bidi_level,
            boundary: unit.boundary,
            whitespace: unit.whitespace,
            flags,
        })
    }

    fn source(&self) -> Range<u32> {
        self.source.clone()
    }

    fn slices(&self) -> Range<usize> {
        self.slices.start as usize..self.slices.end as usize
    }

    const fn has_direct_slice(&self) -> bool {
        self.flags & DIRECT_SLICE != 0
    }

    const fn direct_slice(&self) -> PreparedInteractionSlice {
        PreparedInteractionSlice {
            source_start: self.source.start,
            source_end: self.source.end,
            advance: self.advance,
        }
    }

    fn advance(&self) -> f64 {
        f64::from(self.advance)
    }

    const fn left(&self) -> PreparedClusterSide {
        PreparedClusterSide::new(
            if self.flags & LEFT_IS_SOURCE_START != 0 {
                self.source.start
            } else {
                self.source.end
            },
            if self.flags & LEFT_IS_UPSTREAM != 0 {
                TextAffinity::Upstream
            } else {
                TextAffinity::Downstream
            },
        )
    }

    const fn right(&self) -> PreparedClusterSide {
        PreparedClusterSide::new(
            if self.flags & LEFT_IS_SOURCE_START != 0 {
                self.source.end
            } else {
                self.source.start
            },
            if self.flags & RIGHT_IS_UPSTREAM != 0 {
                TextAffinity::Upstream
            } else {
                TextAffinity::Downstream
            },
        )
    }
}

/// Borrowed interaction unit with access to its line-local shaping slices.
#[derive(Clone, Copy, Debug)]
pub struct PreparedInteractionUnitView<'a> {
    unit: &'a PreparedInteractionUnitRecord,
    slice_table: &'a [PreparedInteractionSlice],
}

impl<'a> PreparedInteractionUnitView<'a> {
    /// Returns every shaping-record contribution in visual order.
    #[must_use]
    pub fn slices(self) -> PreparedInteractionSlices<'a> {
        if self.unit.has_direct_slice() {
            PreparedInteractionSlices::direct(self.unit.direct_slice())
        } else {
            PreparedInteractionSlices::retained(&self.slice_table[self.unit.slices()])
        }
    }

    /// Returns the paragraph-local UTF-8 source range.
    #[must_use]
    pub fn source(self) -> Range<u32> {
        self.unit.source()
    }

    /// Returns the visual inline advance.
    #[must_use]
    pub fn advance(self) -> f64 {
        self.unit.advance()
    }

    /// Returns the resolved bidi level.
    #[must_use]
    pub const fn bidi_level(self) -> u8 {
        self.unit.bidi_level
    }

    /// Returns the Unicode boundary fact.
    #[must_use]
    pub const fn boundary(self) -> ClusterBoundary {
        self.unit.boundary
    }

    /// Returns the whitespace classification.
    #[must_use]
    pub const fn whitespace(self) -> ClusterWhitespace {
        self.unit.whitespace
    }

    /// Returns whether this unit is an eligible Western space.
    #[must_use]
    pub const fn is_western_justification_opportunity(self) -> bool {
        self.unit.flags & WESTERN_JUSTIFICATION != 0
    }

    /// Returns the position reached from the visual left side.
    #[must_use]
    pub const fn left(self) -> PreparedClusterSide {
        self.unit.left()
    }

    /// Returns the position reached from the visual right side.
    #[must_use]
    pub const fn right(self) -> PreparedClusterSide {
        self.unit.right()
    }
}

/// Allocation-free traversal of one interaction unit's shaping slices.
#[derive(Clone, Debug)]
pub struct PreparedInteractionSlices<'a> {
    direct: Option<PreparedInteractionSlice>,
    retained: core::slice::Iter<'a, PreparedInteractionSlice>,
}

impl<'a> PreparedInteractionSlices<'a> {
    fn direct(slice: PreparedInteractionSlice) -> Self {
        Self {
            direct: Some(slice),
            retained: [].iter(),
        }
    }

    fn retained(slices: &'a [PreparedInteractionSlice]) -> Self {
        Self {
            direct: None,
            retained: slices.iter(),
        }
    }

    /// Returns a fresh traversal over the same slices.
    #[must_use]
    pub fn iter(&self) -> Self {
        self.clone()
    }

    /// Returns the number of remaining slices.
    #[must_use]
    pub fn len(&self) -> usize {
        usize::from(self.direct.is_some()) + self.retained.len()
    }

    /// Returns whether no slice remains.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns a slice by traversal-local index.
    #[must_use]
    pub fn get(&self, index: usize) -> Option<PreparedInteractionSlice> {
        self.iter().nth(index)
    }

    /// Returns the first slice.
    #[must_use]
    pub fn first(&self) -> Option<PreparedInteractionSlice> {
        self.get(0)
    }
}

impl Iterator for PreparedInteractionSlices<'_> {
    type Item = PreparedInteractionSlice;

    fn next(&mut self) -> Option<Self::Item> {
        self.direct.take().or_else(|| self.retained.next().copied())
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}

impl DoubleEndedIterator for PreparedInteractionSlices<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.retained
            .next_back()
            .copied()
            .or_else(|| self.direct.take())
    }
}

impl ExactSizeIterator for PreparedInteractionSlices<'_> {}
impl core::iter::FusedIterator for PreparedInteractionSlices<'_> {}

/// Allocation-free traversal of line-local interaction units.
#[derive(Clone, Debug)]
pub struct PreparedInteractionUnits<'a> {
    units: core::slice::Iter<'a, PreparedInteractionUnitRecord>,
    slices: &'a [PreparedInteractionSlice],
}

impl<'a> PreparedInteractionUnits<'a> {
    pub(crate) fn new(
        units: &'a [PreparedInteractionUnitRecord],
        slices: &'a [PreparedInteractionSlice],
    ) -> Self {
        Self {
            units: units.iter(),
            slices,
        }
    }

    /// Returns another iterator over the remaining units.
    #[must_use]
    pub fn iter(&self) -> Self {
        self.clone()
    }

    /// Returns the number of remaining units.
    #[must_use]
    pub fn len(&self) -> usize {
        self.units.len()
    }

    /// Returns whether no units remain.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.units.len() == 0
    }

    fn view(&self, unit: &'a PreparedInteractionUnitRecord) -> PreparedInteractionUnitView<'a> {
        PreparedInteractionUnitView {
            unit,
            slice_table: self.slices,
        }
    }
}

impl<'a> Iterator for PreparedInteractionUnits<'a> {
    type Item = PreparedInteractionUnitView<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let unit = self.units.next()?;
        Some(self.view(unit))
    }

    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        let unit = self.units.nth(n)?;
        Some(self.view(unit))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.units.size_hint()
    }
}

impl<'a> DoubleEndedIterator for PreparedInteractionUnits<'a> {
    fn next_back(&mut self) -> Option<Self::Item> {
        let unit = self.units.next_back()?;
        Some(self.view(unit))
    }

    fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
        let unit = self.units.nth_back(n)?;
        Some(self.view(unit))
    }
}

impl ExactSizeIterator for PreparedInteractionUnits<'_> {}
impl core::iter::FusedIterator for PreparedInteractionUnits<'_> {}
