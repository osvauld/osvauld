//! Unified Lua Runtime
//!
//! Single runtime implementation used by both shell (viewer) and kunki (node).
//!
//! ## Features
//!
//! - **OS thread spawning**: Uses `std::thread::spawn` (mlua::Lua is NOT Send for tokio)
//! - **Timer support**: JS-style `setTimeout`/`setInterval` via Scheduler
//! - **Validation**: Built-in `validate_ops()` method for permission checking
//! - **Optional UI**: Enable/disable UI bindings based on configuration
//!
//! ## Usage
//!
//! ```ignore
//! let (thread, cmd_tx) = LuaRuntime::spawn(LuaRuntimeConfig {
//!     page_id: "page123".into(),
//!     app_name: "My App".into(),
//!     scribe,
//!     user_did: "did:key:...".into(),
//!     user_name: "Alice".into(),
//!     user_role: "viewer".into(),
//!     lua_code: app_lua,
//!     ui_enabled: true,
//!     ui_tx: Some(ui_channel),
//!     query_tx: Some(query_channel),
//! })?;
//! ```

use std::sync::Arc;

use mlua::{Function, Lua, ObjectLike, Table, Value};
use parking_lot::Mutex;
use serde_json::Value as JsonValue;
use tokio::sync::mpsc;
use tracing::{debug, info, trace, warn};

use crate::scribe_handle::ScribeHandle;
use butler::LoroDelta;

use crate::bindings::binding::{data_to_ui_mutation, process_binding_data, BindingManager};
use crate::bindings::convert::{json_to_lua, lua_to_json};
use crate::bindings::derivation::DerivationBindings;
use crate::bindings::emoji::EmojiBindings;
use crate::bindings::layout::LayoutBindings;
use crate::bindings::page::PageBindings;
use crate::bindings::peers::PeersBindings;
use crate::bindings::permit::PermitBindings;
use crate::bindings::scribe::ScribeBindings;
use crate::bindings::ui::{UiBindings, UiSharedState};
use crate::commands::{DebugState, LuaCommand, ValidationContext, ValidationResult};
use crate::scheduler::Scheduler;
use crate::ui_types::{UiMutation, UiQuery};
use crate::{LUA_API_MODULE, LUA_BINDING_MODULE, LUA_DATE_MODULE, LUA_PRESENCE_MODULE};

mod buffered_ui;
mod debug;
mod handlers;
mod timer_bindings;
#[cfg(test)]
mod validation_tests;

use buffered_ui::BufferedUiBindings;
pub use buffered_ui::BufferedUiState;
use debug::collect_lua_globals;
use timer_bindings::register_timer_functions;

// Configuration

/// Configuration for creating a LuaRuntime
pub struct LuaRuntimeConfig {
    /// Page ID this runtime is for
    pub page_id: String,

    /// App name (for logging)
    pub app_name: String,

    /// Reference to the Scribe handle for CRDT operations
    pub scribe: Arc<dyn ScribeHandle>,

    /// User's DID
    pub user_did: String,

    /// User's display name
    pub user_name: String,

    /// User's role (owner, viewer, collaborator, etc.)
    pub user_role: String,

    /// Lua code to execute
    pub lua_code: String,

    /// Enable UI bindings (ui:set, ui:get, etc.)
    pub ui_enabled: bool,

    /// Channel for sending UI mutations (required if ui_enabled)
    pub ui_tx: Option<mpsc::Sender<UiMutation>>,

    /// Channel for sending UI queries (required if ui_enabled)
    pub query_tx: Option<mpsc::Sender<UiQuery>>,

    /// Channel for in-page app navigation (page:open_app triggers tab switch)
    pub navigate_tx: Option<std::sync::mpsc::Sender<String>>,
}

// Lua Runtime

/// Result of a single step of the event loop
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepResult {
    /// A command was processed
    Processed,
    /// No command available (idle)
    Idle,
    /// Shutdown was requested
    Shutdown,
}

/// Unified Lua runtime for both shell and node
pub struct LuaRuntime {
    /// Lua state
    lua: Lua,

    /// Page ID
    page_id: String,

    /// Timer scheduler (shared with TimerBindings)
    scheduler: Arc<Mutex<Scheduler>>,

    /// Command receiver
    cmd_rx: mpsc::Receiver<LuaCommand>,

    /// UI mutation sender (if UI enabled)
    ui_tx: Option<mpsc::Sender<UiMutation>>,

    /// UI shared state (for event bus, pending mutations)
    ui_shared: Arc<Mutex<UiSharedState>>,

    /// Binding manager for declarative layer → UI sync
    binding_manager: Arc<Mutex<BindingManager>>,

    /// Scribe handle for fetching layer data
    scribe: Arc<dyn ScribeHandle>,

    /// Whether UI is enabled
    ui_enabled: bool,

    /// Cached handler existence (populated after on_init)
    handler_cache: HandlerCache,
}

/// Cache of which Lua handler functions exist (avoids repeated globals lookups)
struct HandlerCache {
    on_layer_discovered: bool,
    on_ephemeral: bool,
    on_peer_joined: bool,
    on_peer_left: bool,
    on_asset_uploaded: bool,
    on_key_pressed: bool,
    on_text_input: bool,
    on_shutdown: bool,
}

impl HandlerCache {
    fn populate(lua: &Lua) -> Self {
        let has = |name: &str| -> bool { lua.globals().get::<Function>(name).is_ok() };
        Self {
            on_layer_discovered: has("on_layer_discovered"),
            on_ephemeral: has("on_ephemeral"),
            on_peer_joined: has("on_peer_joined"),
            on_peer_left: has("on_peer_left"),
            on_asset_uploaded: has("on_asset_uploaded"),
            on_key_pressed: has("on_key_pressed"),
            on_text_input: has("on_text_input"),
            on_shutdown: has("on_shutdown"),
        }
    }
}

impl LuaRuntime {
    /// Spawn Lua runtime on an OS thread
    ///
    /// **Threading**: Uses `std::thread::spawn` because mlua::Lua is NOT Send for tokio
    /// **Returns**: (thread handle, command sender)
    pub fn spawn(
        config: LuaRuntimeConfig,
    ) -> Result<(std::thread::JoinHandle<()>, mpsc::Sender<LuaCommand>), String> {
        let (cmd_tx, cmd_rx) = mpsc::channel(64);

        let handle = std::thread::spawn(move || {
            let mut runtime = match Self::new_internal(config, cmd_rx) {
                Ok(r) => r,
                Err(e) => {
                    tracing::error!(error = %e, "Failed to create LuaRuntime");
                    return;
                }
            };

            runtime.run();
        });

        Ok((handle, cmd_tx))
    }

    /// Create a headless runtime on the current thread (for testing)
    ///
    /// **Returns**: (runtime, command sender)
    /// **Usage**: Test harness creates runtime, sends commands, calls step() to pump
    pub fn new_headless(
        config: LuaRuntimeConfig,
    ) -> Result<(Self, mpsc::Sender<LuaCommand>), String> {
        let (cmd_tx, cmd_rx) = mpsc::channel(64);
        let runtime = Self::new_internal(config, cmd_rx)?;
        Ok((runtime, cmd_tx))
    }

    /// Call on_init handler (separated from run() for test harness)
    ///
    /// **Context**: Runs the Lua on_init function and re-populates handler cache
    pub fn call_on_init(&mut self) {
        if self.has_function("on_init") {
            if let Err(e) = self.call_handler("on_init", ()) {
                warn!(page_id = %self.page_id, error = %e, "on_init failed");
            } else {
                debug!(page_id = %self.page_id, "on_init completed");
            }
            self.flush_mutations();
            self.handler_cache = HandlerCache::populate(&self.lua);
        }
    }

    /// Process a single step of the event loop
    ///
    /// Fires due timers and tries to receive one command.
    /// Returns StepResult indicating what happened.
    pub fn step(&mut self) -> StepResult {
        // Fire any due timers
        self.fire_due_timers();

        // Try to receive one command (non-blocking)
        match self.cmd_rx.try_recv() {
            Ok(cmd) => {
                let should_exit = self.handle_command(cmd);
                self.flush_mutations();
                if should_exit {
                    StepResult::Shutdown
                } else {
                    StepResult::Processed
                }
            }
            Err(tokio::sync::mpsc::error::TryRecvError::Empty) => StepResult::Idle,
            Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => StepResult::Shutdown,
        }
    }

    /// Create runtime (internal, called on the OS thread)
    fn new_internal(
        config: LuaRuntimeConfig,
        cmd_rx: mpsc::Receiver<LuaCommand>,
    ) -> Result<Self, String> {
        let lua = Lua::new();
        let scheduler = Arc::new(Mutex::new(Scheduler::new()));
        let ui_shared = Arc::new(Mutex::new(UiSharedState::new()));

        // Create _timers table for storing callbacks
        let timers_table: Table = lua
            .create_table()
            .map_err(|e| format!("Failed to create _timers table: {}", e))?;
        lua.globals()
            .set("_timers", timers_table)
            .map_err(|e| format!("Failed to set _timers global: {}", e))?;

        // Register timer API (using table with functions for dot syntax)
        let timer_table = register_timer_functions(&lua, scheduler.clone())
            .map_err(|e| format!("Failed to create timer table: {}", e))?;
        lua.globals()
            .set("timer", timer_table)
            .map_err(|e| format!("Failed to set timer global: {}", e))?;

        // Register scribe bindings (unified CRDT API)
        // Use with_ui() when UI is enabled to support scribe:bind()
        let our_name = Some(config.user_name.clone());
        let scribe = if config.ui_enabled {
            if let Some(ref ui_tx) = config.ui_tx {
                ScribeBindings::with_ui(
                    config.scribe.clone(),
                    config.page_id.clone(),
                    config.user_did.clone(),
                    our_name,
                    ui_tx.clone(),
                )
            } else {
                ScribeBindings::new(
                    config.scribe.clone(),
                    config.page_id.clone(),
                    config.user_did.clone(),
                    our_name,
                )
            }
        } else {
            ScribeBindings::new(
                config.scribe.clone(),
                config.page_id.clone(),
                config.user_did.clone(),
                our_name,
            )
        };
        let binding_manager = scribe.binding_manager();
        lua.globals()
            .set("scribe", scribe)
            .map_err(|e| format!("Failed to set scribe global: {}", e))?;

        // Register derivation bindings
        let derivation = DerivationBindings::new(config.scribe.clone());
        lua.globals()
            .set("derivation", derivation)
            .map_err(|e| format!("Failed to set derivation global: {}", e))?;

        // Register permit bindings
        let permit = PermitBindings::new(
            config.page_id.clone(),
            config.user_did.clone(),
            config.user_name.clone(),
            config.user_role.clone(),
        );
        lua.globals()
            .set("permit", permit)
            .map_err(|e| format!("Failed to set permit global: {}", e))?;

        // Register peers bindings
        let peers = PeersBindings::new(config.scribe.clone());
        lua.globals()
            .set("peers", peers)
            .map_err(|e| format!("Failed to set peers global: {}", e))?;

        // Register page bindings
        let page = PageBindings {
            navigate_tx: config.navigate_tx,
        };
        lua.globals()
            .set("page", page)
            .map_err(|e| format!("Failed to set page global: {}", e))?;

        // Register UI bindings (real or stub)
        if config.ui_enabled {
            if let (Some(_ui_tx), Some(query_tx)) = (config.ui_tx.clone(), config.query_tx.clone())
            {
                let ui = UiBindings {
                    app_id: config.page_id.clone(),
                    query_tx,
                    shared: ui_shared.clone(),
                };
                lua.globals()
                    .set("ui", ui)
                    .map_err(|e| format!("Failed to set ui global: {}", e))?;

                // UI-only bindings: layout, emoji
                let layout = LayoutBindings::new();
                lua.globals()
                    .set("layout", layout)
                    .map_err(|e| format!("Failed to set layout global: {}", e))?;

                let emoji = EmojiBindings;
                lua.globals()
                    .set("emoji", emoji)
                    .map_err(|e| format!("Failed to set emoji global: {}", e))?;
            } else {
                return Err("ui_enabled=true requires ui_tx and query_tx".into());
            }
        } else {
            let ui = BufferedUiBindings::new();
            lua.globals()
                .set("ui", ui)
                .map_err(|e| format!("Failed to set ui global: {}", e))?;
        }

        // Load API module
        let api_module: Value = lua
            .load(LUA_API_MODULE)
            .eval()
            .map_err(|e| format!("Failed to load api module: {}", e))?;
        lua.globals()
            .set("api", api_module)
            .map_err(|e| format!("Failed to set api global: {}", e))?;

        // Load datetime module
        let datetime_module: Value = lua
            .load(LUA_DATE_MODULE)
            .eval()
            .map_err(|e| format!("Failed to load datetime module: {}", e))?;
        lua.globals()
            .set("datetime", datetime_module)
            .map_err(|e| format!("Failed to set datetime global: {}", e))?;

        // Load presence module
        let presence_module: Value = lua
            .load(LUA_PRESENCE_MODULE)
            .eval()
            .map_err(|e| format!("Failed to load presence module: {}", e))?;
        lua.globals()
            .set("presence_lib", presence_module)
            .map_err(|e| format!("Failed to set presence_lib global: {}", e))?;

        // Load binding module for reactive UI updates
        let binding_module: Value = lua
            .load(LUA_BINDING_MODULE)
            .eval()
            .map_err(|e| format!("Failed to load binding module: {}", e))?;
        lua.globals()
            .set("binding", binding_module)
            .map_err(|e| format!("Failed to set binding global: {}", e))?;

        // Load app code
        lua.load(&config.lua_code)
            .exec()
            .map_err(|e| format!("Failed to load Lua code: {}", e))?;

        info!(
            page_id = %config.page_id,
            app_name = %config.app_name,
            ui_enabled = config.ui_enabled,
            "LuaRuntime created"
        );

        let handler_cache = HandlerCache::populate(&lua);

        Ok(Self {
            lua,
            page_id: config.page_id,
            scheduler,
            cmd_rx,
            ui_tx: config.ui_tx,
            ui_shared,
            binding_manager,
            scribe: config.scribe,
            ui_enabled: config.ui_enabled,
            handler_cache,
        })
    }

    /// Run the event loop (blocking)
    fn run(&mut self) {
        info!(page_id = %self.page_id, "Lua runtime event loop started");

        self.call_on_init();

        loop {
            match self.step() {
                StepResult::Shutdown => return,
                StepResult::Processed => {
                    // Processed a command, loop immediately to check for more
                }
                StepResult::Idle => {
                    // No command available — sleep briefly to yield CPU, then retry
                    // Timer resolution is handled by fire_due_timers() in step()
                    let sleep_ms = {
                        let scheduler = self.scheduler.lock();
                        scheduler
                            .time_until_next()
                            .min(std::time::Duration::from_millis(16))
                    };
                    std::thread::sleep(sleep_ms.min(std::time::Duration::from_millis(1)));
                }
            }
        }
    }

    /// Check if a Lua function is defined
    fn has_function(&self, name: &str) -> bool {
        self.lua.globals().get::<Function>(name).is_ok()
    }

    /// Call a Lua function with arguments.
    fn call_handler<A: mlua::IntoLuaMulti>(&self, name: &str, args: A) -> Result<(), String> {
        let func: Function = self
            .lua
            .globals()
            .get(name)
            .map_err(|e| format!("Function '{}' not found: {}", name, e))?;

        func.call::<()>(args)
            .map_err(|e| format!("Error calling '{}': {}", name, e))
    }

    /// Handle a command
    ///
    /// Returns true if shutdown was requested
    fn handle_command(&mut self, cmd: LuaCommand) -> bool {
        match cmd {
            LuaCommand::Shutdown => {
                if self.handler_cache.on_shutdown {
                    if let Err(e) = self.call_handler("on_shutdown", ()) {
                        warn!(page_id = %self.page_id, error = %e, "on_shutdown failed");
                    }
                }
                info!(page_id = %self.page_id, "Shutdown requested");
                true
            }

            LuaCommand::LayerChanged {
                layer_name,
                created,
                delta,
                full_data,
            } => {
                let result = if created {
                    self.handle_layer_discovered(&layer_name)
                } else {
                    self.handle_loro_change(&layer_name, delta, full_data)
                };
                if let Err(e) = result {
                    warn!(page_id = %self.page_id, layer = %layer_name, error = %e, "Layer change error");
                }
                false
            }

            LuaCommand::TimerFired { timer_id } => {
                self.fire_timer(timer_id);
                false
            }

            LuaCommand::Ephemeral { user_did, payload } => {
                self.handle_ephemeral(&user_did, &payload);
                false
            }

            LuaCommand::StructuredEphemeral {
                from_did,
                func,
                args,
            } => {
                self.handle_structured_ephemeral(&from_did, &func, &args);
                false
            }

            LuaCommand::PeerJoined { user_did } => {
                self.handle_peer_joined(&user_did);
                false
            }

            LuaCommand::PeerLeft { user_did } => {
                self.handle_peer_left(&user_did);
                false
            }

            LuaCommand::AssetUploaded {
                hash,
                filename,
                mime_type,
                size,
            } => {
                self.handle_asset_uploaded(&hash, &filename, &mime_type, size);
                false
            }

            LuaCommand::UiCallback {
                callback_name,
                args,
            } => {
                if let Err(e) = self.handle_ui_callback(&callback_name, args) {
                    warn!(page_id = %self.page_id, callback = %callback_name, error = %e, "UI callback error");
                }
                false
            }

            LuaCommand::UiEvent { event } => {
                if let Err(e) = self.handle_ui_event(event) {
                    warn!(page_id = %self.page_id, error = %e, "UI event error");
                }
                false
            }

            LuaCommand::Validate { ctx, response_tx } => {
                let result = self.validate_ops(&ctx);
                let _ = response_tx.send(result);
                false
            }

            LuaCommand::RebuildDerivation => {
                if let Ok(derivation) = self.lua.globals().get::<mlua::AnyUserData>("derivation") {
                    if let Err(e) = derivation.call_method::<()>("rebuild_all", ()) {
                        warn!(error = %e, "derivation:rebuild_all() failed");
                    }
                }
                false
            }

            LuaCommand::DebugEval { code, response_tx } => {
                let result = self.lua.load(&code).eval::<Value>();
                let json_result = match result {
                    Ok(v) => lua_to_json(&v).map_err(|e| e.to_string()),
                    Err(e) => Err(e.to_string()),
                };
                let _ = response_tx.send(json_result);
                false
            }

            LuaCommand::DebugGetState { response_tx } => {
                let state = DebugState {
                    globals: collect_lua_globals(&self.lua),
                    timers: self.scheduler.lock().active_timer_count(),
                    has_on_init: self.has_function("on_init"),
                };
                let _ = response_tx.send(state);
                false
            }
        }
    }

    /// Handle Loro layer change
    ///
    /// **Ops**: Structured operations extracted from update (same format as validation)
    /// **Delta**: Incremental delta for surgical VecModel ops (Insert/Remove)
    /// **full_data**: Complete layer state for Replace operations or fallback (None when delta available)
    fn handle_loro_change(
        &self,
        layer_name: &str,
        delta: Option<LoroDelta>,
        full_data: Option<JsonValue>,
    ) -> Result<(), String> {
        trace!(
            page_id = %self.page_id,
            layer = %layer_name,
            has_delta = delta.is_some(),
            has_full_data = full_data.is_some(),
            "handle_loro_change ENTRY"
        );

        // Process through binding system (handles delta surgically when available)
        let bindings_processed =
            self.process_bindings(layer_name, full_data.as_ref(), delta.as_ref());

        if bindings_processed {
            self.trigger_derivation(layer_name);
            return Ok(());
        }

        self.trigger_derivation(layer_name);

        Ok(())
    }

    /// Trigger derivation engine for a layer change (node-only, no-op on clients)
    ///
    /// **Context**: Scribe normalizes layer names to bare form (strips page_id prefix),
    /// but Lua derivation rules register with prefixed patterns (page_id .. "/orders/*").
    /// Re-add page_id prefix so on_source_change pattern matching works.
    fn trigger_derivation(&self, layer_name: &str) {
        if let Ok(derivation) = self.lua.globals().get::<mlua::AnyUserData>("derivation") {
            let prefixed = format!("{}/{}", self.page_id, layer_name);
            if let Err(e) = derivation.call_method::<()>("on_source_change", prefixed) {
                warn!(page_id = %self.page_id, layer = %layer_name, error = %e, "derivation:on_source_change() failed");
            }
        }
    }

    /// Process layer change through binding system
    ///
    /// **Delta-first**: When delta is available for non-wildcard bindings without filter,
    /// converts delta directly to surgical VecModelOps (Insert/Remove) — O(1) per change.
    /// Falls back to Replace (full state) only when delta is unavailable, for wildcards,
    /// or when filter is set (filter needs full state to re-evaluate).
    ///
    /// Returns true if any bindings were processed, false otherwise.
    fn process_bindings(
        &self,
        layer_name: &str,
        full_data: Option<&JsonValue>,
        delta: Option<&LoroDelta>,
    ) -> bool {
        if !self.ui_enabled {
            return false;
        }

        let manager = self.binding_manager.lock();
        let bindings = manager.get_bindings_for_layer(layer_name);

        if bindings.is_empty() {
            let all_bindings: Vec<_> = manager
                .all_bindings()
                .map(|b| format!("{}→{}", b.expanded_pattern, b.ui_property))
                .collect();
            debug!(
                page_id = %self.page_id,
                layer = %layer_name,
                registered_bindings = ?all_bindings,
                "No binding found for layer"
            );
            return false;
        }

        for binding in bindings {
            let ui_property = &binding.ui_property;

            // Delta-first path: try surgical updates for non-wildcard bindings
            if let Some(delta) = delta {
                if !binding.is_wildcard {
                    let layer_for_transform = if binding.is_wildcard {
                        Some(layer_name)
                    } else {
                        None
                    };
                    let ops = crate::bindings::binding::convert_delta_for_binding(
                        &self.lua,
                        binding,
                        delta,
                        layer_for_transform,
                    );
                    if !ops.is_empty() {
                        let mutation = UiMutation {
                            app_id: self.page_id.clone(),
                            properties: vec![],
                            model_ops: ops,
                        };
                        if let Some(ref ui_tx) = self.ui_tx {
                            if let Err(e) = ui_tx.try_send(mutation) {
                                warn!(page_id = %self.page_id, ui_property = %ui_property, error = %e, "Failed to send delta binding mutation");
                            } else {
                                debug!(page_id = %self.page_id, layer = %layer_name, ui_property = %ui_property, "Delta binding synced to UI");
                            }
                        }
                        continue;
                    }
                    // Delta conversion returned empty (Map/Text delta) — fall through to Replace
                }
            }

            // Fallback path: Replace (initial load, wildcards, filter bindings, no delta)
            let data = match full_data {
                Some(data) => data,
                None => {
                    // No full_data and delta path didn't work — skip this binding
                    debug!(page_id = %self.page_id, layer = %layer_name, ui_property = %ui_property,
                        "No full_data available for fallback Replace");
                    continue;
                }
            };

            let processed = match process_binding_data(
                &self.lua,
                binding,
                data,
                if binding.is_wildcard {
                    Some(layer_name)
                } else {
                    None
                },
            ) {
                Ok(data) => data,
                Err(e) => {
                    warn!(page_id = %self.page_id, layer = %layer_name, ui_property = %ui_property, error = %e,
                        "Failed to process binding data");
                    continue;
                }
            };

            if let Some(ref ui_tx) = self.ui_tx {
                if let Some(mutation) = data_to_ui_mutation(&self.page_id, ui_property, processed) {
                    if let Err(e) = ui_tx.try_send(mutation) {
                        warn!(page_id = %self.page_id, ui_property = %ui_property, error = %e,
                            "Failed to send binding UI mutation");
                    } else {
                        debug!(page_id = %self.page_id, layer = %layer_name, ui_property = %ui_property,
                            "Binding auto-synced to UI (Replace fallback)");
                    }
                }
            }
        }

        true
    }

    // Validation

    /// Validate operations against app's validation logic
    pub fn validate_ops(&self, ctx: &ValidationContext) -> Result<ValidationResult, String> {
        if !self.has_function("validate_ops") {
            return Ok(ValidationResult::default());
        }

        let ops_json = serde_json::to_value(&ctx.ops)
            .map_err(|e| format!("Failed to serialize ops: {}", e))?;
        let ops_lua =
            json_to_lua(&self.lua, &ops_json).map_err(|e| format!("Ops to Lua: {}", e))?;

        let func: Function = self.lua.globals().get("validate_ops").unwrap();
        let result: bool = func
            .call((
                ctx.layer_name.clone(),
                ops_lua,
                ctx.from_did.clone(),
                ctx.role.clone(),
                ctx.page_id.clone(),
            ))
            .map_err(|e| format!("validate_ops error: {}", e))?;

        Ok(ValidationResult {
            valid: result,
            error: if result {
                None
            } else {
                Some("Validation failed".into())
            },
            failed_op_index: None,
        })
    }
}
