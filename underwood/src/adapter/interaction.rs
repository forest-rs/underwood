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

/// One paragraph-local cursor step supplied by a formation backend.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreparedCursorStep {
    target: PreparedClusterSide,
    source: Option<Range<u32>>,
}

impl PreparedCursorStep {
    /// Creates a step and the complete interaction unit crossed by it, when any.
    #[must_use]
    pub const fn new(target: PreparedClusterSide, source: Option<Range<u32>>) -> Self {
        Self { target, source }
    }

    /// Returns the destination position.
    #[must_use]
    pub const fn target(&self) -> PreparedClusterSide {
        self.target
    }

    /// Returns the complete interaction unit crossed by this step.
    ///
    /// A transition across a soft wrap carries no source unit.
    #[must_use]
    pub fn source(&self) -> Option<Range<u32>> {
        self.source.clone()
    }
}

/// Paragraph-local caret placement for one cursor position.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PreparedCaret {
    line: u32,
    inline: f64,
}

impl PreparedCaret {
    /// Creates a caret placement in one prepared line.
    pub fn try_new(line: u32, inline: f64) -> Result<Self, PreparationError> {
        if !inline.is_finite() || inline < 0.0 {
            return Err(PreparationError::invalid_output());
        }
        Ok(Self { line, inline })
    }

    /// Returns the prepared line index.
    #[must_use]
    pub const fn line(self) -> u32 {
        self.line
    }

    /// Returns the inline-axis caret coordinate within the line.
    #[must_use]
    pub const fn inline(self) -> f64 {
        self.inline
    }
}

/// Paragraph-local cursor transitions supplied by a formation backend.
///
/// Underwood maps these positions into semantic snapshot positions without
/// reconstructing bidi or soft-wrap cursor rules.
#[derive(Clone, Debug, PartialEq)]
pub struct PreparedCursorMovement {
    position: PreparedClusterSide,
    caret: PreparedCaret,
    previous_visual: Option<PreparedCursorStep>,
    next_visual: Option<PreparedCursorStep>,
    previous_logical: Option<PreparedCursorStep>,
    next_logical: Option<PreparedCursorStep>,
}

impl PreparedCursorMovement {
    /// Creates the movement facts for one paragraph-local position.
    #[must_use]
    pub const fn new(
        position: PreparedClusterSide,
        caret: PreparedCaret,
        previous_visual: Option<PreparedCursorStep>,
        next_visual: Option<PreparedCursorStep>,
        previous_logical: Option<PreparedCursorStep>,
        next_logical: Option<PreparedCursorStep>,
    ) -> Self {
        Self {
            position,
            caret,
            previous_visual,
            next_visual,
            previous_logical,
            next_logical,
        }
    }

    /// Returns the source position for these transitions.
    #[must_use]
    pub const fn position(&self) -> PreparedClusterSide {
        self.position
    }

    /// Returns the exact paragraph-local caret placement.
    #[must_use]
    pub const fn caret(&self) -> PreparedCaret {
        self.caret
    }

    /// Returns the preceding position in visual order.
    #[must_use]
    pub const fn previous_visual(&self) -> Option<&PreparedCursorStep> {
        self.previous_visual.as_ref()
    }

    /// Returns the following position in visual order.
    #[must_use]
    pub const fn next_visual(&self) -> Option<&PreparedCursorStep> {
        self.next_visual.as_ref()
    }

    /// Returns the preceding interaction-unit boundary in logical order.
    #[must_use]
    pub const fn previous_logical(&self) -> Option<&PreparedCursorStep> {
        self.previous_logical.as_ref()
    }

    /// Returns the following interaction-unit boundary in logical order.
    #[must_use]
    pub const fn next_logical(&self) -> Option<&PreparedCursorStep> {
        self.next_logical.as_ref()
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
    slices: Vec<PreparedInteractionSlice>,
    advance: f64,
    bidi_level: u8,
    boundary: ClusterBoundary,
    whitespace: ClusterWhitespace,
    western_justification_opportunity: bool,
    left: PreparedClusterSide,
    right: PreparedClusterSide,
}

impl PreparedInteractionUnit {
    pub(crate) fn slice_capacity(&self) -> usize {
        self.slices.capacity()
    }

    /// Validates one source-complete interaction unit and its visual slices.
    pub fn try_new(
        source: Range<u32>,
        slices: impl IntoIterator<Item = PreparedInteractionSlice>,
        bidi_level: u8,
        boundary: ClusterBoundary,
        whitespace: ClusterWhitespace,
        left: PreparedClusterSide,
        right: PreparedClusterSide,
    ) -> Result<Self, PreparationError> {
        Self::try_new_with_justification(
            source, slices, bidi_level, boundary, whitespace, false, left, right,
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
        slices: impl IntoIterator<Item = PreparedInteractionSlice>,
        bidi_level: u8,
        boundary: ClusterBoundary,
        whitespace: ClusterWhitespace,
        western_justification_opportunity: bool,
        left: PreparedClusterSide,
        right: PreparedClusterSide,
    ) -> Result<Self, PreparationError> {
        let slices: Vec<_> = slices.into_iter().collect();
        if source.start >= source.end
            || !matches!(left.offset, offset if offset == source.start || offset == source.end)
            || !matches!(right.offset, offset if offset == source.start || offset == source.end)
            || left.offset == right.offset
            || western_justification_opportunity && whitespace != ClusterWhitespace::Space
        {
            return Err(PreparationError::invalid_output());
        }
        let mut coverage: Vec<_> = slices.iter().map(|slice| slice.source.clone()).collect();
        coverage.sort_unstable_by_key(|range| range.start);
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
        let advance = slices.iter().try_fold(0.0, |total, slice| {
            let total = total + slice.advance;
            total.is_finite().then_some(total)
        });
        let Some(advance) = advance else {
            return Err(PreparationError::invalid_output());
        };
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

    /// Returns the paragraph-local UTF-8 source range.
    #[must_use]
    pub fn source(&self) -> Range<u32> {
        self.source.clone()
    }

    /// Returns every shaping-record contribution in visual order.
    #[must_use]
    pub fn slices(&self) -> &[PreparedInteractionSlice] {
        &self.slices
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
