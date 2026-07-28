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
    pub(crate) fn try_new(source: Range<u32>, advance: f64) -> Result<Self, PreparationError> {
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
    advance: f32,
    bidi_level: u8,
    boundary: ClusterBoundary,
    whitespace: ClusterWhitespace,
    flags: u8,
}

impl PreparedInteractionUnit {
    /// Validates one interaction-unit record.
    ///
    /// The checked paragraph builder separately accepts this unit's shaping
    /// slices, proves their exact source coverage, and derives the visual
    /// advance before publishing the canonical artifact.
    pub fn try_new(
        source: Range<u32>,
        bidi_level: u8,
        boundary: ClusterBoundary,
        whitespace: ClusterWhitespace,
        left: PreparedClusterSide,
        right: PreparedClusterSide,
    ) -> Result<Self, PreparationError> {
        if source.start >= source.end
            || !matches!(left.offset, offset if offset == source.start || offset == source.end)
            || !matches!(right.offset, offset if offset == source.start || offset == source.end)
            || left.offset == right.offset
        {
            return Err(PreparationError::invalid_output());
        }
        let mut flags = 0;
        if left.offset == source.start {
            flags |= LEFT_IS_SOURCE_START;
        }
        if left.affinity == TextAffinity::Upstream {
            flags |= LEFT_IS_UPSTREAM;
        }
        if right.affinity == TextAffinity::Upstream {
            flags |= RIGHT_IS_UPSTREAM;
        }
        Ok(Self {
            source,
            advance: 0.0,
            bidi_level,
            boundary,
            whitespace,
            flags,
        })
    }

    /// Returns the paragraph-local UTF-8 source range.
    #[must_use]
    pub fn source(&self) -> Range<u32> {
        self.source.clone()
    }

    pub(crate) fn bind_advance(&mut self, advance: f64) -> Result<(), PreparationError> {
        self.advance =
            compact_shaping_coordinate(advance).ok_or_else(PreparationError::invalid_output)?;
        Ok(())
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

    /// Returns the position reached from the visual left side.
    #[must_use]
    pub const fn left(&self) -> PreparedClusterSide {
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

    /// Returns the position reached from the visual right side.
    #[must_use]
    pub const fn right(&self) -> PreparedClusterSide {
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

    const fn direct_slice(&self) -> PreparedInteractionSlice {
        PreparedInteractionSlice {
            source_start: self.source.start,
            source_end: self.source.end,
            advance: self.advance,
        }
    }

    pub(crate) fn retained_advance(&self) -> f64 {
        f64::from(self.advance)
    }
}

const LEFT_IS_SOURCE_START: u8 = 1 << 0;
const LEFT_IS_UPSTREAM: u8 = 1 << 1;
const RIGHT_IS_UPSTREAM: u8 = 1 << 2;

/// Exceptional retained shaping slices for one multi-slice interaction unit.
#[derive(Debug)]
pub(crate) struct PreparedInteractionSliceSpill {
    pub(crate) unit: u32,
    pub(crate) slices: Range<u32>,
}

/// Borrowed interaction unit with access to its line-local shaping slices.
#[derive(Clone, Copy, Debug)]
pub struct PreparedInteractionUnitView<'a> {
    unit: &'a PreparedInteractionUnit,
    spilled_slices: Option<&'a [PreparedInteractionSlice]>,
}

impl<'a> PreparedInteractionUnitView<'a> {
    /// Returns every shaping-record contribution in visual order.
    #[must_use]
    pub fn slices(
        self,
    ) -> impl DoubleEndedIterator<Item = PreparedInteractionSlice> + core::iter::FusedIterator + Clone + 'a
    {
        let (direct, retained) = match self.spilled_slices {
            Some(slices) => (None, slices),
            None => (Some(self.unit.direct_slice()), [].as_slice()),
        };
        direct.into_iter().chain(retained.iter().copied())
    }

    /// Returns the paragraph-local UTF-8 source range.
    #[must_use]
    pub fn source(self) -> Range<u32> {
        self.unit.source()
    }

    /// Returns the visual inline advance.
    #[must_use]
    pub fn advance(self) -> f64 {
        self.unit.retained_advance()
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

pub(crate) fn prepared_interaction_unit_view<'a>(
    units: &'a [PreparedInteractionUnit],
    slices: &'a [PreparedInteractionSlice],
    spills: &'a [PreparedInteractionSliceSpill],
    index: usize,
) -> Option<PreparedInteractionUnitView<'a>> {
    let unit = units.get(index)?;
    let global_index = u32::try_from(index).expect("validated interaction tables fit u32 indexes");
    let spilled_slices = (!spills.is_empty())
        .then(|| {
            spills
                .binary_search_by_key(&global_index, |spill| spill.unit)
                .ok()
        })
        .flatten()
        .map(|index| {
            let range = spills[index].slices.clone();
            &slices[range.start as usize..range.end as usize]
        });
    Some(PreparedInteractionUnitView {
        unit,
        spilled_slices,
    })
}
