//! Headless Lua runtime for node and AI
//!
//! **Context**: Same Lua as apps, no UI bindings
//! **All access via Scribe actor (read + write)**
//!
//! Used by:
//! - kunki (node server) for server-side validation and logic
//! - AI runtime for LLM-driven operations

pub mod bindings;

use mlua::{Lua, Function, ObjectLike, Value, UserData, UserDataMethods};
use ractor::ActorRef;
use std::collections::HashMap;
use std::sync::Mutex;
use tokio::sync::mpsc;
use tracing::{debug, info};

use crate::scribe::{ScribeMessage, PageEvent, EphemeralEvent};

/// Stub UI bindings for headless runtime
///
/// Provides minimal ui:get/set that stores values in memory (for testing).
/// This allows apps that use ui:get/set to work in headless mode.
struct StubUiBindings {
    /// In-memory storage for ui values
    values: Mutex<HashMap<String, serde_json::Value>>,
}

impl StubUiBindings {
    fn new() -> Self {
        Self {
            values: Mutex::new(HashMap::new()),
        }
    }
}

impl UserData for StubUiBindings {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        // ui:get(key) -> value
        methods.add_method("get", |lua, this, key: String| {
            let values = this.values.lock().unwrap();
            match values.get(&key) {
                Some(v) => bindings::json_to_lua(lua, v),
                None => Ok(Value::Nil),
            }
        });

        // ui:set(key, value) - store in memory
        methods.add_method("set", |_lua, this, (key, value): (String, Value)| {
            let json_value = bindings::lua_to_json(&value)
                .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
            this.values.lock().unwrap().insert(key, json_value);
            Ok(())
        });

        // ui:push, ui:insert, ui:remove, ui:clear, ui:update - no-ops for headless
        methods.add_method("push", |_lua, _this, (_model, _item): (String, Value)| Ok(()));
        methods.add_method("insert", |_lua, _this, (_model, _idx, _item): (String, usize, Value)| Ok(()));
        methods.add_method("remove", |_lua, _this, (_model, _idx): (String, usize)| Ok(()));
        methods.add_method("clear", |_lua, _this, _model: String| Ok(()));
        methods.add_method("update", |_lua, _this, (_model, _idx, _item): (String, usize, Value)| Ok(()));
    }
}

/// Butler bindings for headless runtime
///
/// Provides butler:send_ephemeral() for broadcasting game state to peers
struct HeadlessButlerBindings {
    scribe_ref: ActorRef<ScribeMessage>,
}

impl HeadlessButlerBindings {
    fn new(scribe_ref: ActorRef<ScribeMessage>) -> Self {
        Self { scribe_ref }
    }
}

impl UserData for HeadlessButlerBindings {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        // butler:send_ephemeral(payload) - broadcast ephemeral to all peers
        methods.add_method("send_ephemeral", |_, this, payload: String| {
            debug!(payload_len = payload.len(), "butler:send_ephemeral called from headless Lua");
            this.scribe_ref.cast(ScribeMessage::SendEphemeral {
                payload: payload.into_bytes(),
            })
            .map_err(|e| mlua::Error::RuntimeError(format!("Failed to send ephemeral: {}", e)))?;
            Ok(())
        });
    }
}

pub use bindings::{
    LoroBindings, PermitBindings, DerivationBindings, PeersBindings, LuaLoroList, LuaLoroMap,
    // Loro <-> Lua
    loro_value_to_lua, lua_to_loro_value,
    // JSON <-> Lua
    json_to_lua, lua_to_json,
    // JSON <-> Loro
    json_to_loro_value, loro_value_to_json,
    // Pattern matching
    matches_layer_pattern,
};

/// Headless Lua runtime for node and AI
///
/// **Same Lua as apps, no UI bindings**
/// **All access via Scribe actor (read + write)**
/// **Supports tick loop and ephemeral events for game logic**
pub struct HeadlessRuntime {
    lua: Lua,
    page_id: String,
    scribe_ref: ActorRef<ScribeMessage>,
    /// Receiver for page events from Scribe (unified event channel)
    page_event_rx: mpsc::Receiver<PageEvent>,
    /// Receiver for ephemeral events from Scribe (player positions, etc.)
    ephemeral_rx: mpsc::Receiver<EphemeralEvent>,
    /// Whether tick is enabled for this runtime
    tick_enabled: bool,
}

impl HeadlessRuntime {
    /// Create runtime for a page
    ///
    /// **Subscribes to Scribe for Loro change events**
    pub async fn new(
        page_id: &str,
        scribe_ref: ActorRef<ScribeMessage>,
        our_did: &str,
        our_name: &str,
        our_role: &str,
    ) -> Result<Self, String> {
        let lua = Lua::new();

        // Setup loro bindings (access to Scribe's layers)
        let loro = LoroBindings::new(scribe_ref.clone(), page_id.to_string());
        lua.globals().set("loro", loro)
            .map_err(|e| format!("Failed to set loro global: {}", e))?;

        // Setup permit bindings
        let permit = PermitBindings::new(
            page_id.to_string(),
            our_did.to_string(),
            our_name.to_string(),
            our_role.to_string(),
        );
        lua.globals().set("permit", permit)
            .map_err(|e| format!("Failed to set permit global: {}", e))?;

        // Setup derivation bindings (node-only operations)
        let derivation = DerivationBindings::new(scribe_ref.clone());
        lua.globals().set("derivation", derivation)
            .map_err(|e| format!("Failed to set derivation global: {}", e))?;

        // Setup peers bindings (for checking subscriber count before broadcast)
        let peers = PeersBindings::new(scribe_ref.clone());
        lua.globals().set("peers", peers)
            .map_err(|e| format!("Failed to set peers global: {}", e))?;

        // Setup stub UI bindings (for headless testing)
        let ui = StubUiBindings::new();
        lua.globals().set("ui", ui)
            .map_err(|e| format!("Failed to set ui global: {}", e))?;

        // Setup butler bindings (for sending ephemeral)
        let butler = HeadlessButlerBindings::new(scribe_ref.clone());
        lua.globals().set("butler", butler)
            .map_err(|e| format!("Failed to set butler global: {}", e))?;

        // Load api module for function export/description
        let api_lib = include_str!("lua_libs/api.lua");
        let api_module: Value = lua.load(api_lib)
            .eval()
            .map_err(|e| format!("Failed to load api module: {}", e))?;
        lua.globals().set("api", api_module)
            .map_err(|e| format!("Failed to set api global: {}", e))?;

        // Subscribe to page events from Scribe (unified event channel)
        let (page_event_tx, page_event_rx) = mpsc::channel(32);
        scribe_ref.cast(ScribeMessage::SubscribeToPageEvents {
            event_tx: page_event_tx,
        }).map_err(|e| format!("Failed to subscribe to page events: {}", e))?;

        // Subscribe to ephemeral events from Scribe (player positions, etc.)
        let (ephemeral_tx, ephemeral_rx) = mpsc::channel(256);
        scribe_ref.cast(ScribeMessage::SubscribeEphemeral {
            tx: ephemeral_tx,
        }).map_err(|e| format!("Failed to subscribe to ephemeral events: {}", e))?;

        info!(page_id = %page_id, "HeadlessRuntime created with ephemeral support");

        Ok(Self {
            lua,
            page_id: page_id.to_string(),
            scribe_ref,
            page_event_rx,
            ephemeral_rx,
            tick_enabled: false,
        })
    }

    /// Load app code into the runtime
    ///
    /// **Context**: Load Lua code from app layer
    pub fn load_code(&self, code: &str) -> Result<(), String> {
        self.lua.load(code)
            .exec()
            .map_err(|e| format!("Failed to load Lua code: {}", e))?;

        debug!(page_id = %self.page_id, "Loaded Lua code");
        Ok(())
    }

    /// Process incoming page events
    ///
    /// **Context**: Call from event loop to handle page events (layer updates)
    /// **Calls**: Lua's `on_loro_change(layer_name, data)` if defined
    pub async fn process_page_events(&mut self) -> Result<(), String> {
        while let Ok(event) = self.page_event_rx.try_recv() {
            self.handle_page_event(event)?;
        }
        Ok(())
    }

    /// Handle a single page event
    fn handle_page_event(&self, event: PageEvent) -> Result<(), String> {
        // Call Lua's on_loro_change if defined (same callback name for compatibility)
        let globals = self.lua.globals();
        if let Ok(func) = globals.get::<Function>("on_loro_change") {
            let layer_name = event.layer_name.clone();
            let data = self.json_to_lua(&event.full_data)?;

            func.call::<()>((layer_name, data))
                .map_err(|e| format!("on_loro_change error: {}", e))?;

            debug!(layer = %event.layer_name, "Called on_loro_change");
        }
        Ok(())
    }

    /// Call an app-defined function with no arguments
    pub fn call_no_args<T: mlua::FromLuaMulti>(&self, func_name: &str) -> Result<T, String> {
        let globals = self.lua.globals();
        let func: Function = globals.get(func_name)
            .map_err(|e| format!("Function '{}' not found: {}", func_name, e))?;

        func.call(())
            .map_err(|e| format!("Error calling '{}': {}", func_name, e))
    }

    /// Call an app-defined function with arguments
    pub fn call_with_args<A: mlua::IntoLuaMulti, T: mlua::FromLuaMulti>(&self, func_name: &str, args: A) -> Result<T, String> {
        let globals = self.lua.globals();
        let func: Function = globals.get(func_name)
            .map_err(|e| format!("Function '{}' not found: {}", func_name, e))?;

        func.call(args)
            .map_err(|e| format!("Error calling '{}': {}", func_name, e))
    }

    /// Check if a function exists
    pub fn has_function(&self, func_name: &str) -> bool {
        self.lua.globals().get::<Value>(func_name)
            .map(|v| matches!(v, Value::Function(_)))
            .unwrap_or(false)
    }

    /// Get the page ID
    pub fn page_id(&self) -> &str {
        &self.page_id
    }

    /// Get the Scribe actor reference
    pub fn scribe_ref(&self) -> &ActorRef<ScribeMessage> {
        &self.scribe_ref
    }

    /// Get the Lua instance for advanced usage
    pub fn lua(&self) -> &Lua {
        &self.lua
    }

    /// Initialize derivation engine after loading app code
    ///
    /// **Context**: Called after load_code() to verify and rebuild derived layers
    /// **Flow**:
    /// 1. Get list of derived layer targets from registered rules
    /// 2. For each target, rebuild from source layers
    ///
    /// **Note**: Calls derivation:rebuild_all() in our own Lua instance
    /// (rules are registered in DerivationBindings, not Scribe's ScribeLuaRuntime)
    pub fn initialize_derivation(&self) -> Result<(), String> {
        // Get the derivation userdata from Lua globals
        let derivation: mlua::AnyUserData = self.lua.globals().get("derivation")
            .map_err(|e| format!("Failed to get derivation global: {}", e))?;

        // Call rebuild_all() method on the userdata
        derivation.call_method::<()>("rebuild_all", ())
            .map_err(|e| format!("Derivation rebuild_all failed: {}", e))?;

        info!(page_id = %self.page_id, "Derivation initialization complete");
        Ok(())
    }

    // ==================== Tick & Ephemeral Support ====================

    /// Enable tick for this runtime
    ///
    /// **Context**: Call before run_tick_loop() to enable periodic tick() calls
    pub fn enable_tick(&mut self) {
        self.tick_enabled = true;
        info!(page_id = %self.page_id, "Tick enabled");
    }

    /// Check if tick is enabled
    pub fn is_tick_enabled(&self) -> bool {
        self.tick_enabled
    }

    /// Process incoming ephemeral events
    ///
    /// **Context**: Call from event loop to handle ephemeral events (player positions, etc.)
    /// **Calls**: Lua's `on_ephemeral(user_did, payload)` if defined
    pub async fn process_ephemeral_events(&mut self) -> Result<(), String> {
        while let Ok(event) = self.ephemeral_rx.try_recv() {
            self.handle_ephemeral_event(event)?;
        }
        Ok(())
    }

    /// Handle a single ephemeral event
    fn handle_ephemeral_event(&self, event: EphemeralEvent) -> Result<(), String> {
        match event {
            EphemeralEvent::Data { user_did, payload } => {
                // Call Lua's on_ephemeral if defined
                let globals = self.lua.globals();
                if let Ok(func) = globals.get::<Function>("on_ephemeral") {
                    let payload_str = String::from_utf8_lossy(&payload).to_string();
                    func.call::<()>((user_did.clone(), payload_str))
                        .map_err(|e| format!("on_ephemeral error: {}", e))?;
                    debug!(page_id = %self.page_id, user_did = %user_did, "Called on_ephemeral");
                }
            }
            EphemeralEvent::PeerJoined { user_did } => {
                // Call Lua's on_peer_joined if defined
                let globals = self.lua.globals();
                if let Ok(func) = globals.get::<Function>("on_peer_joined") {
                    func.call::<()>(user_did.clone())
                        .map_err(|e| format!("on_peer_joined error: {}", e))?;
                    info!(page_id = %self.page_id, user_did = %user_did, "Called on_peer_joined");
                }
            }
            EphemeralEvent::PeerLeft { user_did } => {
                // Call Lua's on_peer_left if defined
                let globals = self.lua.globals();
                if let Ok(func) = globals.get::<Function>("on_peer_left") {
                    func.call::<()>(user_did.clone())
                        .map_err(|e| format!("on_peer_left error: {}", e))?;
                    info!(page_id = %self.page_id, user_did = %user_did, "Called on_peer_left");
                }
            }
        }
        Ok(())
    }

    /// Call Lua's tick() function once
    ///
    /// **Context**: Called periodically by run_tick_loop()
    /// **Returns**: Ok(true) if tick was called, Ok(false) if tick() not defined
    pub fn tick(&self) -> Result<bool, String> {
        let globals = self.lua.globals();
        if let Ok(func) = globals.get::<Function>("tick") {
            func.call::<()>(())
                .map_err(|e| format!("tick() error: {}", e))?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Run the tick loop
    ///
    /// **Context**: Main game loop for node-side game logic
    /// **Flow**:
    /// 1. Process page events (CRDT changes)
    /// 2. Process ephemeral events (player positions)
    /// 3. Call tick() for game logic (enemy AI, etc.)
    /// 4. Sleep for frame duration
    ///
    /// **Parameters**:
    /// - `tick_rate_hz`: Ticks per second (e.g., 30 for 30fps)
    /// - `shutdown_rx`: Channel to receive shutdown signal
    pub async fn run_tick_loop(
        &mut self,
        tick_rate_hz: u32,
        mut shutdown_rx: mpsc::Receiver<()>,
    ) -> Result<(), String> {
        if !self.tick_enabled {
            return Err("Tick not enabled. Call enable_tick() first.".to_string());
        }

        let frame_duration = std::time::Duration::from_micros(1_000_000 / tick_rate_hz as u64);
        info!(page_id = %self.page_id, tick_rate_hz = tick_rate_hz, "Starting tick loop");

        // Call on_init if defined
        if self.has_function("on_init") {
            self.call_no_args::<()>("on_init")?;
            info!(page_id = %self.page_id, "Called on_init");
        }

        loop {
            let frame_start = std::time::Instant::now();

            // Check for shutdown
            if shutdown_rx.try_recv().is_ok() {
                info!(page_id = %self.page_id, "Tick loop shutdown requested");
                break;
            }

            // Process page events (CRDT changes)
            self.process_page_events().await?;

            // Process ephemeral events (player positions, etc.)
            self.process_ephemeral_events().await?;

            // Call tick() for game logic
            self.tick()?;

            // Sleep for remaining frame time
            let elapsed = frame_start.elapsed();
            if elapsed < frame_duration {
                tokio::time::sleep(frame_duration - elapsed).await;
            }
        }

        Ok(())
    }

    /// Convert serde_json::Value to Lua Value
    fn json_to_lua(&self, json: &serde_json::Value) -> Result<Value, String> {
        match json {
            serde_json::Value::Null => Ok(Value::Nil),
            serde_json::Value::Bool(b) => Ok(Value::Boolean(*b)),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Ok(Value::Integer(i))
                } else if let Some(f) = n.as_f64() {
                    Ok(Value::Number(f))
                } else {
                    Ok(Value::Nil)
                }
            }
            serde_json::Value::String(s) => {
                let lua_str = self.lua.create_string(s)
                    .map_err(|e| format!("Failed to create Lua string: {}", e))?;
                Ok(Value::String(lua_str))
            }
            serde_json::Value::Array(arr) => {
                let table = self.lua.create_table()
                    .map_err(|e| format!("Failed to create Lua table: {}", e))?;
                for (i, v) in arr.iter().enumerate() {
                    let lua_v = self.json_to_lua(v)?;
                    table.set(i + 1, lua_v)
                        .map_err(|e| format!("Failed to set array element: {}", e))?;
                }
                Ok(Value::Table(table))
            }
            serde_json::Value::Object(obj) => {
                let table = self.lua.create_table()
                    .map_err(|e| format!("Failed to create Lua table: {}", e))?;
                for (k, v) in obj {
                    let lua_v = self.json_to_lua(v)?;
                    table.set(k.clone(), lua_v)
                        .map_err(|e| format!("Failed to set object field: {}", e))?;
                }
                Ok(Value::Table(table))
            }
        }
    }
}
