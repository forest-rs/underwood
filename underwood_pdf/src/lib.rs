// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Exact, deliberately narrow PDF lowering for prepared Underwood text scenes.
//!
//! [`to_pdf`] consumes the same public [`TextScene`] that a screen renderer
//! consumes. Underwood remains responsible for shaping, fallback, bidi,
//! formation, and source ownership; this crate translates supported prepared
//! observations into one Krilla PDF page.
//!
//! The current adapter accepts solid sRGB paint and default variable-font
//! instances. Unsupported paint, synthetic emboldening or skew, and non-default
//! normalized variation coordinates fail explicitly before serialization.

use std::fmt::{Display, Formatter};
use std::ops::Range;

use krilla::Document;
use krilla::color::rgb;
use krilla::geom::{PathBuilder, Point as KrillaPoint, Rect as KrillaRect, Transform};
use krilla::num::NormalizedF32;
use krilla::page::PageSettings;
use krilla::paint::{Fill, FillRule};
use krilla::text::{Font, GlyphId, KrillaGlyph};
use underwood::{
    Affine, Brush, DocumentSnapshot, FontData, PaintSlot, Point, Rect, SceneFragmentId,
    SceneFragmentView, SceneGlyphInstanceId, SceneGlyphView, SceneSourceAccess, SnapshotTextRange,
    TextScene, Vec2,
};

/// Dimensions and scene origin for one exported PDF page.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PdfPage {
    width: f32,
    height: f32,
    origin: Point,
}

impl PdfPage {
    /// Creates a positive finite PDF page with a scene origin at `(0, 0)`.
    pub fn new(width: f32, height: f32) -> Result<Self, PdfError> {
        if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
            return Err(PdfError::new(PdfErrorKind::InvalidPage, None));
        }
        Ok(Self {
            width,
            height,
            origin: Point::ZERO,
        })
    }

    /// Places the Underwood scene at this finite page-space origin.
    pub fn with_origin(mut self, origin: Point) -> Result<Self, PdfError> {
        if !origin.x.is_finite() || !origin.y.is_finite() {
            return Err(PdfError::new(PdfErrorKind::InvalidPage, None));
        }
        self.origin = origin;
        Ok(self)
    }

    /// Returns the page width in PDF points.
    #[must_use]
    pub const fn width(self) -> f32 {
        self.width
    }

    /// Returns the page height in PDF points.
    #[must_use]
    pub const fn height(self) -> f32 {
        self.height
    }

    /// Returns the page-space origin applied to scene coordinates.
    #[must_use]
    pub const fn origin(self) -> Point {
        self.origin
    }
}

/// Stable category for a PDF lowering failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum PdfErrorKind {
    /// The requested page dimensions or scene origin are invalid.
    InvalidPage,
    /// The snapshot and prepared scene do not identify the same revision.
    WrongSnapshot,
    /// The scene was prepared without authored-source provenance.
    MissingSourceCapability,
    /// A scene source cannot be resolved to valid UTF-8 in the snapshot.
    InvalidSource,
    /// A scene paint is absent or is not a supported solid sRGB brush.
    UnsupportedPaint,
    /// A non-default variable-font instance cannot yet be represented exactly.
    UnsupportedVariation,
    /// Synthetic emboldening or skew cannot yet be represented exactly.
    UnsupportedSynthesis,
    /// Font bytes or the selected collection index are invalid for Krilla.
    InvalidFont,
    /// A finite Underwood coordinate cannot be represented by Krilla's `f32` geometry.
    CoordinateOutOfRange,
    /// Krilla rejected the finished document.
    Serialization,
}

/// Failure to lower a prepared scene without approximation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PdfError {
    kind: PdfErrorKind,
    fragment: Option<SceneFragmentId>,
}

impl PdfError {
    const fn new(kind: PdfErrorKind, fragment: Option<SceneFragmentId>) -> Self {
        Self { kind, fragment }
    }

    /// Returns the stable failure category.
    #[must_use]
    pub const fn kind(&self) -> PdfErrorKind {
        self.kind
    }

    /// Returns the affected prepared fragment, when the failure is fragment-local.
    #[must_use]
    pub const fn fragment(&self) -> Option<SceneFragmentId> {
        self.fragment
    }
}

impl Display for PdfError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        let message = match self.kind {
            PdfErrorKind::InvalidPage => "PDF page geometry is invalid",
            PdfErrorKind::WrongSnapshot => {
                "the document snapshot does not match the prepared text scene"
            }
            PdfErrorKind::MissingSourceCapability => {
                "the text scene was prepared without source provenance"
            }
            PdfErrorKind::InvalidSource => {
                "prepared glyph source is absent or not valid UTF-8 in the snapshot"
            }
            PdfErrorKind::UnsupportedPaint => "prepared paint is not a supported solid sRGB brush",
            PdfErrorKind::UnsupportedVariation => {
                "non-default normalized font coordinates are not yet supported by the PDF adapter"
            }
            PdfErrorKind::UnsupportedSynthesis => {
                "synthetic emboldening or skew is not yet supported by the PDF adapter"
            }
            PdfErrorKind::InvalidFont => "prepared font data is not valid for Krilla",
            PdfErrorKind::CoordinateOutOfRange => {
                "prepared scene geometry is outside Krilla's finite f32 range"
            }
            PdfErrorKind::Serialization => "Krilla failed to serialize the PDF document",
        };
        if let Some(fragment) = self.fragment {
            write!(formatter, "{message} in {fragment:?}")
        } else {
            formatter.write_str(message)
        }
    }
}

impl std::error::Error for PdfError {}

/// Lowers one prepared text scene into one PDF page.
///
/// The snapshot is required because a prepared scene intentionally carries
/// revision-bound source ranges rather than duplicate Unicode strings. The
/// adapter validates the complete scene before starting serialization, so an
/// unsupported fragment never produces a partial PDF.
pub fn to_pdf(
    scene: &TextScene,
    snapshot: &DocumentSnapshot,
    page: PdfPage,
) -> Result<Vec<u8>, PdfError> {
    validate_snapshot(scene, snapshot)?;
    let sources = scene
        .sources()
        .map_err(|_| PdfError::new(PdfErrorKind::MissingSourceCapability, None))?;
    let (lines, mut fonts) = prepare_scene(scene, sources, snapshot)?;

    let mut document = Document::new();
    let settings = PageSettings::from_wh(page.width, page.height)
        .ok_or_else(|| PdfError::new(PdfErrorKind::InvalidPage, None))?;
    let mut pdf_page = document.start_page_with(settings);
    let mut surface = pdf_page.surface();
    let origin = Transform::from_translate(
        finite_f32(page.origin.x, None)?,
        finite_f32(page.origin.y, None)?,
    );
    surface.push_transform(&origin);

    for line in &lines {
        for group in &line.groups {
            let font = cached_font(&mut fonts, &group.font, group.fragment)?;
            let fill = solid_fill(scene, group.paint, group.fragment)?;
            surface.set_fill(Some(fill));

            let clipped = if let Some(clip) = group.paint_clip {
                let path = clip_path(clip, group.fragment)?;
                surface.push_clip_path(&path, &FillRule::NonZero);
                true
            } else {
                false
            };

            let transform = krilla_transform(group.transform, group.fragment)?;
            surface.push_transform(&transform);
            for glyphs in group
                .glyphs
                .chunk_by(|first, second| first.text_carrier == second.text_carrier)
            {
                draw_prepared_glyphs(
                    &mut surface,
                    group.fragment,
                    glyphs,
                    group.font_size,
                    &line.text,
                    font.clone(),
                    !glyphs[0].text_carrier,
                )?;
            }
            surface.pop();
            if clipped {
                surface.pop();
            }
        }
    }

    surface.pop();
    surface.finish();
    pdf_page.finish();
    document
        .finish()
        .map_err(|_| PdfError::new(PdfErrorKind::Serialization, None))
}

struct PreparedLine {
    text: String,
    groups: Vec<PreparedGroup>,
}

struct PreparedGroup {
    fragment: SceneFragmentId,
    glyphs: Vec<PreparedGlyph>,
    paint: PaintSlot,
    transform: Affine,
    paint_clip: Option<Rect>,
    font: FontData,
    font_size: f32,
    bidi_level: u8,
}

struct PreparedGlyph {
    id: u32,
    position: Point,
    advance: Vec2,
    text_range: Range<usize>,
    text_carrier: bool,
}

struct LineSourceMap {
    text: String,
    segments: Vec<LineSourceSegment>,
}

struct LineSourceSegment {
    source: SnapshotTextRange,
    text_range: Range<usize>,
}

type FontCache = Vec<(FontData, Font)>;

fn prepare_scene(
    scene: &TextScene,
    sources: SceneSourceAccess<'_>,
    snapshot: &DocumentSnapshot,
) -> Result<(Vec<PreparedLine>, FontCache), PdfError> {
    let mut fonts = Vec::new();
    for fragment in scene.fragments() {
        validate_fragment(scene, sources, snapshot, fragment, &mut fonts)?;
    }

    let mut lines = Vec::with_capacity(scene.lines().len());
    for line in scene.lines() {
        let map = LineSourceMap::new(snapshot, sources.for_line(line))?;
        let mut groups: Vec<PreparedGroup> = Vec::new();
        let mut seen_instances: Vec<SceneGlyphInstanceId> = Vec::new();
        for fragment_index in line.fragment_range() {
            let fragment = scene
                .fragment(fragment_index)
                .ok_or_else(|| PdfError::new(PdfErrorKind::InvalidSource, None))?;
            if groups.last().is_none_or(|group| !group.matches(fragment)) {
                groups.push(PreparedGroup::new(fragment));
            }
            let group = groups.last_mut().expect("a group was just established");
            for glyph in fragment.glyphs() {
                let text_carrier = claim_text_carrier(&mut seen_instances, glyph.instance_id());
                group.glyphs.push(PreparedGlyph {
                    id: glyph.id(),
                    position: glyph.position(),
                    advance: glyph.advance(),
                    text_range: map.glyph_range(sources, glyph, fragment.id())?,
                    text_carrier,
                });
            }
        }
        for group in &mut groups {
            group.normalize_cluster_ranges();
        }
        lines.push(PreparedLine {
            text: map.text,
            groups,
        });
    }
    Ok((lines, fonts))
}

fn claim_text_carrier<Identity: Copy + Eq>(seen: &mut Vec<Identity>, identity: Identity) -> bool {
    if seen.contains(&identity) {
        false
    } else {
        seen.push(identity);
        true
    }
}

fn validate_fragment(
    scene: &TextScene,
    sources: SceneSourceAccess<'_>,
    snapshot: &DocumentSnapshot,
    fragment: SceneFragmentView<'_>,
    fonts: &mut FontCache,
) -> Result<(), PdfError> {
    if !fragment.font_size().is_finite() || fragment.font_size() <= 0.0 {
        return Err(PdfError::new(
            PdfErrorKind::CoordinateOutOfRange,
            Some(fragment.id()),
        ));
    }
    if fragment
        .normalized_coords()
        .iter()
        .any(|coordinate| *coordinate != 0)
    {
        return Err(PdfError::new(
            PdfErrorKind::UnsupportedVariation,
            Some(fragment.id()),
        ));
    }
    if fragment.synthesis().embolden() || fragment.synthesis().skew_degrees().is_some() {
        return Err(PdfError::new(
            PdfErrorKind::UnsupportedSynthesis,
            Some(fragment.id()),
        ));
    }
    let _ = solid_fill(scene, fragment.paint(), fragment.id())?;
    let _ = krilla_transform(fragment.transform(), fragment.id())?;
    if let Some(clip) = fragment.paint_clip() {
        let _ = clip_path(clip, fragment.id())?;
    }
    for glyph in fragment.glyphs() {
        let _ = glyph_text(sources, snapshot, glyph, fragment.id())?;
        let _ = finite_f32(glyph.position().x, Some(fragment.id()))?;
        let _ = finite_f32(glyph.position().y, Some(fragment.id()))?;
        let _ = finite_f32(glyph.advance().x, Some(fragment.id()))?;
        let _ = finite_f32(glyph.advance().y, Some(fragment.id()))?;
    }
    let _ = cached_font(fonts, fragment.font(), fragment.id())?;
    Ok(())
}

impl PreparedGroup {
    fn new(fragment: SceneFragmentView<'_>) -> Self {
        Self {
            fragment: fragment.id(),
            glyphs: Vec::new(),
            paint: fragment.paint(),
            transform: fragment.transform(),
            paint_clip: fragment.paint_clip(),
            font: fragment.font().clone(),
            font_size: fragment.font_size(),
            bidi_level: fragment.bidi_level(),
        }
    }

    fn matches(&self, fragment: SceneFragmentView<'_>) -> bool {
        self.paint == fragment.paint()
            && self.transform == fragment.transform()
            && self.paint_clip == fragment.paint_clip()
            && self.font == *fragment.font()
            && self.font_size == fragment.font_size()
            && self.bidi_level == fragment.bidi_level()
    }

    fn normalize_cluster_ranges(&mut self) {
        let mut start = 0;
        while start < self.glyphs.len() {
            let range = self.glyphs[start].text_range.clone();
            let mut end = start + 1;
            while end < self.glyphs.len() && self.glyphs[end].text_range == range {
                end += 1;
            }
            if end - start > 1 {
                if self.bidi_level & 1 == 1 {
                    for glyph in &mut self.glyphs[start..end - 1] {
                        glyph.text_range = range.end..range.end;
                    }
                } else {
                    for glyph in &mut self.glyphs[start + 1..end] {
                        glyph.text_range = range.end..range.end;
                    }
                }
            }
            start = end;
        }
    }
}

impl LineSourceMap {
    fn new(
        snapshot: &DocumentSnapshot,
        sources: impl ExactSizeIterator<Item = SnapshotTextRange>,
    ) -> Result<Self, PdfError> {
        let mut text = String::new();
        let mut segments = Vec::with_capacity(sources.len());
        for source in sources {
            let source_text = source_text(snapshot, &source, None)?;
            let start = text.len();
            text.push_str(source_text);
            segments.push(LineSourceSegment {
                source,
                text_range: start..text.len(),
            });
        }
        Ok(Self { text, segments })
    }

    fn glyph_range(
        &self,
        sources: SceneSourceAccess<'_>,
        glyph: SceneGlyphView<'_>,
        fragment: SceneFragmentId,
    ) -> Result<Range<usize>, PdfError> {
        let ranges: Vec<_> = sources
            .for_glyph(glyph)
            .map(|source| self.map_source(&source))
            .collect::<Option<_>>()
            .ok_or_else(|| PdfError::new(PdfErrorKind::InvalidSource, Some(fragment)))?;
        let Some(first) = ranges.first() else {
            return Err(PdfError::new(PdfErrorKind::InvalidSource, Some(fragment)));
        };
        let mut end = first.end;
        for range in ranges.iter().skip(1) {
            if range.start != end {
                return Err(PdfError::new(PdfErrorKind::InvalidSource, Some(fragment)));
            }
            end = range.end;
        }
        Ok(first.start..end)
    }

    fn map_source(&self, source: &SnapshotTextRange) -> Option<Range<usize>> {
        let segment = self.segments.iter().find(|segment| {
            segment.source.revision() == source.revision()
                && segment.source.text() == source.text()
                && segment.source.bytes().start <= source.bytes().start
                && segment.source.bytes().end >= source.bytes().end
        })?;
        let start = usize::try_from(
            source
                .bytes()
                .start
                .checked_sub(segment.source.bytes().start)?,
        )
        .ok()?;
        let end = usize::try_from(
            source
                .bytes()
                .end
                .checked_sub(segment.source.bytes().start)?,
        )
        .ok()?;
        Some(segment.text_range.start + start..segment.text_range.start + end)
    }
}

fn clip_path(clip: Rect, fragment: SceneFragmentId) -> Result<krilla::geom::Path, PdfError> {
    let rect = KrillaRect::from_ltrb(
        finite_f32(clip.x0, Some(fragment))?,
        finite_f32(clip.y0, Some(fragment))?,
        finite_f32(clip.x1, Some(fragment))?,
        finite_f32(clip.y1, Some(fragment))?,
    )
    .ok_or_else(|| PdfError::new(PdfErrorKind::CoordinateOutOfRange, Some(fragment)))?;
    let mut path = PathBuilder::new();
    path.push_rect(rect);
    path.finish()
        .ok_or_else(|| PdfError::new(PdfErrorKind::CoordinateOutOfRange, Some(fragment)))
}

fn draw_prepared_glyphs(
    surface: &mut krilla::surface::Surface<'_>,
    fragment: SceneFragmentId,
    prepared: &[PreparedGlyph],
    font_size: f32,
    text: &str,
    font: Font,
    outlined: bool,
) -> Result<(), PdfError> {
    let Some(first) = prepared.first() else {
        return Ok(());
    };
    let start_x = finite_f32(first.position.x, Some(fragment))?;
    let start_y = finite_f32(first.position.y, Some(fragment))?;
    let mut cursor_x = first.position.x;
    let mut cursor_y = first.position.y;
    let mut glyphs = Vec::with_capacity(prepared.len());
    for glyph in prepared {
        glyphs.push(KrillaGlyph::new(
            GlyphId::new(glyph.id),
            finite_f32(glyph.advance.x, Some(fragment))? / font_size,
            finite_f32(glyph.position.x - cursor_x, Some(fragment))? / font_size,
            finite_f32(cursor_y - glyph.position.y, Some(fragment))? / font_size,
            finite_f32(glyph.advance.y, Some(fragment))? / font_size,
            glyph.text_range.clone(),
            None,
        ));
        cursor_x += glyph.advance.x;
        cursor_y -= glyph.advance.y;
    }
    surface.draw_glyphs(
        KrillaPoint::from_xy(start_x, start_y),
        &glyphs,
        font,
        text,
        font_size,
        outlined,
    );
    Ok(())
}

fn solid_fill(
    scene: &TextScene,
    paint: PaintSlot,
    fragment: SceneFragmentId,
) -> Result<Fill, PdfError> {
    let brush = scene
        .paint()
        .brush(paint)
        .ok_or_else(|| PdfError::new(PdfErrorKind::UnsupportedPaint, Some(fragment)))?;
    let Brush::Solid(color) = brush else {
        return Err(PdfError::new(
            PdfErrorKind::UnsupportedPaint,
            Some(fragment),
        ));
    };
    let rgba = color.to_rgba8();
    Ok(Fill {
        paint: rgb::Color::new(rgba.r, rgba.g, rgba.b).into(),
        opacity: NormalizedF32::new(f32::from(rgba.a) / 255.0)
            .expect("an RGBA8 alpha is normalized"),
        rule: FillRule::NonZero,
    })
}

fn cached_font(
    fonts: &mut FontCache,
    font_data: &FontData,
    fragment: SceneFragmentId,
) -> Result<Font, PdfError> {
    if let Some((_, font)) = fonts.iter().find(|(candidate, _)| candidate == font_data) {
        return Ok(font.clone());
    }
    let (data, _) = font_data.data.clone().into_raw_parts();
    let font = Font::new(data.into(), font_data.index)
        .ok_or_else(|| PdfError::new(PdfErrorKind::InvalidFont, Some(fragment)))?;
    fonts.push((font_data.clone(), font.clone()));
    Ok(font)
}

fn glyph_text(
    sources: SceneSourceAccess<'_>,
    snapshot: &DocumentSnapshot,
    glyph: SceneGlyphView<'_>,
    fragment: SceneFragmentId,
) -> Result<String, PdfError> {
    let mut text = String::new();
    for source in sources.for_glyph(glyph) {
        text.push_str(source_text(snapshot, &source, Some(fragment))?);
    }
    if text.is_empty() {
        return Err(PdfError::new(PdfErrorKind::InvalidSource, Some(fragment)));
    }
    Ok(text)
}

fn source_text<'a>(
    snapshot: &'a DocumentSnapshot,
    source: &SnapshotTextRange,
    fragment: Option<SceneFragmentId>,
) -> Result<&'a str, PdfError> {
    if source.revision() != snapshot.revision() {
        return Err(PdfError::new(PdfErrorKind::InvalidSource, fragment));
    }
    let leaf = snapshot
        .text(source.text())
        .ok_or_else(|| PdfError::new(PdfErrorKind::InvalidSource, fragment))?;
    let bytes = source.bytes();
    let start = usize::try_from(bytes.start)
        .map_err(|_| PdfError::new(PdfErrorKind::InvalidSource, fragment))?;
    let end = usize::try_from(bytes.end)
        .map_err(|_| PdfError::new(PdfErrorKind::InvalidSource, fragment))?;
    leaf.get(start..end)
        .ok_or_else(|| PdfError::new(PdfErrorKind::InvalidSource, fragment))
}

fn krilla_transform(transform: Affine, fragment: SceneFragmentId) -> Result<Transform, PdfError> {
    let [a, b, c, d, e, f] = transform.as_coeffs();
    Ok(Transform::from_row(
        finite_f32(a, Some(fragment))?,
        finite_f32(b, Some(fragment))?,
        finite_f32(c, Some(fragment))?,
        finite_f32(d, Some(fragment))?,
        finite_f32(e, Some(fragment))?,
        finite_f32(f, Some(fragment))?,
    ))
}

fn validate_snapshot(scene: &TextScene, snapshot: &DocumentSnapshot) -> Result<(), PdfError> {
    if scene.document() != snapshot.id() || scene.revision() != snapshot.revision() {
        return Err(PdfError::new(PdfErrorKind::WrongSnapshot, None));
    }
    Ok(())
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "Krilla geometry is f32; bounds are checked before the narrowing conversion"
)]
fn finite_f32(value: f64, fragment: Option<SceneFragmentId>) -> Result<f32, PdfError> {
    if !value.is_finite() || value < f64::from(f32::MIN) || value > f64::from(f32::MAX) {
        return Err(PdfError::new(PdfErrorKind::CoordinateOutOfRange, fragment));
    }
    Ok(value as f32)
}

#[cfg(test)]
mod tests {
    use super::{PdfErrorKind, PdfPage, claim_text_carrier};
    use underwood::Point;

    #[test]
    fn page_geometry_is_validated_before_export() {
        assert_eq!(
            PdfPage::new(0.0, 100.0)
                .expect_err("zero-width page must fail")
                .kind(),
            PdfErrorKind::InvalidPage
        );
        assert_eq!(
            PdfPage::new(100.0, 100.0)
                .expect("positive page must pass")
                .with_origin(Point::new(f64::NAN, 0.0))
                .expect_err("non-finite origin must fail")
                .kind(),
            PdfErrorKind::InvalidPage
        );
    }

    #[test]
    fn only_first_partial_paint_observation_carries_text() {
        let mut seen = Vec::new();

        assert!(claim_text_carrier(&mut seen, 17_u8));
        assert!(!claim_text_carrier(&mut seen, 17_u8));
        assert!(claim_text_carrier(&mut seen, 18_u8));
    }
}
