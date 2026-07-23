// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Native event and software-presentation host for the showcase.

use std::fmt::{Display, Formatter};
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::{Duration, Instant};

use softbuffer::{Context, Surface};
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalPosition, LogicalSize};
use winit::event::{ElementState, Ime, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{Window, WindowAttributes, WindowId};

use underwood::{Point, Rect};

use crate::interaction::{EditorEvent, EditorKey, ImeInput, InputModifiers, PointerState};

const FRAME_INTERVAL: Duration = Duration::from_millis(33);

/// User-visible showcase commands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Command {
    TogglePaint,
    ToggleAxisAnimation,
    ToggleGuides,
    Reset,
}

/// One complete unpremultiplied RGBA8 frame ready for presentation.
#[derive(Clone, Debug)]
pub(crate) struct Frame {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) rgba: Vec<u8>,
    pub(crate) window_title: String,
    pub(crate) ime_cursor_area: Option<Rect>,
}

/// Rendering and interaction surface supplied by the showcase proper.
pub(crate) trait HostApplication {
    fn render(
        &mut self,
        width: u32,
        height: u32,
        scale_factor: f64,
        elapsed: Duration,
    ) -> Result<Frame, String>;

    fn command(&mut self, command: Command);

    fn editor_event(&mut self, event: EditorEvent);

    fn animation_enabled(&self) -> bool;
}

/// Runs a showcase application in a native window until it closes.
pub(crate) fn run(app: impl HostApplication + 'static) -> Result<(), HostError> {
    let event_loop = EventLoop::new().map_err(HostError::event_loop)?;
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut host = NativeHost::new(app);
    event_loop
        .run_app(&mut host)
        .map_err(HostError::event_loop)?;
    if let Some(error) = host.fatal_error {
        return Err(error);
    }
    Ok(())
}

struct NativeHost<A> {
    app: A,
    context: Option<Context<Arc<Window>>>,
    surface: Option<Surface<Arc<Window>, Arc<Window>>>,
    window: Option<Arc<Window>>,
    started: Instant,
    next_frame: Instant,
    cursor_position: Option<Point>,
    modifiers: ModifiersState,
    fatal_error: Option<HostError>,
}

impl<A> NativeHost<A> {
    fn new(app: A) -> Self {
        let now = Instant::now();
        Self {
            app,
            context: None,
            surface: None,
            window: None,
            started: now,
            next_frame: now,
            cursor_position: None,
            modifiers: ModifiersState::default(),
            fatal_error: None,
        }
    }

    fn fail(&mut self, event_loop: &ActiveEventLoop, error: impl Into<HostError>) {
        self.fatal_error = Some(error.into());
        event_loop.exit();
    }
}

impl<A: HostApplication> NativeHost<A> {
    fn redraw(&mut self, event_loop: &ActiveEventLoop) {
        let Some(window) = self.window.as_ref().cloned() else {
            return;
        };
        let size = window.inner_size();
        let (Some(width), Some(height)) =
            (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
        else {
            return;
        };

        let frame = match self.app.render(
            size.width,
            size.height,
            window.scale_factor(),
            self.started.elapsed(),
        ) {
            Ok(frame) => frame,
            Err(error) => {
                self.fail(event_loop, HostError::render(error));
                return;
            }
        };
        if frame.width != size.width || frame.height != size.height {
            self.fail(
                event_loop,
                HostError::render(format!(
                    "renderer returned {}x{} for a {}x{} window",
                    frame.width, frame.height, size.width, size.height
                )),
            );
            return;
        }

        let presentation = match self.surface.as_mut() {
            Some(surface) => present_frame(surface, width, height, &frame.rgba),
            None => return,
        };
        if let Err(error) = presentation {
            self.fail(event_loop, error);
            return;
        }
        window.set_title(&frame.window_title);
        if let Some(cursor) = frame.ime_cursor_area {
            window.set_ime_cursor_area(
                LogicalPosition::new(cursor.x0, cursor.y0),
                LogicalSize::new(cursor.width().max(1.0), cursor.height().max(1.0)),
            );
        }
    }

    fn dispatch(&mut self, command: Command) {
        self.app.command(command);
        self.next_frame = Instant::now();
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    fn dispatch_editor(&mut self, event: EditorEvent) {
        self.app.editor_event(event);
        self.next_frame = Instant::now();
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

impl<A: HostApplication> ApplicationHandler for NativeHost<A> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attributes = WindowAttributes::default()
            .with_title("Underwood — live document")
            .with_inner_size(LogicalSize::new(1_100.0, 800.0))
            .with_min_inner_size(LogicalSize::new(520.0, 520.0));
        let window = match event_loop.create_window(attributes) {
            Ok(window) => Arc::new(window),
            Err(error) => {
                self.fail(event_loop, HostError::window(error));
                return;
            }
        };
        window.set_ime_allowed(true);
        let context = match Context::new(Arc::clone(&window)) {
            Ok(context) => context,
            Err(error) => {
                self.fail(event_loop, HostError::softbuffer(error));
                return;
            }
        };
        let surface = match Surface::new(&context, Arc::clone(&window)) {
            Ok(surface) => surface,
            Err(error) => {
                self.fail(event_loop, HostError::softbuffer(error));
                return;
            }
        };
        window.request_redraw();
        self.context = Some(context);
        self.surface = Some(surface);
        self.window = Some(window);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(window) = &self.window else {
            return;
        };
        if window.id() != window_id {
            return;
        }

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                window.request_redraw();
            }
            WindowEvent::RedrawRequested => self.redraw(event_loop),
            WindowEvent::Focused(focused) => {
                self.dispatch_editor(EditorEvent::Focused(focused));
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers = modifiers.state();
            }
            WindowEvent::CursorMoved { position, .. } => {
                let position = position.to_logical::<f64>(window.scale_factor());
                let point = Point::new(position.x, position.y);
                self.cursor_position = Some(point);
                self.dispatch_editor(EditorEvent::PointerMoved(point));
            }
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => {
                if let Some(point) = self.cursor_position {
                    let modifiers = InputModifiers {
                        extend: self.modifiers.shift_key(),
                        add: self.modifiers.alt_key(),
                    };
                    let state = match state {
                        ElementState::Pressed => PointerState::Pressed,
                        ElementState::Released => PointerState::Released,
                    };
                    self.dispatch_editor(EditorEvent::PointerButton {
                        state,
                        point,
                        modifiers,
                    });
                }
            }
            WindowEvent::Ime(ime) => {
                let input = match ime {
                    Ime::Enabled => ImeInput::Enabled,
                    Ime::Preedit(text, selection) => ImeInput::Preedit { text, selection },
                    Ime::Commit(text) => ImeInput::Commit(text),
                    Ime::Disabled => ImeInput::Disabled,
                };
                self.dispatch_editor(EditorEvent::Ime(input));
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                let key = event.logical_key.as_ref();
                if key == Key::Named(NamedKey::Escape) {
                    event_loop.exit();
                } else if !event.repeat
                    && let Some(command) = command_for_key(key.clone())
                {
                    self.dispatch(command);
                } else if let Some(editor_key) = editor_key_for_key(key) {
                    self.dispatch_editor(EditorEvent::Key {
                        key: editor_key,
                        extend: self.modifiers.shift_key(),
                    });
                } else if !self.modifiers.control_key()
                    && !self.modifiers.super_key()
                    && !self.modifiers.alt_key()
                    && let Some(text) = event.text
                    && text.chars().all(|character| !character.is_control())
                {
                    self.dispatch_editor(EditorEvent::Text(text.to_string()));
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if !self.app.animation_enabled() {
            event_loop.set_control_flow(ControlFlow::Wait);
            return;
        }

        let now = Instant::now();
        if now >= self.next_frame {
            if let Some(window) = &self.window {
                window.request_redraw();
            }
            while self.next_frame <= now {
                self.next_frame += FRAME_INTERVAL;
            }
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(self.next_frame));
    }
}

fn command_for_key(key: Key<&str>) -> Option<Command> {
    match key {
        Key::Named(NamedKey::F2) => Some(Command::TogglePaint),
        Key::Named(NamedKey::F3) => Some(Command::ToggleAxisAnimation),
        Key::Named(NamedKey::F4) => Some(Command::ToggleGuides),
        Key::Named(NamedKey::F5) => Some(Command::Reset),
        _ => None,
    }
}

fn editor_key_for_key(key: Key<&str>) -> Option<EditorKey> {
    match key {
        Key::Named(NamedKey::ArrowLeft) => Some(EditorKey::MoveLeft),
        Key::Named(NamedKey::ArrowRight) => Some(EditorKey::MoveRight),
        Key::Named(NamedKey::Backspace) => Some(EditorKey::Backspace),
        Key::Named(NamedKey::Delete) => Some(EditorKey::Delete),
        Key::Named(NamedKey::Enter) => Some(EditorKey::Enter),
        _ => None,
    }
}

fn copy_rgba_to_softbuffer(target: &mut [u32], rgba: &[u8]) -> Result<(), HostError> {
    let expected = target
        .len()
        .checked_mul(4)
        .ok_or_else(|| HostError::render("window buffer is too large"))?;
    if rgba.len() != expected {
        return Err(HostError::render(format!(
            "RGBA frame has {} bytes; expected {expected}",
            rgba.len()
        )));
    }
    for (pixel, channels) in target.iter_mut().zip(rgba.chunks_exact(4)) {
        *pixel =
            u32::from(channels[2]) | (u32::from(channels[1]) << 8) | (u32::from(channels[0]) << 16);
    }
    Ok(())
}

fn present_frame(
    surface: &mut Surface<Arc<Window>, Arc<Window>>,
    width: NonZeroU32,
    height: NonZeroU32,
    rgba: &[u8],
) -> Result<(), HostError> {
    surface
        .resize(width, height)
        .map_err(HostError::softbuffer)?;
    let mut buffer = surface.buffer_mut().map_err(HostError::softbuffer)?;
    copy_rgba_to_softbuffer(&mut buffer, rgba)?;
    buffer.present().map_err(HostError::softbuffer)
}

/// Fatal native-host error.
#[derive(Debug)]
pub(crate) struct HostError(String);

impl HostError {
    fn event_loop(error: impl Display) -> Self {
        Self(format!("event loop failed: {error}"))
    }

    fn window(error: impl Display) -> Self {
        Self(format!("window creation failed: {error}"))
    }

    fn softbuffer(error: impl Display) -> Self {
        Self(format!("software presentation failed: {error}"))
    }

    fn render(error: impl Display) -> Self {
        Self(format!("showcase rendering failed: {error}"))
    }
}

impl Display for HostError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for HostError {}

#[cfg(test)]
mod tests {
    use super::{Command, command_for_key, copy_rgba_to_softbuffer, editor_key_for_key};
    use crate::interaction::EditorKey;
    use winit::keyboard::{Key, NamedKey};

    #[test]
    fn shortcuts_follow_logical_characters() {
        assert_eq!(
            command_for_key(Key::Named(NamedKey::F2)),
            Some(Command::TogglePaint)
        );
        assert_eq!(
            editor_key_for_key(Key::Named(NamedKey::ArrowLeft)),
            Some(EditorKey::MoveLeft)
        );
        assert_eq!(command_for_key(Key::Character("P")), None);
        assert_eq!(command_for_key(Key::Named(NamedKey::Space)), None);
        assert_eq!(command_for_key(Key::Character("?")), None);
    }

    #[test]
    fn rgba_conversion_matches_softbuffer_channel_contract() {
        let mut target = [0_u32; 2];
        copy_rgba_to_softbuffer(
            &mut target,
            &[0x12, 0x34, 0x56, 0xff, 0xab, 0xcd, 0xef, 0x00],
        )
        .expect("matching buffers must convert");
        assert_eq!(target, [0x0012_3456, 0x00ab_cdef]);
    }

    #[test]
    fn rgba_conversion_rejects_wrong_frame_size() {
        let mut target = [0_u32; 2];
        let error = copy_rgba_to_softbuffer(&mut target, &[0; 4])
            .expect_err("short frame must be rejected");
        assert!(error.to_string().contains("expected 8"));
    }
}
