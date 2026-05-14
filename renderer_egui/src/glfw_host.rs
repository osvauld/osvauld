//! GLFW + egui_glow window host.
//!
//! Replaces `eframe` because eframe→winit clashes with Slint's main-thread
//! event loop. GLFW has no event-loop singleton and tolerates being driven
//! from a worker thread on Linux/Wayland; macOS (Cocoa main-thread) deferred.
//!
//! Lifecycle: `GlfwEguiHost::new`, then `run(|ctx| ...)` per-frame.

#![allow(deprecated)] // egui::Context::run is still the right API at this level

use egui::{Event, Modifiers, PointerButton, Pos2, RawInput, Rect, Vec2, ViewportId};
use glfw::{Action, Context as _, Key, MouseButton, WindowEvent};
use std::sync::Arc;
use std::time::Instant;

/// One GLFW window + GL context + egui context. Single-window per host;
/// multiple egui apps each construct their own host on their own thread.
pub struct GlfwEguiHost {
    glfw: glfw::Glfw,
    window: glfw::PWindow,
    events: glfw::GlfwReceiver<(f64, WindowEvent)>,
    gl: Arc<glow::Context>,
    painter: egui_glow::Painter,
    ctx: egui::Context,
    input: InputState,
    start: Instant,
}

impl GlfwEguiHost {
    /// Open a GLFW window and prepare an egui context against it.
    ///
    /// GL 3.3 Core (egui_glow's desktop target) with vsync.
    pub fn new(
        title: &str,
        size: (u32, u32),
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let mut glfw = glfw::init(glfw::fail_on_errors)?;
        glfw.window_hint(glfw::WindowHint::ContextVersion(3, 3));
        glfw.window_hint(glfw::WindowHint::OpenGlProfile(
            glfw::OpenGlProfileHint::Core,
        ));
        glfw.window_hint(glfw::WindowHint::OpenGlForwardCompat(true));

        let (mut window, events) = glfw
            .create_window(size.0, size.1, title, glfw::WindowMode::Windowed)
            .ok_or("failed to create GLFW window")?;

        window.make_current();
        window.set_all_polling(true);
        glfw.set_swap_interval(glfw::SwapInterval::Sync(1));

        // SAFETY: `get_proc_address` returns valid GL function pointers for
        // the GL context we just made current on this thread.
        let gl = unsafe {
            glow::Context::from_loader_function(|s| {
                window
                    .get_proc_address(s)
                    .map(|f| f as *const std::ffi::c_void)
                    .unwrap_or(std::ptr::null())
            })
        };
        let gl = Arc::new(gl);
        let painter = egui_glow::Painter::new(gl.clone(), "", None, false)?;
        let ctx = egui::Context::default();

        Ok(Self {
            glfw,
            window,
            events,
            gl,
            painter,
            ctx,
            input: InputState::default(),
            start: Instant::now(),
        })
    }

    /// Access the egui context — useful for `request_repaint()` from outside
    /// the frame closure (e.g. on a scribe wake-up).
    pub fn egui_ctx(&self) -> &egui::Context {
        &self.ctx
    }

    /// Block on the event loop, calling `on_frame(ctx)` each repaint.
    /// Reactive — apps wanting continuous animation must call `request_repaint`.
    pub fn run<F>(mut self, mut on_frame: F) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
    where
        F: FnMut(&egui::Context),
    {
        while !self.window.should_close() {
            self.glfw.poll_events();
            let (width, height) = self.window.get_framebuffer_size();
            let native_pixels_per_point = self.window.get_content_scale().0.max(1.0);
            // Effective ppp = DPI * zoom. egui expects screen_rect AND pointer
            // events in logical points; reporting raw physical-px coords against
            // a zoom-scaled layout lands hover on the wrong row at non-1.0 zoom.
            let effective_pixels_per_point = native_pixels_per_point * self.ctx.zoom_factor();

            for (_, ev) in glfw::flush_messages(&self.events) {
                self.input.on_event(ev, &mut self.window);
            }

            let raw_input = self.input.take(
                width,
                height,
                native_pixels_per_point,
                effective_pixels_per_point,
                self.start.elapsed(),
            );

            let full_output = self.ctx.run(raw_input, |ctx| on_frame(ctx));
            let clipped = self
                .ctx
                .tessellate(full_output.shapes, full_output.pixels_per_point);

            // SAFETY: GL context is current on this thread for this host's lifetime.
            unsafe {
                use glow::HasContext as _;
                self.gl.clear_color(0.10, 0.11, 0.13, 1.0);
                self.gl.clear(glow::COLOR_BUFFER_BIT);
            }
            self.painter.paint_and_update_textures(
                [width as u32, height as u32],
                full_output.pixels_per_point,
                &clipped,
                &full_output.textures_delta,
            );

            self.window.swap_buffers();
        }
        self.painter.destroy();
        Ok(())
    }
}

/// Per-frame accumulator of `egui::RawInput` from GLFW events. Pointer
/// position is sticky across frames; events are per-frame.
struct InputState {
    pointer_pos: Option<Pos2>,
    modifiers: Modifiers,
    events: Vec<Event>,
}

impl Default for InputState {
    fn default() -> Self {
        Self {
            pointer_pos: None,
            modifiers: Modifiers::default(),
            events: Vec::new(),
        }
    }
}

impl InputState {
    fn on_event(&mut self, ev: WindowEvent, window: &mut glfw::Window) {
        match ev {
            WindowEvent::Close => window.set_should_close(true),
            WindowEvent::CursorPos(x, y) => {
                let p = Pos2::new(x as f32, y as f32);
                self.pointer_pos = Some(p);
                self.events.push(Event::PointerMoved(p));
            }
            WindowEvent::MouseButton(btn, action, mods) => {
                let Some(pos) = self.pointer_pos else { return };
                self.modifiers = map_mods(mods);
                let button = match btn {
                    MouseButton::Button1 => PointerButton::Primary,
                    MouseButton::Button2 => PointerButton::Secondary,
                    MouseButton::Button3 => PointerButton::Middle,
                    _ => return,
                };
                self.events.push(Event::PointerButton {
                    pos,
                    button,
                    pressed: action == Action::Press,
                    modifiers: self.modifiers,
                });
            }
            WindowEvent::Scroll(dx, dy) => {
                self.events.push(Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Line,
                    delta: Vec2::new(dx as f32, dy as f32),
                    modifiers: self.modifiers,
                    phase: egui::TouchPhase::Move,
                });
            }
            WindowEvent::Key(key, _scan, action, mods) => {
                self.modifiers = map_mods(mods);
                if let Some(k) = map_key(key) {
                    let pressed = matches!(action, Action::Press | Action::Repeat);
                    self.events.push(Event::Key {
                        key: k,
                        physical_key: Some(k),
                        pressed,
                        repeat: action == Action::Repeat,
                        modifiers: self.modifiers,
                    });
                }
            }
            WindowEvent::Char(ch) => self.events.push(Event::Text(ch.to_string())),
            _ => {}
        }
    }

    fn take(
        &mut self,
        width: i32,
        height: i32,
        native_pixels_per_point: f32,
        effective_pixels_per_point: f32,
        elapsed: std::time::Duration,
    ) -> RawInput {
        // screen_rect in logical points (effective ppp, not native) so layout
        // coords agree with pointer events.
        let screen = Rect::from_min_size(
            Pos2::ZERO,
            Vec2::new(
                width as f32 / effective_pixels_per_point,
                height as f32 / effective_pixels_per_point,
            ),
        );
        let mut viewports = std::collections::HashMap::default();
        let mut vi = egui::ViewportInfo::default();
        // egui multiplies native by zoom internally; we only report native.
        vi.native_pixels_per_point = Some(native_pixels_per_point);
        viewports.insert(ViewportId::ROOT, vi);

        // GLFW cursor coords are physical px; convert to logical points so
        // contains_pointer checks against widget layout coords.
        let scale = 1.0 / effective_pixels_per_point;
        let mut events = std::mem::take(&mut self.events);
        for ev in &mut events {
            match ev {
                Event::PointerMoved(p) => {
                    *p = Pos2::new(p.x * scale, p.y * scale);
                }
                Event::PointerButton { pos, .. } => {
                    *pos = Pos2::new(pos.x * scale, pos.y * scale);
                }
                _ => {}
            }
        }

        RawInput {
            viewport_id: ViewportId::ROOT,
            viewports,
            screen_rect: Some(screen),
            time: Some(elapsed.as_secs_f64()),
            predicted_dt: 1.0 / 60.0,
            modifiers: self.modifiers,
            events,
            hovered_files: vec![],
            dropped_files: vec![],
            focused: true,
            system_theme: None,
            max_texture_side: None,
            safe_area_insets: Default::default(),
        }
    }
}

fn map_mods(m: glfw::Modifiers) -> Modifiers {
    Modifiers {
        alt: m.contains(glfw::Modifiers::Alt),
        ctrl: m.contains(glfw::Modifiers::Control),
        shift: m.contains(glfw::Modifiers::Shift),
        mac_cmd: false,
        command: m.contains(glfw::Modifiers::Control),
    }
}

fn map_key(k: Key) -> Option<egui::Key> {
    use egui::Key as E;
    Some(match k {
        Key::Backspace => E::Backspace,
        Key::Enter => E::Enter,
        Key::Tab => E::Tab,
        Key::Space => E::Space,
        Key::Escape => E::Escape,
        Key::Left => E::ArrowLeft,
        Key::Right => E::ArrowRight,
        Key::Up => E::ArrowUp,
        Key::Down => E::ArrowDown,
        Key::Delete => E::Delete,
        Key::Home => E::Home,
        Key::End => E::End,
        Key::PageUp => E::PageUp,
        Key::PageDown => E::PageDown,
        Key::A => E::A,
        Key::C => E::C,
        Key::V => E::V,
        Key::X => E::X,
        Key::Z => E::Z,
        _ => return None,
    })
}
