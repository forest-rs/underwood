// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Capability-checked operation sessions over immutable scenes.

use super::*;

/// Exact point interaction over a committed text scene.
#[derive(Clone, Copy, Debug)]
pub struct SceneInteraction<'a> {
    scene: &'a TextScene,
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

    /// Returns the first logical caret retained by this scene's feature policy.
    #[must_use]
    pub fn start_position(self) -> Option<SnapshotTextPosition> {
        self.scene.start_position()
    }

    /// Returns the final logical caret retained by this scene's feature policy.
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

    /// Moves every selection through the represented cursor topology.
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

    /// Moves one position through the represented cursor topology.
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
