// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Deterministic editor policy for the native showcase.

use std::fmt::Write as _;

use underwood::{
    CompositionId, CompositionScene, CompositionSession, CompositionUpdate, DocumentRevision,
    EditableSurface, EditableSurfaceElement, Point, Rect, SemanticId, SnapshotTextPosition,
    SnapshotTextSelectionSet, TextId, TextMovement, TextScene, TextSelectionMode,
};

use crate::content::{ActionVisual, ShowcaseContent};
use crate::presentation::{EditorOverlay, SelectionOverlay};

/// Example-owned action associated with one semantic document node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ShowcaseAction {
    VisitUrl {
        label: &'static str,
        url: &'static str,
    },
}

impl ShowcaseAction {
    /// Creates one URL-shaped host action without adding URL semantics to core.
    pub(crate) const fn visit_url(label: &'static str, url: &'static str) -> Self {
        Self::VisitUrl { label, url }
    }

    /// Returns the user-visible action label.
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::VisitUrl { label, .. } => label,
        }
    }

    /// Returns the example-owned URL target.
    pub(crate) const fn url(self) -> &'static str {
        match self {
            Self::VisitUrl { url, .. } => url,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ActionEntry {
    semantic: SemanticId,
    action: ShowcaseAction,
}

/// Scene-bound mapping from semantic identities to application actions.
#[derive(Clone, Debug, Default)]
pub(crate) struct ActionRegistry {
    revision: Option<DocumentRevision>,
    entries: Vec<ActionEntry>,
}

impl ActionRegistry {
    /// Resolves authored text leaves to semantic identities in one exact scene.
    pub(crate) fn bind(
        scene: &TextScene,
        bindings: impl IntoIterator<Item = (TextId, ShowcaseAction)>,
    ) -> Result<Self, String> {
        let mut entries = Vec::new();
        for (text, action) in bindings {
            let semantic = scene
                .semantics()
                .find(|semantic| {
                    semantic
                        .source()
                        .is_some_and(|source| source.text() == text)
                })
                .map(underwood::SemanticFragment::semantic_id)
                .ok_or_else(|| String::from("action text is absent from the prepared scene"))?;
            if entries
                .iter()
                .any(|entry: &ActionEntry| entry.semantic == semantic)
            {
                return Err(String::from(
                    "more than one action targets the same semantic node",
                ));
            }
            entries.push(ActionEntry { semantic, action });
        }
        Ok(Self {
            revision: Some(scene.revision()),
            entries,
        })
    }

    fn action(&self, revision: DocumentRevision, semantic: SemanticId) -> Option<ShowcaseAction> {
        (self.revision == Some(revision))
            .then(|| {
                self.entries
                    .iter()
                    .find(|entry| entry.semantic == semantic)
                    .map(|entry| entry.action)
            })
            .flatten()
    }
}

/// Pointer icon requested by showcase interaction policy.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum PointerAffordance {
    #[default]
    Arrow,
    Text,
    Action,
}

/// Host-directed effects produced by one editor event.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct EditorResponse {
    pub(crate) activation: Option<ShowcaseAction>,
    pub(crate) pointer: PointerAffordance,
}

/// Modifier state attached to one pointer or keyboard gesture.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct InputModifiers {
    pub(crate) extend: bool,
    pub(crate) add: bool,
}

/// Press/release state for the primary pointer button.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PointerState {
    Pressed,
    Released,
}

/// Editor commands translated from platform key conventions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EditorKey {
    MoveLeft,
    MoveRight,
    Backspace,
    Delete,
    Enter,
}

/// Toolkit-neutral projection of Winit's event-feed IME model.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ImeInput {
    Enabled,
    Preedit {
        text: String,
        selection: Option<(usize, usize)>,
    },
    Commit(String),
    Disabled,
}

/// One input observation delivered by the native host.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum EditorEvent {
    Focused(bool),
    PointerMoved(Point),
    PointerButton {
        state: PointerState,
        point: Point,
        modifiers: InputModifiers,
    },
    Key {
        key: EditorKey,
        extend: bool,
    },
    Text(String),
    Ime(ImeInput),
}

#[derive(Clone, Debug)]
struct DragSelection {
    anchor: SnapshotTextPosition,
}

#[derive(Clone, Copy, Debug)]
struct ActionHit {
    semantic: SemanticId,
    action: ShowcaseAction,
    position: SnapshotTextPosition,
}

#[derive(Clone, Copy, Debug)]
struct PressedAction {
    hit: ActionHit,
    revision: DocumentRevision,
    origin: Point,
}

#[derive(Clone, Copy, Debug)]
struct ActionHandling {
    response: EditorResponse,
    visual: ActionVisual,
    consumed: bool,
    selection_anchor: Option<SnapshotTextPosition>,
}

#[derive(Clone, Debug, Default)]
struct SemanticActionState {
    hovered: Option<ActionHit>,
    pressed: Option<PressedAction>,
    selecting: bool,
    status: Option<String>,
}

const ACTION_DRAG_THRESHOLD: f64 = 4.0;

impl SemanticActionState {
    fn handle(
        &mut self,
        event: &EditorEvent,
        scene: Option<&TextScene>,
        registry: &ActionRegistry,
    ) -> ActionHandling {
        match event {
            EditorEvent::Focused(false) => {
                self.hovered = None;
                self.pressed = None;
                self.selecting = false;
                self.status = None;
                self.handling(false, None, PointerAffordance::Arrow)
            }
            EditorEvent::PointerMoved(point) => {
                if self.selecting {
                    self.hovered = None;
                    return self.handling(false, None, PointerAffordance::Text);
                }
                self.hovered = exact_action(scene, registry, *point);
                if let Some(pressed) = self.pressed {
                    if point.distance(pressed.origin) >= ACTION_DRAG_THRESHOLD {
                        self.hovered = None;
                        self.pressed = None;
                        self.selecting = true;
                        self.status = None;
                        let mut handling = self.handling(false, None, PointerAffordance::Text);
                        handling.selection_anchor = Some(pressed.hit.position);
                        return handling;
                    }
                    if self
                        .hovered
                        .is_some_and(|hovered| hovered.semantic == pressed.hit.semantic)
                    {
                        self.status = Some(format!(
                            "pressed semantic action: {}",
                            pressed.hit.action.label()
                        ));
                    } else {
                        self.status = Some(format!(
                            "release away to cancel: {}",
                            pressed.hit.action.label()
                        ));
                    }
                } else {
                    self.status = self
                        .hovered
                        .map(|hovered| format!("semantic action: {}", hovered.action.label()));
                }
                self.handling(false, None, pointer_affordance(scene, registry, *point))
            }
            EditorEvent::PointerButton {
                state: PointerState::Pressed,
                point,
                modifiers,
            } if *modifiers == InputModifiers::default() => {
                let Some(hit) = exact_action(scene, registry, *point) else {
                    self.hovered = None;
                    self.pressed = None;
                    self.status = None;
                    return self.handling(false, None, pointer_affordance(scene, registry, *point));
                };
                let revision = scene
                    .expect("an exact action hit requires a committed scene")
                    .revision();
                self.hovered = Some(hit);
                self.pressed = Some(PressedAction {
                    hit,
                    revision,
                    origin: *point,
                });
                self.status = Some(format!("pressed semantic action: {}", hit.action.label()));
                self.handling(true, None, PointerAffordance::Action)
            }
            EditorEvent::PointerButton {
                state: PointerState::Released,
                point,
                ..
            } => {
                if self.selecting {
                    self.selecting = false;
                    self.hovered = exact_action(scene, registry, *point);
                    self.status = None;
                    return self.handling(false, None, pointer_affordance(scene, registry, *point));
                }
                let Some(pressed) = self.pressed.take() else {
                    return self.handling(false, None, pointer_affordance(scene, registry, *point));
                };
                self.hovered = exact_action(scene, registry, *point);
                let activation = self.hovered.and_then(|hovered| {
                    let scene = scene?;
                    (point.distance(pressed.origin) < ACTION_DRAG_THRESHOLD
                        && scene.revision() == pressed.revision
                        && hovered.semantic == pressed.hit.semantic
                        && hovered.action == pressed.hit.action)
                        .then_some(pressed.hit.action)
                });
                if let Some(action) = activation {
                    self.status = Some(format!("activation requested: {}", action.label()));
                } else {
                    self.status = Some(format!(
                        "semantic action cancelled: {}",
                        pressed.hit.action.label()
                    ));
                }
                self.handling(
                    true,
                    activation,
                    pointer_affordance(scene, registry, *point),
                )
            }
            _ => {
                let pointer = self
                    .hovered
                    .map_or(PointerAffordance::Arrow, |_| PointerAffordance::Action);
                self.handling(false, None, pointer)
            }
        }
    }

    fn handling(
        &self,
        consumed: bool,
        activation: Option<ShowcaseAction>,
        pointer: PointerAffordance,
    ) -> ActionHandling {
        let visual = if self.pressed.is_some()
            && self.hovered.is_some_and(|hovered| {
                self.pressed
                    .is_some_and(|pressed| pressed.hit.semantic == hovered.semantic)
            }) {
            ActionVisual::Pressed
        } else if self.hovered.is_some() {
            ActionVisual::Hovered
        } else {
            ActionVisual::Idle
        };
        ActionHandling {
            response: EditorResponse {
                activation,
                pointer,
            },
            visual,
            consumed,
            selection_anchor: None,
        }
    }

    fn status(&self) -> Option<&str> {
        self.status.as_deref()
    }
}

fn exact_action(
    scene: Option<&TextScene>,
    registry: &ActionRegistry,
    point: Point,
) -> Option<ActionHit> {
    let scene = scene?;
    let hit = scene.hit_test(point)?;
    registry
        .action(scene.revision(), hit.semantic_id())
        .map(|action| ActionHit {
            semantic: hit.semantic_id(),
            action,
            position: *hit.position(),
        })
}

fn pointer_affordance(
    scene: Option<&TextScene>,
    registry: &ActionRegistry,
    point: Point,
) -> PointerAffordance {
    let Some(scene) = scene else {
        return PointerAffordance::Arrow;
    };
    let Some(hit) = scene.hit_test(point) else {
        return PointerAffordance::Arrow;
    };
    if registry
        .action(scene.revision(), hit.semantic_id())
        .is_some()
    {
        PointerAffordance::Action
    } else {
        PointerAffordance::Text
    }
}

/// Showcase-owned focus, gesture, selection, and native composition state.
#[derive(Clone, Debug)]
pub(crate) struct EditorState {
    selections: Option<SnapshotTextSelectionSet>,
    drag: Option<DragSelection>,
    composition: Option<CompositionSession>,
    focused: bool,
    ime_enabled: bool,
    next_composition: u128,
    status: String,
    actions: SemanticActionState,
}

impl Default for EditorState {
    fn default() -> Self {
        Self {
            selections: None,
            drag: None,
            composition: None,
            focused: false,
            ime_enabled: false,
            next_composition: 1,
            status: String::from("click text to place an exact caret"),
            actions: SemanticActionState::default(),
        }
    }
}

impl EditorState {
    /// Applies one host event against the latest committed scene.
    #[cfg(test)]
    pub(crate) fn handle(
        &mut self,
        event: EditorEvent,
        content: &mut ShowcaseContent,
        scene: Option<&TextScene>,
    ) -> EditorResponse {
        self.handle_with_actions(event, content, scene, &ActionRegistry::default())
    }

    /// Applies one host event with an exact scene-bound semantic action registry.
    pub(crate) fn handle_with_actions(
        &mut self,
        event: EditorEvent,
        content: &mut ShowcaseContent,
        scene: Option<&TextScene>,
        registry: &ActionRegistry,
    ) -> EditorResponse {
        let action = self.actions.handle(&event, scene, registry);
        content.set_action_visual(action.visual);
        if let Some(anchor) = action.selection_anchor {
            let result = scene.map_or_else(
                || Err(String::from("action drag lost its committed scene")),
                |scene| self.begin_drag_at_position(scene, anchor),
            );
            if let Err(error) = result {
                self.status = error;
                return action.response;
            }
        }
        if action.consumed {
            return action.response;
        }
        let result = match event {
            EditorEvent::Focused(focused) => {
                self.focused = focused;
                if !focused {
                    self.cancel_composition();
                    self.drag = None;
                }
                Ok(())
            }
            EditorEvent::PointerMoved(point) => {
                scene.map_or(Ok(()), |scene| self.pointer_moved(scene, point))
            }
            EditorEvent::PointerButton {
                state,
                point,
                modifiers,
            } => scene.map_or(Ok(()), |scene| match state {
                PointerState::Pressed => self.pointer_pressed(scene, point, modifiers),
                PointerState::Released => self.pointer_released(scene, point),
            }),
            EditorEvent::Key { key, extend } => {
                scene.map_or(Ok(()), |scene| self.key(content, scene, key, extend))
            }
            EditorEvent::Text(text) => {
                scene.map_or(Ok(()), |scene| self.insert(content, scene, &text))
            }
            EditorEvent::Ime(ime) => scene.map_or(Ok(()), |scene| self.ime(content, scene, ime)),
        };
        if let Err(error) = result {
            self.status = error;
        }
        action.response
    }

    /// Returns whether focus policy requires caret animation frames.
    pub(crate) fn caret_animation_enabled(&self) -> bool {
        self.focused && self.selections.is_some()
    }

    /// Returns terse, live interaction evidence for the window title.
    pub(crate) fn status(&self) -> &str {
        self.actions.status().unwrap_or(&self.status)
    }

    /// Records a host-side preparation failure without panicking the event loop.
    pub(crate) fn report_error(&mut self, error: impl Into<String>) {
        self.status = error.into();
    }

    /// Returns the current committed selection set.
    #[cfg(test)]
    pub(crate) fn selections(&self) -> Option<&SnapshotTextSelectionSet> {
        self.selections.as_ref()
    }

    /// Returns the active generated-text session.
    pub(crate) fn composition(&self) -> Option<&CompositionSession> {
        self.composition.as_ref()
    }

    /// Clears host interaction state after a document reset.
    pub(crate) fn reset(&mut self) {
        let focused = self.focused;
        let ime_enabled = self.ime_enabled;
        *self = Self::default();
        self.focused = focused;
        self.ime_enabled = ime_enabled;
    }

    /// Builds committed selection and caret geometry from the exact scene map.
    pub(crate) fn committed_overlay(
        &self,
        scene: &TextScene,
        caret_visible: bool,
    ) -> EditorOverlay {
        let Some(selections) = self
            .selections
            .as_ref()
            .filter(|selections| selections.revision() == scene.revision())
        else {
            return EditorOverlay::default();
        };
        let selection_geometry = scene.selection_geometry(selections).unwrap_or_default();
        let carets = selections
            .selections()
            .iter()
            .filter_map(|selection| scene.caret(selection.extent()))
            .map(|caret| caret.bounds())
            .collect();
        EditorOverlay {
            selections: selection_geometry
                .into_iter()
                .map(|rect| SelectionOverlay {
                    bounds: rect.bounds(),
                    selection: rect.selection(),
                })
                .collect(),
            carets,
            caret_visible: caret_visible && self.focused,
            ..EditorOverlay::default()
        }
    }

    /// Builds marked-text, selected-clause, and caret geometry for one epoch.
    pub(crate) fn composition_overlay(
        &self,
        content: &ShowcaseContent,
        scene: &CompositionScene,
        caret_visible: bool,
    ) -> EditorOverlay {
        let Some(composition) = self.composition.as_ref() else {
            return EditorOverlay::default();
        };
        let marked_text = scene
            .composition_geometry(composition)
            .unwrap_or_default()
            .into_iter()
            .map(|rect| rect.bounds())
            .collect();
        let preedit_selection = scene
            .composition_selection_geometry(composition)
            .unwrap_or_default()
            .into_iter()
            .map(|rect| rect.bounds())
            .collect();
        let carets = composition_caret(content, scene, composition)
            .into_iter()
            .collect();
        EditorOverlay {
            marked_text,
            preedit_selection,
            carets,
            caret_visible: caret_visible && self.focused,
            ..EditorOverlay::default()
        }
    }

    /// Returns the current scene-space primary caret for the native candidate window.
    pub(crate) fn ime_cursor_rect(
        &self,
        content: &ShowcaseContent,
        committed: Option<&TextScene>,
        composition_scene: Option<&CompositionScene>,
    ) -> Option<Rect> {
        if let (Some(scene), Some(composition)) = (composition_scene, self.composition.as_ref()) {
            return composition_caret(content, scene, composition);
        }
        let scene = committed?;
        let primary = self.selections.as_ref()?.primary()?;
        scene.caret(primary.extent()).map(|caret| caret.bounds())
    }

    fn pointer_pressed(
        &mut self,
        scene: &TextScene,
        point: Point,
        modifiers: InputModifiers,
    ) -> Result<(), String> {
        self.cancel_composition();
        let hit = scene
            .hit_test_closest(point)
            .ok_or_else(|| String::from("no selectable text at pointer"))?;
        let position = *hit.position();
        let mut siblings = self
            .current_selections(scene)
            .map(|selections| selections.selections().to_vec())
            .unwrap_or_default();

        let primary = if modifiers.extend {
            let anchor = siblings
                .first()
                .map_or(position, |selection| *selection.anchor());
            let selection = scene
                .selection(&anchor, &position, TextSelectionMode::Visual)
                .map_err(|error| format!("visual extension rejected: {error}"))?;
            if siblings.is_empty() {
                siblings.push(selection.clone());
            } else {
                siblings[0] = selection.clone();
            }
            selection
        } else {
            scene
                .collapsed_selection(&position)
                .map_err(|error| format!("caret placement rejected: {error}"))?
        };

        if !modifiers.extend {
            if modifiers.add {
                siblings.insert(0, primary.clone());
            } else {
                siblings.clear();
                siblings.push(primary.clone());
            }
        }
        self.selections = Some(
            scene
                .selection_set(siblings)
                .map_err(|error| format!("independent caret rejected: {error}"))?,
        );
        self.drag = Some(DragSelection {
            anchor: *primary.anchor(),
        });
        self.describe_selection("pointer");
        Ok(())
    }

    fn begin_drag_at_position(
        &mut self,
        scene: &TextScene,
        anchor: SnapshotTextPosition,
    ) -> Result<(), String> {
        self.cancel_composition();
        let selection = scene
            .collapsed_selection(&anchor)
            .map_err(|error| format!("action drag anchor rejected: {error}"))?;
        self.selections = Some(
            scene
                .selection_set([selection])
                .map_err(|error| format!("action drag selection rejected: {error}"))?,
        );
        self.drag = Some(DragSelection { anchor });
        Ok(())
    }

    fn pointer_moved(&mut self, scene: &TextScene, point: Point) -> Result<(), String> {
        let Some(drag) = self.drag.clone() else {
            return Ok(());
        };
        self.extend_drag(scene, point, drag.anchor)
    }

    fn pointer_released(&mut self, scene: &TextScene, point: Point) -> Result<(), String> {
        let Some(drag) = self.drag.take() else {
            return Ok(());
        };
        self.extend_drag(scene, point, drag.anchor)
    }

    fn extend_drag(
        &mut self,
        scene: &TextScene,
        point: Point,
        anchor: SnapshotTextPosition,
    ) -> Result<(), String> {
        let hit = scene
            .hit_test_closest(point)
            .ok_or_else(|| String::from("drag left selectable text"))?;
        let primary = scene
            .selection(&anchor, hit.position(), TextSelectionMode::Visual)
            .map_err(|error| format!("visual drag rejected: {error}"))?;
        let mut selections = self
            .current_selections(scene)
            .map(|selections| selections.selections().to_vec())
            .unwrap_or_default();
        if selections.is_empty() {
            selections.push(primary);
        } else {
            selections[0] = primary;
        }
        self.selections = Some(
            scene
                .selection_set(selections)
                .map_err(|error| format!("visual drag overlaps another caret: {error}"))?,
        );
        self.describe_selection("visual drag");
        Ok(())
    }

    fn key(
        &mut self,
        content: &mut ShowcaseContent,
        scene: &TextScene,
        key: EditorKey,
        extend: bool,
    ) -> Result<(), String> {
        if self.composition.is_some() {
            return Ok(());
        }
        match key {
            EditorKey::MoveLeft | EditorKey::MoveRight => {
                let movement = if key == EditorKey::MoveLeft {
                    TextMovement::PreviousVisual
                } else {
                    TextMovement::NextVisual
                };
                let selections = self
                    .current_selections(scene)
                    .ok_or_else(|| String::from("click text before moving the caret"))?;
                self.selections = Some(
                    scene
                        .move_selections(selections, movement, extend)
                        .map_err(|error| format!("caret movement rejected: {error}"))?,
                );
                self.describe_selection(if extend {
                    "keyboard selection"
                } else {
                    "caret move"
                });
                Ok(())
            }
            EditorKey::Backspace => self.delete(content, scene, TextMovement::PreviousLogical),
            EditorKey::Delete => self.delete(content, scene, TextMovement::NextLogical),
            EditorKey::Enter => self.insert(content, scene, "\n"),
        }
    }

    fn insert(
        &mut self,
        content: &mut ShowcaseContent,
        scene: &TextScene,
        text: &str,
    ) -> Result<(), String> {
        if text.is_empty() || self.composition.is_some() {
            return Ok(());
        }
        let selections = self
            .current_selections(scene)
            .ok_or_else(|| String::from("click text before typing"))?;
        let applied = content
            .replace_selections(selections, text)
            .map_err(|error| format!("text transaction rejected: {error}"))?;
        self.selections = Some(applied.selections);
        self.status = format!(
            "atomic insert at {} carets; {} paragraph(s) changed",
            self.selection_count(),
            applied.changed_paragraphs
        );
        Ok(())
    }

    fn delete(
        &mut self,
        content: &mut ShowcaseContent,
        scene: &TextScene,
        movement: TextMovement,
    ) -> Result<(), String> {
        let selections = self
            .current_selections(scene)
            .ok_or_else(|| String::from("click text before deleting"))?;
        let mut deletion = Vec::with_capacity(selections.selections().len());
        for selection in selections.selections() {
            if selection.is_collapsed() {
                let single = scene
                    .selection_set([selection.clone()])
                    .map_err(|error| format!("deletion source rejected: {error}"))?;
                let extended = scene
                    .move_selections(&single, movement, true)
                    .map_err(|error| format!("logical deletion movement rejected: {error}"))?;
                deletion.extend_from_slice(extended.selections());
            } else {
                deletion.push(selection.clone());
            }
        }
        let deletion = scene
            .selection_set(deletion)
            .map_err(|error| format!("combined deletion rejected: {error}"))?;
        let applied = content
            .replace_selections(&deletion, "")
            .map_err(|error| format!("deletion transaction rejected: {error}"))?;
        self.selections = Some(applied.selections);
        self.status = format!(
            "logical grapheme delete at {} carets; {} paragraph(s) changed",
            self.selection_count(),
            applied.changed_paragraphs
        );
        Ok(())
    }

    fn ime(
        &mut self,
        content: &mut ShowcaseContent,
        scene: &TextScene,
        input: ImeInput,
    ) -> Result<(), String> {
        match input {
            ImeInput::Enabled => {
                self.ime_enabled = true;
                self.status = String::from("native IME event feed enabled");
                Ok(())
            }
            ImeInput::Preedit { text, selection } => {
                self.ensure_composition(scene)?;
                let session = self
                    .composition
                    .as_mut()
                    .expect("composition was just initialized");
                let mut update = CompositionUpdate::new(text);
                if let Some((start, end)) = selection {
                    let start = u32::try_from(start)
                        .map_err(|_| String::from("IME selection start exceeds u32"))?;
                    let end = u32::try_from(end)
                        .map_err(|_| String::from("IME selection end exceeds u32"))?;
                    update = update.with_selection(start..end);
                }
                let epoch = session
                    .update(session.epoch(), update)
                    .map_err(|error| format!("preedit update rejected: {error}"))?;
                self.status = format!(
                    "IME preedit epoch {} ({} UTF-8 bytes, document unchanged)",
                    epoch.get(),
                    session.text().len()
                );
                Ok(())
            }
            ImeInput::Commit(text) => {
                let applied = if let Some(composition) = self.composition.take() {
                    content
                        .commit_composition(composition, &text)
                        .map_err(|error| format!("IME commit rejected: {error}"))?
                } else {
                    let selections = self
                        .current_selections(scene)
                        .ok_or_else(|| String::from("click text before IME commit"))?;
                    content
                        .replace_selections(selections, &text)
                        .map_err(|error| format!("IME insertion rejected: {error}"))?
                };
                self.selections = Some(applied.selections);
                self.status = format!(
                    "IME committed once; {} paragraph(s) changed",
                    applied.changed_paragraphs
                );
                Ok(())
            }
            ImeInput::Disabled => {
                self.ime_enabled = false;
                self.cancel_composition();
                self.status = String::from("native IME disabled; transient preedit cancelled");
                Ok(())
            }
        }
    }

    fn ensure_composition(&mut self, scene: &TextScene) -> Result<(), String> {
        if self.composition.is_some() {
            return Ok(());
        }
        let selections = self
            .current_selections(scene)
            .ok_or_else(|| String::from("click text before starting IME"))?
            .clone();
        let id = CompositionId::from_bytes(self.next_composition.to_be_bytes());
        self.next_composition = self.next_composition.wrapping_add(1).max(1);
        let start = scene
            .begin_composition(&selections, id)
            .map_err(|error| format!("composition start rejected: {error}"))?;
        let normalized = start.selections().clone();
        let changed = start.selection_changed();
        self.composition = Some(start.into_session());
        self.selections = Some(normalized);
        if changed {
            self.status = String::from("IME normalized multiple selections to the primary caret");
        }
        Ok(())
    }

    fn cancel_composition(&mut self) {
        if let Some(composition) = self.composition.take() {
            self.selections = Some(composition.cancel());
        }
    }

    fn current_selections(&self, scene: &TextScene) -> Option<&SnapshotTextSelectionSet> {
        self.selections.as_ref().filter(|selections| {
            selections.document() == scene.document() && selections.revision() == scene.revision()
        })
    }

    fn selection_count(&self) -> usize {
        self.selections
            .as_ref()
            .map_or(0, |selections| selections.selections().len())
    }

    fn describe_selection(&mut self, action: &str) {
        let Some(selections) = self.selections.as_ref() else {
            return;
        };
        let range_count = selections
            .primary()
            .map_or(0, |selection| selection.ranges().len());
        self.status.clear();
        let _ = write!(
            self.status,
            "{action}: {} independent selection(s), primary has {range_count} logical range(s)",
            selections.selections().len()
        );
    }
}

fn composition_caret(
    content: &ShowcaseContent,
    scene: &CompositionScene,
    composition: &CompositionSession,
) -> Option<Rect> {
    let text = composition.target().primary()?.ranges().first()?.text();
    let surface =
        EditableSurface::new(&content.snapshot(), [EditableSurfaceElement::text(text)]).ok()?;
    surface
        .bind_composition(scene, composition)
        .ok()?
        .caret_rect()
}

#[cfg(test)]
mod tests {
    use super::{
        ACTION_DRAG_THRESHOLD, ActionRegistry, EditorEvent, EditorKey, EditorState, ImeInput,
        InputModifiers, PointerAffordance, PointerState, ShowcaseAction,
    };
    use crate::content::ShowcaseContent;
    use underwood::Point;

    const TEST_ACTION: ShowcaseAction = ShowcaseAction::visit_url(
        "forest-rs/underwood on GitHub",
        "https://github.com/forest-rs/underwood",
    );

    #[test]
    fn wrapped_bidi_semantic_action_activates_from_exact_hits() {
        let mut content = ShowcaseContent::new_deterministic().expect("showcase must initialize");
        let initial = content.prepare(300.0, 0.5).expect("scene must prepare");
        let action_text = content.action_text();
        let registry = ActionRegistry::bind(&initial.scene, [(action_text, TEST_ACTION)])
            .expect("authored action must bind to one semantic node");
        let points = exact_text_hits(&initial.scene, action_text);
        let lines: std::collections::BTreeSet<_> =
            points.iter().map(|(line, _, _)| *line).collect();
        let levels: std::collections::BTreeSet<_> =
            points.iter().map(|(_, level, _)| level & 1).collect();
        assert!(lines.len() > 1, "actionable semantic leaf must wrap");
        assert_eq!(levels, [0, 1].into_iter().collect());

        for point in [
            points.first().expect("action must expose an exact hit").2,
            points.last().expect("action must expose an exact hit").2,
        ] {
            let mut editor = EditorState::default();
            let hover = editor.handle_with_actions(
                EditorEvent::PointerMoved(point),
                &mut content,
                Some(&initial.scene),
                &registry,
            );
            assert_eq!(hover.pointer, PointerAffordance::Action);
            let pressed = editor.handle_with_actions(
                EditorEvent::PointerButton {
                    state: PointerState::Pressed,
                    point,
                    modifiers: InputModifiers::default(),
                },
                &mut content,
                Some(&initial.scene),
                &registry,
            );
            assert_eq!(pressed.activation, None);
            let released = editor.handle_with_actions(
                EditorEvent::PointerButton {
                    state: PointerState::Released,
                    point,
                    modifiers: InputModifiers::default(),
                },
                &mut content,
                Some(&initial.scene),
                &registry,
            );
            assert_eq!(released.activation, Some(TEST_ACTION));
            assert!(
                editor.selections().is_none(),
                "action press must not place a caret"
            );
        }

        let hovered = content
            .prepare(300.0, 0.5)
            .expect("hover paint must prepare without changing text physics");
        assert_eq!(hovered.work.shape().paragraphs(), 0);
        assert_eq!(hovered.work.flow().paragraphs(), 0);
        assert!(hovered.work.paint().paragraphs() > 0);
    }

    #[test]
    fn semantic_action_cancels_outside_and_non_action_text_still_edits() {
        let mut content = ShowcaseContent::new_deterministic().expect("showcase must initialize");
        let initial = content.prepare(420.0, 0.5).expect("scene must prepare");
        let registry = ActionRegistry::bind(&initial.scene, [(content.action_text(), TEST_ACTION)])
            .expect("authored action must bind to one semantic node");
        let action = exact_text_hits(&initial.scene, content.action_text())[0].2;
        let editable = two_points_in_text_leaf(&initial.scene, content.editable_text()).0;
        let mut editor = EditorState::default();
        editor.handle_with_actions(
            EditorEvent::PointerButton {
                state: PointerState::Pressed,
                point: action,
                modifiers: InputModifiers::default(),
            },
            &mut content,
            Some(&initial.scene),
            &registry,
        );
        let cancelled = editor.handle_with_actions(
            EditorEvent::PointerButton {
                state: PointerState::Released,
                point: editable,
                modifiers: InputModifiers::default(),
            },
            &mut content,
            Some(&initial.scene),
            &registry,
        );
        assert_eq!(cancelled.activation, None);
        assert_eq!(cancelled.pointer, PointerAffordance::Text);
        assert!(editor.status().contains("cancelled"));
        assert!(editor.selections().is_none());

        editor.handle_with_actions(
            EditorEvent::PointerButton {
                state: PointerState::Pressed,
                point: editable,
                modifiers: InputModifiers::default(),
            },
            &mut content,
            Some(&initial.scene),
            &registry,
        );
        editor.handle_with_actions(
            EditorEvent::PointerButton {
                state: PointerState::Released,
                point: editable,
                modifiers: InputModifiers::default(),
            },
            &mut content,
            Some(&initial.scene),
            &registry,
        );
        assert!(
            editor.selections().is_some(),
            "a non-action exact hit must continue through editor policy"
        );
    }

    #[test]
    fn dragging_from_action_text_selects_instead_of_activating() {
        let mut content = ShowcaseContent::new_deterministic().expect("showcase must initialize");
        let initial = content.prepare(300.0, 0.5).expect("scene must prepare");
        let registry = ActionRegistry::bind(&initial.scene, [(content.action_text(), TEST_ACTION)])
            .expect("authored action must bind to one semantic node");
        let points = caret_points(&initial.scene, content.action_text());
        let (start, start_position) = points
            .iter()
            .find(|(point, position)| {
                position.byte() == 0
                    && initial
                        .scene
                        .hit_test(*point)
                        .is_some_and(|hit| hit.bidi_level() & 1 == 0)
            })
            .copied()
            .expect("action must expose its leading Latin caret");
        let end = points
            .iter()
            .find(|(point, position)| {
                if initial
                    .scene
                    .hit_test(*point)
                    .is_none_or(|hit| hit.bidi_level() & 1 != 1)
                {
                    return false;
                }
                initial
                    .scene
                    .selection(
                        position,
                        &start_position,
                        underwood::TextSelectionMode::Visual,
                    )
                    .and_then(|selection| initial.scene.selection_set([selection]))
                    .and_then(|selections| initial.scene.selection_geometry(&selections))
                    .is_ok_and(|geometry| geometry.iter().any(|rect| rect.bidi_level() & 1 == 1))
            })
            .map(|(point, _)| *point)
            .expect("action must expose an Arabic selection interior");
        assert!(start.distance(end) >= ACTION_DRAG_THRESHOLD);
        let mut editor = EditorState::default();
        editor.handle_with_actions(
            EditorEvent::PointerButton {
                state: PointerState::Pressed,
                point: start,
                modifiers: InputModifiers::default(),
            },
            &mut content,
            Some(&initial.scene),
            &registry,
        );
        editor.handle_with_actions(
            EditorEvent::PointerMoved(end),
            &mut content,
            Some(&initial.scene),
            &registry,
        );
        let released = editor.handle_with_actions(
            EditorEvent::PointerButton {
                state: PointerState::Released,
                point: end,
                modifiers: InputModifiers::default(),
            },
            &mut content,
            Some(&initial.scene),
            &registry,
        );

        assert_eq!(released.activation, None, "a drag must not follow the link");
        let selections = editor
            .selections()
            .expect("dragging from a link must become a text selection");
        let geometry = initial
            .scene
            .selection_geometry(selections)
            .expect("selection geometry must resolve");
        assert!(
            geometry.iter().any(|rect| rect.bidi_level() & 1 == 1),
            "the drag must highlight the Arabic run: {geometry:?}; selection={selections:?}; status={}",
            editor.status()
        );
    }

    #[test]
    fn two_pointer_carets_publish_one_atomic_insertion() {
        let mut content = ShowcaseContent::new_deterministic().expect("showcase must initialize");
        let initial = content.prepare(760.0, 0.5).expect("scene must prepare");
        let (first, second) = two_points_in_text_leaf(&initial.scene, content.editable_text());
        let mut editor = EditorState::default();
        editor.handle(
            EditorEvent::Focused(true),
            &mut content,
            Some(&initial.scene),
        );
        click(&mut editor, &mut content, &initial.scene, first, false);
        click(&mut editor, &mut content, &initial.scene, second, true);
        assert_eq!(
            editor
                .selections()
                .expect("two carets must exist")
                .selections()
                .len(),
            2,
            "{}",
            editor.status()
        );

        editor.handle(
            EditorEvent::Text(String::from("X")),
            &mut content,
            Some(&initial.scene),
        );
        let revision = content.snapshot().revision();
        assert_eq!(
            editor
                .selections()
                .expect("post-edit carets must be returned")
                .revision(),
            revision,
            "the host must retain transaction-returned positions"
        );
        assert!(editor.status().contains("atomic insert at 2 carets"));
        let edited = content
            .prepare(760.0, 0.5)
            .expect("edited scene must prepare");
        assert_eq!(edited.work.shape().paragraphs(), 1);
        assert_eq!(edited.work.reused_paragraphs(), 9);
    }

    #[test]
    fn mixed_bidi_drag_retains_disjoint_logical_ranges() {
        let mut content = ShowcaseContent::new_deterministic().expect("showcase must initialize");
        let committed = content.prepare(760.0, 0.5).expect("scene must prepare");
        let (start, end) = disjoint_visual_points(&committed.scene, content.editable_text());
        let mut editor = EditorState::default();
        editor.handle(
            EditorEvent::Focused(true),
            &mut content,
            Some(&committed.scene),
        );
        editor.handle(
            EditorEvent::PointerButton {
                state: PointerState::Pressed,
                point: start,
                modifiers: InputModifiers::default(),
            },
            &mut content,
            Some(&committed.scene),
        );
        editor.handle(
            EditorEvent::PointerMoved(end),
            &mut content,
            Some(&committed.scene),
        );
        editor.handle(
            EditorEvent::PointerButton {
                state: PointerState::Released,
                point: end,
                modifiers: InputModifiers::default(),
            },
            &mut content,
            Some(&committed.scene),
        );
        let primary = editor
            .selections()
            .and_then(underwood::SnapshotTextSelectionSet::primary)
            .expect("visual drag must leave one primary selection");
        assert_eq!(primary.mode(), underwood::TextSelectionMode::Visual);
        assert!(
            primary.ranges().len() > 1,
            "mixed-bidi visual selection must not be flattened to one logical union"
        );
        assert!(editor.status().contains("primary has"));
    }

    #[test]
    fn backspace_removes_one_real_extended_grapheme() {
        let mut content = ShowcaseContent::new_deterministic().expect("showcase must initialize");
        let committed = content.prepare(760.0, 0.5).expect("scene must prepare");
        let text = content.editable_value();
        let cluster_end = text
            .find("e\u{301}")
            .map(|start| start + "e\u{301}".len())
            .expect("editor specimen must contain a combining grapheme");
        let point = point_for_byte(
            &committed.scene,
            content.editable_text(),
            u32::try_from(cluster_end).expect("test string fits u32"),
        );
        let mut editor = EditorState::default();
        editor.handle(
            EditorEvent::Focused(true),
            &mut content,
            Some(&committed.scene),
        );
        click(&mut editor, &mut content, &committed.scene, point, false);
        editor.handle(
            EditorEvent::Key {
                key: EditorKey::Backspace,
                extend: false,
            },
            &mut content,
            Some(&committed.scene),
        );
        let edited = content.editable_value();
        assert!(edited.contains("caf."), "edited text: {edited:?}");
        assert!(!edited.contains("cafe"), "edited text: {edited:?}");
        assert!(editor.status().contains("logical grapheme delete"));
    }

    #[test]
    fn preedit_is_transient_and_commit_publishes_once() {
        let mut content = ShowcaseContent::new_deterministic().expect("showcase must initialize");
        let committed = content.prepare(760.0, 0.5).expect("scene must prepare");
        let point = two_points_in_text_leaf(&committed.scene, content.editable_text()).0;
        let mut editor = EditorState::default();
        editor.handle(
            EditorEvent::Focused(true),
            &mut content,
            Some(&committed.scene),
        );
        click(&mut editor, &mut content, &committed.scene, point, false);
        let revision = content.snapshot().revision();
        editor.handle(
            EditorEvent::Ime(ImeInput::Preedit {
                text: String::from("مرحبا"),
                selection: Some((10, 10)),
            }),
            &mut content,
            Some(&committed.scene),
        );
        assert_eq!(content.snapshot().revision(), revision);
        let projected = content
            .prepare_composition(
                760.0,
                0.5,
                editor.composition().expect("preedit session must exist"),
            )
            .expect("preedit must prepare");
        let overlay = editor.composition_overlay(&content, &projected.scene, true);
        assert!(!overlay.marked_text.is_empty());
        assert!(!overlay.carets.is_empty());

        editor.handle(
            EditorEvent::Ime(ImeInput::Disabled),
            &mut content,
            Some(&committed.scene),
        );
        assert_eq!(content.snapshot().revision(), revision);
        assert!(editor.composition().is_none());

        editor.handle(
            EditorEvent::Ime(ImeInput::Preedit {
                text: String::from("مرحبا"),
                selection: Some((10, 10)),
            }),
            &mut content,
            Some(&committed.scene),
        );

        editor.handle(
            EditorEvent::Ime(ImeInput::Commit(String::from("مرحبا"))),
            &mut content,
            Some(&committed.scene),
        );
        assert_ne!(content.snapshot().revision(), revision);
        assert!(editor.composition().is_none());
        assert!(editor.status().contains("committed once"));
    }

    #[cfg(target_vendor = "apple")]
    #[test]
    fn native_ime_chinese_commit_prepares_through_system_fallback() {
        let mut content = ShowcaseContent::new().expect("showcase must initialize");
        let committed = content.prepare(760.0, 0.5).expect("scene must prepare");
        let point = two_points_in_text_leaf(&committed.scene, content.editable_text()).0;
        let mut editor = EditorState::default();
        editor.handle(
            EditorEvent::Focused(true),
            &mut content,
            Some(&committed.scene),
        );
        click(&mut editor, &mut content, &committed.scene, point, false);
        let revision = content.snapshot().revision();
        let chinese = "中文输入";
        editor.handle(
            EditorEvent::Ime(ImeInput::Preedit {
                text: chinese.to_owned(),
                selection: Some((chinese.len(), chinese.len())),
            }),
            &mut content,
            Some(&committed.scene),
        );
        assert_eq!(content.snapshot().revision(), revision);
        editor.handle(
            EditorEvent::Ime(ImeInput::Commit(chinese.to_owned())),
            &mut content,
            Some(&committed.scene),
        );

        let prepared = content
            .prepare(760.0, 0.5)
            .expect("committed Han text must resolve through the native font catalog");
        assert_ne!(prepared.scene.revision(), revision);
        assert!(prepared.scene.fragments().iter().any(|fragment| {
            fragment
                .source()
                .is_some_and(|source| source.text() == content.editable_text())
                && fragment.script() == *b"Hani"
        }));
        assert!(editor.status().contains("committed once"));
    }

    fn click(
        editor: &mut EditorState,
        content: &mut ShowcaseContent,
        scene: &underwood::TextScene,
        point: Point,
        add: bool,
    ) {
        let modifiers = InputModifiers { extend: false, add };
        editor.handle(
            EditorEvent::PointerButton {
                state: PointerState::Pressed,
                point,
                modifiers,
            },
            content,
            Some(scene),
        );
        editor.handle(
            EditorEvent::PointerButton {
                state: PointerState::Released,
                point,
                modifiers,
            },
            content,
            Some(scene),
        );
    }

    fn two_points_in_text_leaf(
        scene: &underwood::TextScene,
        text: underwood::TextId,
    ) -> (Point, Point) {
        let points = caret_points(scene, text);
        if let (Some(first), Some(second)) = (points.first(), points.last())
            && first.1 != second.1
        {
            return (first.0, second.0);
        }
        panic!("showcase must contain two fragments in one semantic text leaf");
    }

    fn exact_text_hits(
        scene: &underwood::TextScene,
        text: underwood::TextId,
    ) -> Vec<(usize, u8, Point)> {
        let mut hits = Vec::new();
        for (line_index, line) in scene.lines().iter().enumerate() {
            let y = line.bounds().center().y;
            let mut x = line.bounds().x0;
            while x <= line.bounds().x1 {
                let point = Point::new(x, y);
                if let Some(hit) = scene.hit_test(point)
                    && hit.position().text() == text
                    && !hits.iter().any(|(existing_line, level, _)| {
                        *existing_line == line_index && *level == hit.bidi_level()
                    })
                {
                    hits.push((line_index, hit.bidi_level(), point));
                }
                x += 0.5;
            }
        }
        hits
    }

    fn disjoint_visual_points(
        scene: &underwood::TextScene,
        text: underwood::TextId,
    ) -> (Point, Point) {
        let candidates = caret_points(scene, text);
        for (first_point, first_position) in &candidates {
            for (second_point, second_position) in &candidates {
                if scene
                    .selection(
                        first_position,
                        second_position,
                        underwood::TextSelectionMode::Visual,
                    )
                    .is_ok_and(|selection| selection.ranges().len() > 1)
                {
                    return (*first_point, *second_point);
                }
            }
        }
        panic!("mixed-bidi editor specimen must expose a disjoint visual selection");
    }

    fn point_for_byte(scene: &underwood::TextScene, text: underwood::TextId, byte: u32) -> Point {
        caret_points(scene, text)
            .into_iter()
            .find(|(_, position)| position.byte() == byte)
            .map(|(point, _)| point)
            .unwrap_or_else(|| panic!("scene must expose byte {byte} as a caret stop"))
    }

    fn caret_points(
        scene: &underwood::TextScene,
        text: underwood::TextId,
    ) -> Vec<(Point, underwood::SnapshotTextPosition)> {
        let mut points = Vec::new();
        let semantic = scene
            .semantics()
            .find(|semantic| {
                semantic
                    .source()
                    .is_some_and(|source| source.text() == text)
            })
            .expect("semantic text leaf must expose layout geometry");
        let semantic_id = semantic.semantic_id();
        let bounds = semantic.bounds();
        for line in scene.lines() {
            let line = line.bounds();
            if line.y1 <= bounds.y0 || line.y0 >= bounds.y1 {
                continue;
            }
            let y = line.center().y;
            let mut x = line.x0.max(bounds.x0);
            let end = line.x1.min(bounds.x1);
            while x <= end {
                let point = Point::new(x, y);
                if let Some(hit) = scene.hit_test(point)
                    && hit.semantic_id() == semantic_id
                {
                    let position = *hit.position();
                    if points.iter().all(|(_, existing)| *existing != position) {
                        points.push((point, position));
                    }
                }
                x += 0.5;
            }
            if let Some(point) = points.last().map(|(point, _)| *point) {
                let point = Point::new((point.x + 0.25).min(end), y);
                let Some(position) = scene
                    .hit_test_closest(point)
                    .filter(|hit| hit.semantic_id() == semantic_id)
                    .map(|hit| *hit.position())
                else {
                    continue;
                };
                if points.iter().all(|(_, existing)| *existing != position) {
                    points.push((point, position));
                }
            }
        }
        points
    }
}
