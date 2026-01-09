//! Lua Worker Thread - Stateless computation engine
//!
//! OS thread per app, owns Lua VM, computes UI operations.
//!
//! **Threading**: Uses `std::thread::spawn` (mlua::Lua is NOT Send for tokio)
//! **Pattern**: Query Loro → Compute → Return operations (stateless)
//! **Communication**: mpsc channels with serializable messages

use mlua::{Lua, Value as LuaValue, Error as LuaError, Table, UserData, UserDataMethods};
use tokio::sync::mpsc;
use ractor::ActorRef;
use butler::{ScribeMessage, LoroDelta, ListOp};
use serde_json::Value as JsonValue;
use std::sync::Arc;
use parking_lot::Mutex;
use crate::vecmodel_ops::{UiMutation, VecModelOp, PropertyUpdate};
use crate::loro_bindings::LoroBindings;
use crate::butler_bindings::ButlerBindings;
use crate::permit_bindings::PermitBindings;

/// Commands sent to Lua worker thread
///
/// **Sender**: Main thread (Slint) or Scribe observer
/// **Receiver**: LuaWorker event loop
#[derive(Debug)]
pub enum LuaWorkerCommand {
    /// Loro layer changed (local write OR peer sync)
    ///
    /// **Delta**: Incremental change (Retain/Insert/Delete operations)
    /// **Full data**: Complete layer state (fallback when delta unavailable)
    LoroChanged {
        layer_name: String,
        /// Delta for incremental UI updates (None = use full_data)
        delta: Option<LoroDelta>,
        full_data: JsonValue,
    },

    /// New layer discovered (from peer sync)
    ///
    /// **Context**: Peer created a new layer that was synced to us
    /// **We do**: Call Lua's `on_layer_discovered(layer_name)` if defined
    LayerDiscovered {
        layer_name: String,
    },

    /// UI callback (button click, input change, etc.)
    UiCallback {
        callback_name: String,
        args: Vec<JsonValue>,
    },

    /// Shutdown worker thread
    Shutdown,
}

/// UI Bindings for Lua - allows setting properties and model operations
///
/// **Usage in Lua**: `ui:set("property_name", value)`
/// **Threading**: Holds sender, sends mutations to Slint thread
struct UiBindings {
    /// App identifier for mutations
    app_id: String,
    /// Channel sender for UI mutations (wrapped for thread safety)
    ui_tx: Arc<Mutex<mpsc::Sender<UiMutation>>>,
}

impl UserData for UiBindings {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        // ui:set(key, value) - Set a property or replace model data
        // Note: Slint uses kebab-case for property names
        methods.add_method("set", |lua, this, (key, value): (String, LuaValue)| {
            // Use property name as-is (Slint expects kebab-case)
            let prop_name = key;

            let json_value = lua_to_json(lua, &value)
                .map_err(|e| LuaError::RuntimeError(e.to_string()))?;

            // Check if value is an array (for VecModel operations)
            let mutation = if let JsonValue::Array(items) = json_value {
                // Array value: Clear model, then push all items
                let mut model_ops = vec![VecModelOp::Clear { model_name: prop_name.clone() }];
                for item in items {
                    model_ops.push(VecModelOp::Push {
                        model_name: prop_name.clone(),
                        item,
                    });
                }
                UiMutation {
                    app_id: this.app_id.clone(),
                    properties: vec![],
                    model_ops,
                }
            } else {
                // Scalar value: PropertyUpdate
                UiMutation {
                    app_id: this.app_id.clone(),
                    properties: vec![PropertyUpdate { key: prop_name, value: json_value }],
                    model_ops: vec![],
                }
            };

            // Send mutation to Slint thread
            let sender = this.ui_tx.lock();
            if let Err(e) = sender.try_send(mutation) {
                tracing::warn!(
                    app_id = %this.app_id,
                    error = %e,
                    "Failed to send UI mutation from Lua"
                );
            }

            Ok(())
        });
    }
}

/// Lua worker thread state
///
/// **Ownership**: Runs on OS thread, owns Lua VM (NOT Rc)
/// **Lifetime**: Lives until app window closes or Shutdown command
pub struct LuaWorker {
    /// Owned Lua VM (mlua::Lua is NOT Send, stays on this thread)
    lua: Lua,

    /// Page identifier (e.g., "my-shop")
    page_id: String,

    /// App name (e.g., "Shop Customer") - used for role detection
    app_name: String,

    /// Channel to send UI mutations to Slint thread (wrapped for sharing with UiBindings)
    ui_tx: Arc<Mutex<mpsc::Sender<UiMutation>>>,

    /// Reference to Scribe actor (for Loro queries)
    scribe_ref: ActorRef<ScribeMessage>,

    /// Channel to receive commands from Slint thread
    cmd_rx: mpsc::Receiver<LuaWorkerCommand>,
}

impl LuaWorker {
    /// Spawn Lua worker thread
    ///
    /// **Parameters**:
    /// - `page_id`: Page identifier (e.g., "my-shop")
    /// - `app_name`: App name (e.g., "Shop Customer") - used for role detection
    /// - `lua_code`: Lua source code to execute
    /// - `scribe_ref`: Reference to Scribe actor
    /// - `ui_tx`: Channel to send UI mutations
    ///
    /// **Returns**: (thread handle, command sender)
    /// **Thread**: Blocks on event loop until Shutdown
    pub fn spawn(
        page_id: String,
        app_name: String,
        lua_code: String,
        scribe_ref: ActorRef<ScribeMessage>,
        ui_tx: mpsc::Sender<UiMutation>,
    ) -> Result<(std::thread::JoinHandle<()>, mpsc::Sender<LuaWorkerCommand>), Box<dyn std::error::Error>> {
        let (cmd_tx, cmd_rx) = mpsc::channel(32);

        let handle = std::thread::spawn(move || {
            // Create Lua VM on this thread (owned, not shared)
            let lua = Lua::new();

            // Load app code
            if let Err(e) = lua.load(&lua_code).exec() {
                tracing::error!(
                    page_id = %page_id,
                    app_name = %app_name,
                    error = %e,
                    "Failed to load Lua code"
                );
                return;
            }

            // Wrap ui_tx in Arc<Mutex> for sharing with UiBindings
            let ui_tx = Arc::new(Mutex::new(ui_tx));

            let mut worker = LuaWorker {
                lua,
                page_id,
                app_name,
                ui_tx,
                scribe_ref,
                cmd_rx,
            };

            // Setup Lua globals (loro, butler, ui)
            if let Err(e) = worker.setup_lua_globals() {
                tracing::error!(
                    error = %e,
                    "Failed to setup Lua globals"
                );
                return;
            }

            // Call on_init() if defined in Lua
            if let Ok(on_init) = worker.lua.globals().get::<mlua::Function>("on_init") {
                if let Err(e) = on_init.call::<()>(()) {
                    tracing::warn!(
                        page_id = %worker.page_id,
                        error = %e,
                        "on_init() failed"
                    );
                } else {
                    tracing::debug!(
                        page_id = %worker.page_id,
                        "on_init() completed"
                    );
                }
            }

            // Run event loop (blocks until Shutdown)
            worker.run();
        });

        Ok((handle, cmd_tx))
    }

    /// Event loop - blocks on cmd_rx until Shutdown
    ///
    /// **Pattern**: Standard mpsc receiver loop
    fn run(&mut self) {
        tracing::info!(
            page_id = %self.page_id,
            "Lua worker thread started"
        );

        loop {
            match self.cmd_rx.blocking_recv() {
                Some(LuaWorkerCommand::LoroChanged { layer_name, delta, full_data }) => {
                    if let Err(e) = self.handle_loro_change(layer_name, delta, full_data) {
                        tracing::error!(
                            page_id = %self.page_id,
                            error = %e,
                            "Loro change handler failed"
                        );
                    }
                }
                Some(LuaWorkerCommand::LayerDiscovered { layer_name }) => {
                    if let Err(e) = self.handle_layer_discovered(&layer_name) {
                        tracing::error!(
                            page_id = %self.page_id,
                            layer = %layer_name,
                            error = %e,
                            "Layer discovered handler failed"
                        );
                    }
                }
                Some(LuaWorkerCommand::UiCallback { callback_name, args }) => {
                    if let Err(e) = self.handle_ui_callback(callback_name, args) {
                        tracing::error!(
                            page_id = %self.page_id,
                            error = %e,
                            "UI callback handler failed"
                        );
                    }
                }
                Some(LuaWorkerCommand::Shutdown) => {
                    tracing::info!(
                        page_id = %self.page_id,
                        "Lua worker shutting down"
                    );
                    break;
                }
                None => {
                    tracing::warn!(
                        page_id = %self.page_id,
                        "Command channel closed, shutting down"
                    );
                    break;
                }
            }
        }
    }

    /// Setup Lua global bindings (loro, butler, ui)
    ///
    /// **Bindings**:
    /// - `loro`: LoroBindings (get_list, get_map)
    /// - `butler`: ButlerBindings (get_layer, get_list)
    /// - `ui`: UI helpers (set_property placeholder)
    fn setup_lua_globals(&self) -> Result<(), LuaError> {
        let globals = self.lua.globals();

        // loro binding
        let loro_binding = LoroBindings::new(
            self.scribe_ref.clone(),
            self.page_id.clone(),
        );
        globals.set("loro", loro_binding)?;

        // butler binding
        let butler_binding = ButlerBindings::new(
            self.scribe_ref.clone(),
            self.page_id.clone(),
        );
        globals.set("butler", butler_binding)?;

        // ui binding - allows Lua to send UI mutations
        let ui_binding = UiBindings {
            app_id: self.page_id.clone(),
            ui_tx: self.ui_tx.clone(),
        };
        globals.set("ui", ui_binding)?;

        // permit binding - provides identity and layer naming helpers
        // page_id = page identifier (e.g., "my-shop")
        // app_name = app name (e.g., "Shop Customer") - used for role detection
        let permit_binding = PermitBindings::new(&self.page_id, &self.app_name);
        globals.set("permit", permit_binding)?;

        tracing::debug!(
            page_id = %self.page_id,
            app_name = %self.app_name,
            "Lua globals initialized (loro, butler, ui, permit)"
        );

        Ok(())
    }

    /// Handle Loro change: convert delta to VecModelOps OR fallback to Lua
    ///
    /// **Fast path**: Delta exists → convert directly to VecModelOps (skip Lua)
    /// **Slow path**: No delta → call Lua with full_data
    fn handle_loro_change(
        &self,
        layer_name: String,
        delta: Option<LoroDelta>,
        full_data: JsonValue,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Fast path: Convert delta directly to VecModelOps (skip Lua)
        if let Some(ref delta) = delta {
            let ops = convert_delta_to_vecmodel_ops(&layer_name, delta);
            if !ops.is_empty() {
                let mutation = UiMutation {
                    app_id: self.page_id.clone(),
                    properties: vec![],
                    model_ops: ops,
                };

                if let Err(e) = self.ui_tx.lock().try_send(mutation) {
                    tracing::warn!(
                        page_id = %self.page_id,
                        layer_name = %layer_name,
                        error = %e,
                        "Failed to send delta-based UI mutation"
                    );
                } else {
                    tracing::debug!(
                        page_id = %self.page_id,
                        layer_name = %layer_name,
                        "Delta-based UI mutation sent (incremental update)"
                    );
                }
                return Ok(());
            }
        }

        // Slow path: Call Lua with full_data (no delta or empty delta)
        let on_loro_change: Result<mlua::Function, _> = self.lua.globals().get("on_loro_change");

        let mutation = match on_loro_change {
            Ok(func) => {
                // Lua has a handler - call it
                let full_data_lua = json_to_lua(&self.lua, &full_data)?;
                let result: LuaValue = func.call((
                    layer_name.clone(),
                    LuaValue::Nil,  // delta not passed to Lua (already handled above)
                    full_data_lua,
                ))?;

                match result {
                    LuaValue::Table(ops_table) => self.parse_operations(ops_table)?,
                    LuaValue::Nil => {
                        // Lua handler returned nil - it handled things internally (via ui:set)
                        // Don't create any mutations, Lua took care of it
                        tracing::debug!(
                            page_id = %self.page_id,
                            layer_name = %layer_name,
                            "on_loro_change returned nil, Lua handled UI updates"
                        );
                        UiMutation {
                            app_id: self.page_id.clone(),
                            properties: vec![],
                            model_ops: vec![],
                        }
                    }
                    _ => {
                        tracing::warn!(
                            page_id = %self.page_id,
                            layer_name = %layer_name,
                            "on_loro_change returned unexpected type, ignoring"
                        );
                        UiMutation {
                            app_id: self.page_id.clone(),
                            properties: vec![],
                            model_ops: vec![],
                        }
                    }
                }
            }
            Err(_) => {
                // No on_loro_change function defined in Lua
                // Only auto-sync if layer_name matches a known model property (no wildcards)
                if layer_name.contains('*') || layer_name.contains(':') {
                    // Skip pattern layers and personal layers - Lua must handle these explicitly
                    tracing::debug!(
                        page_id = %self.page_id,
                        layer_name = %layer_name,
                        "Skipping auto-sync for pattern/personal layer (no on_loro_change handler)"
                    );
                    UiMutation {
                        app_id: self.page_id.clone(),
                        properties: vec![],
                        model_ops: vec![],
                    }
                } else {
                    // Simple layer name - try direct sync to UI property
                    self.full_data_to_mutation(&layer_name, &full_data)
                }
            }
        };

        // Send to Slint thread (non-blocking send)
        if !mutation.properties.is_empty() || !mutation.model_ops.is_empty() {
            if let Err(e) = self.ui_tx.lock().try_send(mutation) {
                tracing::warn!(
                    page_id = %self.page_id,
                    layer_name = %layer_name,
                    error = %e,
                    "Failed to send UI mutation (channel full or closed)"
                );
            } else {
                tracing::debug!(
                    page_id = %self.page_id,
                    layer_name = %layer_name,
                    "UI mutation sent to Slint thread"
                );
            }
        }

        Ok(())
    }

    /// Convert full_data to UiMutation (Clear + Push all)
    ///
    /// **Context**: Fallback when no delta or no Lua handler
    fn full_data_to_mutation(&self, layer_name: &str, full_data: &JsonValue) -> UiMutation {
        if let JsonValue::Array(items) = full_data {
            let mut model_ops = vec![VecModelOp::Clear { model_name: layer_name.to_string() }];
            for item in items {
                model_ops.push(VecModelOp::Push {
                    model_name: layer_name.to_string(),
                    item: item.clone(),
                });
            }
            UiMutation {
                app_id: self.page_id.clone(),
                properties: vec![],
                model_ops,
            }
        } else {
            // Scalar value - treat as property update
            UiMutation {
                app_id: self.page_id.clone(),
                properties: vec![PropertyUpdate {
                    key: layer_name.to_string(),
                    value: full_data.clone(),
                }],
                model_ops: vec![],
            }
        }
    }

    /// Handle layer discovered event from peer sync
    ///
    /// **Context**: A new layer was synced from a peer
    /// **Calls**: `on_layer_discovered(layer_name)` in Lua if defined
    fn handle_layer_discovered(
        &self,
        layer_name: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Get on_layer_discovered function from Lua (optional)
        let func: mlua::Function = match self.lua.globals().get("on_layer_discovered") {
            Ok(f) => f,
            Err(_) => {
                // No handler defined - that's OK for apps that don't need layer discovery
                tracing::debug!(
                    page_id = %self.page_id,
                    layer = %layer_name,
                    "No on_layer_discovered handler in Lua (skipping)"
                );
                return Ok(());
            }
        };

        tracing::info!(
            page_id = %self.page_id,
            layer = %layer_name,
            "Calling Lua on_layer_discovered"
        );

        // Call Lua function with layer name
        match func.call::<()>(layer_name.to_string()) {
            Ok(()) => {
                tracing::debug!(
                    page_id = %self.page_id,
                    layer = %layer_name,
                    "on_layer_discovered handler completed"
                );
            }
            Err(e) => {
                tracing::error!(
                    page_id = %self.page_id,
                    layer = %layer_name,
                    error = %e,
                    "on_layer_discovered handler failed"
                );
                return Err(Box::new(e));
            }
        }

        Ok(())
    }

    /// Handle UI callback: call Lua, parse operations, send to UI
    ///
    /// **Calls**: `callback_name(args...)` in Lua (uses snake_case throughout)
    /// **Example**: Slint `create_order` → Lua `create_order`
    fn handle_ui_callback(
        &self,
        callback_name: String,
        args: Vec<JsonValue>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // No conversion needed - using snake_case consistently in Slint and Lua
        tracing::info!(
            page_id = %self.page_id,
            callback = %callback_name,
            "Looking up Lua callback"
        );

        // Get callback function from Lua (optional - not all callbacks need Lua handlers)
        let callback: mlua::Function = match self.lua.globals().get(callback_name.as_str()) {
            Ok(func) => {
                tracing::info!(
                    page_id = %self.page_id,
                    callback = %callback_name,
                    "Found Lua handler, calling..."
                );
                func
            }
            Err(_) => {
                // No Lua handler for this callback - that's OK, just skip
                tracing::warn!(
                    page_id = %self.page_id,
                    callback = %callback_name,
                    "No Lua handler for callback (skipping)"
                );
                return Ok(());
            }
        };

        // Convert args to Lua values
        let lua_args: Result<Vec<LuaValue>, _> = args.iter()
            .map(|arg| json_to_lua(&self.lua, arg))
            .collect();
        let lua_args = lua_args?;

        // Call Lua function (may return nil or operations table)
        let result: LuaValue = callback.call(mlua::MultiValue::from_vec(lua_args))?;

        // Parse operations table → UiMutation (if returned)
        let mutation = match result {
            LuaValue::Table(ops_table) => self.parse_operations(ops_table)?,
            LuaValue::Nil => {
                // No operations returned - create empty mutation
                tracing::debug!(
                    page_id = %self.page_id,
                    callback = %callback_name,
                    "Callback returned nil (no UI mutations)"
                );
                UiMutation {
                    app_id: self.page_id.clone(),
                    properties: vec![],
                    model_ops: vec![],
                }
            }
            other => {
                return Err(format!(
                    "Callback '{}' returned {:?}, expected table or nil",
                    callback_name, other
                ).into());
            }
        };

        // Only send if there are actual mutations
        if !mutation.properties.is_empty() || !mutation.model_ops.is_empty() {
            if let Err(e) = self.ui_tx.lock().try_send(mutation) {
                tracing::warn!(
                    page_id = %self.page_id,
                    callback = %callback_name,
                    error = %e,
                    "Failed to send UI mutation"
                );
            } else {
                tracing::debug!(
                    page_id = %self.page_id,
                    callback = %callback_name,
                    "UI mutation sent to Slint thread"
                );
            }
        }

        Ok(())
    }

    /// Parse Lua operations table → UiMutation
    ///
    /// **Expected format**:
    /// ```lua
    /// return {
    ///     properties = {
    ///         { key = "draft", value = "" },
    ///         { key = "message_count", value = 42 }
    ///     },
    ///     models = {
    ///         { op = "push", model = "messages", item = {...} },
    ///         { op = "clear", model = "messages" }
    ///     }
    /// }
    /// ```
    fn parse_operations(&self, ops: Table) -> Result<UiMutation, Box<dyn std::error::Error>> {
        let mut properties = vec![];
        let mut model_ops = vec![];

        // Parse properties array
        if let Ok(props_table) = ops.get::<Table>("properties") {
            for pair in props_table.pairs::<usize, Table>() {
                let (_, prop_entry) = pair?;
                let key: String = prop_entry.get("key")?;
                let value_lua: LuaValue = prop_entry.get("value")?;
                let value_json = lua_to_json(&self.lua, &value_lua)?;

                properties.push(PropertyUpdate { key, value: value_json });
            }
        }

        // Parse models array
        if let Ok(models_table) = ops.get::<Table>("models") {
            for pair in models_table.pairs::<usize, Table>() {
                let (_, model_entry) = pair?;
                let op_str: String = model_entry.get("op")?;

                let vecmodel_op = match op_str.as_str() {
                    "push" => {
                        let model_name: String = model_entry.get("model")?;
                        let item_lua: LuaValue = model_entry.get("item")?;
                        let item = lua_to_json(&self.lua, &item_lua)?;
                        VecModelOp::Push { model_name, item }
                    }
                    "insert" => {
                        let model_name: String = model_entry.get("model")?;
                        let index: usize = model_entry.get("index")?;
                        let item_lua: LuaValue = model_entry.get("item")?;
                        let item = lua_to_json(&self.lua, &item_lua)?;
                        VecModelOp::Insert { model_name, index, item }
                    }
                    "remove" => {
                        let model_name: String = model_entry.get("model")?;
                        let index: usize = model_entry.get("index")?;
                        VecModelOp::Remove { model_name, index }
                    }
                    "set" => {
                        let model_name: String = model_entry.get("model")?;
                        let index: usize = model_entry.get("index")?;
                        let item_lua: LuaValue = model_entry.get("item")?;
                        let item = lua_to_json(&self.lua, &item_lua)?;
                        VecModelOp::Set { model_name, index, item }
                    }
                    "clear" => {
                        let model_name: String = model_entry.get("model")?;
                        VecModelOp::Clear { model_name }
                    }
                    _ => {
                        tracing::warn!(op = %op_str, "Unknown VecModel operation");
                        continue;
                    }
                };

                model_ops.push(vecmodel_op);
            }
        }

        Ok(UiMutation {
            app_id: self.page_id.clone(),
            properties,
            model_ops,
        })
    }
}

/// Convert JSON to Lua value
fn json_to_lua(lua: &Lua, value: &JsonValue) -> Result<LuaValue, LuaError> {
    match value {
        JsonValue::Null => Ok(LuaValue::Nil),
        JsonValue::Bool(b) => Ok(LuaValue::Boolean(*b)),
        JsonValue::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(LuaValue::Integer(i))
            } else if let Some(f) = n.as_f64() {
                Ok(LuaValue::Number(f))
            } else {
                Err(LuaError::RuntimeError(format!("Invalid number: {}", n)))
            }
        }
        JsonValue::String(s) => Ok(LuaValue::String(lua.create_string(s)?)),
        JsonValue::Array(arr) => {
            let table = lua.create_table()?;
            for (i, item) in arr.iter().enumerate() {
                table.set(i + 1, json_to_lua(lua, item)?)?;
            }
            Ok(LuaValue::Table(table))
        }
        JsonValue::Object(obj) => {
            let table = lua.create_table()?;
            for (key, val) in obj {
                table.set(key.as_str(), json_to_lua(lua, val)?)?;
            }
            Ok(LuaValue::Table(table))
        }
    }
}

/// Convert Lua value to JSON
fn lua_to_json(lua: &Lua, value: &LuaValue) -> Result<JsonValue, LuaError> {
    match value {
        LuaValue::Nil => Ok(JsonValue::Null),
        LuaValue::Boolean(b) => Ok(JsonValue::Bool(*b)),
        LuaValue::Integer(i) => Ok(serde_json::json!(*i)),
        LuaValue::Number(f) => Ok(serde_json::json!(*f)),
        LuaValue::String(s) => {
            let str_val = s.to_str()?;
            Ok(JsonValue::String(str_val.to_string()))
        }
        LuaValue::Table(table) => {
            // Check if it's an array (sequential integer keys starting from 1)
            let mut is_array = true;
            let mut max_index = 0;

            for pair in table.clone().pairs::<LuaValue, LuaValue>() {
                let (key, _) = pair?;
                match key {
                    LuaValue::Integer(i) if i > 0 => {
                        max_index = max_index.max(i as usize);
                    }
                    _ => {
                        is_array = false;
                        break;
                    }
                }
            }

            if is_array {
                // Convert as array (including empty tables)
                let mut arr = Vec::with_capacity(max_index);
                for i in 1..=max_index {
                    let val: LuaValue = table.get(i)?;
                    arr.push(lua_to_json(lua, &val)?);
                }
                Ok(JsonValue::Array(arr))
            } else {
                // Convert as object (has non-integer keys)
                let mut obj = serde_json::Map::new();
                for pair in table.clone().pairs::<String, LuaValue>() {
                    let (key, val) = pair?;
                    obj.insert(key, lua_to_json(lua, &val)?);
                }
                Ok(JsonValue::Object(obj))
            }
        }
        _ => Err(LuaError::RuntimeError(format!(
            "Unsupported Lua type for JSON conversion: {:?}",
            value
        ))),
    }
}

/// Convert Loro delta to VecModel operations
///
/// **Context**: Loro provides deltas (Retain/Insert/Delete), we convert to VecModelOps
/// **Example**: [Retain(3), Insert([item]), Delete(1)] → [Insert at 3, Remove at 4]
///
/// **Important**: This enables incremental UI updates instead of full replacement
fn convert_delta_to_vecmodel_ops(model_name: &str, delta: &LoroDelta) -> Vec<VecModelOp> {
    match delta {
        LoroDelta::List { ops } => {
            let mut result = Vec::new();
            let mut index = 0;

            for op in ops {
                match op {
                    ListOp::Retain { count } => {
                        // Skip N items (advance cursor)
                        index += count;
                    }
                    ListOp::Insert { values } => {
                        // Insert items at current position
                        for value in values {
                            result.push(VecModelOp::Insert {
                                model_name: model_name.to_string(),
                                index,
                                item: value.clone(),
                            });
                            index += 1;
                        }
                    }
                    ListOp::Delete { count } => {
                        // Remove N items at current position
                        // Note: Don't increment index - next item slides down
                        for _ in 0..*count {
                            result.push(VecModelOp::Remove {
                                model_name: model_name.to_string(),
                                index,
                            });
                        }
                    }
                }
            }

            result
        }
        LoroDelta::Map { updated } => {
            // For maps, convert each updated key to a property update
            // This is less common - typically we use full_data for maps
            let mut result = Vec::new();

            for (key, value) in updated {
                if let Some(val) = value {
                    // Key was set/updated - we could emit a Set operation
                    // but VecModelOp is designed for arrays
                    // For now, return empty and let fallback handle maps
                    tracing::debug!(
                        model = %model_name,
                        key = %key,
                        "Map delta - falling back to full_data"
                    );
                }
            }

            result
        }
        LoroDelta::Text { .. } => {
            // Text deltas not yet supported for VecModel
            tracing::debug!(
                model = %model_name,
                "Text delta - falling back to full_data"
            );
            vec![]
        }
    }
}
