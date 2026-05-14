//! egui runtime: owns the Lua VM and drives `on_frame` each repaint.
//!
//! Spawned on its own OS thread; `Lua` is created inside so it lives where it
//! runs. Bindings: `ui` (LuaUi userdata) and `scribe` (from lua_runtime).
//! `on_init` runs once before the first frame; `on_frame(ui)` runs on every repaint.

#![allow(deprecated)] // egui::CentralPanel::show is still the right API at this level

use butler::{Butler, PageUpdate, ScribeMessage};
use lua_runtime::{json_to_lua, lua_to_json_err, ActorScribeHandle, LuaCommand, ScribeBindings};
use mlua::{Function, Lua, RegistryKey, Value as LuaValue};
use ractor::ActorRef;
use std::sync::mpsc::Receiver as StdReceiver;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc;

use crate::ui_bindings::{LuaUi, UI_PTR};

/// Driver for an uploaded Lua UI. Frame-driven by `GlfwEguiHost::run`.
pub struct EguiApp {
    page_id: String,
    app_name: String,
    lua: Lua,
    /// `on_frame(ui)` function — looked up at init, re-resolved on reload.
    on_frame: Option<Function>,
    /// Stable LuaUi userdata; passed to `on_frame` each call.
    ui_key: RegistryKey,
    /// Held for hot reload (control-socket `refresh_app`). `None` when headless.
    scribe_ref: Option<ActorRef<ScribeMessage>>,
    /// Forwarded page-update queue from the wake-up thread; drained each frame.
    /// `None` for the headless variant.
    page_update_rx: Option<StdReceiver<PageUpdate>>,
    /// External command queue (control-socket DebugEval bridge). Drained
    /// non-blocking at the top of each frame.
    lua_rx: Option<mpsc::Receiver<LuaCommand>>,
}

impl EguiApp {
    /// Construct a stateless EguiApp for dev/testing.
    ///
    /// Skips scribe + butler wiring; exercises only the UI binding surface.
    pub fn new_headless(
        app_name: String,
        lua_code: String,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let page_id = format!("dev:{}", app_name);
        tracing::info!(app_name = %app_name, "Starting headless egui app (no scribe)");

        let lua = Lua::new();
        lua.load(&lua_code)
            .set_name(&format!("{}:{}", page_id, app_name))
            .exec()?;

        let ui_ud = lua.create_userdata(LuaUi)?;
        let ui_key = lua.create_registry_value(ui_ud)?;

        if let Ok(init_fn) = lua.globals().get::<Function>("on_init") {
            if let Err(e) = init_fn.call::<()>(()) {
                tracing::error!(error = %e, "Lua on_init() error");
                return Err(Box::new(e));
            }
        }

        let on_frame = lua.globals().get::<Function>("on_frame").ok();
        Ok(Self {
            page_id,
            app_name,
            lua,
            on_frame,
            ui_key,
            scribe_ref: None,
            page_update_rx: None,
            lua_rx: None,
        })
    }

    /// Create the app: build a Lua VM, load user code, register `scribe` + `ui`
    /// bindings, and call `on_init` if defined. Runs on the runtime thread
    /// before `GlfwEguiHost::run`.
    pub fn new(
        page_id: String,
        app_name: String,
        butler: Arc<Butler>,
        scribe_ref: ActorRef<ScribeMessage>,
        lua_code: String,
        page_update_rx: StdReceiver<PageUpdate>,
        lua_rx: mpsc::Receiver<LuaCommand>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let app_ctx = butler.app_context(&page_id);
        let our_did = app_ctx.user_did;

        tracing::info!(
            page_id = %page_id,
            app_name = %app_name,
            our_did = %our_did,
            "Starting egui app",
        );

        let lua = Lua::new();

        lua.load(&lua_code)
            .set_name(&format!("{}:{}", page_id, app_name))
            .exec()?;

        let scribe_bindings = ScribeBindings::new(
            ActorScribeHandle::new(scribe_ref.clone()),
            page_id.clone(),
            our_did.clone(),
            None,
        );
        lua.globals().set("scribe", scribe_bindings)?;

        let ui_ud = lua.create_userdata(LuaUi)?;
        let ui_key = lua.create_registry_value(ui_ud)?;

        if let Ok(init_fn) = lua.globals().get::<Function>("on_init") {
            if let Err(e) = init_fn.call::<()>(()) {
                tracing::error!(page_id = %page_id, error = %e, "Lua on_init() error");
                return Err(Box::new(e));
            }
            tracing::debug!(page_id = %page_id, "Called on_init");
        }

        let on_frame = lua.globals().get::<Function>("on_frame").ok();
        if on_frame.is_none() {
            tracing::warn!(
                page_id = %page_id,
                "App did not define on_frame — window will be blank",
            );
        }

        Ok(Self {
            page_id,
            app_name,
            lua,
            on_frame,
            ui_key,
            scribe_ref: Some(scribe_ref),
            page_update_rx: Some(page_update_rx),
            lua_rx: Some(lua_rx),
        })
    }

    /// Reload the Lua VM from the latest app files in scribe.
    ///
    /// Invoked on `LayerChanged` for our app layer. Re-execs app.lua against
    /// the same VM, so globals (`scribe`, `ui`, user state) survive the reload.
    fn reload_app(&mut self) -> std::result::Result<(), String> {
        let scribe = self
            .scribe_ref
            .as_ref()
            .ok_or_else(|| "no scribe ref for reload (headless variant)".to_string())?;
        let (tx, rx) = tokio::sync::oneshot::channel();
        scribe
            .cast(ScribeMessage::GetAppFiles {
                app_name: self.app_name.clone(),
                reply: tx,
            })
            .map_err(|e| format!("GetAppFiles cast: {}", e))?;
        // egui frame thread is a plain OS thread (no tokio worker) — blocking_recv ok.
        let files = rx
            .blocking_recv()
            .map_err(|e| format!("GetAppFiles reply: {}", e))?
            .map_err(|e| format!("GetAppFiles: {}", e))?;
        let lua_code = files
            .get("app.lua")
            .ok_or_else(|| "app.lua missing from refreshed layer".to_string())?
            .clone();
        self.lua
            .load(&lua_code)
            .set_name(format!("{}:{}:reload", self.page_id, self.app_name))
            .exec()
            .map_err(|e| format!("re-exec app.lua: {}", e))?;
        self.on_frame = self.lua.globals().get::<Function>("on_frame").ok();
        if let Ok(init_fn) = self.lua.globals().get::<Function>("on_init") {
            if let Err(e) = init_fn.call::<()>(()) {
                tracing::warn!(error = %e, "on_init() failed after reload");
            }
        }
        tracing::info!(page_id = %self.page_id, "egui app reloaded");
        Ok(())
    }

    /// Drain external `LuaCommand` requests (control-socket eval bridge).
    /// Only `DebugEval` is handled; other variants are accepted-and-logged.
    fn process_lua_commands(&mut self) {
        let rx = match self.lua_rx.as_mut() {
            Some(r) => r,
            None => return,
        };
        loop {
            match rx.try_recv() {
                Ok(LuaCommand::DebugEval { code, response_tx }) => {
                    let result: Result<serde_json::Value, String> =
                        match self.lua.load(&code).eval::<LuaValue>() {
                            Ok(v) => lua_to_json_err(&v).map_err(|e| e.to_string()),
                            Err(e) => Err(e.to_string()),
                        };
                    let _ = response_tx.send(result);
                }
                Ok(other) => {
                    tracing::debug!(
                        page_id = %self.page_id,
                        cmd = ?std::mem::discriminant(&other),
                        "egui app received LuaCommand variant it does not handle yet",
                    );
                }
                Err(mpsc::error::TryRecvError::Empty) => break,
                Err(mpsc::error::TryRecvError::Disconnected) => {
                    self.lua_rx = None;
                    break;
                }
            }
        }
    }

    /// Drain pending page updates and dispatch them to Lua callbacks
    /// (`on_ephemeral`, `on_peer_joined`, `on_peer_left`).
    fn process_sync_updates(&mut self) {
        // Drain into a local Vec first so we don't hold a borrow of
        // `page_update_rx` across `reload_app` which needs `&mut self`.
        let mut updates = Vec::new();
        if let Some(rx) = self.page_update_rx.as_ref() {
            while let Ok(update) = rx.try_recv() {
                updates.push(update);
            }
        }
        for update in updates {
            match update {
                PageUpdate::StructuredEphemeral {
                    from_did,
                    func,
                    args,
                } => {
                    let Ok(on_eph) = self.lua.globals().get::<Function>("on_ephemeral") else {
                        continue;
                    };
                    match json_to_lua(&self.lua, &args) {
                        Ok(args_lua) => {
                            if let Err(e) =
                                on_eph.call::<()>((from_did.clone(), func.clone(), args_lua))
                            {
                                tracing::warn!(
                                    page_id = %self.page_id,
                                    error = %e,
                                    func = %func,
                                    "on_ephemeral failed",
                                );
                            }
                        }
                        Err(e) => tracing::warn!(error = %e, func = %func, "json→lua failed"),
                    }
                }
                PageUpdate::PeerSubscribed { did, .. } => {
                    if let Ok(cb) = self.lua.globals().get::<Function>("on_peer_joined") {
                        if let Err(e) = cb.call::<()>(did.clone()) {
                            tracing::warn!(error = %e, did = %did, "on_peer_joined failed");
                        }
                    }
                }
                PageUpdate::PeerUnsubscribed { did } => {
                    if let Ok(cb) = self.lua.globals().get::<Function>("on_peer_left") {
                        if let Err(e) = cb.call::<()>(did.clone()) {
                            tracing::warn!(error = %e, did = %did, "on_peer_left failed");
                        }
                    }
                }
                PageUpdate::LayerChanged { layer, created, .. } => {
                    // Hot reload: if our own app code layer changed, re-exec app.lua.
                    let expected = format!("app:{}", self.app_name);
                    if layer == expected && !created {
                        if let Err(e) = self.reload_app() {
                            tracing::error!(error = %e, "app reload failed");
                        }
                    }
                }
                // Other variants still triggered a repaint via the wake-up
                // thread; reactive bindings re-read fresh data on next frame.
                _ => {}
            }
        }
    }
}

impl EguiApp {
    /// Drive one frame: wrap a `CentralPanel`, then call into Lua's `on_frame`.
    /// Installs thread-local `UI_PTR` for the panel's `Ui` while Lua runs.
    pub fn frame(&mut self, ctx: &egui::Context) {
        // Drain eval requests first — they change Lua state the next on_frame
        // should reflect. Then drain peer updates before drawing.
        self.process_lua_commands();
        self.process_sync_updates();

        let on_frame = match self.on_frame.as_ref() {
            Some(f) => f.clone(),
            None => {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.label(format!("{} (no on_frame defined)", self.app_name));
                });
                return;
            }
        };

        let ui_val: mlua::AnyUserData = match self.lua.registry_value(&self.ui_key) {
            Ok(v) => v,
            Err(e) => {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.colored_label(
                        egui::Color32::RED,
                        format!("egui runtime: failed to fetch ui handle: {}", e),
                    );
                });
                return;
            }
        };

        let page_id = self.page_id.clone();
        egui::CentralPanel::default().show(ctx, |ui| {
            UI_PTR.with(|c| c.set(ui as *mut _));
            let t0 = Instant::now();
            let result = on_frame.call::<()>(ui_val);
            let elapsed = t0.elapsed();
            UI_PTR.with(|c| c.set(std::ptr::null_mut()));

            if let Err(e) = result {
                tracing::error!(page_id = %page_id, error = %e, "Lua on_frame() error");
                // TextEdit (not Label) so the traceback is selectable + copyable.
                let mut msg = format!("Lua error: {}", e);
                ui.horizontal(|ui| {
                    if ui.button("Copy").clicked() {
                        ui.ctx().copy_text(msg.clone());
                    }
                    ui.colored_label(egui::Color32::RED, "Lua on_frame error (selectable below)");
                });
                ui.add(
                    egui::TextEdit::multiline(&mut msg)
                        .desired_width(f32::INFINITY)
                        .desired_rows(10)
                        .font(egui::TextStyle::Monospace)
                        .text_color(egui::Color32::from_rgb(255, 120, 120))
                        .interactive(true),
                );
            }

            // Dev signal: > 16 ms misses 60Hz; catches runaway scripts.
            if elapsed.as_millis() > 16 {
                tracing::warn!(
                    page_id = %page_id,
                    elapsed_ms = elapsed.as_millis() as u64,
                    "Lua on_frame() exceeded frame budget",
                );
            }
        });
    }
}
