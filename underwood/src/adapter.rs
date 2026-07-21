// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Pre-stable, backend-facing paragraph preparation contract.
//!
//! Successful outputs own every retained font, coordinate, and glyph record.
//! No backend-specific type crosses this boundary.

use alloc::vec::Vec;
use core::fmt;
use core::ops::Range;

use crate::{FontData, PaintSlot, ParagraphId, Rect, Vec2};

/// Prepares analyzed, itemized, and shaped data for one paragraph.
pub trait ParagraphPreparation {
    /// Produces validated, owned prepared data for `input`.
    fn prepare(
        &mut self,
        input: ParagraphInput<'_>,
    ) -> Result<ParagraphPreparationOutput, PreparationError>;
}

/// Borrowed projection of one semantic paragraph.
#[derive(Clone, Copy, Debug)]
pub struct ParagraphInput<'a> {
    paragraph: ParagraphId,
    text: &'a str,
    font_size: f32,
    paint_runs: &'a [PaintRun],
}

impl<'a> ParagraphInput<'a> {
    pub(crate) const fn new(
        paragraph: ParagraphId,
        text: &'a str,
        font_size: f32,
        paint_runs: &'a [PaintRun],
    ) -> Self {
        Self {
            paragraph,
            text,
            font_size,
            paint_runs,
        }
    }

    /// Returns the paragraph identity.
    #[must_use]
    pub const fn paragraph(&self) -> ParagraphId {
        self.paragraph
    }

    /// Returns the complete projected UTF-8 paragraph.
    #[must_use]
    pub const fn text(&self) -> &str {
        self.text
    }

    /// Returns the paragraph font size in scene units.
    #[must_use]
    pub const fn font_size(&self) -> f32 {
        self.font_size
    }

    /// Returns source-ordered paint metadata covering the paragraph.
    #[must_use]
    pub const fn paint_runs(&self) -> &[PaintRun] {
        self.paint_runs
    }
}

/// Paint slot over a paragraph-local UTF-8 byte range.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PaintRun {
    bytes: Range<u32>,
    slot: PaintSlot,
}

impl PaintRun {
    pub(crate) const fn new(bytes: Range<u32>, slot: PaintSlot) -> Self {
        Self { bytes, slot }
    }

    /// Returns the paragraph-local UTF-8 byte range.
    #[must_use]
    pub fn bytes(&self) -> Range<u32> {
        self.bytes.clone()
    }

    /// Returns the paint slot for this range.
    #[must_use]
    pub const fn slot(&self) -> PaintSlot {
        self.slot
    }
}

/// Owned paragraph data and exact work performed to produce it.
#[derive(Clone, Debug)]
pub struct ParagraphPreparationOutput {
    paragraph: PreparedParagraph,
    work: PreparationWork,
}

impl ParagraphPreparationOutput {
    /// Pairs validated prepared data with actual backend work.
    #[must_use]
    pub const fn new(paragraph: PreparedParagraph, work: PreparationWork) -> Self {
        Self { paragraph, work }
    }

    /// Returns the prepared paragraph.
    #[must_use]
    pub const fn paragraph(&self) -> &PreparedParagraph {
        &self.paragraph
    }

    /// Returns the work performed by the adapter.
    #[must_use]
    pub const fn work(&self) -> PreparationWork {
        self.work
    }

    pub(crate) fn into_paragraph(self) -> PreparedParagraph {
        self.paragraph
    }
}

/// Actual adapter work performed during one preparation call.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PreparationWork {
    analyzed: bool,
    itemized: bool,
    shaped_runs: u32,
    shaped_glyphs: u32,
}

impl PreparationWork {
    /// Creates a work record from backend observations.
    #[must_use]
    pub const fn new(analyzed: bool, itemized: bool, shaped_runs: u32, shaped_glyphs: u32) -> Self {
        Self {
            analyzed,
            itemized,
            shaped_runs,
            shaped_glyphs,
        }
    }

    /// Returns whether Unicode analysis ran.
    #[must_use]
    pub const fn analyzed(self) -> bool {
        self.analyzed
    }

    /// Returns whether itemization ran.
    #[must_use]
    pub const fn itemized(self) -> bool {
        self.itemized
    }

    /// Returns the number of shaped runs.
    #[must_use]
    pub const fn shaped_runs(self) -> u32 {
        self.shaped_runs
    }

    /// Returns the number of shaped glyphs.
    #[must_use]
    pub const fn shaped_glyphs(self) -> u32 {
        self.shaped_glyphs
    }
}

/// Validated owned preparation for one paragraph.
#[derive(Clone, Debug)]
pub struct PreparedParagraph {
    paragraph: ParagraphId,
    text_len: u32,
    runs: Vec<PreparedRun>,
}

impl PreparedParagraph {
    /// Validates and collects source-ordered shaped runs.
    pub fn try_from_runs(
        paragraph: ParagraphId,
        text_len: u32,
        runs: impl IntoIterator<Item = PreparedRun>,
    ) -> Result<Self, PreparationError> {
        let runs: Vec<_> = runs.into_iter().collect();
        let mut previous_end = 0;
        for run in &runs {
            if run.source.start != previous_end || run.source.end > text_len {
                return Err(PreparationError::invalid_output());
            }
            previous_end = run.source.end;
        }
        if previous_end != text_len {
            return Err(PreparationError::invalid_output());
        }
        Ok(Self {
            paragraph,
            text_len,
            runs,
        })
    }

    /// Returns the paragraph identity.
    #[must_use]
    pub const fn paragraph(&self) -> ParagraphId {
        self.paragraph
    }

    /// Returns the projected paragraph length in UTF-8 bytes.
    #[must_use]
    pub const fn text_len(&self) -> u32 {
        self.text_len
    }

    /// Returns the source-ordered prepared runs.
    #[must_use]
    pub fn runs(&self) -> &[PreparedRun] {
        &self.runs
    }
}

/// One shaped run with a single font instance and bidi level.
#[derive(Clone, Debug)]
pub struct PreparedRun {
    source: Range<u32>,
    bidi_level: u8,
    script: [u8; 4],
    font: FontData,
    font_size: f32,
    normalized_coords: Vec<i16>,
    glyphs: Vec<PreparedGlyph>,
}

impl PreparedRun {
    /// Validates and owns one shaped run.
    pub fn try_new(
        source: Range<u32>,
        bidi_level: u8,
        script: [u8; 4],
        font: FontData,
        font_size: f32,
        normalized_coords: impl IntoIterator<Item = i16>,
        glyphs: impl IntoIterator<Item = PreparedGlyph>,
    ) -> Result<Self, PreparationError> {
        if source.start >= source.end || !font_size.is_finite() || font_size <= 0.0 {
            return Err(PreparationError::invalid_output());
        }
        let glyphs: Vec<_> = glyphs.into_iter().collect();
        if glyphs.is_empty()
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
            normalized_coords: normalized_coords.into_iter().collect(),
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

    /// Returns normalized variation coordinates.
    #[must_use]
    pub fn normalized_coords(&self) -> &[i16] {
        &self.normalized_coords
    }

    /// Returns glyphs in backend-provided visual order.
    #[must_use]
    pub fn glyphs(&self) -> &[PreparedGlyph] {
        &self.glyphs
    }
}

/// One shaped glyph with paragraph source and paint coverage.
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
            || paint.segments.first().is_none_or(|segment| {
                segment.source.start != source.start
                    || paint
                        .segments
                        .last()
                        .is_none_or(|last| last.source.end != source.end)
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

    /// Returns source and local-clip paint coverage.
    #[must_use]
    pub const fn paint(&self) -> &GlyphPaintCoverage {
        &self.paint
    }
}

/// Complete source-ordered paint coverage for one glyph.
#[derive(Clone, Debug)]
pub struct GlyphPaintCoverage {
    segments: Vec<GlyphPaintSegment>,
}

impl GlyphPaintCoverage {
    /// Validates non-empty, contiguous, source-ordered segments.
    pub fn try_from_segments(
        segments: impl IntoIterator<Item = GlyphPaintSegment>,
    ) -> Result<Self, PreparationError> {
        let segments: Vec<_> = segments.into_iter().collect();
        if segments.is_empty()
            || segments
                .windows(2)
                .any(|pair| pair[0].source.end != pair[1].source.start)
        {
            return Err(PreparationError::unsupported_paint_coverage());
        }
        Ok(Self { segments })
    }

    /// Returns source-ordered coverage segments.
    #[must_use]
    pub fn segments(&self) -> &[GlyphPaintSegment] {
        &self.segments
    }
}

/// Paint and local clip for one source portion of a shaped glyph.
#[derive(Clone, Debug)]
pub struct GlyphPaintSegment {
    source: Range<u32>,
    slot: PaintSlot,
    local_clip: Rect,
}

impl GlyphPaintSegment {
    /// Creates a finite, non-empty coverage segment.
    pub fn new(
        source: Range<u32>,
        slot: PaintSlot,
        local_clip: Rect,
    ) -> Result<Self, PreparationError> {
        if source.start >= source.end
            || !local_clip.x0.is_finite()
            || !local_clip.y0.is_finite()
            || !local_clip.x1.is_finite()
            || !local_clip.y1.is_finite()
            || local_clip.width() < 0.0
            || local_clip.height() < 0.0
        {
            return Err(PreparationError::unsupported_paint_coverage());
        }
        Ok(Self {
            source,
            slot,
            local_clip,
        })
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

    /// Returns glyph-local clip geometry.
    #[must_use]
    pub const fn local_clip(&self) -> Rect {
        self.local_clip
    }
}

/// Stable category for adapter and prepared-output failures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum PreparationErrorKind {
    /// Required Unicode data or another capability is unavailable.
    MissingCapability,
    /// No usable font is available for the source.
    MissingFont,
    /// Faithful source-to-paint coverage cannot be represented.
    UnsupportedPaintCoverage,
    /// Adapter output violates the owned preparation contract.
    InvalidOutput,
    /// Work was cancelled before publication.
    Cancelled,
}

/// Concrete paragraph-preparation error.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct PreparationError {
    kind: PreparationErrorKind,
}

impl PreparationError {
    /// Creates an error for unavailable Unicode or shaping capabilities.
    #[must_use]
    pub const fn missing_capability() -> Self {
        Self {
            kind: PreparationErrorKind::MissingCapability,
        }
    }

    /// Creates an error for missing usable fonts.
    #[must_use]
    pub const fn missing_font() -> Self {
        Self {
            kind: PreparationErrorKind::MissingFont,
        }
    }

    /// Creates an error for paint coverage that cannot be represented faithfully.
    #[must_use]
    pub const fn unsupported_paint_coverage() -> Self {
        Self {
            kind: PreparationErrorKind::UnsupportedPaintCoverage,
        }
    }

    /// Creates an error for invalid backend output.
    #[must_use]
    pub const fn invalid_output() -> Self {
        Self {
            kind: PreparationErrorKind::InvalidOutput,
        }
    }

    /// Creates an error for cancelled work.
    #[must_use]
    pub const fn cancelled() -> Self {
        Self {
            kind: PreparationErrorKind::Cancelled,
        }
    }

    /// Returns the stable error category.
    #[must_use]
    pub const fn kind(&self) -> PreparationErrorKind {
        self.kind
    }
}

impl fmt::Display for PreparationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "paragraph preparation failed: {:?}", self.kind)
    }
}

impl core::error::Error for PreparationError {}

#[cfg(test)]
mod tests {
    use alloc::vec;

    use peniko::Blob;

    use super::{
        GlyphPaintCoverage, GlyphPaintSegment, PreparationErrorKind, PreparedGlyph,
        PreparedParagraph, PreparedRun,
    };
    use crate::{DocumentId, FontData, PaintSlot, ParagraphId, Rect, Vec2};

    #[test]
    fn prepared_paragraph_rejects_a_gap_between_runs() {
        let paragraph = ParagraphId {
            document: DocumentId::from_bytes(*b"adapter-test-001"),
            index: 0,
        };
        let first = run(0..1);
        let second = run(2..3);
        let error = PreparedParagraph::try_from_runs(paragraph, 3, [first, second])
            .expect_err("source gaps must be rejected at the adapter boundary");
        assert_eq!(
            error.kind(),
            PreparationErrorKind::InvalidOutput,
            "a source gap is invalid adapter output"
        );
    }

    fn run(source: core::ops::Range<u32>) -> PreparedRun {
        let coverage = GlyphPaintCoverage::try_from_segments([GlyphPaintSegment::new(
            source.clone(),
            PaintSlot::new(0),
            Rect::new(0., 0., 1., 1.),
        )
        .expect("test coverage is finite")])
        .expect("test coverage is contiguous");
        let glyph =
            PreparedGlyph::try_new(1, source.clone(), Vec2::new(1., 0.), Vec2::ZERO, coverage)
                .expect("test glyph is valid");
        PreparedRun::try_new(
            source,
            0,
            *b"Latn",
            FontData::new(Blob::from(vec![0_u8]), 0),
            16.,
            [],
            [glyph],
        )
        .expect("test run is internally valid")
    }
}
