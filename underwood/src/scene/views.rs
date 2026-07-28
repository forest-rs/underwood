// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Positioned public views over persistent paragraph-local scene records.

use super::*;
use core::marker::PhantomData;

fn source_map<'a>(
    requested: &SceneFeaturePolicy,
    positioned: PositionedSegment<'a>,
) -> Result<&'a ParagraphSourceMap, MissingSceneCapability> {
    let paragraph = positioned.segment.paragraph;
    let resident = positioned.segment.geometry.features;
    positioned
        .segment
        .geometry
        .source_map
        .as_ref()
        .filter(|_| resident.has_sources())
        .ok_or_else(|| {
            MissingSceneCapability::new(
                Some(paragraph),
                SceneFeatures::DISPLAY.with_sources(),
                requested.features_for(paragraph),
                resident,
            )
        })
}

/// Allocation-free view of visual lines in one committed scene.
#[derive(Clone, Debug)]
pub struct SceneLines<'a, T = SnapshotTextRange> {
    revision: DocumentRevision,
    core: &'a SceneCore,
    requested: &'a SceneFeaturePolicy,
    segments: SpineSegments<'a>,
    current: Option<(PositionedSegment<'a>, usize)>,
    remaining: usize,
    source: PhantomData<fn() -> T>,
}

impl<'a, T> SceneLines<'a, T> {
    pub(super) fn new(
        revision: DocumentRevision,
        core: &'a SceneCore,
        requested: &'a SceneFeaturePolicy,
    ) -> Self {
        Self {
            revision,
            core,
            requested,
            segments: core.spine.segments(),
            current: None,
            remaining: core.spine.summary().lines,
            source: PhantomData,
        }
    }

    /// Returns a fresh iterator over every line.
    #[must_use]
    pub fn iter(&self) -> Self {
        Self::new(self.revision, self.core, self.requested)
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
    pub fn get(&self, index: usize) -> Option<SceneLineView<'a, T>> {
        self.core
            .spine
            .positioned_line(index)
            .map(|line| SceneLineView::new(self.revision, self.requested, line))
    }

    /// Returns the first visual line.
    #[must_use]
    pub fn first(&self) -> Option<SceneLineView<'a, T>> {
        self.get(0)
    }
}

impl<'a, T> Iterator for SceneLines<'a, T> {
    type Item = SceneLineView<'a, T>;

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
                return Some(SceneLineView::new(self.revision, self.requested, line));
            }
            self.current = self.segments.next().map(|positioned| (positioned, 0));
            self.current?;
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

impl<T> ExactSizeIterator for SceneLines<'_, T> {}

/// Allocation-free visual-line traversal for a transient projected scene.
pub type ProjectedSceneLines<'a> = SceneLines<'a, ProjectedTextSource>;

/// One visual line with scene origin, ordinals, and revision applied lazily.
#[derive(Debug)]
pub struct SceneLineView<'a, T = SnapshotTextRange> {
    revision: DocumentRevision,
    requested: &'a SceneFeaturePolicy,
    positioned: PositionedLine<'a>,
    source: PhantomData<fn() -> T>,
}

impl<T> Copy for SceneLineView<'_, T> {}

impl<T> Clone for SceneLineView<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'a, T> SceneLineView<'a, T> {
    pub(super) fn new(
        revision: DocumentRevision,
        requested: &'a SceneFeaturePolicy,
        positioned: PositionedLine<'a>,
    ) -> Self {
        Self {
            revision,
            requested,
            positioned,
            source: PhantomData,
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
            .line(self.positioned.local)
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
            + self.local().adjustment.opportunity_expansion
                * f64::from(self.local().adjustment.expanded_opportunities)
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
}

impl<'a> SceneLineView<'a> {
    /// Iterates source-complete snapshot slices represented by the line.
    ///
    /// # Errors
    ///
    /// Returns [`MissingSceneCapability`] when provenance was not retained for
    /// this paragraph.
    pub fn sources(self) -> Result<SnapshotSources<'a>, MissingSceneCapability> {
        Ok(SnapshotSources::new(
            self.revision,
            source_map(self.requested, self.positioned.position)?,
            SourceReference::Projected(self.prepared().source().into()),
        ))
    }
}

impl<'a> SceneLineView<'a, ProjectedTextSource> {
    /// Iterates authored and generated source slices represented by the line.
    ///
    /// # Errors
    ///
    /// Returns [`MissingSceneCapability`] when provenance was not retained for
    /// this paragraph.
    pub fn sources(self) -> Result<ProjectedSources<'a>, MissingSceneCapability> {
        Ok(ProjectedSources::new(
            self.revision,
            source_map(self.requested, self.positioned.position)?,
            SourceReference::Projected(self.prepared().source().into()),
        ))
    }
}

/// One transient visual line with scene placement applied lazily.
pub type ProjectedSceneLineView<'a> = SceneLineView<'a, ProjectedTextSource>;

/// Allocation-free view of paint-homogeneous fragments in one committed scene.
#[derive(Clone, Debug)]
pub struct SceneFragments<'a, T = SnapshotTextRange> {
    revision: DocumentRevision,
    core: &'a SceneCore,
    requested: &'a SceneFeaturePolicy,
    segments: SpineSegments<'a>,
    current: Option<(PositionedSegment<'a>, usize)>,
    remaining: usize,
    source: PhantomData<fn() -> T>,
}

impl<'a, T> SceneFragments<'a, T> {
    pub(super) fn new(
        revision: DocumentRevision,
        core: &'a SceneCore,
        requested: &'a SceneFeaturePolicy,
    ) -> Self {
        Self {
            revision,
            core,
            requested,
            segments: core.spine.segments(),
            current: None,
            remaining: core.spine.summary().fragments,
            source: PhantomData,
        }
    }

    /// Returns a fresh iterator over every fragment.
    #[must_use]
    pub fn iter(&self) -> Self {
        Self::new(self.revision, self.core, self.requested)
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
    pub fn get(&self, index: usize) -> Option<SceneFragmentView<'a, T>> {
        self.core
            .spine
            .positioned_fragment(index)
            .map(|fragment| SceneFragmentView::new(self.revision, self.requested, fragment))
    }
}

impl<'a, T> Iterator for SceneFragments<'a, T> {
    type Item = SceneFragmentView<'a, T>;

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
                return Some(SceneFragmentView::new(
                    self.revision,
                    self.requested,
                    fragment,
                ));
            }
            self.current = self.segments.next().map(|positioned| (positioned, 0));
            self.current?;
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

impl<T> ExactSizeIterator for SceneFragments<'_, T> {}

/// Allocation-free fragment traversal for a transient projected scene.
pub type ProjectedSceneFragments<'a> = SceneFragments<'a, ProjectedTextSource>;

/// One paint-homogeneous fragment positioned in scene space.
#[derive(Debug)]
pub struct SceneFragmentView<'a, T = SnapshotTextRange> {
    revision: DocumentRevision,
    requested: &'a SceneFeaturePolicy,
    positioned: PositionedFragment<'a>,
    source: PhantomData<fn() -> T>,
}

impl<T> Copy for SceneFragmentView<'_, T> {}

impl<T> Clone for SceneFragmentView<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'a, T> SceneFragmentView<'a, T> {
    pub(super) fn new(
        revision: DocumentRevision,
        requested: &'a SceneFeaturePolicy,
        positioned: PositionedFragment<'a>,
    ) -> Self {
        Self {
            revision,
            requested,
            positioned,
            source: PhantomData,
        }
    }

    fn local(self) -> &'a CachedFragment {
        &self.positioned.position.segment.paint.fragments[self.positioned.local]
    }

    fn prepared_line(self) -> PreparedLineView<'a> {
        let geometry = &self.positioned.position.segment.geometry;
        geometry
            .artifact
            .line(self.local().line as usize)
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
            .run(self.local().run as usize)
            .expect("paint fragment indexes the canonical artifact")
    }

    fn prepared_glyph(self, glyph: usize) -> PreparedGlyphView<'a> {
        self.prepared_run()
            .glyph(glyph)
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
        let expansion = self.cached_line().adjustment.opportunity_expansion;
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

    fn observe_glyph(self, local: usize, inline_origin: f64) -> SceneGlyphView<'a, T> {
        let inline_advance_adjustment = self.inline_advance_adjustment(self.prepared_glyph(local));
        SceneGlyphView {
            revision: self.revision,
            fragment: self,
            local,
            inline_origin,
            inline_advance_adjustment,
            source: PhantomData,
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
    pub fn glyphs(self) -> SceneGlyphs<'a, T> {
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

impl<'a> SceneFragmentView<'a> {
    /// Returns the first source slice covered by this fragment.
    pub fn source(self) -> Result<Option<SnapshotTextRange>, MissingSceneCapability> {
        Ok(self.sources()?.next())
    }

    /// Iterates every source slice covered by this fragment.
    pub fn sources(self) -> Result<SnapshotSources<'a>, MissingSceneCapability> {
        Ok(SnapshotSources::from_fragment(
            self.revision,
            source_map(self.requested, self.positioned.position)?,
            self,
        ))
    }
}

impl<'a> SceneFragmentView<'a, ProjectedTextSource> {
    /// Returns the first authored or generated source slice.
    pub fn source(self) -> Result<Option<ProjectedTextSource>, MissingSceneCapability> {
        Ok(self.sources()?.next())
    }

    /// Iterates every authored and generated source slice.
    pub fn sources(self) -> Result<ProjectedSources<'a>, MissingSceneCapability> {
        Ok(ProjectedSources::from_fragment(
            self.revision,
            source_map(self.requested, self.positioned.position)?,
            self,
        ))
    }
}

/// One transient paint fragment positioned in scene space.
pub type ProjectedSceneFragmentView<'a> = SceneFragmentView<'a, ProjectedTextSource>;

/// Allocation-free shaped glyph views inside one fragment.
#[derive(Clone, Debug)]
pub struct SceneGlyphs<'a, T = SnapshotTextRange> {
    revision: DocumentRevision,
    fragment: SceneFragmentView<'a, T>,
    front: usize,
    back: usize,
    front_inline: f64,
    back_inline: Option<f64>,
}

impl<'a, T> SceneGlyphs<'a, T> {
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
    pub fn get(&self, index: usize) -> Option<SceneGlyphView<'a, T>> {
        let glyphs = self.fragment.local().glyphs.clone();
        (index < glyphs.len()).then(|| {
            let local = glyphs.start as usize + index;
            self.fragment
                .observe_glyph(local, self.fragment.inline_origin_for(local))
        })
    }

    /// Returns the first glyph.
    #[must_use]
    pub fn first(&self) -> Option<SceneGlyphView<'a, T>> {
        self.get(0)
    }
}

impl<'a, T> Iterator for SceneGlyphs<'a, T> {
    type Item = SceneGlyphView<'a, T>;

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

impl<T> DoubleEndedIterator for SceneGlyphs<'_, T> {
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
            source: PhantomData,
        })
    }
}

impl<T> ExactSizeIterator for SceneGlyphs<'_, T> {}

/// Allocation-free shaped glyph views inside one transient fragment.
pub type ProjectedSceneGlyphs<'a> = SceneGlyphs<'a, ProjectedTextSource>;

/// One shaped glyph observation positioned in scene space.
#[derive(Debug)]
pub struct SceneGlyphView<'a, T = SnapshotTextRange> {
    revision: DocumentRevision,
    fragment: SceneFragmentView<'a, T>,
    local: usize,
    inline_origin: f64,
    inline_advance_adjustment: f64,
    source: PhantomData<fn() -> T>,
}

impl<T> Copy for SceneGlyphView<'_, T> {}

impl<T> Clone for SceneGlyphView<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'a, T> SceneGlyphView<'a, T> {
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
}

impl<'a> SceneGlyphView<'a> {
    /// Iterates source-complete glyph provenance.
    pub fn sources(self) -> Result<SnapshotSources<'a>, MissingSceneCapability> {
        Ok(SnapshotSources::new(
            self.revision,
            source_map(self.fragment.requested, self.fragment.positioned.position)?,
            self.fragment.source_reference(self.local),
        ))
    }
}

impl<'a> SceneGlyphView<'a, ProjectedTextSource> {
    /// Iterates source-complete authored and generated provenance.
    pub fn sources(self) -> Result<ProjectedSources<'a>, MissingSceneCapability> {
        Ok(ProjectedSources::new(
            self.revision,
            source_map(self.fragment.requested, self.fragment.positioned.position)?,
            self.fragment.source_reference(self.local),
        ))
    }
}

/// One transient shaped glyph observation positioned in scene space.
pub type ProjectedSceneGlyphView<'a> = SceneGlyphView<'a, ProjectedTextSource>;

/// Typed source iterator over paragraph-local provenance.
#[derive(Clone, Debug)]
pub struct TextSources<'a, T> {
    revision: DocumentRevision,
    ranges: SourceRangeSequence<'a>,
    source: PhantomData<fn() -> T>,
}

impl<'a, T> TextSources<'a, T> {
    fn new(
        revision: DocumentRevision,
        map: &'a ParagraphSourceMap,
        source: SourceReference,
    ) -> Self {
        Self {
            revision,
            ranges: SourceRangeSequence::new(map, SourceReferences::One(source)),
            source: PhantomData,
        }
    }

    fn from_fragment(
        revision: DocumentRevision,
        map: &'a ParagraphSourceMap,
        fragment: SceneFragmentView<'a, T>,
    ) -> Self {
        let glyphs = fragment.local().glyphs.clone();
        Self {
            revision,
            ranges: SourceRangeSequence::new(
                map,
                SourceReferences::Glyphs {
                    run: fragment.prepared_run(),
                    start: glyphs.start as usize,
                    end: glyphs.end as usize,
                    segment: (fragment.local().segment != WHOLE_GLYPH_PAINT)
                        .then_some(fragment.local().segment as usize),
                },
            ),
            source: PhantomData,
        }
    }

    /// Returns a fresh iterator over every source range.
    #[must_use]
    pub fn iter(&self) -> Self {
        Self {
            revision: self.revision,
            ranges: self.ranges.clone(),
            source: PhantomData,
        }
    }
}

/// Iterator that stamps paragraph-local committed ranges with a scene revision.
pub type SnapshotSources<'a> = TextSources<'a, SnapshotTextRange>;

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
pub type ProjectedSources<'a> = TextSources<'a, ProjectedTextSource>;

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

/// Borrowed source-complete interaction unit.
///
/// The view retains no per-hit allocation. Its source iterator resolves the
/// paragraph-local relation map lazily and stamps the current scene revision
/// at observation time.
#[derive(Debug)]
pub struct TextUnitView<'a, T = SnapshotTextRange> {
    revision: DocumentRevision,
    source_map: &'a ParagraphSourceMap,
    source: SourceSpan,
    output: PhantomData<fn() -> T>,
}

impl<T> Copy for TextUnitView<'_, T> {}

impl<T> Clone for TextUnitView<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'a, T> TextUnitView<'a, T> {
    pub(super) const fn new(
        revision: DocumentRevision,
        source_map: &'a ParagraphSourceMap,
        source: SourceSpan,
    ) -> Self {
        Self {
            revision,
            source_map,
            source,
            output: PhantomData,
        }
    }
}

/// Borrowed source-complete interaction unit in one immutable snapshot.
pub type SnapshotTextUnitView<'a> = TextUnitView<'a>;

impl<'a> TextUnitView<'a> {
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
pub type ProjectedTextUnitView<'a> = TextUnitView<'a, ProjectedTextSource>;

impl<'a> TextUnitView<'a, ProjectedTextSource> {
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
        run: PreparedRunView<'a>,
        start: usize,
        end: usize,
        segment: Option<usize>,
    },
}

impl SourceReferences<'_> {
    fn len(self) -> usize {
        match self {
            Self::One(_) => 1,
            Self::Glyphs { start, end, .. } => end - start,
        }
    }

    fn get(self, index: usize) -> Option<SourceReference> {
        match self {
            Self::One(source) => (index == 0).then_some(source),
            Self::Glyphs {
                run,
                start,
                end,
                segment,
            } => {
                let index = start.checked_add(index)?;
                if index >= end {
                    return None;
                }
                let glyph = run.glyph(index)?;
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
