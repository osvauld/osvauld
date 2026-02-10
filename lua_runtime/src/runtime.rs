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

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use mlua::{Function, Lua, ObjectLike, Table, UserData, UserDataMethods, Value};
use parking_lot::Mutex;
use serde_json::Value as JsonValue;
use tokio::sync::mpsc;
use tracing::{debug, info, trace, warn};

use butler::LoroDelta;
use crate::scribe_handle::ScribeHandle;

use crate::bindings::{
    json_to_lua, lua_to_json, process_binding_data, BindingManager,
    DerivationBindings, EmojiBindings, LayoutBindings, PageBindings, PeersBindings,
    PermitBindings, ScribeBindings, UiBindings, UiSharedState,
};
use crate::commands::{DebugState, LuaCommand, UiEventType, ValidationContext, ValidationResult};
use crate::event_bus::EventDelivery;
use crate::scheduler::Scheduler;
use crate::ui_types::{UiMutation, UiQuery};
use crate::{LUA_API_MODULE, LUA_DATE_MODULE, LUA_PRESENCE_MODULE, LUA_BINDING_MODULE};

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

// Buffered UI Bindings (for headless/test mode)

/// Buffered UI bindings for headless runtime
///
/// Stores properties in-memory and buffers all mutations for test inspection.
/// Implements the full ui:set/get/push/insert/remove/clear/update/subscribe/emit API.
pub struct BufferedUiBindings {
    state: Arc<std::sync::Mutex<BufferedUiState>>,
}

/// Internal state for BufferedUiBindings
pub struct BufferedUiState {
    /// Property values (scalar values set via ui:set)
    pub properties: HashMap<String, serde_json::Value>,
    /// Accumulated mutations (for test inspection via drain_mutations)
    pub mutations: Vec<UiMutation>,
    /// Model data (arrays set via ui:set with array values)
    pub models: HashMap<String, Vec<serde_json::Value>>,
}

impl BufferedUiState {
    fn new() -> Self {
        Self {
            properties: HashMap::new(),
            mutations: Vec::new(),
            models: HashMap::new(),
        }
    }
}

impl BufferedUiBindings {
    pub fn new() -> Self {
        Self {
            state: Arc::new(std::sync::Mutex::new(BufferedUiState::new())),
        }
    }

    /// Get the shared state for external inspection
    pub fn state(&self) -> Arc<std::sync::Mutex<BufferedUiState>> {
        self.state.clone()
    }
}

impl UserData for BufferedUiBindings {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("get", |lua, this, key: String| {
            let state = this.state.lock().unwrap();
            match state.properties.get(&key) {
                Some(v) => json_to_lua(lua, v),
                None => Ok(Value::Nil),
            }
        });

        methods.add_method("set", |_lua, this, (key, value): (String, Value)| {
            let json_value =
                lua_to_json(&value).map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
            let mut state = this.state.lock().unwrap();

            if let serde_json::Value::Array(ref items) = json_value {
                state.models.insert(key.clone(), items.clone());
                state.mutations.push(UiMutation {
                    app_id: String::new(),
                    properties: vec![],
                    model_ops: vec![crate::ui_types::VecModelOp::Replace {
                        model_name: key,
                        items: items.clone(),
                    }],
                });
            } else {
                state.properties.insert(key.clone(), json_value.clone());
                state.mutations.push(UiMutation {
                    app_id: String::new(),
                    properties: vec![crate::ui_types::PropertyUpdate {
                        key,
                        value: json_value,
                    }],
                    model_ops: vec![],
                });
            }
            Ok(())
        });

        methods.add_method("push", |_lua, this, (model_name, item): (String, Value)| {
            let item_json =
                lua_to_json(&item).map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
            let mut state = this.state.lock().unwrap();
            state.models.entry(model_name.clone()).or_default().push(item_json.clone());
            state.mutations.push(UiMutation {
                app_id: String::new(),
                properties: vec![],
                model_ops: vec![crate::ui_types::VecModelOp::Push {
                    model_name,
                    item: item_json,
                }],
            });
            Ok(())
        });

        methods.add_method(
            "insert",
            |_lua, this, (model_name, index, item): (String, usize, Value)| {
                let item_json =
                    lua_to_json(&item).map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
                let mut state = this.state.lock().unwrap();
                let model = state.models.entry(model_name.clone()).or_default();
                if index <= model.len() {
                    model.insert(index, item_json.clone());
                }
                state.mutations.push(UiMutation {
                    app_id: String::new(),
                    properties: vec![],
                    model_ops: vec![crate::ui_types::VecModelOp::Insert {
                        model_name,
                        index,
                        item: item_json,
                    }],
                });
                Ok(())
            },
        );

        methods.add_method("remove", |_lua, this, (model_name, index): (String, usize)| {
            let mut state = this.state.lock().unwrap();
            if let Some(model) = state.models.get_mut(&model_name) {
                if index < model.len() {
                    model.remove(index);
                }
            }
            state.mutations.push(UiMutation {
                app_id: String::new(),
                properties: vec![],
                model_ops: vec![crate::ui_types::VecModelOp::Remove {
                    model_name,
                    index,
                }],
            });
            Ok(())
        });

        methods.add_method("clear", |_lua, this, model_name: String| {
            let mut state = this.state.lock().unwrap();
            state.models.insert(model_name.clone(), vec![]);
            state.mutations.push(UiMutation {
                app_id: String::new(),
                properties: vec![],
                model_ops: vec![crate::ui_types::VecModelOp::Clear {
                    model_name,
                }],
            });
            Ok(())
        });

        methods.add_method(
            "update",
            |_lua, this, (model_name, index, item): (String, usize, Value)| {
                let item_json =
                    lua_to_json(&item).map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
                let mut state = this.state.lock().unwrap();
                if let Some(model) = state.models.get_mut(&model_name) {
                    if index < model.len() {
                        model[index] = item_json.clone();
                    }
                }
                state.mutations.push(UiMutation {
                    app_id: String::new(),
                    properties: vec![],
                    model_ops: vec![crate::ui_types::VecModelOp::Set {
                        model_name,
                        index,
                        item: item_json,
                    }],
                });
                Ok(())
            },
        );

        methods.add_method(
            "subscribe",
            |_lua, _this, _args: mlua::MultiValue| Ok(0i64),
        );
        methods.add_method("unsubscribe", |_lua, _this, _id: i64| Ok(false));
        methods.add_method("emit", |_lua, _this, _args: (String, Value)| Ok(0i64));
    }
}

// Timer Bindings

/// Register timer functions on a Lua table
///
/// Creates timer.setTimeout, timer.setInterval, timer.clear as regular functions
/// that can be called with dot syntax (timer.setInterval(ms, callback))
fn register_timer_functions(lua: &Lua, scheduler: Arc<Mutex<Scheduler>>) -> mlua::Result<Table> {
    let timer_table = lua.create_table()?;

    // timer.setTimeout(ms, callback) -> id
    let sched = scheduler.clone();
    timer_table.set("setTimeout", lua.create_function(move |lua, (ms, callback): (u64, Function)| {
        let id = {
            let mut scheduler = sched.lock();
            scheduler.register_timer(ms, false)
        };
        let timers: Table = lua.globals().get("_timers")?;
        timers.set(id, callback)?;
        trace!(timer_id = id, ms, "setTimeout registered");
        Ok(id)
    })?)?;

    // timer.setInterval(ms, callback) -> id
    let sched = scheduler.clone();
    timer_table.set("setInterval", lua.create_function(move |lua, (ms, callback): (u64, Function)| {
        let id = {
            let mut scheduler = sched.lock();
            scheduler.register_timer(ms, true)
        };
        let timers: Table = lua.globals().get("_timers")?;
        timers.set(id, callback)?;
        trace!(timer_id = id, ms, "setInterval registered");
        Ok(id)
    })?)?;

    // timer.clear(id)
    let sched = scheduler.clone();
    timer_table.set("clear", lua.create_function(move |lua, id: u64| {
        {
            let mut scheduler = sched.lock();
            scheduler.clear_timer(id);
        }
        let timers: Table = lua.globals().get("_timers")?;
        timers.set(id, Value::Nil)?;
        trace!(timer_id = id, "Timer cleared");
        Ok(())
    })?)?;

    Ok(timer_table)
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
    on_loro_change: bool,
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
        let has = |name: &str| -> bool {
            lua.globals().get::<Function>(name).is_ok()
        };
        Self {
            on_loro_change: has("on_loro_change"),
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
            if let Err(e) = self.call_no_args::<()>("on_init") {
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
            handlers: Arc::new(RwLock::new(Vec::new())),
            page_id: config.page_id.clone(),
            navigate_tx: config.navigate_tx,
        };
        lua.globals()
            .set("page", page)
            .map_err(|e| format!("Failed to set page global: {}", e))?;

        // Register UI bindings (real or stub)
        if config.ui_enabled {
            if let (Some(ui_tx), Some(query_tx)) = (config.ui_tx.clone(), config.query_tx.clone()) {
                let ui = UiBindings {
                    app_id: config.page_id.clone(),
                    ui_tx,
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
                        scheduler.time_until_next().min(std::time::Duration::from_millis(16))
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

    /// Call a Lua function with no arguments
    fn call_no_args<R: mlua::FromLuaMulti>(&self, name: &str) -> Result<R, String> {
        let func: Function = self
            .lua
            .globals()
            .get(name)
            .map_err(|e| format!("Function '{}' not found: {}", name, e))?;

        func.call(())
            .map_err(|e| format!("Error calling '{}': {}", name, e))
    }

    /// Handle a command
    ///
    /// Returns true if shutdown was requested
    fn handle_command(&mut self, cmd: LuaCommand) -> bool {
        match cmd {
            LuaCommand::Shutdown => {
                if self.handler_cache.on_shutdown {
                    if let Err(e) = self.call_no_args::<()>("on_shutdown") {
                        warn!(page_id = %self.page_id, error = %e, "on_shutdown failed");
                    }
                }
                info!(page_id = %self.page_id, "Shutdown requested");
                true
            }

            LuaCommand::LoroChanged {
                layer_name,
                ops,
                delta,
                full_data,
            } => {
                if let Err(e) = self.handle_loro_change(&layer_name, ops, delta, full_data) {
                    warn!(page_id = %self.page_id, layer = %layer_name, error = %e, "Loro change error");
                }
                false
            }

            LuaCommand::LayerDiscovered { layer_name } => {
                if let Err(e) = self.handle_layer_discovered(&layer_name) {
                    warn!(page_id = %self.page_id, layer = %layer_name, error = %e, "Layer discovered error");
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

            LuaCommand::StructuredEphemeral { from_did, func, args } => {
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

            LuaCommand::UiCallback { callback_name, args } => {
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
                    globals: self.collect_lua_globals(),
                    timers: self.scheduler.lock().active_timer_count(),
                    has_on_init: self.has_function("on_init"),
                    has_on_loro_change: self.handler_cache.on_loro_change,
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
        ops: Option<Vec<butler::JsonOp>>,
        delta: Option<LoroDelta>,
        full_data: Option<JsonValue>,
    ) -> Result<(), String> {
        let ops_count = ops.as_ref().map(|o| o.len()).unwrap_or(0);
        trace!(
            page_id = %self.page_id,
            layer = %layer_name,
            has_delta = delta.is_some(),
            has_full_data = full_data.is_some(),
            ops_count = ops_count,
            "handle_loro_change ENTRY"
        );

        // Process through binding system (handles delta surgically when available)
        let bindings_processed = self.process_bindings(layer_name, full_data.as_ref(), delta.as_ref());

        if bindings_processed {
            self.trigger_derivation(layer_name);
            return Ok(());
        }

        // Legacy path: on_loro_change(layer_name, ops) callback
        if self.handler_cache.on_loro_change {
            let func: Function = self.lua.globals().get("on_loro_change").unwrap();

            // Convert ops to Lua table (or nil if None)
            let lua_ops = if let Some(ref op_vec) = ops {
                let ops_json = serde_json::to_value(op_vec)
                    .map_err(|e| format!("Ops to JSON: {}", e))?;
                json_to_lua(&self.lua, &ops_json).map_err(|e| format!("Ops to Lua: {}", e))?
            } else {
                Value::Nil
            };

            if let Err(e) = func.call::<()>((layer_name, lua_ops)) {
                warn!(page_id = %self.page_id, layer = %layer_name, error = %e, "on_loro_change error");
            }
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
            let all_bindings: Vec<_> = manager.all_bindings()
                .map(|b| format!("{}→{}", b.expanded_pattern, b.ui_property))
                .collect();
            info!(
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
                    let layer_for_transform = if binding.is_wildcard { Some(layer_name) } else { None };
                    let ops = crate::bindings::binding::convert_delta_for_binding(
                        &self.lua, binding, delta, layer_for_transform,
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
                if binding.is_wildcard { Some(layer_name) } else { None },
            ) {
                Ok(data) => data,
                Err(e) => {
                    warn!(page_id = %self.page_id, layer = %layer_name, ui_property = %ui_property, error = %e,
                        "Failed to process binding data");
                    continue;
                }
            };

            if let Some(ref ui_tx) = self.ui_tx {
                if let Some(mutation) = crate::bindings::data_to_ui_mutation(&self.page_id, ui_property, processed) {
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

    /// Handle layer discovered
    ///
    /// **Context**: New layer arriving via sync (e.g., derived/orders_summary first appearance)
    /// **We do**: Process bindings for the discovered layer, then call Lua callback
    /// **Why**: Without binding processing here, first-time layer arrivals are invisible to UI
    fn handle_layer_discovered(&self, layer_name: &str) -> Result<(), String> {
        // Process bindings for the discovered layer (fetches full data from Scribe)
        if self.ui_enabled {
            let has_bindings = {
                let manager = self.binding_manager.lock();
                !manager.get_bindings_for_layer(layer_name).is_empty()
            };

            if has_bindings {
                if let Ok(data) = self.fetch_layer_data(layer_name) {
                    self.process_bindings(layer_name, Some(&data), None);
                }
            }
        }

        if self.handler_cache.on_layer_discovered {
            let func: Function = self.lua.globals().get("on_layer_discovered").unwrap();
            if let Err(e) = func.call::<()>(layer_name.to_string()) {
                warn!(page_id = %self.page_id, layer = %layer_name, error = %e, "on_layer_discovered error");
            }
        }

        self.trigger_derivation(layer_name);

        Ok(())
    }

    /// Fetch layer data as JSON from Scribe
    fn fetch_layer_data(&self, layer_name: &str) -> Result<JsonValue, String> {
        match self.scribe.get_layer_json(layer_name)? {
            Some(data) => Ok(data),
            None => Ok(JsonValue::Array(vec![])),
        }
    }

    /// Fire due timers
    fn fire_due_timers(&self) {
        let fired_ids = {
            let mut scheduler = self.scheduler.lock();
            scheduler.fire_due_timers()
        };

        for timer_id in fired_ids {
            self.fire_timer(timer_id);
        }
    }

    /// Fire a single timer callback
    fn fire_timer(&self, timer_id: u64) {
        let timers: Result<Table, _> = self.lua.globals().get("_timers");
        if let Ok(timers) = timers {
            let callback: Result<Function, _> = timers.get(timer_id);
            if let Ok(func) = callback {
                if let Err(e) = func.call::<()>(()) {
                    warn!(timer_id, error = %e, "Timer callback error");
                }
            }

            // Remove one-shot timer callback
            let is_one_shot = {
                let scheduler = self.scheduler.lock();
                !scheduler.timers.contains_key(&timer_id)
            };
            if is_one_shot {
                let _ = timers.set(timer_id, Value::Nil);
            }
        }
    }

    /// Handle ephemeral message
    fn handle_ephemeral(&self, user_did: &str, payload: &[u8]) {
        if self.handler_cache.on_ephemeral {
            let payload_str = String::from_utf8_lossy(payload);
            let func: Function = self.lua.globals().get("on_ephemeral").unwrap();
            if let Err(e) = func.call::<()>((user_did, payload_str.to_string())) {
                warn!(page_id = %self.page_id, user = %user_did, error = %e, "on_ephemeral error");
            }
        }
    }

    /// Handle structured ephemeral message
    fn handle_structured_ephemeral(&self, from_did: &str, func_name: &str, args: &JsonValue) {
        debug!(page_id = %self.page_id, from_did = %from_did, func = %func_name, "handle_structured_ephemeral: received");

        if self.handler_cache.on_ephemeral {
            let lua_args = match json_to_lua(&self.lua, args) {
                Ok(v) => v,
                Err(e) => {
                    warn!(page_id = %self.page_id, error = %e, "Args conversion error");
                    return;
                }
            };

            let func: Function = self.lua.globals().get("on_ephemeral").unwrap();
            if let Err(e) = func.call::<()>((from_did, func_name, lua_args)) {
                warn!(page_id = %self.page_id, from = %from_did, func = %func_name, error = %e, "on_ephemeral error");
            } else {
                debug!(page_id = %self.page_id, from_did = %from_did, func = %func_name, "on_ephemeral called successfully");
            }
        } else {
            debug!(page_id = %self.page_id, "on_ephemeral function not defined");
        }
    }

    /// Handle peer joined
    fn handle_peer_joined(&self, user_did: &str) {
        if self.handler_cache.on_peer_joined {
            let func: Function = self.lua.globals().get("on_peer_joined").unwrap();
            if let Err(e) = func.call::<()>(user_did.to_string()) {
                warn!(page_id = %self.page_id, user = %user_did, error = %e, "on_peer_joined error");
            } else {
                info!(page_id = %self.page_id, user = %user_did, "on_peer_joined executed");
            }
        }
    }

    /// Handle peer left
    fn handle_peer_left(&self, user_did: &str) {
        if self.handler_cache.on_peer_left {
            let func: Function = self.lua.globals().get("on_peer_left").unwrap();
            if let Err(e) = func.call::<()>(user_did.to_string()) {
                warn!(page_id = %self.page_id, user = %user_did, error = %e, "on_peer_left error");
            } else {
                info!(page_id = %self.page_id, user = %user_did, "on_peer_left executed");
            }
        }
    }

    /// Handle asset uploaded
    fn handle_asset_uploaded(&self, hash: &str, filename: &str, mime_type: &str, size: u64) {
        if self.handler_cache.on_asset_uploaded {
            let table = match self.lua.create_table() {
                Ok(t) => t,
                Err(e) => {
                    warn!(page_id = %self.page_id, error = %e, "Failed to create table");
                    return;
                }
            };

            let _ = table.set("hash", hash);
            let _ = table.set("filename", filename);
            let _ = table.set("mime_type", mime_type);
            let _ = table.set("size", size);

            let func: Function = self.lua.globals().get("on_asset_uploaded").unwrap();
            if let Err(e) = func.call::<()>(table) {
                warn!(page_id = %self.page_id, hash = %hash, error = %e, "on_asset_uploaded error");
            } else {
                info!(page_id = %self.page_id, hash = %hash, filename = %filename, "on_asset_uploaded executed");
            }
        }
    }

    /// Handle UI callback
    fn handle_ui_callback(&self, name: &str, args: Vec<JsonValue>) -> Result<(), String> {
        if !self.has_function(name) {
            debug!(page_id = %self.page_id, callback = %name, "No handler found");
            return Ok(());
        }

        let func: Function = self.lua.globals().get(name).unwrap();
        let lua_args: Vec<Value> = args
            .iter()
            .filter_map(|a| json_to_lua(&self.lua, a).ok())
            .collect();

        if let Err(e) = func.call::<()>(mlua::MultiValue::from_iter(lua_args)) {
            warn!(page_id = %self.page_id, callback = %name, error = %e, "UI callback error");
        }

        Ok(())
    }

    /// Handle UI event
    fn handle_ui_event(&self, event: UiEventType) -> Result<(), String> {
        match event {
            UiEventType::KeyPressed { key } => {
                if self.handler_cache.on_key_pressed {
                    let func: Function = self.lua.globals().get("on_key_pressed").unwrap();
                    if let Err(e) = func.call::<()>(key) {
                        warn!(error = %e, "on_key_pressed error");
                    }
                }
            }
            UiEventType::TextChanged { element: _, text } => {
                if self.handler_cache.on_text_input {
                    let func: Function = self.lua.globals().get("on_text_input").unwrap();
                    if let Err(e) = func.call::<()>(text) {
                        warn!(error = %e, "on_text_input error");
                    }
                }
            }
            UiEventType::Custom { name, data } => {
                // Dispatch to event bus
                let event = crate::event_bus::Event::new(&name, crate::event_bus::EventSource::Human)
                    .with_data(data);

                let deliveries = self.ui_shared.lock().event_bus.emit(event);
                for (subscriber_id, _callback_key, delivery) in deliveries {
                    self.dispatch_event_delivery(subscriber_id, delivery);
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Dispatch event delivery to subscriber
    fn dispatch_event_delivery(&self, subscriber_id: u64, delivery: EventDelivery) {
        let shared = self.ui_shared.lock();
        if let Some(registry_key) = shared.callback_keys.get(&subscriber_id) {
            let callback: Result<Function, _> = self.lua.registry_value(registry_key);
            drop(shared);

            if let Ok(callback) = callback {
                match delivery {
                    EventDelivery::Single(evt) => {
                        if let Ok(event_lua) =
                            crate::bindings::event_to_lua(&self.lua, &evt)
                        {
                            let _ = callback.call::<()>(event_lua);
                        }
                    }
                    EventDelivery::Batch(events) => {
                        if let Ok(events_lua) = self.lua.create_table() {
                            for (i, evt) in events.iter().enumerate() {
                                if let Ok(event_lua) =
                                    crate::bindings::event_to_lua(&self.lua, evt)
                                {
                                    let _ = events_lua.set(i + 1, event_lua);
                                }
                            }
                            let _ = callback.call::<()>(events_lua);
                        }
                    }
                }
            }
        }
    }

    /// Flush accumulated UI mutations
    fn flush_mutations(&self) {
        if !self.ui_enabled {
            return;
        }

        let mut shared = self.ui_shared.lock();
        let properties = std::mem::take(&mut shared.pending_properties);
        let model_ops = std::mem::take(&mut shared.pending_model_ops);
        drop(shared);

        if properties.is_empty() && model_ops.is_empty() {
            return;
        }

        let mutation = UiMutation {
            app_id: self.page_id.clone(),
            properties,
            model_ops,
        };

        debug!(
            page_id = %self.page_id,
            prop_count = mutation.properties.len(),
            op_count = mutation.model_ops.len(),
            "Flushing UI mutations"
        );

        if let Some(ref tx) = self.ui_tx {
            if let Err(e) = tx.try_send(mutation) {
                warn!(page_id = %self.page_id, error = %e, "Failed to send mutation");
            }
        }
    }

    /// Collect non-builtin Lua globals
    fn collect_lua_globals(&self) -> Vec<String> {
        let builtins = [
            "_G",
            "_VERSION",
            "_timers",
            "assert",
            "collectgarbage",
            "dofile",
            "error",
            "getmetatable",
            "ipairs",
            "load",
            "loadfile",
            "next",
            "pairs",
            "pcall",
            "print",
            "rawequal",
            "rawget",
            "rawlen",
            "rawset",
            "require",
            "select",
            "setmetatable",
            "tonumber",
            "tostring",
            "type",
            "warn",
            "xpcall",
            "coroutine",
            "debug",
            "io",
            "math",
            "os",
            "package",
            "string",
            "table",
            "utf8",
            "scribe",
            "ui",
            "page",
            "permit",
            "peers",
            "derivation",
            "layout",
            "emoji",
            "datetime",
            "timer",
            "api",
            "presence_lib",
        ];

        let mut globals = Vec::new();
        if let Ok(pairs) = self
            .lua
            .globals()
            .pairs::<String, Value>()
            .collect::<Result<Vec<_>, _>>()
        {
            for (name, _) in pairs {
                if !builtins.contains(&name.as_str()) {
                    globals.push(name);
                }
            }
        }
        globals.sort();
        globals
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

/// Test-only validation runtime that doesn't require a full Scribe actor
///
/// **Purpose**: Enables unit testing of validate_ops without actor setup
#[cfg(test)]
pub struct TestValidationRuntime {
    lua: Lua,
    page_id: String,
}

#[cfg(test)]
impl TestValidationRuntime {
    /// Create a test runtime for validation testing
    pub fn new(page_id: &str) -> Result<Self, String> {
        let lua = Lua::new();
        Ok(Self {
            lua,
            page_id: page_id.to_string(),
        })
    }

    /// Load validation code
    pub fn load_validation_code(&self, code: &str) -> Result<(), String> {
        self.lua.load(code)
            .exec()
            .map_err(|e| format!("Failed to load validation code: {}", e))?;
        Ok(())
    }

    /// Check if a function exists
    pub fn has_function(&self, name: &str) -> bool {
        self.lua.globals()
            .get::<Value>(name)
            .map(|v| matches!(v, Value::Function(_)))
            .unwrap_or(false)
    }

    /// Validate operations
    pub fn validate_ops(
        &self,
        layer_name: &str,
        ops: &[serde_json::Value],
        from_did: &str,
        role: &str,
    ) -> Result<(bool, Option<String>), String> {
        let globals = self.lua.globals();

        let validate_fn: Function = match globals.get("validate_ops") {
            Ok(f) => f,
            Err(_) => {
                return Ok((true, None));
            }
        };

        let ops_table = self.lua.create_table()
            .map_err(|e| format!("Failed to create ops table: {}", e))?;

        for (i, op_json) in ops.iter().enumerate() {
            let op_lua = json_to_lua(&self.lua, op_json)
                .map_err(|e| format!("Failed to convert op to Lua: {}", e))?;
            ops_table.set(i + 1, op_lua)
                .map_err(|e| format!("Failed to add op to table: {}", e))?;
        }

        let result: bool = validate_fn.call((
            layer_name,
            ops_table,
            from_did,
            role,
            self.page_id.as_str(),
        )).map_err(|e| format!("validate_ops error: {}", e))?;

        Ok((result, if result { None } else { Some("Validation failed".to_string()) }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_ops_basic() {
        // No validate_ops function → allows all
        let rt = TestValidationRuntime::new("test-page").unwrap();
        assert!(!rt.has_function("validate_ops"));
        let ops = vec![serde_json::json!({"op": "insert", "path": "messages", "value": {"text": "hello"}})];
        let (passed, _) = rt.validate_ops("messages", &ops, "did:key:user", "viewer").unwrap();
        assert!(passed, "Should allow ops when no validate_ops function");

        // Returns true
        let rt = TestValidationRuntime::new("test-page").unwrap();
        rt.load_validation_code(r#"function validate_ops(l, o, f, r, p) return true end"#).unwrap();
        let (passed, error) = rt.validate_ops("messages", &ops, "did:key:user", "viewer").unwrap();
        assert!(passed);
        assert!(error.is_none());

        // Returns false
        let rt = TestValidationRuntime::new("test-page").unwrap();
        rt.load_validation_code(r#"function validate_ops(l, o, f, r, p) return false end"#).unwrap();
        let (passed, error) = rt.validate_ops("messages", &ops, "did:key:user", "viewer").unwrap();
        assert!(!passed);
        assert!(error.is_some());

        // Empty ops
        let rt = TestValidationRuntime::new("test-page").unwrap();
        rt.load_validation_code(r#"function validate_ops(l, ops, f, r, p) return #ops == 0 end"#).unwrap();
        let (passed, _) = rt.validate_ops("layer", &[], "did:key:user", "viewer").unwrap();
        assert!(passed, "Empty ops should pass");
        let (passed, _) = rt.validate_ops("layer", &ops, "did:key:user", "viewer").unwrap();
        assert!(!passed, "Non-empty ops should fail");
    }

    #[test]
    fn test_validate_ops_args_and_logic() {
        // Receives correct args
        let rt = TestValidationRuntime::new("test-page-123").unwrap();
        rt.load_validation_code(r#"
            function validate_ops(layer_name, ops, from_did, role, page_id)
                if layer_name ~= "orders" then return false end
                if from_did ~= "did:key:alice" then return false end
                if role ~= "owner" then return false end
                if page_id ~= "test-page-123" then return false end
                if #ops ~= 2 then return false end
                return true
            end
        "#).unwrap();
        let ops = vec![
            serde_json::json!({"op": "insert", "value": 1}),
            serde_json::json!({"op": "insert", "value": 2}),
        ];
        let (passed, _) = rt.validate_ops("orders", &ops, "did:key:alice", "owner").unwrap();
        assert!(passed, "All arguments should match");

        // Can access op fields
        let rt = TestValidationRuntime::new("test-page").unwrap();
        rt.load_validation_code(r#"
            function validate_ops(layer_name, ops, from_did, role, page_id)
                for _, op in ipairs(ops) do
                    if op.op == "insert" and op.path == "messages" then
                        if op.value and op.value.text == "forbidden" then return false end
                    end
                end
                return true
            end
        "#).unwrap();
        let allowed = vec![serde_json::json!({"op": "insert", "path": "messages", "value": {"text": "hello"}})];
        let (passed, _) = rt.validate_ops("messages", &allowed, "did:key:user", "viewer").unwrap();
        assert!(passed, "Normal messages allowed");
        let forbidden = vec![serde_json::json!({"op": "insert", "path": "messages", "value": {"text": "forbidden"}})];
        let (passed, _) = rt.validate_ops("messages", &forbidden, "did:key:user", "viewer").unwrap();
        assert!(!passed, "Forbidden messages rejected");

        // Role-based access
        let rt = TestValidationRuntime::new("test-page").unwrap();
        rt.load_validation_code(r#"
            function validate_ops(layer_name, ops, from_did, role, page_id)
                if layer_name == "orders" and role ~= "owner" then return false end
                return true
            end
        "#).unwrap();
        let ops = vec![serde_json::json!({"op": "insert"})];
        let (passed, _) = rt.validate_ops("orders", &ops, "did:key:owner", "owner").unwrap();
        assert!(passed, "Owner can modify orders");
        let (passed, _) = rt.validate_ops("orders", &ops, "did:key:viewer", "viewer").unwrap();
        assert!(!passed, "Viewer cannot modify orders");
    }

    #[test]
    fn test_validate_ops_error_handling() {
        // Lua error propagation
        let rt = TestValidationRuntime::new("test-page").unwrap();
        rt.load_validation_code(r#"
            function validate_ops(l, o, f, r, p)
                error("Something went wrong!")
            end
        "#).unwrap();
        let ops = vec![serde_json::json!({"op": "insert"})];
        let result = rt.validate_ops("layer", &ops, "did:key:user", "viewer");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Something went wrong"));

        // Invalid Lua code
        let rt = TestValidationRuntime::new("test-page").unwrap();
        let result = rt.load_validation_code("function validate_ops( -- missing closing paren");
        assert!(result.is_err(), "Invalid Lua should fail to load");
    }
}
