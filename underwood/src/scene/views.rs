// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Positioned public views over persistent paragraph-local scene records.

use super::*;

/// Allocation-free view of visual lines in one committed scene.
#[derive(Clone, Debug)]
pub struct SceneLines<'a> {
    revision: DocumentRevision,
    spine: &'a SceneSpine,
    segments: SpineSegments<'a>,
    current: Option<(PositionedSegment<'a>, usize)>,
    remaining: usize,
}

impl<'a> SceneLines<'a> {
    pub(super) fn new(revision: DocumentRevision, spine: &'a SceneSpine) -> Self {
        Self {
            revision,
            spine,
            segments: spine.segments(),
            current: None,
            remaining: spine.summary().lines,
        }
    }

    /// Returns a fresh iterator over every line.
    #[must_use]
    pub fn iter(&self) -> Self {
        Self::new(self.revision, self.spine)
    }

    /// Returns the number of visual lines.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.remaining
    }

    /// Returns whether the scene contains no visual line.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.remaining == 0
    }

    /// Returns a positioned line by global visual index.
    #[must_use]
    pub fn get(&self, index: usize) -> Option<SceneLineView<'a>> {
        self.spine
            .positioned_line(index)
            .map(|line| SceneLineView::new(self.revision, line))
    }

    /// Returns the first visual line.
    #[must_use]
    pub fn first(&self) -> Option<SceneLineView<'a>> {
        self.get(0)
    }

    /// Returns the final visual line.
    #[must_use]
    pub fn last(&self) -> Option<SceneLineView<'a>> {
        self.remaining
            .checked_sub(1)
            .and_then(|index| self.get(index))
    }
}

impl<'a> Iterator for SceneLines<'a> {
    type Item = SceneLineView<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some((positioned, local)) = &mut self.current
                && *local < positioned.segment.geometry.lines.len()
            {
                let line = PositionedLine {
                    position: *positioned,
                    local: *local,
                };
                *local += 1;
                self.remaining -= 1;
                return Some(SceneLineView::new(self.revision, line));
            }
            self.current = self.segments.next().map(|positioned| (positioned, 0));
            self.current?;
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

impl ExactSizeIterator for SceneLines<'_> {}

impl<'a> IntoIterator for &'a SceneLines<'a> {
    type Item = SceneLineView<'a>;
    type IntoIter = SceneLines<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// Allocation-free view of visual lines in one transient projected scene.
#[derive(Clone, Debug)]
pub struct ProjectedSceneLines<'a> {
    inner: SceneLines<'a>,
}

impl<'a> ProjectedSceneLines<'a> {
    pub(super) fn new(revision: DocumentRevision, spine: &'a SceneSpine) -> Self {
        Self {
            inner: SceneLines::new(revision, spine),
        }
    }

    /// Returns a fresh iterator over every line.
    #[must_use]
    pub fn iter(&self) -> Self {
        Self {
            inner: self.inner.iter(),
        }
    }

    /// Returns the number of visual lines.
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Returns whether the scene contains no visual line.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Returns a positioned line by global visual index.
    #[must_use]
    pub fn get(&self, index: usize) -> Option<ProjectedSceneLineView<'a>> {
        self.inner.get(index).map(ProjectedSceneLineView::new)
    }

    /// Returns the first visual line.
    #[must_use]
    pub fn first(&self) -> Option<ProjectedSceneLineView<'a>> {
        self.get(0)
    }

    /// Returns the final visual line.
    #[must_use]
    pub fn last(&self) -> Option<ProjectedSceneLineView<'a>> {
        self.len().checked_sub(1).and_then(|index| self.get(index))
    }
}

impl<'a> Iterator for ProjectedSceneLines<'a> {
    type Item = ProjectedSceneLineView<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(ProjectedSceneLineView::new)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl ExactSizeIterator for ProjectedSceneLines<'_> {}

/// One transient visual line with scene placement applied lazily.
#[derive(Clone, Copy, Debug)]
pub struct ProjectedSceneLineView<'a> {
    inner: SceneLineView<'a>,
}

impl<'a> ProjectedSceneLineView<'a> {
    fn new(inner: SceneLineView<'a>) -> Self {
        Self { inner }
    }

    /// Returns scene-space line bounds.
    #[must_use]
    pub fn bounds(self) -> Rect {
        self.inner.bounds()
    }

    /// Returns the actual inline advance, including trailing whitespace.
    #[must_use]
    pub fn advance(self) -> f64 {
        self.inner.advance()
    }

    /// Iterates authored and generated source slices represented by the line.
    pub(crate) fn sources(self) -> ProjectedSources<'a> {
        self.inner.projected_sources()
    }

    /// Returns the global scene-fragment range painted by this line.
    #[must_use]
    pub fn fragment_range(self) -> Range<usize> {
        self.inner.fragment_range()
    }

    /// Returns why this line ended.
    #[must_use]
    pub fn break_reason(self) -> LineBreakReason {
        self.inner.break_reason()
    }

    /// Returns the scene-space baseline.
    #[must_use]
    pub fn baseline(self) -> f64 {
        self.inner.baseline()
    }

    /// Returns the maximum font ascent contributing to this line.
    #[must_use]
    pub fn content_ascent(self) -> f64 {
        self.inner.content_ascent()
    }

    /// Returns the maximum font descent contributing to this line.
    #[must_use]
    pub fn content_descent(self) -> f64 {
        self.inner.content_descent()
    }

    /// Returns immutable post-formation placement and expansion evidence.
    #[must_use]
    pub fn adjustment(self) -> LineAdjustment {
        self.inner.adjustment()
    }
}

/// One visual line with scene origin, ordinals, and revision applied lazily.
#[derive(Clone, Copy, Debug)]
pub struct SceneLineView<'a> {
    revision: DocumentRevision,
    positioned: PositionedLine<'a>,
}

impl<'a> SceneLineView<'a> {
    fn new(revision: DocumentRevision, positioned: PositionedLine<'a>) -> Self {
        Self {
            revision,
            positioned,
        }
    }

    fn local(self) -> &'a CachedLine {
        &self.positioned.position.segment.geometry.lines[self.positioned.local]
    }

    fn prepared(self) -> PreparedLineView<'a> {
        self.positioned
            .position
            .segment
            .geometry
            .artifact
            .lines()
            .get(self.positioned.local)
            .expect("positioned line indexes the canonical artifact")
    }

    /// Returns scene-space line bounds.
    #[must_use]
    pub fn bounds(self) -> Rect {
        self.local().bounds + self.translate()
    }

    /// Returns the actual inline advance, including trailing whitespace.
    #[must_use]
    pub fn advance(self) -> f64 {
        self.prepared().advance()
            + self.local().adjustment.opportunity_expansion()
                * f64::from(
                    u32::try_from(self.local().adjustment.expanded_opportunities())
                        .expect("validated line adjustment opportunity count fits u32"),
                )
    }

    /// Iterates source-complete snapshot slices represented by the line.
    pub(crate) fn sources(self) -> SnapshotSources<'a> {
        let geometry = &self.positioned.position.segment.geometry;
        SnapshotSources::new(
            self.revision,
            geometry
                .source_map
                .as_ref()
                .expect("source-capable lines retain a paragraph source map"),
            SourceReference::Projected(self.prepared().source().into()),
        )
    }

    /// Returns the global scene-fragment range painted by this line.
    #[must_use]
    pub fn fragment_range(self) -> Range<usize> {
        let base = self.positioned.position.position.fragment_base;
        let fragments = &self.positioned.position.segment.paint.fragments;
        let line = u32::try_from(self.positioned.local)
            .expect("validated scene line index fits the retained fragment index");
        let start = fragments.partition_point(|fragment| fragment.line < line);
        let end = fragments.partition_point(|fragment| fragment.line <= line);
        base + start..base + end
    }

    /// Returns why this line ended.
    #[must_use]
    pub fn break_reason(self) -> LineBreakReason {
        self.prepared().break_reason()
    }

    /// Returns the scene-space baseline.
    #[must_use]
    pub fn baseline(self) -> f64 {
        self.local().bounds.y0
            + self.prepared().baseline()
            + self.positioned.position.position.block_origin
    }

    /// Returns the maximum font ascent contributing to this line.
    #[must_use]
    pub fn content_ascent(self) -> f64 {
        self.prepared().content_ascent()
    }

    /// Returns the maximum font descent contributing to this line.
    #[must_use]
    pub fn content_descent(self) -> f64 {
        self.prepared().content_descent()
    }

    /// Returns immutable post-formation placement and expansion evidence.
    #[must_use]
    pub fn adjustment(self) -> LineAdjustment {
        self.local().adjustment
    }

    fn translate(self) -> Vec2 {
        Vec2::new(0.0, self.positioned.position.position.block_origin)
    }

    fn projected_sources(self) -> ProjectedSources<'a> {
        let geometry = &self.positioned.position.segment.geometry;
        ProjectedSources::new(
            self.revision,
            geometry
                .source_map
                .as_ref()
                .expect("source-capable lines retain a paragraph source map"),
            SourceReference::Projected(self.prepared().source().into()),
        )
    }
}

/// Allocation-free view of paint-homogeneous fragments in one committed scene.
#[derive(Clone, Debug)]
pub struct SceneFragments<'a> {
    revision: DocumentRevision,
    spine: &'a SceneSpine,
    segments: SpineSegments<'a>,
    current: Option<(PositionedSegment<'a>, usize)>,
    remaining: usize,
}

impl<'a> SceneFragments<'a> {
    pub(super) fn new(revision: DocumentRevision, spine: &'a SceneSpine) -> Self {
        Self {
            revision,
            spine,
            segments: spine.segments(),
            current: None,
            remaining: spine.summary().fragments,
        }
    }

    /// Returns a fresh iterator over every fragment.
    #[must_use]
    pub fn iter(&self) -> Self {
        Self::new(self.revision, self.spine)
    }

    /// Returns the number of fragments.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.remaining
    }

    /// Returns whether the scene contains no painted fragment.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.remaining == 0
    }

    /// Returns a fragment by global visual index.
    #[must_use]
    pub fn get(&self, index: usize) -> Option<SceneFragmentView<'a>> {
        self.spine
            .positioned_fragment(index)
            .map(|fragment| SceneFragmentView::new(self.revision, fragment))
    }

    /// Returns the first fragment.
    #[must_use]
    pub fn first(&self) -> Option<SceneFragmentView<'a>> {
        self.get(0)
    }

    /// Returns the final fragment.
    #[must_use]
    pub fn last(&self) -> Option<SceneFragmentView<'a>> {
        self.remaining
            .checked_sub(1)
            .and_then(|index| self.get(index))
    }
}

impl<'a> Iterator for SceneFragments<'a> {
    type Item = SceneFragmentView<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some((positioned, local)) = &mut self.current
                && *local < positioned.segment.paint.fragments.len()
            {
                let fragment = PositionedFragment {
                    position: *positioned,
                    local: *local,
                };
                *local += 1;
                self.remaining -= 1;
                return Some(SceneFragmentView::new(self.revision, fragment));
            }
            self.current = self.segments.next().map(|positioned| (positioned, 0));
            self.current?;
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

impl ExactSizeIterator for SceneFragments<'_> {}

impl<'a> IntoIterator for &'a SceneFragments<'a> {
    type Item = SceneFragmentView<'a>;
    type IntoIter = SceneFragments<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// Allocation-free view of fragments in one transient projected scene.
#[derive(Clone, Debug)]
pub struct ProjectedSceneFragments<'a> {
    inner: SceneFragments<'a>,
}

impl<'a> ProjectedSceneFragments<'a> {
    pub(super) fn new(revision: DocumentRevision, spine: &'a SceneSpine) -> Self {
        Self {
            inner: SceneFragments::new(revision, spine),
        }
    }

    /// Returns a fresh iterator over every fragment.
    #[must_use]
    pub fn iter(&self) -> Self {
        Self {
            inner: self.inner.iter(),
        }
    }

    /// Returns the number of fragments.
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Returns whether the scene contains no painted fragment.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Returns a fragment by global visual index.
    #[must_use]
    pub fn get(&self, index: usize) -> Option<ProjectedSceneFragmentView<'a>> {
        self.inner.get(index).map(ProjectedSceneFragmentView::new)
    }

    /// Returns the first fragment.
    #[must_use]
    pub fn first(&self) -> Option<ProjectedSceneFragmentView<'a>> {
        self.get(0)
    }

    /// Returns the final fragment.
    #[must_use]
    pub fn last(&self) -> Option<ProjectedSceneFragmentView<'a>> {
        self.len().checked_sub(1).and_then(|index| self.get(index))
    }
}

impl<'a> Iterator for ProjectedSceneFragments<'a> {
    type Item = ProjectedSceneFragmentView<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(ProjectedSceneFragmentView::new)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl ExactSizeIterator for ProjectedSceneFragments<'_> {}

/// One transient paint fragment positioned in scene space.
#[derive(Clone, Copy, Debug)]
pub struct ProjectedSceneFragmentView<'a> {
    inner: SceneFragmentView<'a>,
}

impl<'a> ProjectedSceneFragmentView<'a> {
    fn new(inner: SceneFragmentView<'a>) -> Self {
        Self { inner }
    }

    /// Returns the retained fragment identity.
    #[must_use]
    pub fn id(self) -> SceneFragmentId {
        self.inner.id()
    }

    /// Returns positioned shaped glyph observations.
    #[must_use]
    pub fn glyphs(self) -> ProjectedSceneGlyphs<'a> {
        ProjectedSceneGlyphs {
            inner: self.inner.glyphs(),
        }
    }

    /// Returns the paint slot.
    #[must_use]
    pub fn paint(self) -> PaintSlot {
        self.inner.paint()
    }

    /// Returns the fragment transform.
    #[must_use]
    pub fn transform(self) -> Affine {
        self.inner.transform()
    }

    /// Returns the first authored or generated source slice.
    #[must_use]
    pub(crate) fn source(self) -> Option<ProjectedTextSource> {
        self.sources().next()
    }

    /// Iterates every authored and generated source slice.
    pub(crate) fn sources(self) -> ProjectedSources<'a> {
        let geometry = &self.inner.positioned.position.segment.geometry;
        ProjectedSources::from_fragment(
            self.inner.revision,
            geometry
                .source_map
                .as_ref()
                .expect("source-capable fragments retain a paragraph source map"),
            self.inner,
        )
    }

    /// Returns an explicit scene-space partial-paint clip.
    #[must_use]
    pub fn paint_clip(self) -> Option<Rect> {
        self.inner.paint_clip()
    }

    /// Returns exact font bytes and face index.
    #[must_use]
    pub fn font(self) -> &'a FontData {
        self.inner.font()
    }

    /// Returns the scene-unit font size.
    #[must_use]
    pub fn font_size(self) -> f32 {
        self.inner.font_size()
    }

    /// Returns synthesis suggestions.
    #[must_use]
    pub fn synthesis(self) -> &'a FontSynthesis {
        self.inner.synthesis()
    }

    /// Returns normalized variation coordinates.
    #[must_use]
    pub fn normalized_coords(self) -> &'a [i16] {
        self.inner.normalized_coords()
    }

    /// Returns the resolved Unicode bidi level.
    #[must_use]
    pub fn bidi_level(self) -> u8 {
        self.inner.bidi_level()
    }

    /// Returns the resolved ISO 15924 script tag.
    #[must_use]
    pub fn script(self) -> [u8; 4] {
        self.inner.script()
    }
}

/// One paint-homogeneous fragment positioned in scene space.
#[derive(Clone, Copy, Debug)]
pub struct SceneFragmentView<'a> {
    revision: DocumentRevision,
    positioned: PositionedFragment<'a>,
}

impl<'a> SceneFragmentView<'a> {
    fn new(revision: DocumentRevision, positioned: PositionedFragment<'a>) -> Self {
        Self {
            revision,
            positioned,
        }
    }

    fn local(self) -> &'a CachedFragment {
        &self.positioned.position.segment.paint.fragments[self.positioned.local]
    }

    fn prepared_line(self) -> PreparedLineView<'a> {
        let geometry = &self.positioned.position.segment.geometry;
        geometry
            .artifact
            .lines()
            .get(self.local().line as usize)
            .expect("paint fragment indexes the canonical line table")
    }

    fn cached_line(self) -> &'a CachedLine {
        self.positioned
            .position
            .segment
            .geometry
            .lines
            .get(self.local().line as usize)
            .expect("paint fragment indexes the retained line table")
    }

    fn prepared_run(self) -> PreparedRunView<'a> {
        self.prepared_line()
            .runs()
            .get(self.local().run as usize)
            .expect("paint fragment indexes the canonical artifact")
    }

    fn prepared_glyph(self, glyph: usize) -> PreparedGlyphView<'a> {
        self.prepared_run()
            .glyphs()
            .get(glyph)
            .expect("paint fragment indexes the canonical glyph table")
    }

    fn source_reference(self, glyph: usize) -> SourceReference {
        let source = if self.local().segment == WHOLE_GLYPH_PAINT {
            self.prepared_glyph(glyph).source()
        } else {
            self.prepared_glyph(glyph)
                .paint()
                .split_segments()
                .expect("split fragment retains split glyph coverage")
                [self.local().segment as usize]
                .source()
        };
        SourceReference::Projected(source.into())
    }

    fn instance(self, glyph: usize) -> usize {
        self.local().instance_start + glyph - self.local().glyphs.start as usize
    }

    fn inline_advance_adjustment(self, glyph: PreparedGlyphView<'_>) -> f64 {
        let expansion = self.cached_line().adjustment.opportunity_expansion();
        if expansion > 0.0
            && self
                .prepared_line()
                .western_justification_opportunity_sources()
                .any(|source| source == glyph.source())
        {
            expansion
        } else {
            0.0
        }
    }

    fn observe_glyph(self, local: usize, inline_origin: f64) -> SceneGlyphView<'a> {
        let inline_advance_adjustment = self.inline_advance_adjustment(self.prepared_glyph(local));
        SceneGlyphView {
            revision: self.revision,
            fragment: self,
            local,
            inline_origin,
            inline_advance_adjustment,
        }
    }

    fn advance_inline(self, local: usize, inline_origin: f64) -> f64 {
        let glyph = self.prepared_glyph(local);
        inline_origin + glyph.advance().x + self.inline_advance_adjustment(glyph)
    }

    fn inline_origin_for(self, local: usize) -> f64 {
        let glyphs = self.local().glyphs.clone();
        debug_assert!(
            (glyphs.start as usize..=glyphs.end as usize).contains(&local),
            "glyph traversal must remain inside its retained fragment"
        );
        (glyphs.start as usize..local).fold(self.local().inline_origin, |inline, glyph| {
            self.advance_inline(glyph, inline)
        })
    }

    /// Returns the retained fragment identity.
    #[must_use]
    pub fn id(self) -> SceneFragmentId {
        SceneFragmentId(fragment_identity(
            self.positioned.position.segment.paragraph,
            self.positioned.local,
        ))
    }

    /// Returns positioned shaped glyph observations.
    #[must_use]
    pub fn glyphs(self) -> SceneGlyphs<'a> {
        let glyphs = self.local().glyphs.clone();
        SceneGlyphs {
            revision: self.revision,
            fragment: self,
            front: glyphs.start as usize,
            back: glyphs.end as usize,
            front_inline: self.local().inline_origin,
            back_inline: None,
        }
    }

    /// Returns the paint slot.
    #[must_use]
    pub fn paint(self) -> PaintSlot {
        self.local().paint
    }

    /// Returns the fragment transform.
    #[must_use]
    pub fn transform(self) -> Affine {
        Affine::IDENTITY
    }

    /// Returns the first source slice covered by this fragment.
    #[must_use]
    pub(crate) fn source(self) -> Option<SnapshotTextRange> {
        self.sources().next()
    }

    /// Iterates every source slice covered by this fragment.
    pub(crate) fn sources(self) -> SnapshotSources<'a> {
        let geometry = &self.positioned.position.segment.geometry;
        SnapshotSources::from_fragment(
            self.revision,
            geometry
                .source_map
                .as_ref()
                .expect("source-capable fragments retain a paragraph source map"),
            self,
        )
    }

    /// Returns an explicit scene-space partial-paint clip.
    #[must_use]
    pub fn paint_clip(self) -> Option<Rect> {
        self.local().paint_clip.map(|clip| clip + self.translate())
    }

    /// Returns exact font bytes and face index.
    #[must_use]
    pub fn font(self) -> &'a FontData {
        self.prepared_run().font()
    }

    /// Returns the scene-unit font size.
    #[must_use]
    pub fn font_size(self) -> f32 {
        self.prepared_run().font_size()
    }

    /// Returns synthesis suggestions.
    #[must_use]
    pub fn synthesis(self) -> &'a FontSynthesis {
        self.prepared_run().synthesis()
    }

    /// Returns normalized variation coordinates.
    #[must_use]
    pub fn normalized_coords(self) -> &'a [i16] {
        self.prepared_run().normalized_coords()
    }

    /// Returns the resolved Unicode bidi level.
    #[must_use]
    pub fn bidi_level(self) -> u8 {
        self.prepared_run().bidi_level()
    }

    /// Returns the resolved ISO 15924 script tag.
    #[must_use]
    pub fn script(self) -> [u8; 4] {
        self.prepared_run().script()
    }

    fn translate(self) -> Vec2 {
        Vec2::new(0.0, self.positioned.position.position.block_origin)
    }
}

/// Allocation-free shaped glyph views inside one fragment.
#[derive(Clone, Debug)]
pub struct SceneGlyphs<'a> {
    revision: DocumentRevision,
    fragment: SceneFragmentView<'a>,
    front: usize,
    back: usize,
    front_inline: f64,
    back_inline: Option<f64>,
}

impl<'a> SceneGlyphs<'a> {
    /// Returns a fresh iterator over every glyph observation.
    #[must_use]
    pub fn iter(&self) -> Self {
        self.fragment.glyphs()
    }

    /// Returns the number of glyph observations.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.back - self.front
    }

    /// Returns whether the fragment has no glyph observation.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.front == self.back
    }

    /// Returns a glyph by fragment-local index.
    #[must_use]
    pub fn get(&self, index: usize) -> Option<SceneGlyphView<'a>> {
        let glyphs = self.fragment.local().glyphs.clone();
        (index < glyphs.len()).then(|| {
            let local = glyphs.start as usize + index;
            self.fragment
                .observe_glyph(local, self.fragment.inline_origin_for(local))
        })
    }

    /// Returns the first glyph.
    #[must_use]
    pub fn first(&self) -> Option<SceneGlyphView<'a>> {
        self.get(0)
    }
}

impl<'a> Iterator for SceneGlyphs<'a> {
    type Item = SceneGlyphView<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.front == self.back {
            return None;
        }
        let local = self.front;
        let glyph = self.fragment.observe_glyph(local, self.front_inline);
        self.front_inline += glyph.advance().x;
        self.front += 1;
        Some(glyph)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.back - self.front;
        (remaining, Some(remaining))
    }
}

impl DoubleEndedIterator for SceneGlyphs<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.front == self.back {
            return None;
        }
        let end = self.back_inline.unwrap_or_else(|| {
            (self.front..self.back).fold(self.front_inline, |inline, glyph| {
                self.fragment.advance_inline(glyph, inline)
            })
        });
        self.back -= 1;
        let glyph = self.fragment.prepared_glyph(self.back);
        let inline_advance_adjustment = self.fragment.inline_advance_adjustment(glyph);
        let inline_origin = end - glyph.advance().x - inline_advance_adjustment;
        self.back_inline = Some(inline_origin);
        Some(SceneGlyphView {
            revision: self.revision,
            fragment: self.fragment,
            local: self.back,
            inline_origin,
            inline_advance_adjustment,
        })
    }
}

impl ExactSizeIterator for SceneGlyphs<'_> {}

/// Allocation-free shaped glyph views inside one transient fragment.
#[derive(Clone, Debug)]
pub struct ProjectedSceneGlyphs<'a> {
    inner: SceneGlyphs<'a>,
}

impl<'a> ProjectedSceneGlyphs<'a> {
    /// Returns a fresh iterator over every glyph observation.
    #[must_use]
    pub fn iter(&self) -> Self {
        Self {
            inner: self.inner.iter(),
        }
    }

    /// Returns the number of glyph observations.
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Returns whether the fragment has no glyph observation.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Returns a glyph by fragment-local index.
    #[must_use]
    pub fn get(&self, index: usize) -> Option<ProjectedSceneGlyphView<'a>> {
        self.inner.get(index).map(ProjectedSceneGlyphView::new)
    }

    /// Returns the first glyph.
    #[must_use]
    pub fn first(&self) -> Option<ProjectedSceneGlyphView<'a>> {
        self.get(0)
    }
}

impl<'a> Iterator for ProjectedSceneGlyphs<'a> {
    type Item = ProjectedSceneGlyphView<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(ProjectedSceneGlyphView::new)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl DoubleEndedIterator for ProjectedSceneGlyphs<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.inner.next_back().map(ProjectedSceneGlyphView::new)
    }
}

impl ExactSizeIterator for ProjectedSceneGlyphs<'_> {}

/// One transient shaped glyph observation positioned in scene space.
#[derive(Clone, Copy, Debug)]
pub struct ProjectedSceneGlyphView<'a> {
    inner: SceneGlyphView<'a>,
}

impl<'a> ProjectedSceneGlyphView<'a> {
    fn new(inner: SceneGlyphView<'a>) -> Self {
        Self { inner }
    }

    /// Returns the identity shared by split-paint observations.
    #[must_use]
    pub fn instance_id(self) -> SceneGlyphInstanceId {
        self.inner.instance_id()
    }

    /// Returns the backend glyph identifier.
    #[must_use]
    pub fn id(self) -> u32 {
        self.inner.id()
    }

    /// Returns the scene-space glyph origin.
    #[must_use]
    pub fn position(self) -> Point {
        self.inner.position()
    }

    /// Returns the shaped advance.
    #[must_use]
    pub fn advance(self) -> Vec2 {
        self.inner.advance()
    }

    /// Iterates source-complete authored and generated provenance.
    pub(crate) fn sources(self) -> ProjectedSources<'a> {
        let geometry = &self.inner.fragment.positioned.position.segment.geometry;
        ProjectedSources::new(
            self.inner.revision,
            geometry
                .source_map
                .as_ref()
                .expect("source-capable glyphs retain a paragraph source map"),
            self.inner.fragment.source_reference(self.inner.local),
        )
    }
}

/// One shaped glyph observation positioned in scene space.
#[derive(Clone, Copy, Debug)]
pub struct SceneGlyphView<'a> {
    revision: DocumentRevision,
    fragment: SceneFragmentView<'a>,
    local: usize,
    inline_origin: f64,
    inline_advance_adjustment: f64,
}

impl<'a> SceneGlyphView<'a> {
    fn prepared(self) -> PreparedGlyphView<'a> {
        self.fragment.prepared_glyph(self.local)
    }

    /// Returns the identity shared by split-paint observations.
    #[must_use]
    pub fn instance_id(self) -> SceneGlyphInstanceId {
        SceneGlyphInstanceId {
            geometry: Arc::as_ptr(&self.fragment.positioned.position.segment.geometry) as usize,
            glyph: self.fragment.instance(self.local),
        }
    }

    /// Returns the backend glyph identifier.
    #[must_use]
    pub fn id(self) -> u32 {
        self.prepared().id()
    }

    /// Returns the scene-space glyph origin.
    #[must_use]
    pub fn position(self) -> Point {
        let offset = self.prepared().offset();
        Point::new(
            self.inline_origin + offset.x,
            self.fragment.cached_line().bounds.y0 + self.fragment.prepared_line().baseline()
                - offset.y,
        ) + self.fragment.translate()
    }

    /// Returns the shaped advance.
    #[must_use]
    pub fn advance(self) -> Vec2 {
        let advance = self.prepared().advance();
        Vec2::new(advance.x + self.inline_advance_adjustment, advance.y)
    }

    /// Iterates source-complete glyph provenance.
    pub(crate) fn sources(self) -> SnapshotSources<'a> {
        let geometry = &self.fragment.positioned.position.segment.geometry;
        SnapshotSources::new(
            self.revision,
            geometry
                .source_map
                .as_ref()
                .expect("source-capable glyphs retain a paragraph source map"),
            self.fragment.source_reference(self.local),
        )
    }
}

/// Iterator that stamps paragraph-local committed ranges with a scene revision.
#[derive(Clone, Debug)]
pub struct SnapshotSources<'a> {
    revision: DocumentRevision,
    ranges: SourceRangeSequence<'a>,
}

impl<'a> SnapshotSources<'a> {
    fn new(
        revision: DocumentRevision,
        map: &'a ParagraphSourceMap,
        source: SourceReference,
    ) -> Self {
        Self {
            revision,
            ranges: SourceRangeSequence::new(map, SourceReferences::One(source)),
        }
    }

    fn from_fragment(
        revision: DocumentRevision,
        map: &'a ParagraphSourceMap,
        fragment: SceneFragmentView<'a>,
    ) -> Self {
        let glyphs = fragment.local().glyphs.clone();
        Self {
            revision,
            ranges: SourceRangeSequence::new(
                map,
                SourceReferences::Glyphs {
                    glyphs: fragment
                        .prepared_run()
                        .glyphs()
                        .slice(glyphs.start as usize..glyphs.end as usize)
                        .expect("paint fragment indexes its prepared run"),
                    segment: (fragment.local().segment != WHOLE_GLYPH_PAINT)
                        .then_some(fragment.local().segment as usize),
                },
            ),
        }
    }

    /// Returns a fresh iterator over every source range.
    #[must_use]
    pub fn iter(&self) -> Self {
        Self {
            revision: self.revision,
            ranges: self.ranges.clone(),
        }
    }

    /// Returns one source range by observation-local index.
    #[must_use]
    pub fn get(&self, index: usize) -> Option<SnapshotTextRange> {
        self.iter().nth(index)
    }

    /// Returns the first source range.
    #[must_use]
    pub fn first(&self) -> Option<SnapshotTextRange> {
        self.get(0)
    }
}

impl Iterator for SnapshotSources<'_> {
    type Item = SnapshotTextRange;

    fn next(&mut self) -> Option<Self::Item> {
        self.ranges
            .next()
            .map(|range| materialize_range(range, self.revision))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.ranges.size_hint()
    }
}

impl DoubleEndedIterator for SnapshotSources<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.ranges
            .next_back()
            .map(|range| materialize_range(range, self.revision))
    }
}

impl ExactSizeIterator for SnapshotSources<'_> {}

/// Iterator over authored and generated source slices in a transient scene.
#[derive(Clone, Debug)]
pub struct ProjectedSources<'a> {
    revision: DocumentRevision,
    ranges: SourceRangeSequence<'a>,
}

impl<'a> ProjectedSources<'a> {
    fn new(
        revision: DocumentRevision,
        map: &'a ParagraphSourceMap,
        source: SourceReference,
    ) -> Self {
        Self {
            revision,
            ranges: SourceRangeSequence::new(map, SourceReferences::One(source)),
        }
    }

    fn from_fragment(
        revision: DocumentRevision,
        map: &'a ParagraphSourceMap,
        fragment: SceneFragmentView<'a>,
    ) -> Self {
        let glyphs = fragment.local().glyphs.clone();
        Self {
            revision,
            ranges: SourceRangeSequence::new(
                map,
                SourceReferences::Glyphs {
                    glyphs: fragment
                        .prepared_run()
                        .glyphs()
                        .slice(glyphs.start as usize..glyphs.end as usize)
                        .expect("paint fragment indexes its prepared run"),
                    segment: (fragment.local().segment != WHOLE_GLYPH_PAINT)
                        .then_some(fragment.local().segment as usize),
                },
            ),
        }
    }

    /// Returns a fresh iterator over every source range.
    #[must_use]
    pub fn iter(&self) -> Self {
        Self {
            revision: self.revision,
            ranges: self.ranges.clone(),
        }
    }

    /// Returns one source range by observation-local index.
    #[must_use]
    pub fn get(&self, index: usize) -> Option<ProjectedTextSource> {
        self.iter().nth(index)
    }

    /// Returns the first source range.
    #[must_use]
    pub fn first(&self) -> Option<ProjectedTextSource> {
        self.get(0)
    }
}

impl Iterator for ProjectedSources<'_> {
    type Item = ProjectedTextSource;

    fn next(&mut self) -> Option<Self::Item> {
        self.ranges
            .next()
            .map(|range| materialize_projected_source(range, self.revision))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.ranges.size_hint()
    }
}

impl DoubleEndedIterator for ProjectedSources<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.ranges
            .next_back()
            .map(|range| materialize_projected_source(range, self.revision))
    }
}

impl ExactSizeIterator for ProjectedSources<'_> {}

/// Borrowed source-complete interaction unit in one immutable snapshot.
///
/// The view retains no per-hit allocation. Its source iterator resolves the
/// paragraph-local relation map lazily and stamps the current scene revision
/// at observation time.
#[derive(Clone, Copy, Debug)]
pub struct SnapshotTextUnitView<'a> {
    revision: DocumentRevision,
    source_map: &'a ParagraphSourceMap,
    source: SourceSpan,
}

impl<'a> SnapshotTextUnitView<'a> {
    pub(super) const fn new(
        revision: DocumentRevision,
        source_map: &'a ParagraphSourceMap,
        source: SourceSpan,
    ) -> Self {
        Self {
            revision,
            source_map,
            source,
        }
    }

    /// Iterates every ordered leaf-local source range without allocating.
    #[must_use]
    pub fn sources(self) -> SnapshotSources<'a> {
        SnapshotSources::new(
            self.revision,
            self.source_map,
            SourceReference::Projected(self.source),
        )
    }

    /// Materializes an owned interaction unit for storage beyond the scene.
    #[must_use]
    pub fn to_owned(self) -> SnapshotTextUnit {
        SnapshotTextUnit::new(self.sources().collect())
    }
}

/// Borrowed source-complete interaction unit in a composition scene.
#[derive(Clone, Copy, Debug)]
pub struct ProjectedTextUnitView<'a> {
    revision: DocumentRevision,
    source_map: &'a ParagraphSourceMap,
    source: SourceSpan,
}

impl<'a> ProjectedTextUnitView<'a> {
    pub(super) const fn new(
        revision: DocumentRevision,
        source_map: &'a ParagraphSourceMap,
        source: SourceSpan,
    ) -> Self {
        Self {
            revision,
            source_map,
            source,
        }
    }

    /// Iterates ordered authored and generated provenance without allocating.
    #[must_use]
    pub fn sources(self) -> ProjectedSources<'a> {
        ProjectedSources::new(
            self.revision,
            self.source_map,
            SourceReference::Projected(self.source),
        )
    }

    /// Materializes an owned projected range for storage beyond the scene.
    #[must_use]
    pub fn to_owned(self) -> ProjectedTextRange {
        ProjectedTextRange::new(self.sources().collect())
    }
}

#[derive(Clone, Copy, Debug)]
enum SourceReferences<'a> {
    One(SourceReference),
    Glyphs {
        glyphs: PreparedGlyphs<'a>,
        segment: Option<usize>,
    },
}

impl SourceReferences<'_> {
    fn len(self) -> usize {
        match self {
            Self::One(_) => 1,
            Self::Glyphs { glyphs, .. } => glyphs.len(),
        }
    }

    fn get(self, index: usize) -> Option<SourceReference> {
        match self {
            Self::One(source) => (index == 0).then_some(source),
            Self::Glyphs { glyphs, segment } => {
                let glyph = glyphs.get(index)?;
                let source = match segment {
                    Some(segment) => glyph.paint().split_segments()?.get(segment)?.source(),
                    None => glyph.source(),
                };
                Some(SourceReference::Projected(source.into()))
            }
        }
    }
}

#[derive(Clone, Debug)]
struct SourceRangeSequence<'a> {
    map: &'a ParagraphSourceMap,
    references: SourceReferences<'a>,
    front_reference: usize,
    back_reference: usize,
    front_ranges: Option<LocalRanges<'a>>,
    back_ranges: Option<LocalRanges<'a>>,
    remaining: usize,
}

impl<'a> SourceRangeSequence<'a> {
    fn new(map: &'a ParagraphSourceMap, references: SourceReferences<'a>) -> Self {
        let back_reference = references.len();
        let remaining = (0..back_reference)
            .filter_map(|index| references.get(index))
            .map(|source| map.ranges(source).len())
            .sum();
        Self {
            map,
            references,
            front_reference: 0,
            back_reference,
            front_ranges: None,
            back_ranges: None,
            remaining,
        }
    }
}

impl Iterator for SourceRangeSequence<'_> {
    type Item = LocalRange;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(ranges) = &mut self.front_ranges
                && let Some(range) = ranges.next()
            {
                self.remaining -= 1;
                return Some(range);
            }
            self.front_ranges = None;
            if self.front_reference < self.back_reference {
                let source = self
                    .references
                    .get(self.front_reference)
                    .expect("source reference bounds remain valid");
                self.front_reference += 1;
                self.front_ranges = Some(self.map.ranges(source));
                continue;
            }
            if let Some(ranges) = &mut self.back_ranges
                && let Some(range) = ranges.next()
            {
                self.remaining -= 1;
                return Some(range);
            }
            return None;
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

impl DoubleEndedIterator for SourceRangeSequence<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(ranges) = &mut self.back_ranges
                && let Some(range) = ranges.next_back()
            {
                self.remaining -= 1;
                return Some(range);
            }
            self.back_ranges = None;
            if self.front_reference < self.back_reference {
                self.back_reference -= 1;
                let source = self
                    .references
                    .get(self.back_reference)
                    .expect("source reference bounds remain valid");
                self.back_ranges = Some(self.map.ranges(source));
                continue;
            }
            if let Some(ranges) = &mut self.front_ranges
                && let Some(range) = ranges.next_back()
            {
                self.remaining -= 1;
                return Some(range);
            }
            return None;
        }
    }
}

impl ExactSizeIterator for SourceRangeSequence<'_> {}

/// Allocation-free semantic observations in document order.
#[derive(Clone, Debug)]
pub struct SceneSemantics<'a> {
    revision: DocumentRevision,
    segments: SpineSegments<'a>,
    current: Option<(PositionedSegment<'a>, usize)>,
}

impl<'a> SceneSemantics<'a> {
    pub(super) fn new(revision: DocumentRevision, spine: &'a SceneSpine) -> Self {
        Self {
            revision,
            segments: spine.segments(),
            current: None,
        }
    }
}

impl<'a> Iterator for SceneSemantics<'a> {
    type Item = SemanticFragmentView<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some((positioned, local)) = &mut self.current
                && *local < positioned.segment.geometry.semantics.len()
            {
                let view = SemanticFragmentView {
                    revision: self.revision,
                    positioned: *positioned,
                    local: *local,
                };
                *local += 1;
                return Some(view);
            }
            self.current = self.segments.next().map(|positioned| (positioned, 0));
            self.current?;
        }
    }
}

/// One semantic observation with scene-space geometry.
#[derive(Clone, Copy, Debug)]
pub struct SemanticFragmentView<'a> {
    revision: DocumentRevision,
    positioned: PositionedSegment<'a>,
    local: usize,
}

impl<'a> SemanticFragmentView<'a> {
    fn local(self) -> &'a CachedSemantic {
        &self.positioned.segment.geometry.semantics[self.local]
    }

    /// Returns the source semantic identity.
    #[must_use]
    pub fn semantic_id(self) -> SemanticId {
        self.local().semantic_id
    }

    /// Returns the paragraph role for block observations.
    #[must_use]
    pub fn paragraph_role(self) -> Option<ParagraphRole> {
        self.local().paragraph_role
    }

    /// Returns the inline role for inline observations.
    #[must_use]
    pub fn inline_role(self) -> Option<InlineRole> {
        self.local().inline_role
    }

    /// Returns snapshot-local source when the observation has exactly one.
    #[must_use]
    pub fn source(self) -> Option<SnapshotTextRange> {
        let source = self.local().source?;
        let source_map = self
            .positioned
            .segment
            .geometry
            .source_map
            .as_ref()
            .expect("semantic capability retains a paragraph source map");
        materialize_optional_snapshot_range(source_map, source, self.revision)
    }

    /// Returns scene-space semantic bounds.
    #[must_use]
    pub fn bounds(self) -> Rect {
        self.local().bounds + Vec2::new(0.0, self.positioned.position.block_origin)
    }
}
