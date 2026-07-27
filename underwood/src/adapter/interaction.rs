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
#[derive(Clone, Debug)]
pub struct PreparedInteractionSlice {
    source: Range<u32>,
    advance: f64,
}

impl PreparedInteractionSlice {
    /// Validates one nonempty shaping-record source and its visual advance.
    pub fn try_new(source: Range<u32>, advance: f64) -> Result<Self, PreparationError> {
        if source.start >= source.end || !advance.is_finite() || advance < 0.0 {
            return Err(PreparationError::invalid_output());
        }
        Ok(Self { source, advance })
    }

    /// Returns the paragraph-local UTF-8 source range.
    #[must_use]
    pub fn source(&self) -> Range<u32> {
        self.source.clone()
    }

    /// Returns this slice's contribution to the unit's inline advance.
    #[must_use]
    pub const fn advance(&self) -> f64 {
        self.advance
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
            if slice.source.start < self.source.start || slice.source.end > self.source.end {
                return Err(PreparationError::invalid_output());
            }
            for previous in &slices[..index] {
                if slice.source.start < previous.source.end
                    && previous.source.start < slice.source.end
                {
                    return Err(PreparationError::invalid_output());
                }
            }
            covered = covered
                .checked_add(slice.source.end - slice.source.start)
                .ok_or_else(PreparationError::invalid_output)?;
            advance += slice.advance;
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

/// Borrowed interaction unit with access to its line-local shaping slices.
#[derive(Clone, Copy, Debug)]
pub struct PreparedInteractionUnitView<'a> {
    unit: &'a PreparedInteractionUnit,
    slices: &'a [PreparedInteractionSlice],
}

impl<'a> PreparedInteractionUnitView<'a> {
    /// Returns every shaping-record contribution in visual order.
    #[must_use]
    pub const fn slices(self) -> &'a [PreparedInteractionSlice] {
        self.slices
    }
}

impl core::ops::Deref for PreparedInteractionUnitView<'_> {
    type Target = PreparedInteractionUnit;

    fn deref(&self) -> &Self::Target {
        self.unit
    }
}

/// Allocation-free traversal of line-local interaction units.
#[derive(Clone, Debug)]
pub struct PreparedInteractionUnits<'a> {
    units: core::slice::Iter<'a, PreparedInteractionUnit>,
    slices: &'a [PreparedInteractionSlice],
}

impl<'a> PreparedInteractionUnits<'a> {
    pub(crate) fn new(
        units: &'a [PreparedInteractionUnit],
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

    fn view(&self, unit: &'a PreparedInteractionUnit) -> PreparedInteractionUnitView<'a> {
        PreparedInteractionUnitView {
            unit,
            slices: &self.slices[unit.slices.clone()],
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
