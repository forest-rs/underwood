// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Capability-checked borrowed scene surfaces.

use super::*;

/// Unconditional renderer-facing access to a committed text scene.
#[derive(Clone, Copy, Debug)]
pub struct SceneDisplay<'a> {
    scene: &'a TextScene,
}

impl<'a> SceneDisplay<'a> {
    pub(super) const fn new(scene: &'a TextScene) -> Self {
        Self { scene }
    }

    /// Returns visual lines in flow order.
    #[must_use]
    pub fn lines(self) -> SceneLines<'a> {
        self.scene.lines()
    }

    /// Returns one visual line by global index.
    #[must_use]
    pub fn line(self, index: usize) -> Option<SceneLineView<'a>> {
        self.scene.line(index)
    }

    /// Returns the number of visual lines.
    #[must_use]
    pub fn line_count(self) -> usize {
        self.scene.line_count()
    }

    /// Returns paint-homogeneous glyph fragments in visual order.
    #[must_use]
    pub fn fragments(self) -> SceneFragments<'a> {
        self.scene.fragments()
    }

    /// Returns one paint fragment by global visual index.
    #[must_use]
    pub fn fragment(self, index: usize) -> Option<SceneFragmentView<'a>> {
        self.scene.fragment(index)
    }

    /// Returns the number of paint fragments.
    #[must_use]
    pub fn fragment_count(self) -> usize {
        self.scene.fragment_count()
    }

    /// Returns immutable paint values referenced by fragment slots.
    #[must_use]
    pub const fn paint(self) -> &'a PaintTable {
        self.scene.paint()
    }
}

/// Source-aware access to a committed text scene.
#[derive(Clone, Copy, Debug)]
pub struct SceneSourceAccess<'a> {
    _scene: &'a TextScene,
}

/// Source-aware access to a transient composition scene.
#[derive(Clone, Copy, Debug)]
pub struct ProjectedSceneSourceAccess<'a> {
    _scene: &'a CompositionScene,
}

/// Semantic structure and geometry from a committed scene.
#[derive(Clone, Copy, Debug)]
pub struct SceneSemanticAccess<'a> {
    scene: &'a TextScene,
}

impl<'a> SceneSemanticAccess<'a> {
    pub(super) const fn new(scene: &'a TextScene) -> Self {
        Self { scene }
    }

    /// Iterates semantic fragments in document order.
    #[must_use]
    pub fn iter(self) -> SceneSemantics<'a> {
        self.scene.semantic_records()
    }
}

/// Semantic structure and geometry from a transient composition scene.
#[derive(Clone, Copy, Debug)]
pub struct ProjectedSceneSemanticAccess<'a> {
    scene: &'a CompositionScene,
}

impl<'a> ProjectedSceneSemanticAccess<'a> {
    pub(super) const fn new(scene: &'a CompositionScene) -> Self {
        Self { scene }
    }

    /// Iterates semantic fragments in document order.
    #[must_use]
    pub fn iter(self) -> SceneSemantics<'a> {
        self.scene.semantic_records()
    }
}

impl<'a> SceneSourceAccess<'a> {
    pub(super) const fn new(scene: &'a TextScene) -> Self {
        Self { _scene: scene }
    }

    /// Returns source provenance for one line from this scene.
    #[must_use]
    pub fn for_line(self, line: SceneLineView<'a>) -> SnapshotSources<'a> {
        line.sources()
    }

    /// Returns source provenance for one paint fragment from this scene.
    #[must_use]
    pub fn for_fragment(self, fragment: SceneFragmentView<'a>) -> SnapshotSources<'a> {
        fragment.sources()
    }

    /// Returns the first source range represented by one paint fragment.
    #[must_use]
    pub fn first_for_fragment(self, fragment: SceneFragmentView<'a>) -> Option<SnapshotTextRange> {
        fragment.source()
    }

    /// Returns source provenance for one shaped glyph from this scene.
    #[must_use]
    pub fn for_glyph(self, glyph: SceneGlyphView<'a>) -> SnapshotSources<'a> {
        glyph.sources()
    }

    /// Returns the first source range represented by one shaped glyph.
    #[must_use]
    pub fn first_for_glyph(self, glyph: SceneGlyphView<'a>) -> Option<SnapshotTextRange> {
        glyph.sources().next()
    }
}

impl<'a> ProjectedSceneSourceAccess<'a> {
    pub(super) const fn new(scene: &'a CompositionScene) -> Self {
        Self { _scene: scene }
    }

    /// Returns source provenance for one transient line.
    #[must_use]
    pub fn for_line(self, line: ProjectedSceneLineView<'a>) -> ProjectedSources<'a> {
        line.sources()
    }

    /// Returns source provenance for one transient paint fragment.
    #[must_use]
    pub fn for_fragment(self, fragment: ProjectedSceneFragmentView<'a>) -> ProjectedSources<'a> {
        fragment.sources()
    }

    /// Returns the first source represented by one transient paint fragment.
    #[must_use]
    pub fn first_for_fragment(
        self,
        fragment: ProjectedSceneFragmentView<'a>,
    ) -> Option<ProjectedTextSource> {
        fragment.source()
    }

    /// Returns source provenance for one transient shaped glyph.
    #[must_use]
    pub fn for_glyph(self, glyph: ProjectedSceneGlyphView<'a>) -> ProjectedSources<'a> {
        glyph.sources()
    }

    /// Returns the first source represented by one transient shaped glyph.
    #[must_use]
    pub fn first_for_glyph(
        self,
        glyph: ProjectedSceneGlyphView<'a>,
    ) -> Option<ProjectedTextSource> {
        glyph.sources().next()
    }
}

/// Exact point interaction over a committed text scene.
#[derive(Clone, Copy, Debug)]
pub struct SceneInteraction<'a> {
    scene: &'a TextScene,
}

/// Selection construction and geometry over a committed scene.
#[derive(Clone, Copy, Debug)]
pub struct SceneSelection<'a> {
    scene: &'a TextScene,
}

impl<'a> SceneSelection<'a> {
    pub(super) const fn new(scene: &'a TextScene) -> Self {
        Self { scene }
    }

    /// Returns an empty set bound to this scene revision.
    #[must_use]
    pub fn empty_set(self) -> SnapshotTextSelectionSet {
        self.scene.empty_selection_set()
    }

    /// Creates one collapsed selection at a represented caret.
    pub fn collapsed(
        self,
        position: &SnapshotTextPosition,
    ) -> Result<SnapshotTextSelection, SelectionError> {
        self.scene.collapsed_selection(position)
    }

    /// Creates one logical or visual selection between represented carets.
    pub fn between(
        self,
        anchor: &SnapshotTextPosition,
        extent: &SnapshotTextPosition,
        mode: TextSelectionMode,
    ) -> Result<SnapshotTextSelection, SelectionError> {
        self.scene.selection_between(anchor, extent, mode)
    }

    /// Validates and owns independent selections in this scene.
    pub fn set(
        self,
        selections: impl IntoIterator<Item = SnapshotTextSelection>,
    ) -> Result<SnapshotTextSelectionSet, SelectionError> {
        self.scene.selection_set(selections)
    }

    /// Returns exact scene-space rectangles for every selected range.
    pub fn geometry(
        self,
        selections: &SnapshotTextSelectionSet,
    ) -> Result<Vec<SceneSelectionRect>, SelectionError> {
        self.scene.selection_geometry(selections)
    }

    /// Returns exact caret geometry.
    #[must_use]
    pub fn caret(self, position: &SnapshotTextPosition) -> Option<SceneCaret> {
        self.scene.caret(position)
    }
}

impl<'a> SceneInteraction<'a> {
    pub(super) const fn new(scene: &'a TextScene) -> Self {
        Self { scene }
    }

    /// Returns the exact interaction unit under `point`.
    #[must_use]
    pub fn hit_test(self, point: Point) -> Option<TextHit<SnapshotTextUnitView<'a>>> {
        self.scene.hit_test(point)
    }

    /// Returns the closest represented interaction-unit side.
    #[must_use]
    pub fn hit_test_closest(self, point: Point) -> Option<TextHit<SnapshotTextUnitView<'a>>> {
        self.scene.hit_test_closest(point)
    }
}

/// Complete selection, navigation, and native-input access to a committed scene.
#[derive(Clone, Copy, Debug)]
pub struct SceneEditing<'a> {
    scene: &'a TextScene,
}

impl<'a> SceneEditing<'a> {
    pub(super) const fn new(scene: &'a TextScene) -> Self {
        Self { scene }
    }

    /// Returns the exact interaction unit under `point`.
    #[must_use]
    pub fn hit_test(self, point: Point) -> Option<TextHit<SnapshotTextUnitView<'a>>> {
        self.scene.hit_test(point)
    }

    /// Returns the closest represented interaction-unit side.
    #[must_use]
    pub fn hit_test_closest(self, point: Point) -> Option<TextHit<SnapshotTextUnitView<'a>>> {
        self.scene.hit_test_closest(point)
    }

    /// Returns the first logical caret position.
    #[must_use]
    pub fn start_position(self) -> Option<SnapshotTextPosition> {
        self.scene.start_position()
    }

    /// Returns the final logical caret position.
    #[must_use]
    pub fn end_position(self) -> Option<SnapshotTextPosition> {
        self.scene.end_position()
    }

    /// Resolves a represented caret at one leaf-local byte boundary.
    #[must_use]
    pub fn position_at(self, text: TextId, byte: u32) -> Option<SnapshotTextPosition> {
        self.scene.position_at(text, byte)
    }

    /// Returns exact caret geometry.
    #[must_use]
    pub fn caret(self, position: &SnapshotTextPosition) -> Option<SceneCaret> {
        self.scene.caret(position)
    }

    /// Creates one collapsed selection at a represented caret.
    pub fn collapsed_selection(
        self,
        position: &SnapshotTextPosition,
    ) -> Result<SnapshotTextSelection, SelectionError> {
        self.scene.collapsed_selection(position)
    }

    /// Creates one logical or visual selection between represented carets.
    pub fn selection_between(
        self,
        anchor: &SnapshotTextPosition,
        extent: &SnapshotTextPosition,
        mode: TextSelectionMode,
    ) -> Result<SnapshotTextSelection, SelectionError> {
        self.scene.selection_between(anchor, extent, mode)
    }

    /// Validates and owns independent selections in this scene.
    pub fn selection_set(
        self,
        selections: impl IntoIterator<Item = SnapshotTextSelection>,
    ) -> Result<SnapshotTextSelectionSet, SelectionError> {
        self.scene.selection_set(selections)
    }

    /// Returns exact scene-space rectangles for every selected range.
    pub fn selection_geometry(
        self,
        selections: &SnapshotTextSelectionSet,
    ) -> Result<Vec<SceneSelectionRect>, SelectionError> {
        self.scene.selection_geometry(selections)
    }

    /// Returns the preceding logical word start, or scene start.
    #[must_use]
    pub fn previous_word_position(
        self,
        position: &SnapshotTextPosition,
    ) -> Option<SnapshotTextPosition> {
        self.scene.previous_word_position(position)
    }

    /// Returns the following logical word start, or scene end.
    #[must_use]
    pub fn next_word_position(
        self,
        position: &SnapshotTextPosition,
    ) -> Option<SnapshotTextPosition> {
        self.scene.next_word_position(position)
    }

    /// Moves every selection through the retained movement graph.
    pub fn move_selections(
        self,
        selections: &SnapshotTextSelectionSet,
        movement: TextMovement,
        extend: bool,
    ) -> Result<SnapshotTextSelectionSet, SelectionError> {
        self.scene.move_selections(selections, movement, extend)
    }

    /// Starts one native composition over the current primary insertion point.
    pub fn begin_composition(
        self,
        selections: &SnapshotTextSelectionSet,
        id: CompositionId,
    ) -> Result<CompositionStart, CompositionError> {
        self.scene.begin_composition(selections, id)
    }
}

/// Display access to one transient composition scene.
#[derive(Clone, Copy, Debug)]
pub struct ProjectedSceneDisplay<'a> {
    scene: &'a CompositionScene,
}

impl<'a> ProjectedSceneDisplay<'a> {
    pub(super) const fn new(scene: &'a CompositionScene) -> Self {
        Self { scene }
    }

    /// Returns transient visual lines in flow order.
    #[must_use]
    pub fn lines(self) -> ProjectedSceneLines<'a> {
        self.scene.lines()
    }

    /// Returns transient paint fragments in visual order.
    #[must_use]
    pub fn fragments(self) -> ProjectedSceneFragments<'a> {
        self.scene.fragments()
    }

    /// Returns immutable paint values referenced by fragment slots.
    #[must_use]
    pub const fn paint(self) -> &'a PaintTable {
        self.scene.paint()
    }
}

/// Exact point interaction over a transient composition scene.
#[derive(Clone, Copy, Debug)]
pub struct ProjectedSceneInteraction<'a> {
    scene: &'a CompositionScene,
}

impl<'a> ProjectedSceneInteraction<'a> {
    pub(super) const fn new(scene: &'a CompositionScene) -> Self {
        Self { scene }
    }

    /// Returns the exact projected interaction unit under `point`.
    #[must_use]
    pub fn hit_test(
        self,
        point: Point,
    ) -> Option<TextHit<ProjectedTextUnitView<'a>, ProjectedTextPosition>> {
        self.scene.hit_test(point)
    }

    /// Returns the closest projected interaction-unit side.
    #[must_use]
    pub fn hit_test_closest(
        self,
        point: Point,
    ) -> Option<TextHit<ProjectedTextUnitView<'a>, ProjectedTextPosition>> {
        self.scene.hit_test_closest(point)
    }
}

/// Native composition interaction over one transient scene epoch.
#[derive(Clone, Copy, Debug)]
pub struct ProjectedSceneEditing<'a> {
    scene: &'a CompositionScene,
}

impl<'a> ProjectedSceneEditing<'a> {
    pub(super) const fn new(scene: &'a CompositionScene) -> Self {
        Self { scene }
    }

    /// Returns the exact projected interaction unit under `point`.
    #[must_use]
    pub fn hit_test(
        self,
        point: Point,
    ) -> Option<TextHit<ProjectedTextUnitView<'a>, ProjectedTextPosition>> {
        self.scene.hit_test(point)
    }

    /// Returns the closest projected interaction-unit side.
    #[must_use]
    pub fn hit_test_closest(
        self,
        point: Point,
    ) -> Option<TextHit<ProjectedTextUnitView<'a>, ProjectedTextPosition>> {
        self.scene.hit_test_closest(point)
    }

    /// Resolves exact scene geometry for one projected caret position.
    #[must_use]
    pub fn caret(
        self,
        position: &ProjectedTextPosition,
    ) -> Option<SceneCaret<ProjectedTextPosition>> {
        self.scene.caret(position)
    }

    /// Moves one position through the retained adapter movement graph.
    #[must_use]
    pub fn move_position(
        self,
        position: &ProjectedTextPosition,
        movement: TextMovement,
    ) -> Option<ProjectedTextPosition> {
        self.scene.move_position(position, movement)
    }

    /// Resolves highlight rectangles for the selected range inside preedit.
    pub fn composition_selection_geometry(
        self,
        session: &CompositionSession,
    ) -> Result<Vec<SceneCompositionRect>, CompositionError> {
        self.scene.composition_selection_geometry(session)
    }

    /// Resolves visual rectangles covering the complete generated preedit.
    pub fn composition_geometry(
        self,
        session: &CompositionSession,
    ) -> Result<Vec<SceneCompositionRect>, CompositionError> {
        self.scene.composition_geometry(session)
    }
}
