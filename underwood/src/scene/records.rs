// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Renderer-neutral lines, glyph fragments, semantics, and geometry results.
//!
//! This module owns immutable scene observations; it explicitly does not own
//! hit-testing policy or retained preparation.

use super::*;

/// One visual line.
#[derive(Clone, Debug)]
pub struct SceneLine<Source = SnapshotTextRange> {
    pub(super) bounds: Rect,
    pub(super) advance: f64,
    pub(super) sources: Vec<Source>,
    pub(super) fragments: Range<usize>,
    pub(super) break_reason: LineBreakReason,
    pub(super) baseline: f64,
    pub(super) content_ascent: f64,
    pub(super) content_descent: f64,
}

impl<Source> SceneLine<Source> {
    /// Returns scene-space line bounds.
    #[must_use]
    pub const fn bounds(&self) -> Rect {
        self.bounds
    }

    /// Returns the actual inline advance, including trailing whitespace.
    ///
    /// Unlike [`Self::bounds`], this value has no minimum hit-area padding.
    #[must_use]
    pub const fn advance(&self) -> f64 {
        self.advance
    }

    /// Returns the source-complete slices represented by the line.
    ///
    /// A line has multiple slices when it crosses semantic text leaves.
    #[must_use]
    pub fn sources(&self) -> &[Source] {
        &self.sources
    }

    /// Returns the contiguous scene-fragment range painted by this line.
    ///
    /// The range indexes [`TextScene::fragments`] or the corresponding
    /// projected scene fragment slice.
    #[must_use]
    pub fn fragment_range(&self) -> Range<usize> {
        self.fragments.clone()
    }

    /// Returns why this line ended.
    #[must_use]
    pub const fn break_reason(&self) -> LineBreakReason {
        self.break_reason
    }

    /// Returns the scene-space baseline.
    #[must_use]
    pub const fn baseline(&self) -> f64 {
        self.baseline
    }

    /// Returns the maximum font ascent contributing to this line.
    #[must_use]
    pub const fn content_ascent(&self) -> f64 {
        self.content_ascent
    }

    /// Returns the maximum font descent contributing to this line.
    #[must_use]
    pub const fn content_descent(&self) -> f64 {
        self.content_descent
    }
}

/// Opaque identity of a fragment within the current retained engine context.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SceneFragmentId(pub(super) u64);

/// Paint-homogeneous shaped glyph fragment.
#[derive(Clone, Debug)]
pub struct SceneFragment<Source = SnapshotTextRange> {
    pub(super) id: SceneFragmentId,
    pub(super) glyphs: Vec<SceneGlyph<Source>>,
    pub(super) paint: PaintSlot,
    pub(super) transform: Affine,
    pub(super) source: Source,
    pub(super) additional_sources: Vec<Source>,
    pub(super) paint_clip: Option<Rect>,
    pub(super) font: FontData,
    pub(super) font_size: f32,
    pub(super) synthesis: FontSynthesis,
    pub(super) normalized_coords: Arc<[i16]>,
    pub(super) bidi_level: u8,
    pub(super) script: [u8; 4],
}

impl<Source> SceneFragment<Source> {
    /// Returns the retained fragment identity.
    #[must_use]
    pub const fn id(&self) -> SceneFragmentId {
        self.id
    }

    /// Returns shaped glyph observations.
    #[must_use]
    pub fn glyphs(&self) -> &[SceneGlyph<Source>] {
        &self.glyphs
    }

    /// Returns the paint slot.
    #[must_use]
    pub const fn paint(&self) -> PaintSlot {
        self.paint
    }

    /// Returns the fragment transform.
    #[must_use]
    pub const fn transform(&self) -> Affine {
        self.transform
    }

    /// Returns the first source slice covered by this fragment when present.
    ///
    /// Use [`Self::sources`] for source-complete provenance because one glyph
    /// may cross semantic text leaves.
    #[must_use]
    pub const fn source(&self) -> Option<&Source> {
        Some(&self.source)
    }

    /// Iterates over every source slice covered by this fragment.
    pub fn sources(&self) -> impl DoubleEndedIterator<Item = &Source> {
        core::iter::once(&self.source).chain(self.additional_sources.iter())
    }

    /// Returns an explicit scene-space partial-paint clip when one is required.
    ///
    /// Ordinary whole-glyph fragments return `None` and must not be clipped to
    /// outline bounds. This is paint-partition geometry, not hit-test, layout,
    /// ink, damage, or viewport geometry.
    #[must_use]
    pub const fn paint_clip(&self) -> Option<Rect> {
        self.paint_clip
    }

    /// Returns the exact font bytes and face index for these glyphs.
    #[must_use]
    pub const fn font(&self) -> &FontData {
        &self.font
    }

    /// Returns the scene-unit font size used to shape and position these glyphs.
    #[must_use]
    pub const fn font_size(&self) -> f32 {
        self.font_size
    }

    /// Returns synthesis suggestions selected for this font instance.
    #[must_use]
    pub const fn synthesis(&self) -> &FontSynthesis {
        &self.synthesis
    }

    /// Returns normalized variation coordinates for the exact font instance.
    #[must_use]
    pub fn normalized_coords(&self) -> &[i16] {
        &self.normalized_coords
    }

    /// Returns the resolved Unicode bidi level for the shaped run.
    #[must_use]
    pub const fn bidi_level(&self) -> u8 {
        self.bidi_level
    }

    /// Returns the resolved ISO 15924 script tag for the shaped run.
    #[must_use]
    pub const fn script(&self) -> [u8; 4] {
        self.script
    }
}

/// One shaped glyph observation.
#[derive(Clone, Debug)]
pub struct SceneGlyph<Source = SnapshotTextRange> {
    pub(super) instance: SceneGlyphInstanceId,
    pub(super) id: u32,
    pub(super) position: Point,
    pub(super) advance: Vec2,
    pub(super) source: Source,
    pub(super) additional_sources: Vec<Source>,
}

impl<Source> SceneGlyph<Source> {
    /// Returns the identity of the shaped glyph instance being painted.
    ///
    /// Multiple observations share this identity when one shaped glyph is
    /// divided into partial-paint fragments.
    #[must_use]
    pub const fn instance_id(&self) -> SceneGlyphInstanceId {
        self.instance
    }

    /// Returns the backend glyph identifier.
    #[must_use]
    pub const fn id(&self) -> u32 {
        self.id
    }

    /// Returns the glyph origin in scene coordinates.
    #[must_use]
    pub const fn position(&self) -> Point {
        self.position
    }

    /// Returns the shaped advance.
    #[must_use]
    pub const fn advance(&self) -> Vec2 {
        self.advance
    }

    /// Returns the first source slice covered by this painted glyph observation.
    ///
    /// Use [`Self::sources`] for source-complete provenance because one shaped
    /// glyph may cross semantic text leaves.
    #[must_use]
    pub const fn source(&self) -> &Source {
        &self.source
    }

    /// Iterates over every source slice covered by this painted glyph observation.
    pub fn sources(&self) -> impl DoubleEndedIterator<Item = &Source> {
        core::iter::once(&self.source).chain(self.additional_sources.iter())
    }
}

/// Opaque identity of one shaped glyph instance in a prepared scene.
///
/// A glyph can occur in more than one paint fragment when style boundaries
/// divide its visible area. Those observations retain one shared identity.
/// Compare identities only within one [`TextScene`] or corresponding projected
/// scene; they are not stable across separate preparations.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SceneGlyphInstanceId(pub(super) usize);

/// Semantic observation with scene geometry.
#[derive(Clone, Debug)]
pub struct SemanticFragment {
    pub(super) semantic_id: SemanticId,
    pub(super) paragraph_role: Option<ParagraphRole>,
    pub(super) inline_role: Option<InlineRole>,
    pub(super) source: Option<SnapshotTextRange>,
    pub(super) bounds: Rect,
}

impl SemanticFragment {
    /// Returns the source semantic identity.
    #[must_use]
    pub const fn semantic_id(&self) -> SemanticId {
        self.semantic_id
    }

    /// Returns the paragraph role, or `None` for an inline semantic fragment.
    #[must_use]
    pub const fn paragraph_role(&self) -> Option<ParagraphRole> {
        self.paragraph_role
    }

    /// Returns the inline role, or `None` for a block-level semantic fragment.
    #[must_use]
    pub const fn inline_role(&self) -> Option<InlineRole> {
        self.inline_role
    }

    /// Returns snapshot-local source when present.
    #[must_use]
    pub const fn source(&self) -> Option<&SnapshotTextRange> {
        self.source.as_ref()
    }

    /// Returns scene-space semantic bounds.
    #[must_use]
    pub const fn bounds(&self) -> Rect {
        self.bounds
    }
}

/// Result of scene-space hit testing.
#[derive(Clone, Debug)]
pub struct TextHit<Source = SnapshotTextUnit, Position = SnapshotTextPosition> {
    pub(super) source: Source,
    pub(super) position: Position,
    pub(super) semantic_id: SemanticId,
    pub(super) bidi_level: u8,
}

impl<Source, Position> TextHit<Source, Position> {
    /// Returns the exact source-complete interaction unit under the point.
    #[must_use]
    pub const fn source(&self) -> &Source {
        &self.source
    }

    /// Returns the collapsed position selected by the interaction-unit side.
    #[must_use]
    pub const fn position(&self) -> &Position {
        &self.position
    }

    /// Returns the semantic text-node identity under the point.
    #[must_use]
    pub const fn semantic_id(&self) -> SemanticId {
        self.semantic_id
    }

    /// Returns the resolved bidi level of the hit interaction unit.
    #[must_use]
    pub const fn bidi_level(&self) -> u8 {
        self.bidi_level
    }
}

/// Exact scene-space caret for one snapshot position.
#[derive(Clone, Copy, Debug)]
pub struct SceneCaret<Position = SnapshotTextPosition> {
    pub(super) position: Position,
    pub(super) bounds: Rect,
}

impl<Position> SceneCaret<Position> {
    /// Returns the revision- or epoch-bound position represented by the caret.
    #[must_use]
    pub const fn position(&self) -> &Position {
        &self.position
    }

    /// Returns scene-space caret bounds.
    #[must_use]
    pub const fn bounds(&self) -> Rect {
        self.bounds
    }
}

/// One visual highlight rectangle owned by a selection and logical range.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneSelectionRect {
    pub(super) selection: usize,
    pub(super) range: usize,
    pub(super) line: usize,
    pub(super) bounds: Rect,
    pub(super) bidi_level: u8,
}

/// One visual highlight rectangle for selected generated preedit text.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneCompositionRect {
    pub(super) line: usize,
    pub(super) bounds: Rect,
    pub(super) bidi_level: u8,
}

impl SceneCompositionRect {
    /// Returns the visual line index within the transient scene.
    #[must_use]
    pub const fn line(self) -> usize {
        self.line
    }

    /// Returns scene-space highlight bounds.
    #[must_use]
    pub const fn bounds(self) -> Rect {
        self.bounds
    }

    /// Returns the bidi level of the covered visual run.
    #[must_use]
    pub const fn bidi_level(self) -> u8 {
        self.bidi_level
    }
}

impl SceneSelectionRect {
    /// Returns the selection index within the requested selection set.
    #[must_use]
    pub const fn selection(self) -> usize {
        self.selection
    }

    /// Returns the logical-range index within the owning selection.
    #[must_use]
    pub const fn range(self) -> usize {
        self.range
    }

    /// Returns the visual line index within the scene.
    #[must_use]
    pub const fn line(self) -> usize {
        self.line
    }

    /// Returns the scene-space highlight bounds.
    #[must_use]
    pub const fn bounds(self) -> Rect {
        self.bounds
    }

    /// Returns the bidi level of the covered visual run.
    #[must_use]
    pub const fn bidi_level(self) -> u8 {
        self.bidi_level
    }
}
