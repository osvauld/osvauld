//! Winit Window Integration
//!
//! Event-driven rendering with winit + wgpu + vello.
//!
//! **Architecture**:
//! ```text
//! winit EventLoop
//!       │
//!       ├── WindowEvent::RedrawRequested
//!       │       └── HumlRenderer.process_updates() → render
//!       │
//!       └── UserEvent::DataChanged
//!               └── window.request_redraw()
//! ```

use std::num::NonZeroUsize;
use std::sync::Arc;

use tracing::{debug, error, info};
use vello::util::{RenderContext, RenderSurface};
use vello::wgpu::PresentMode;
use vello::{AaConfig, AaSupport, Renderer as VelloRenderer, RendererOptions, Scene};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy};
use winit::window::{Window, WindowId};

use crate::layout::LayoutEngine;
use crate::vello_backend::VelloBackend;
use crate::HumlRenderer;

/// Custom events for the application
#[derive(Debug, Clone)]
pub enum AppEvent {
    /// Data changed, trigger re-render
    DataChanged(Vec<String>),
    /// Navigate to screen
    Navigate(String),
    /// Close the window
    Close,
}

/// Window state for a HUML renderer
pub struct HumlWindow<'s> {
    /// Winit window
    window: Arc<Window>,
    /// Vello render context
    render_cx: RenderContext,
    /// Vello surface
    surface: RenderSurface<'s>,
    /// Vello renderer
    vello_renderer: VelloRenderer,
    /// HUML renderer
    huml_renderer: HumlRenderer,
    /// Layout engine
    layout_engine: LayoutEngine,
    /// Vello backend
    vello_backend: VelloBackend,
    /// Current scene
    scene: Scene,
    /// Current window size
    width: u32,
    height: u32,
    /// Mouse position
    mouse_pos: (f32, f32),
    /// Needs re-layout
    needs_layout: bool,
}

impl<'s> HumlWindow<'s> {
    /// Create a new HUML window
    pub async fn new(
        window: Arc<Window>,
        huml_renderer: HumlRenderer,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let size = window.inner_size();
        let width = size.width.max(1);
        let height = size.height.max(1);

        // Create render context
        let mut render_cx = RenderContext::new();

        // Create surface
        let surface = render_cx
            .create_surface(
                window.clone(),
                width,
                height,
                PresentMode::AutoVsync,
            )
            .await?;

        // Create vello renderer
        let device = &render_cx.devices[surface.dev_id].device;
        let vello_renderer = VelloRenderer::new(
            device,
            RendererOptions {
                use_cpu: false,
                antialiasing_support: AaSupport::all(),
                num_init_threads: NonZeroUsize::new(1),
                pipeline_cache: None,
            },
        )?;

        let layout_engine = LayoutEngine::new();
        let vello_backend = VelloBackend::new();

        Ok(Self {
            window,
            render_cx,
            surface,
            vello_renderer,
            huml_renderer,
            layout_engine,
            vello_backend,
            scene: Scene::new(),
            width,
            height,
            mouse_pos: (0.0, 0.0),
            needs_layout: true,
        })
    }

    /// Handle window resize
    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }

        self.width = width;
        self.height = height;
        self.needs_layout = true;

        self.render_cx
            .resize_surface(&mut self.surface, width, height);

        debug!(width, height, "Window resized");
    }

    /// Render a frame
    pub fn render(&mut self) {
        // Process any pending updates in the HUML renderer
        self.huml_renderer.process_updates();

        // Rebuild layout if needed
        if self.needs_layout {
            self.layout_engine
                .build_from_render_tree(self.huml_renderer.render_tree());
            self.layout_engine
                .compute_layout(self.width as f32, self.height as f32);

            // Copy layouts to vello backend
            for (node_id, layout) in self.layout_engine.get_all_layouts() {
                self.vello_backend.set_layout(node_id, layout);
            }

            self.needs_layout = false;
        }

        // Clear and rebuild scene
        self.scene.reset();
        self.vello_backend
            .render_tree(&mut self.scene, self.huml_renderer.render_tree());

        // Render to texture then blit to surface
        let device = &self.render_cx.devices[self.surface.dev_id].device;
        let queue = &self.render_cx.devices[self.surface.dev_id].queue;

        // Render to the surface's target texture
        self.vello_renderer
            .render_to_texture(
                device,
                queue,
                &self.scene,
                &self.surface.target_view,
                &vello::RenderParams {
                    base_color: vello::peniko::Color::new([0.98, 0.98, 0.98, 1.0]),
                    width: self.width,
                    height: self.height,
                    antialiasing_method: AaConfig::Msaa16,
                },
            )
            .expect("Render failed");

        // Get surface texture and blit
        let surface_texture = self.surface.surface.get_current_texture().unwrap();
        let surface_view = surface_texture.texture.create_view(&Default::default());

        // Create encoder and blit from target texture to surface
        let mut encoder = device.create_command_encoder(&Default::default());
        self.surface.blitter.copy(device, &mut encoder, &self.surface.target_view, &surface_view);
        queue.submit(Some(encoder.finish()));

        surface_texture.present();
    }

    /// Handle mouse click
    pub fn on_click(&mut self, x: f32, y: f32) {
        debug!(x, y, "Click");
        // TODO: Hit testing against layout nodes
        self.window.request_redraw();
    }

    /// Handle mouse move
    pub fn on_mouse_move(&mut self, x: f32, y: f32) {
        self.mouse_pos = (x, y);
    }

    /// Request a redraw
    pub fn request_redraw(&self) {
        self.window.request_redraw();
    }

    /// Mark as needing layout
    pub fn invalidate_layout(&mut self) {
        self.needs_layout = true;
    }

    /// Get the window
    pub fn window(&self) -> &Window {
        &self.window
    }

    /// Get the HUML renderer
    pub fn huml_renderer(&self) -> &HumlRenderer {
        &self.huml_renderer
    }

    /// Get mutable HUML renderer
    pub fn huml_renderer_mut(&mut self) -> &mut HumlRenderer {
        &mut self.huml_renderer
    }
}

/// Application handler for winit
pub struct HumlApp<'s> {
    /// Event loop proxy for sending custom events
    event_proxy: EventLoopProxy<AppEvent>,
    /// Window (created on resume)
    window: Option<HumlWindow<'s>>,
    /// HUML renderer (stored until window is created)
    pending_renderer: Option<HumlRenderer>,
    /// Initial window size
    initial_size: (u32, u32),
}

impl<'s> HumlApp<'s> {
    /// Create a new application
    pub fn new(
        event_proxy: EventLoopProxy<AppEvent>,
        huml_renderer: HumlRenderer,
        initial_size: (u32, u32),
    ) -> Self {
        Self {
            event_proxy,
            window: None,
            pending_renderer: Some(huml_renderer),
            initial_size,
        }
    }

    /// Get the event proxy for sending events
    pub fn event_proxy(&self) -> EventLoopProxy<AppEvent> {
        self.event_proxy.clone()
    }
}

impl ApplicationHandler<AppEvent> for HumlApp<'_> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        // Create window
        let window_attrs = Window::default_attributes()
            .with_title("HUML Viewer")
            .with_inner_size(LogicalSize::new(self.initial_size.0, self.initial_size.1));

        let window = Arc::new(
            event_loop
                .create_window(window_attrs)
                .expect("Failed to create window"),
        );

        // Create HUML window
        if let Some(renderer) = self.pending_renderer.take() {
            let window_clone = window.clone();
            match pollster::block_on(HumlWindow::new(window_clone, renderer)) {
                Ok(huml_window) => {
                    self.window = Some(huml_window);
                    info!("HUML window created");
                }
                Err(e) => {
                    error!(error = %e, "Failed to create HUML window");
                }
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(window) = self.window.as_mut() else {
            return;
        };

        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }

            WindowEvent::Resized(size) => {
                window.resize(size.width, size.height);
                window.request_redraw();
            }

            WindowEvent::RedrawRequested => {
                window.render();
            }

            WindowEvent::CursorMoved { position, .. } => {
                window.on_mouse_move(position.x as f32, position.y as f32);
            }

            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                let (x, y) = window.mouse_pos;
                window.on_click(x, y);
            }

            _ => {}
        }
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: AppEvent) {
        let Some(window) = self.window.as_mut() else {
            return;
        };

        match event {
            AppEvent::DataChanged(fields) => {
                debug!(?fields, "Data changed");
                for field in fields {
                    window
                        .huml_renderer_mut()
                        .update_source(&field, serde_json::Value::Null);
                }
                window.invalidate_layout();
                window.request_redraw();
            }

            AppEvent::Navigate(screen) => {
                window.huml_renderer_mut().navigate(&screen);
                window.invalidate_layout();
                window.request_redraw();
            }

            AppEvent::Close => {
                // Will be handled on next event loop iteration
            }
        }
    }
}

/// Run a HUML window with the given renderer
pub fn run_window(
    huml_renderer: HumlRenderer,
    width: u32,
    height: u32,
) -> Result<EventLoopProxy<AppEvent>, Box<dyn std::error::Error>> {
    let event_loop = EventLoop::with_user_event().build()?;
    let proxy = event_loop.create_proxy();

    let mut app = HumlApp::new(proxy.clone(), huml_renderer, (width, height));

    event_loop.run_app(&mut app)?;

    Ok(proxy)
}

#[cfg(test)]
mod tests {
    // Window tests require a display, skip in CI
}
