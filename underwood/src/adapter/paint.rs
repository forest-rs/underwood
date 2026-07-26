// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Source-complete paint coverage for prepared glyphs.
//!
//! This module owns validation of glyph paint partitions; it explicitly does
//! not own brush values or renderer execution.

use super::*;

/// Complete source-ordered paint coverage for one glyph.
#[derive(Clone, Debug)]
pub struct GlyphPaintCoverage {
    segments: GlyphPaintSegments,
}

#[derive(Clone, Debug)]
enum GlyphPaintSegments {
    Whole(GlyphPaintSegment),
    Split(Box<[GlyphPaintSegment]>),
}

impl GlyphPaintCoverage {
    pub(crate) fn segment_capacity(&self) -> usize {
        match &self.segments {
            GlyphPaintSegments::Whole(_) => 0,
            GlyphPaintSegments::Split(segments) => segments.len(),
        }
    }

    /// Creates whole-glyph coverage with no renderer clip.
    pub fn whole(source: Range<u32>, slot: PaintSlot) -> Result<Self, PreparationError> {
        Ok(Self {
            segments: GlyphPaintSegments::Whole(GlyphPaintSegment::whole(source, slot)?),
        })
    }

    /// Validates non-empty, contiguous, source-ordered segments.
    ///
    /// One unclipped segment represents ordinary whole-glyph paint. Several
    /// segments require an explicit clip for every segment; mixing clipped and
    /// unclipped coverage would make the paint boundary ambiguous.
    pub fn try_from_segments(
        segments: impl IntoIterator<Item = GlyphPaintSegment>,
    ) -> Result<Self, PreparationError> {
        let segments: Vec<_> = segments.into_iter().collect();
        let clipped = segments
            .iter()
            .filter(|segment| segment.clip.is_some())
            .count();
        if segments.is_empty()
            || segments
                .windows(2)
                .any(|pair| pair[0].source.end != pair[1].source.start)
            || (clipped != 0 && clipped != segments.len())
            || (clipped == 0 && segments.len() != 1)
            || (clipped != 0 && segments.len() < 2)
        {
            return Err(PreparationError::unsupported_paint_coverage());
        }
        let segments = if clipped == 0 {
            let [segment] = segments
                .try_into()
                .map_err(|_| PreparationError::unsupported_paint_coverage())?;
            GlyphPaintSegments::Whole(segment)
        } else {
            GlyphPaintSegments::Split(segments.into_boxed_slice())
        };
        Ok(Self { segments })
    }

    /// Returns source-ordered coverage segments.
    #[must_use]
    pub fn segments(&self) -> &[GlyphPaintSegment] {
        match &self.segments {
            GlyphPaintSegments::Whole(segment) => core::slice::from_ref(segment),
            GlyphPaintSegments::Split(segments) => segments,
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
    /// Creates ordinary whole-glyph paint without a renderer clip.
    pub fn whole(source: Range<u32>, slot: PaintSlot) -> Result<Self, PreparationError> {
        Self::validate(source, slot, None)
    }

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
        Self::validate(source, slot, Some(clip))
    }

    fn validate(
        source: Range<u32>,
        slot: PaintSlot,
        clip: Option<Rect>,
    ) -> Result<Self, PreparationError> {
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
