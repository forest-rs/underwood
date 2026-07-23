// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Live native proof of Underwood's retained document pipeline.

use std::time::{Duration, Instant};

use crate::content::{PreparedCompositionFrame, PreparedDocumentFrame, ShowcaseContent};
use crate::host::{self, Command, Frame, HostApplication};
use crate::interaction::{
    ActionRegistry, EditorEvent, EditorResponse, EditorState, ShowcaseAction,
};
use crate::presentation::{self, FrameLayout};
use imaging_vello_cpu::VelloCpuRenderer;
use underwood::{TextScene, WorkReport};

type AnyError = Box<dyn std::error::Error>;

const SOURCE_ACTION: ShowcaseAction = ShowcaseAction::visit_url(
    "forest-rs/underwood on GitHub",
    "https://github.com/forest-rs/underwood",
);

enum PreparedFrame {
    Committed(PreparedDocumentFrame),
    Composition(PreparedCompositionFrame),
}

pub(crate) fn run() -> Result<(), AnyError> {
    host::run(ShowcaseApp::new()?)?;
    Ok(())
}

struct ShowcaseApp {
    content: ShowcaseContent,
    renderer: VelloCpuRenderer,
    axis_animation: AxisAnimation,
    editor: EditorState,
    action_registry: ActionRegistry,
    show_guides: bool,
    last_elapsed: Duration,
    last_layout: Option<FrameLayout>,
    last_committed_scene: Option<TextScene>,
    evidence_work: Option<WorkReport>,
    capture_next_work: bool,
}

impl ShowcaseApp {
    fn new() -> Result<Self, AnyError> {
        Ok(Self {
            content: ShowcaseContent::new()?,
            renderer: VelloCpuRenderer::new(1, 1),
            axis_animation: AxisAnimation::new(),
            editor: EditorState::default(),
            action_registry: ActionRegistry::default(),
            show_guides: false,
            last_elapsed: Duration::ZERO,
            last_layout: None,
            last_committed_scene: None,
            evidence_work: None,
            capture_next_work: true,
        })
    }

    fn refresh_stale_interaction_scene(&mut self) {
        let revision = self.content.snapshot().revision();
        if self
            .last_committed_scene
            .as_ref()
            .is_some_and(|scene| scene.revision() == revision)
        {
            return;
        }
        let Some(layout) = self.last_layout else {
            return;
        };
        match self.content.prepare(
            layout.content_width,
            self.axis_animation.phase(self.last_elapsed),
        ) {
            Ok(prepared) => {
                if let Err(error) = self.remember_committed_scene(prepared.scene) {
                    self.editor.report_error(error);
                }
            }
            Err(error) => self
                .editor
                .report_error(format!("interaction reprepare failed: {error}")),
        }
    }

    fn remember_committed_scene(&mut self, scene: TextScene) -> Result<(), String> {
        self.action_registry =
            ActionRegistry::bind(&scene, [(self.content.action_text(), SOURCE_ACTION)])?;
        self.last_committed_scene = Some(scene);
        Ok(())
    }
}

impl HostApplication for ShowcaseApp {
    fn render(
        &mut self,
        width: u32,
        height: u32,
        scale_factor: f64,
        elapsed: Duration,
    ) -> Result<Frame, String> {
        self.last_elapsed = elapsed;
        let layout = FrameLayout::new(width, height, scale_factor);
        self.last_layout = Some(layout);
        let axis_phase = self.axis_animation.phase(elapsed);
        let prepare_started = Instant::now();
        let caret_visible = (elapsed.as_millis() / 530).is_multiple_of(2);
        let composition = self.editor.composition().cloned();
        let prepared = if let Some(composition) = composition.as_ref() {
            PreparedFrame::Composition(
                self.content
                    .prepare_composition(layout.content_width, axis_phase, composition)
                    .map_err(|error| error.to_string())?,
            )
        } else {
            PreparedFrame::Committed(
                self.content
                    .prepare(layout.content_width, axis_phase)
                    .map_err(|error| error.to_string())?,
            )
        };
        let prepare_ms = prepare_started.elapsed().as_secs_f64() * 1_000.0;
        let render_started = Instant::now();
        let (scene, work, line_count, axis_weight, clipped, ime_cursor_area, mode) = match prepared
        {
            PreparedFrame::Composition(prepared) => {
                let composition = composition
                    .as_ref()
                    .expect("composition frame requires the captured session");
                let overlay =
                    self.editor
                        .composition_overlay(&self.content, &prepared.scene, caret_visible);
                let scene = presentation::record_composition_frame(
                    &prepared.scene,
                    layout,
                    self.show_guides,
                    &overlay,
                )
                .map_err(|error| error.to_string())?;
                let ime_cursor = self.editor.ime_cursor_rect(
                    &self.content,
                    self.last_committed_scene.as_ref(),
                    Some(&prepared.scene),
                );
                (
                    scene,
                    prepared.work,
                    prepared.line_count,
                    prepared.axis_weight,
                    layout.lines_are_clipped(prepared.scene.lines()),
                    ime_cursor.map(|rect| layout.window_rect(rect)),
                    format!("IME {}", composition.epoch().get()),
                )
            }
            PreparedFrame::Committed(prepared) => {
                let overlay = self
                    .editor
                    .committed_overlay(&prepared.scene, caret_visible);
                let scene =
                    presentation::record_frame(&prepared.scene, layout, self.show_guides, &overlay)
                        .map_err(|error| error.to_string())?;
                let ime_cursor =
                    self.editor
                        .ime_cursor_rect(&self.content, Some(&prepared.scene), None);
                let clipped = layout.document_is_clipped(&prepared.scene);
                self.remember_committed_scene(prepared.scene)?;
                (
                    scene,
                    prepared.work,
                    prepared.line_count,
                    prepared.axis_weight,
                    clipped,
                    ime_cursor.map(|rect| layout.window_rect(rect)),
                    String::from("EDIT"),
                )
            }
        };
        let image = self
            .renderer
            .render_scene(
                &scene,
                u16::try_from(width).map_err(|error| error.to_string())?,
                u16::try_from(height).map_err(|error| error.to_string())?,
            )
            .map_err(|error| error.to_string())?;
        let render_ms = render_started.elapsed().as_secs_f64() * 1_000.0;
        if self.capture_next_work || has_text_physics_work(&work) {
            self.evidence_work = Some(work.clone());
            self.capture_next_work = false;
        }
        let evidence = self.evidence_work.as_ref().unwrap_or(&work);
        let window_title = format!(
            "Underwood — {mode} · {} · {} lines · wght {:.0} · shape {} · flow {} · paint {} · reused {} · prep {:.1} ms · render {:.1} ms{}{}",
            self.editor.status(),
            line_count,
            axis_weight,
            evidence.shape().paragraphs(),
            evidence.flow().paragraphs(),
            evidence.paint().paragraphs(),
            evidence.reused_paragraphs(),
            prepare_ms,
            render_ms,
            if self.axis_animation.is_enabled() {
                " · AXIS LIVE"
            } else {
                ""
            },
            if clipped { " · CLIPPED" } else { "" }
        );
        Ok(Frame {
            width: image.width,
            height: image.height,
            rgba: image.data,
            window_title,
            ime_cursor_area,
        })
    }

    fn command(&mut self, command: Command) {
        match command {
            Command::TogglePaint => {
                self.axis_animation.pause(self.last_elapsed);
                self.content.toggle_paint();
                self.capture_next_work = true;
            }
            Command::ToggleAxisAnimation => self.axis_animation.toggle(self.last_elapsed),
            Command::ToggleGuides => {
                self.axis_animation.pause(self.last_elapsed);
                self.show_guides = !self.show_guides;
            }
            Command::Reset => {
                self.content.reset();
                self.editor.reset();
                self.action_registry = ActionRegistry::default();
                self.axis_animation.reset();
                self.show_guides = false;
                self.last_committed_scene = None;
                self.capture_next_work = true;
            }
        }
    }

    fn editor_event(&mut self, event: EditorEvent) -> EditorResponse {
        self.refresh_stale_interaction_scene();
        let revision = self.content.snapshot().revision();
        let event = self
            .last_layout
            .map_or(event.clone(), |layout| match event {
                EditorEvent::PointerMoved(point) => {
                    EditorEvent::PointerMoved(layout.document_point(point))
                }
                EditorEvent::PointerButton {
                    state,
                    point,
                    modifiers,
                } => EditorEvent::PointerButton {
                    state,
                    point: layout.document_point(point),
                    modifiers,
                },
                other => other,
            });
        if !matches!(
            event,
            EditorEvent::Focused(_) | EditorEvent::Ime(crate::interaction::ImeInput::Enabled)
        ) {
            self.axis_animation.pause(self.last_elapsed);
        }
        let response = self.editor.handle_with_actions(
            event,
            &mut self.content,
            self.last_committed_scene.as_ref(),
            &self.action_registry,
        );
        if self.content.snapshot().revision() != revision {
            self.capture_next_work = true;
        }
        response
    }

    fn animation_enabled(&self) -> bool {
        self.axis_animation.is_enabled() || self.editor.caret_animation_enabled()
    }
}

fn has_text_physics_work(work: &WorkReport) -> bool {
    work.analysis().paragraphs() > 0
        || work.font_selection().paragraphs() > 0
        || work.shape().paragraphs() > 0
        || work.flow().paragraphs() > 0
        || work.geometry().paragraphs() > 0
}

const AXIS_RADIANS_PER_SECOND: f64 = 0.42;
const INITIAL_AXIS_ANGLE: f64 = 0.242_365_851_038_963_2;

/// A pauseable animation clock whose authored resting phase is continuous.
#[derive(Clone, Copy, Debug)]
struct AxisAnimation {
    active_since: Option<Duration>,
    accumulated: Duration,
}

impl AxisAnimation {
    const fn new() -> Self {
        Self {
            active_since: None,
            accumulated: Duration::ZERO,
        }
    }

    const fn is_enabled(self) -> bool {
        self.active_since.is_some()
    }

    fn toggle(&mut self, now: Duration) {
        if self.is_enabled() {
            self.pause(now);
        } else {
            self.active_since = Some(now);
        }
    }

    fn pause(&mut self, now: Duration) {
        if let Some(started) = self.active_since.take() {
            self.accumulated += now.saturating_sub(started);
        }
    }

    fn reset(&mut self) {
        *self = Self::new();
    }

    #[expect(
        clippy::cast_possible_truncation,
        reason = "the animation phase is finite and clamped to the f32 interval 0..=1"
    )]
    fn phase(self, now: Duration) -> f32 {
        let active = self
            .active_since
            .map_or(Duration::ZERO, |started| now.saturating_sub(started));
        let elapsed = self.accumulated + active;
        ((INITIAL_AXIS_ANGLE + elapsed.as_secs_f64() * AXIS_RADIANS_PER_SECOND).sin() * 0.5 + 0.5)
            .clamp(0.0, 1.0) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::{AXIS_RADIANS_PER_SECOND, AxisAnimation, INITIAL_AXIS_ANGLE};
    use std::f64::consts::PI;
    use std::time::Duration;

    #[test]
    fn animation_pauses_and_resumes_without_a_phase_jump() {
        let mut animation = AxisAnimation::new();
        let resting = animation.phase(Duration::ZERO);
        assert!((resting - 0.62).abs() < 0.000_01);

        animation.toggle(Duration::from_secs(2));
        assert_eq!(animation.phase(Duration::from_secs(2)), resting);
        let moving = animation.phase(Duration::from_secs(3));

        animation.toggle(Duration::from_secs(3));
        assert_eq!(animation.phase(Duration::from_secs(30)), moving);
        animation.toggle(Duration::from_secs(30));
        assert_eq!(animation.phase(Duration::from_secs(30)), moving);

        let mut uninterrupted = AxisAnimation::new();
        uninterrupted.toggle(Duration::ZERO);
        assert_eq!(
            animation.phase(Duration::from_secs(31)),
            uninterrupted.phase(Duration::from_secs(2))
        );
    }

    #[test]
    fn animation_reaches_both_authored_extrema_and_resets() {
        let mut animation = AxisAnimation::new();
        animation.toggle(Duration::ZERO);
        let peak_seconds = (PI * 0.5 - INITIAL_AXIS_ANGLE) / AXIS_RADIANS_PER_SECOND;
        let trough_seconds = (PI * 1.5 - INITIAL_AXIS_ANGLE) / AXIS_RADIANS_PER_SECOND;
        assert!((animation.phase(Duration::from_secs_f64(peak_seconds)) - 1.0).abs() < 0.000_01);
        assert!(
            animation
                .phase(Duration::from_secs_f64(trough_seconds))
                .abs()
                < 0.000_01
        );

        animation.reset();
        assert!(!animation.is_enabled());
        assert!((animation.phase(Duration::from_secs(99)) - 0.62).abs() < 0.000_01);
    }
}
