// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Live native proof of Underwood's retained document pipeline.

use std::time::{Duration, Instant};

use crate::content::ShowcaseContent;
use crate::host::{self, Command, Frame, HostApplication};
use crate::presentation::{self, FrameLayout};
use imaging_vello_cpu::VelloCpuRenderer;

type AnyError = Box<dyn std::error::Error>;

pub(crate) fn run() -> Result<(), AnyError> {
    host::run(ShowcaseApp::new()?)?;
    Ok(())
}

struct ShowcaseApp {
    content: ShowcaseContent,
    renderer: VelloCpuRenderer,
    axis_animation: AxisAnimation,
    show_guides: bool,
    last_elapsed: Duration,
}

impl ShowcaseApp {
    fn new() -> Result<Self, AnyError> {
        Ok(Self {
            content: ShowcaseContent::new()?,
            renderer: VelloCpuRenderer::new(1, 1),
            axis_animation: AxisAnimation::new(),
            show_guides: false,
            last_elapsed: Duration::ZERO,
        })
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
        let axis_phase = self.axis_animation.phase(elapsed);
        let prepare_started = Instant::now();
        let prepared = self
            .content
            .prepare(layout.content_width, axis_phase)
            .map_err(|error| error.to_string())?;
        let prepare_ms = prepare_started.elapsed().as_secs_f64() * 1_000.0;
        let render_started = Instant::now();
        let scene = presentation::record_frame(&prepared.scene, layout, self.show_guides)
            .map_err(|error| error.to_string())?;
        let image = self
            .renderer
            .render_scene(
                &scene,
                u16::try_from(width).map_err(|error| error.to_string())?,
                u16::try_from(height).map_err(|error| error.to_string())?,
            )
            .map_err(|error| error.to_string())?;
        let render_ms = render_started.elapsed().as_secs_f64() * 1_000.0;
        let work = &prepared.work;
        let clipped = layout.document_is_clipped(&prepared.scene);
        let window_title = format!(
            "Underwood — {} lines · wght {:.0} · shape {} · flow {} · paint {} · reused {} · prep {:.1} ms · render {:.1} ms{}{}",
            prepared.line_count,
            prepared.axis_weight,
            work.shape().paragraphs(),
            work.flow().paragraphs(),
            work.paint().paragraphs(),
            work.reused_paragraphs(),
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
        })
    }

    fn command(&mut self, command: Command) {
        match command {
            Command::ToggleLocalEdit => {
                self.axis_animation.pause(self.last_elapsed);
                self.content.toggle_edit();
            }
            Command::TogglePaint => {
                self.axis_animation.pause(self.last_elapsed);
                self.content.toggle_paint();
            }
            Command::ToggleAxisAnimation => self.axis_animation.toggle(self.last_elapsed),
            Command::ToggleGuides => {
                self.axis_animation.pause(self.last_elapsed);
                self.show_guides = !self.show_guides;
            }
            Command::Reset => {
                self.content.reset();
                self.axis_animation.reset();
                self.show_guides = false;
            }
        }
    }

    fn animation_enabled(&self) -> bool {
        self.axis_animation.is_enabled()
    }
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
