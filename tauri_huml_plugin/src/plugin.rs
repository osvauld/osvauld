//! Tauri HUML Plugin Core
//!
//! Creates winit windows for HUML template rendering with a single shared event loop.
//!
//! **Architecture:**
//! - One global event loop thread manages all HUML windows
//! - Windows are created/destroyed via channel messages
//! - Vello renders directly to winit window surfaces (GPU-accelerated)
//! - Data changes trigger reactive updates (only dirty nodes re-rendered)

use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;

use anyhow::Error;
use crossbeam_channel::{bounded, unbounded, Receiver, Sender};
use serde_json::Value;
use tauri::{AppHandle, Manager};

use cel_runtime::CelEvaluator;
use huml_renderer::layout::LayoutEngine;
use huml_renderer::vello_backend::VelloBackend;
use huml_renderer::{HumlRenderer, ParsedTemplate, QueryBridgeHandle};
use vello::util::RenderContext;
use vello::wgpu::PresentMode;
use vello::{AaConfig, AaSupport, Renderer as VelloRenderer, RendererOptions, Scene};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::window::{Window, WindowAttributes, WindowId};

// =============================================================================
// Global Event Loop Manager
// =============================================================================

/// Request sent to the HUML window manager thread
pub enum ManagerRequest {
    /// Create a new window
    CreateWindow {
        label: String,
        title: String,
        width: u32,
        height: u32,
        template: ParsedTemplate,
        cel: Arc<CelEvaluator>,
        query_bridge: Option<QueryBridgeHandle>,
        initial_query_data: HashMap<String, Vec<Value>>,
        reply: Sender<Result<(), String>>,
    },
    /// Close a window by label
    CloseWindow { label: String },
    /// Send data to a window
    UpdateData { label: String, field: String, value: Value },
    /// Navigate to screen
    Navigate { label: String, screen: String },
}

/// Global manager handle (singleton)
static MANAGER: OnceLock<ManagerHandle> = OnceLock::new();

/// Handle to communicate with the window manager thread
struct ManagerHandle {
    request_tx: Sender<ManagerRequest>,
    event_proxy: Mutex<Option<EventLoopProxy<ManagerRequest>>>,
}

/// State for a single HUML window
struct HumlWindowState {
    label: String,
    window: Arc<Window>,
    surface: vello::util::RenderSurface<'static>,
    vello_renderer: VelloRenderer,
    huml_renderer: HumlRenderer,
    layout_engine: LayoutEngine,
    vello_backend: VelloBackend,
    scene: Scene,
    width: u32,
    height: u32,
    mouse_pos: (f32, f32),
    needs_layout: bool,
    focused_input: Option<huml_renderer::NodeId>,
    #[allow(dead_code)]
    query_bridge: Option<QueryBridgeHandle>,
}

/// The main application that handles multiple windows
struct HumlWindowManager {
    render_cx: Option<RenderContext>,
    windows: HashMap<WindowId, HumlWindowState>,
    label_to_window_id: HashMap<String, WindowId>,
    pending_creates: Vec<PendingWindow>,
    request_rx: Receiver<ManagerRequest>,
}

struct PendingWindow {
    label: String,
    title: String,
    width: u32,
    height: u32,
    template: ParsedTemplate,
    cel: Arc<CelEvaluator>,
    query_bridge: Option<QueryBridgeHandle>,
    initial_query_data: HashMap<String, Vec<Value>>,
    reply: Sender<Result<(), String>>,
}

impl HumlWindowManager {
    fn new(request_rx: Receiver<ManagerRequest>) -> Self {
        Self {
            render_cx: None,
            windows: HashMap::new(),
            label_to_window_id: HashMap::new(),
            pending_creates: Vec::new(),
            request_rx,
        }
    }

    fn process_requests(&mut self, event_loop: &ActiveEventLoop) {
        while let Ok(request) = self.request_rx.try_recv() {
            self.handle_request(request, event_loop);
        }

        // Create pending windows
        self.create_pending_windows(event_loop);
    }

    fn handle_request(&mut self, request: ManagerRequest, event_loop: &ActiveEventLoop) {
        match request {
            ManagerRequest::CreateWindow {
                label, title, width, height, template, cel, query_bridge, initial_query_data, reply,
            } => {
                self.pending_creates.push(PendingWindow {
                    label, title, width, height, template, cel, query_bridge, initial_query_data, reply,
                });
                self.create_pending_windows(event_loop);
            }
            ManagerRequest::CloseWindow { label } => {
                if let Some(window_id) = self.label_to_window_id.remove(&label) {
                    self.windows.remove(&window_id);
                    tracing::info!(label = %label, "HUML window closed");
                }
            }
            ManagerRequest::UpdateData { label, field, value } => {
                if let Some(window_id) = self.label_to_window_id.get(&label) {
                    if let Some(state) = self.windows.get_mut(window_id) {
                        state.huml_renderer.update_source(&field, value);
                        state.needs_layout = true;
                        state.window.request_redraw();
                    }
                }
            }
            ManagerRequest::Navigate { label, screen } => {
                if let Some(window_id) = self.label_to_window_id.get(&label) {
                    if let Some(state) = self.windows.get_mut(window_id) {
                        state.huml_renderer.navigate(&screen);
                        state.needs_layout = true;
                        state.window.request_redraw();
                    }
                }
            }
        }
    }

    fn create_pending_windows(&mut self, event_loop: &ActiveEventLoop) {
        let pending = std::mem::take(&mut self.pending_creates);

        for req in pending {
            let result = self.create_window_internal(
                event_loop,
                req.label.clone(),
                &req.title,
                req.width,
                req.height,
                req.template,
                req.cel,
                req.query_bridge,
                req.initial_query_data,
            );
            match result {
                Ok(()) => {
                    let _ = req.reply.send(Ok(()));
                }
                Err(e) => {
                    let _ = req.reply.send(Err(e.to_string()));
                }
            }
        }
    }

    fn create_window_internal(
        &mut self,
        event_loop: &ActiveEventLoop,
        label: String,
        title: &str,
        width: u32,
        height: u32,
        template: ParsedTemplate,
        cel: Arc<CelEvaluator>,
        query_bridge: Option<QueryBridgeHandle>,
        initial_query_data: HashMap<String, Vec<Value>>,
    ) -> Result<(), Error> {
        // Check if already exists
        if self.label_to_window_id.contains_key(&label) {
            return Err(Error::msg(format!("Window '{}' already exists", label)));
        }

        // Create window
        let attrs = WindowAttributes::default()
            .with_title(title)
            .with_inner_size(LogicalSize::new(width, height));

        let window = Arc::new(event_loop.create_window(attrs)?);
        let window_id = window.id();

        // Initialize render context if needed
        if self.render_cx.is_none() {
            self.render_cx = Some(RenderContext::new());
        }
        let render_cx = self.render_cx.as_mut().unwrap();

        // Create surface
        let size = window.inner_size();
        let w = size.width.max(1);
        let h = size.height.max(1);

        let surface = pollster::block_on(render_cx.create_surface(
            window.clone(),
            w,
            h,
            PresentMode::AutoVsync,
        ))?;

        // SAFETY: Surface lifetime extended - window stored alongside
        let surface: vello::util::RenderSurface<'static> = unsafe {
            std::mem::transmute(surface)
        };

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

        // Create HUML renderer
        let mut huml_renderer = HumlRenderer::with_query_bridge(template, cel, query_bridge.clone());
        if !initial_query_data.is_empty() {
            huml_renderer.load_initial_query_data(initial_query_data);
        }

        let state = HumlWindowState {
            label: label.clone(),
            window: window.clone(),
            surface,
            vello_renderer,
            huml_renderer,
            layout_engine: LayoutEngine::new(),
            vello_backend: VelloBackend::new(),
            scene: Scene::new(),
            width: w,
            height: h,
            mouse_pos: (0.0, 0.0),
            needs_layout: true,
            focused_input: None,
            query_bridge,
        };

        self.windows.insert(window_id, state);
        self.label_to_window_id.insert(label.clone(), window_id);

        window.request_redraw();
        tracing::info!(label = %label, "HUML window created");

        Ok(())
    }

    fn render_window(&mut self, window_id: WindowId) {
        let Some(state) = self.windows.get_mut(&window_id) else { return };
        let Some(render_cx) = &self.render_cx else { return };

        // Process updates
        state.huml_renderer.process_updates();

        // Rebuild layout if needed
        if state.needs_layout {
            state.layout_engine.build_from_render_tree(state.huml_renderer.render_tree());
            state.layout_engine.compute_layout(state.width as f32, state.height as f32);

            for (node_id, layout) in state.layout_engine.get_all_layouts() {
                state.vello_backend.set_layout(node_id, layout);
            }
            state.needs_layout = false;
        }

        // Build scene
        state.scene.reset();
        state.vello_backend.render_tree(&mut state.scene, state.huml_renderer.render_tree());

        // Render
        let device = &render_cx.devices[state.surface.dev_id].device;
        let queue = &render_cx.devices[state.surface.dev_id].queue;

        if let Err(e) = state.vello_renderer.render_to_texture(
            device,
            queue,
            &state.scene,
            &state.surface.target_view,
            &vello::RenderParams {
                base_color: vello::peniko::Color::new([0.98, 0.98, 0.98, 1.0]),
                width: state.width,
                height: state.height,
                antialiasing_method: AaConfig::Msaa16,
            },
        ) {
            tracing::error!("Render failed: {}", e);
            return;
        }

        // Present
        let Ok(surface_texture) = state.surface.surface.get_current_texture() else { return };
        let surface_view = surface_texture.texture.create_view(&Default::default());

        let mut encoder = device.create_command_encoder(&Default::default());
        state.surface.blitter.copy(device, &mut encoder, &state.surface.target_view, &surface_view);
        queue.submit(Some(encoder.finish()));

        surface_texture.present();
    }

    fn resize_window(&mut self, window_id: WindowId, width: u32, height: u32) {
        let Some(state) = self.windows.get_mut(&window_id) else { return };
        let Some(render_cx) = &self.render_cx else { return };

        let w = width.max(1);
        let h = height.max(1);
        state.width = w;
        state.height = h;
        state.needs_layout = true;

        render_cx.resize_surface(&mut state.surface, w, h);
        state.window.request_redraw();
    }

    fn handle_click(&mut self, window_id: WindowId) {
        let Some(state) = self.windows.get_mut(&window_id) else { return };

        let (x, y) = state.mouse_pos;

        if let Some(node_id) = state.vello_backend.hit_test(state.huml_renderer.render_tree(), x, y) {
            if let Some(node) = state.huml_renderer.render_tree().get(node_id) {
                match node {
                    huml_renderer::RenderNode::Button { .. } => {
                        state.focused_input = None;
                        if state.huml_renderer.on_button_click(node_id) {
                            state.needs_layout = true;
                            state.window.request_redraw();
                        }
                    }
                    huml_renderer::RenderNode::Input { name, .. } => {
                        tracing::debug!(name = %name, "Focused input");
                        state.focused_input = Some(node_id);
                        state.window.request_redraw();
                    }
                    _ => {
                        state.focused_input = None;
                    }
                }
            }
        } else {
            state.focused_input = None;
        }
    }

    fn handle_keyboard(&mut self, window_id: WindowId, event: &winit::event::KeyEvent) {
        let Some(state) = self.windows.get_mut(&window_id) else { return };

        if event.state != ElementState::Pressed {
            return;
        }

        let Some(node_id) = state.focused_input else { return };

        let field_name = if let Some(huml_renderer::RenderNode::Input { name, .. }) =
            state.huml_renderer.render_tree().get(node_id)
        {
            Some(name.clone())
        } else {
            None
        };

        let Some(field_name) = field_name else { return };

        let current = state.huml_renderer
            .get_source(&field_name)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let new_value = match &event.logical_key {
            winit::keyboard::Key::Named(winit::keyboard::NamedKey::Backspace) => {
                let mut chars: Vec<char> = current.chars().collect();
                chars.pop();
                Some(chars.into_iter().collect::<String>())
            }
            winit::keyboard::Key::Named(winit::keyboard::NamedKey::Space) => {
                Some(format!("{} ", current))
            }
            winit::keyboard::Key::Named(winit::keyboard::NamedKey::Enter) => None,
            winit::keyboard::Key::Character(c) => Some(format!("{}{}", current, c)),
            _ => None,
        };

        if let Some(new_value) = new_value {
            state.huml_renderer.update_source(&field_name, Value::String(new_value));
            state.huml_renderer.build_render_tree();
            state.needs_layout = true;
            state.window.request_redraw();
        }
    }
}

impl ApplicationHandler<ManagerRequest> for HumlWindowManager {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.process_requests(event_loop);
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: ManagerRequest) {
        self.handle_request(event, event_loop);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, window_id: WindowId, event: WindowEvent) {
        // Process any pending requests
        self.process_requests(event_loop);

        match event {
            WindowEvent::CloseRequested => {
                // Find and remove the window
                let label = self.windows.get(&window_id).map(|s| s.label.clone());
                if let Some(label) = label {
                    self.label_to_window_id.remove(&label);
                    self.windows.remove(&window_id);
                    tracing::info!(label = %label, "HUML window closed by user");
                }
            }
            WindowEvent::Resized(size) => {
                self.resize_window(window_id, size.width, size.height);
            }
            WindowEvent::RedrawRequested => {
                self.render_window(window_id);
            }
            WindowEvent::CursorMoved { position, .. } => {
                if let Some(state) = self.windows.get_mut(&window_id) {
                    state.mouse_pos = (position.x as f32, position.y as f32);
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                self.handle_click(window_id);
            }
            WindowEvent::KeyboardInput { event, .. } => {
                self.handle_keyboard(window_id, &event);
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.process_requests(event_loop);
    }
}

/// Start the global HUML window manager thread
fn ensure_manager_started() -> &'static ManagerHandle {
    MANAGER.get_or_init(|| {
        let (request_tx, request_rx) = unbounded::<ManagerRequest>();
        let request_tx_clone = request_tx.clone();

        thread::spawn(move || {
            // Create event loop
            #[cfg(target_os = "linux")]
            let event_loop: EventLoop<ManagerRequest> = {
                use winit::platform::wayland::EventLoopBuilderExtWayland;
                EventLoop::with_user_event()
                    .with_any_thread(true)
                    .build()
                    .expect("Failed to create event loop")
            };

            #[cfg(not(target_os = "linux"))]
            let event_loop: EventLoop<ManagerRequest> = EventLoop::with_user_event()
                .build()
                .expect("Failed to create event loop");

            // Store proxy for sending requests
            let proxy = event_loop.create_proxy();
            if let Some(handle) = MANAGER.get() {
                *handle.event_proxy.lock().unwrap() = Some(proxy);
            }

            event_loop.set_control_flow(ControlFlow::Wait);

            let mut manager = HumlWindowManager::new(request_rx);
            if let Err(e) = event_loop.run_app(&mut manager) {
                tracing::error!("HUML window manager error: {}", e);
            }
        });

        ManagerHandle {
            request_tx: request_tx_clone,
            event_proxy: Mutex::new(None),
        }
    })
}

// =============================================================================
// Public API
// =============================================================================

/// State shared between Tauri and HUML windows
pub struct HumlPluginState {
    /// Labels of active windows
    pub active_labels: std::collections::HashSet<String>,
}

impl HumlPluginState {
    pub fn new() -> Self {
        Self {
            active_labels: std::collections::HashSet::new(),
        }
    }
}

impl Default for HumlPluginState {
    fn default() -> Self {
        Self::new()
    }
}

pub type SharedState = Arc<Mutex<HumlPluginState>>;

/// Command sent to a HUML window (kept for API compatibility)
#[derive(Debug, Clone)]
pub enum WindowCommand {
    Close,
    UpdateData { field: String, value: Value },
    Navigate { screen: String },
}

/// Initialize the HUML plugin
pub fn init(app: &AppHandle) {
    let state: SharedState = Arc::new(Mutex::new(HumlPluginState::new()));
    app.manage(state);

    // Start the manager thread
    ensure_manager_started();

    tracing::info!("HUML plugin initialized with shared event loop");
}

/// Create a new HUML window
pub fn create_window(
    app: &AppHandle,
    label: &str,
    title: &str,
    width: u32,
    height: u32,
    template: ParsedTemplate,
    cel: Arc<CelEvaluator>,
    query_bridge: Option<QueryBridgeHandle>,
    initial_query_data: HashMap<String, Vec<Value>>,
) -> Result<(), Error> {
    let state = app
        .try_state::<SharedState>()
        .ok_or_else(|| Error::msg("HUML plugin not initialized"))?;

    // Check if label exists
    {
        let guard = state.lock().unwrap();
        if guard.active_labels.contains(label) {
            return Err(Error::msg(format!("Window '{}' already exists", label)));
        }
    }

    let manager = ensure_manager_started();

    // Create reply channel
    let (reply_tx, reply_rx) = bounded(1);

    let request = ManagerRequest::CreateWindow {
        label: label.to_string(),
        title: title.to_string(),
        width,
        height,
        template,
        cel,
        query_bridge,
        initial_query_data,
        reply: reply_tx,
    };

    // Send via event proxy if available (wakes up event loop immediately)
    {
        let proxy_guard = manager.event_proxy.lock().unwrap();
        if let Some(proxy) = proxy_guard.as_ref() {
            let _ = proxy.send_event(request);
        } else {
            // Fallback to channel
            manager.request_tx.send(request)?;
        }
    }

    // Wait for reply
    match reply_rx.recv_timeout(std::time::Duration::from_secs(10)) {
        Ok(Ok(())) => {
            let mut guard = state.lock().unwrap();
            guard.active_labels.insert(label.to_string());
            Ok(())
        }
        Ok(Err(e)) => Err(Error::msg(e)),
        Err(_) => Err(Error::msg("Timeout waiting for window creation")),
    }
}

/// Close a HUML window
pub fn close_window(app: &AppHandle, label: &str) -> Result<(), Error> {
    let state = app
        .try_state::<SharedState>()
        .ok_or_else(|| Error::msg("HUML plugin not initialized"))?;

    let manager = ensure_manager_started();

    let request = ManagerRequest::CloseWindow { label: label.to_string() };

    {
        let proxy_guard = manager.event_proxy.lock().unwrap();
        if let Some(proxy) = proxy_guard.as_ref() {
            let _ = proxy.send_event(request);
        } else {
            manager.request_tx.send(request)?;
        }
    }

    let mut guard = state.lock().unwrap();
    guard.active_labels.remove(label);

    Ok(())
}

/// Send data to a window
pub fn send_data(app: &AppHandle, label: &str, field: &str, value: Value) -> Result<(), Error> {
    let _state = app
        .try_state::<SharedState>()
        .ok_or_else(|| Error::msg("HUML plugin not initialized"))?;

    let manager = ensure_manager_started();

    let request = ManagerRequest::UpdateData {
        label: label.to_string(),
        field: field.to_string(),
        value,
    };

    {
        let proxy_guard = manager.event_proxy.lock().unwrap();
        if let Some(proxy) = proxy_guard.as_ref() {
            let _ = proxy.send_event(request);
        } else {
            manager.request_tx.send(request)?;
        }
    }

    Ok(())
}

/// Navigate to a screen
pub fn navigate_screen(app: &AppHandle, label: &str, screen: &str) -> Result<(), Error> {
    let _state = app
        .try_state::<SharedState>()
        .ok_or_else(|| Error::msg("HUML plugin not initialized"))?;

    let manager = ensure_manager_started();

    let request = ManagerRequest::Navigate {
        label: label.to_string(),
        screen: screen.to_string(),
    };

    {
        let proxy_guard = manager.event_proxy.lock().unwrap();
        if let Some(proxy) = proxy_guard.as_ref() {
            let _ = proxy.send_event(request);
        } else {
            manager.request_tx.send(request)?;
        }
    }

    Ok(())
}

// =============================================================================
// AppHandle Extension
// =============================================================================

/// Extension trait for AppHandle to create HUML windows.
pub trait AppHandleExt {
    fn create_huml_window(
        &self,
        label: &str,
        title: &str,
        width: u32,
        height: u32,
        template: ParsedTemplate,
        cel: Arc<CelEvaluator>,
        query_bridge: Option<QueryBridgeHandle>,
        initial_query_data: HashMap<String, Vec<Value>>,
    ) -> Result<(), Error>;

    fn close_huml_window(&self, label: &str) -> Result<(), Error>;
    fn send_huml_data(&self, label: &str, field: &str, value: Value) -> Result<(), Error>;
    fn navigate_huml_screen(&self, label: &str, screen: &str) -> Result<(), Error>;
}

impl AppHandleExt for AppHandle {
    fn create_huml_window(
        &self,
        label: &str,
        title: &str,
        width: u32,
        height: u32,
        template: ParsedTemplate,
        cel: Arc<CelEvaluator>,
        query_bridge: Option<QueryBridgeHandle>,
        initial_query_data: HashMap<String, Vec<Value>>,
    ) -> Result<(), Error> {
        create_window(self, label, title, width, height, template, cel, query_bridge, initial_query_data)
    }

    fn close_huml_window(&self, label: &str) -> Result<(), Error> {
        close_window(self, label)
    }

    fn send_huml_data(&self, label: &str, field: &str, value: Value) -> Result<(), Error> {
        send_data(self, label, field, value)
    }

    fn navigate_huml_screen(&self, label: &str, screen: &str) -> Result<(), Error> {
        navigate_screen(self, label, screen)
    }
}
