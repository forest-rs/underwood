// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Exceptional split-paint coverage for prepared glyphs.
//!
//! Ordinary glyphs retain no paint metadata: core binds their source range to
//! the authoritative paragraph paint runs. This module owns only validation of
//! explicit clipped partitions for a glyph spanning paint boundaries; it
//! explicitly does not own brush values or renderer execution.

use super::*;

/// Complete source-ordered paint coverage for one glyph.
#[derive(Clone, Debug)]
pub struct GlyphPaintCoverage {
    segments: GlyphPaintSegments,
}

#[derive(Clone, Debug)]
enum GlyphPaintSegments {
    Whole,
    Split(Box<[GlyphPaintSegment]>),
}

impl GlyphPaintCoverage {
    /// Marks an ordinary glyph whose complete source has one core-owned paint.
    #[must_use]
    pub const fn whole() -> Self {
        Self {
            segments: GlyphPaintSegments::Whole,
        }
    }

    /// Validates a source-complete clipped split across paint boundaries.
    ///
    /// Ordinary glyphs must use [`Self::whole`]. A split requires at least two
    /// contiguous source segments, each with an explicit glyph-local clip.
    pub fn try_from_segments(
        segments: impl IntoIterator<Item = GlyphPaintSegment>,
    ) -> Result<Self, PreparationError> {
        let segments: Vec<_> = segments.into_iter().collect();
        if segments.len() < 2
            || segments
                .windows(2)
                .any(|pair| pair[0].source.end != pair[1].source.start)
        {
            return Err(PreparationError::unsupported_paint_coverage());
        }
        Ok(Self {
            segments: GlyphPaintSegments::Split(segments.into_boxed_slice()),
        })
    }

    /// Returns whether core should bind the complete glyph from its source.
    #[must_use]
    pub const fn is_whole(&self) -> bool {
        matches!(self.segments, GlyphPaintSegments::Whole)
    }

    /// Returns explicit source-ordered split segments, when present.
    #[must_use]
    pub fn split_segments(&self) -> Option<&[GlyphPaintSegment]> {
        match &self.segments {
            GlyphPaintSegments::Whole => None,
            GlyphPaintSegments::Split(segments) => Some(segments),
        }
    }
}

/// Paint ownership for one source portion of a shaped glyph.
#[derive(Clone, Debug)]
pub struct GlyphPaintSegment {
    source: Range<u32>,
    slot: PaintSlot,
    clip: Option<Rect>,
}

impl GlyphPaintSegment {
    /// Creates partial-glyph paint with explicit post-synthesis glyph-local clip geometry.
    ///
    /// The adapter must account for synthetic skew or emboldening when it derives this
    /// rectangle. Scene lowering translates the clip by the positioned glyph origin, and
    /// renderers must not apply [`FontSynthesis`] to the clip a second time.
    pub fn clipped(
        source: Range<u32>,
        slot: PaintSlot,
        clip: Rect,
    ) -> Result<Self, PreparationError> {
        let clip = Some(clip);
        if source.start >= source.end
            || clip.is_some_and(|clip| {
                !clip.x0.is_finite()
                    || !clip.y0.is_finite()
                    || !clip.x1.is_finite()
                    || !clip.y1.is_finite()
                    || clip.width() < 0.0
                    || clip.height() < 0.0
            })
        {
            return Err(PreparationError::unsupported_paint_coverage());
        }
        Ok(Self { source, slot, clip })
    }

    /// Returns the paragraph-local UTF-8 source range.
    #[must_use]
    pub fn source(&self) -> Range<u32> {
        self.source.clone()
    }

    /// Returns the segment paint slot.
    #[must_use]
    pub const fn slot(&self) -> PaintSlot {
        self.slot
    }

    /// Returns post-synthesis glyph-local partial-paint clip geometry when one is required.
    #[must_use]
    pub const fn clip(&self) -> Option<Rect> {
        self.clip
    }
}
