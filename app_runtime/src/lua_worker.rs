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
use crate::butler_bindings::ButlerBindings;
// Re-exported from butler
use crate::{LoroBindings, PermitBindings, json_to_lua, lua_to_json, matches_layer_pattern};

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
        methods.add_method("set", |_lua, this, (key, value): (String, LuaValue)| {
            // Use property name as-is (Slint expects kebab-case)
            let prop_name = key;

            let json_value = lua_to_json(&value)
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

    /// Page identifier (UUID, e.g., "bf57890c-70ce-46e3-b205-f419163ab0f4")
    page_id: String,

    /// App name (e.g., "Shop Customer") - for logging
    app_name: String,

    /// User's DID (e.g., "did:key:z6Mk...")
    user_did: String,

    /// User's role from permit (e.g., "owner", "viewer")
    user_role: String,

    /// Channel to send UI mutations to Slint thread (wrapped for sharing with UiBindings)
    ui_tx: Arc<Mutex<mpsc::Sender<UiMutation>>>,

    /// Reference to Scribe actor (for Loro queries)
    scribe_ref: ActorRef<ScribeMessage>,

    /// Channel to receive commands from Slint thread
    cmd_rx: mpsc::Receiver<LuaWorkerCommand>,

    /// Registered page:on_change() handlers (shared with PageBindings)
    page_handlers: Arc<std::sync::RwLock<Vec<PageHandler>>>,
}

impl LuaWorker {
    /// Spawn Lua worker thread
    ///
    /// **Parameters**:
    /// - `page_id`: Page UUID (e.g., "bf57890c-70ce-46e3-b205-f419163ab0f4")
    /// - `app_name`: App name (e.g., "Shop Customer") - for logging
    /// - `user_did`: User's DID (e.g., "did:key:z6Mk...") - for permit bindings
    /// - `user_role`: User's role from permit (e.g., "owner", "viewer") - for permit bindings
    /// - `lua_code`: Lua source code to execute
    /// - `scribe_ref`: Reference to Scribe actor
    /// - `ui_tx`: Channel to send UI mutations
    ///
    /// **Returns**: (thread handle, command sender)
    /// **Thread**: Blocks on event loop until Shutdown
    pub fn spawn(
        page_id: String,
        app_name: String,
        user_did: String,
        user_role: String,
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

            // Create shared page handlers (shared between PageBindings and LuaWorker)
            let page_handlers = Arc::new(std::sync::RwLock::new(Vec::new()));

            let mut worker = LuaWorker {
                lua,
                page_id,
                app_name,
                user_did,
                user_role,
                ui_tx,
                scribe_ref,
                cmd_rx,
                page_handlers,
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
        // Uses real page_id (UUID), user_did, and role from permit
        let permit_binding = PermitBindings::new(
            self.page_id.clone(),
            self.user_did.clone(),
            self.user_role.clone(),
        );
        globals.set("permit", permit_binding)?;

        // page binding - provides page:on_change() for registering layer event handlers
        let page_binding = PageBindings {
            handlers: self.page_handlers.clone(),
            page_id: self.page_id.clone(),
        };
        globals.set("page", page_binding)?;

        tracing::info!(
            page_id = %self.page_id,
            user_did = %self.user_did,
            user_role = %self.user_role,
            app_name = %self.app_name,
            "Lua globals initialized (loro, butler, ui, permit, page)"
        );

        Ok(())
    }

    /// Handle Loro change: convert delta to VecModelOps OR fallback to Lua
    ///
    /// **Fast path**: Delta exists AND simple layer name → convert directly to VecModelOps
    /// **Slow path**: Dynamic layers OR no delta → call Lua with full_data
    ///
    /// Dynamic layers (those with '/' like `{page_id}/orders/{did}`) require Lua
    /// aggregation because multiple layers map to a single UI model.
    fn handle_loro_change(
        &self,
        layer_name: String,
        delta: Option<LoroDelta>,
        full_data: JsonValue,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Dynamic layers require Lua aggregation - skip fast path
        // Examples: "{page_id}/orders/{did}" needs Lua to aggregate all orders
        // Simple layers like "products" can use fast path (1:1 layer→model mapping)
        let is_dynamic_layer = layer_name.contains('/');

        // Fast path: Convert delta directly to VecModelOps (skip Lua)
        // Only for simple layer names where layer_name == model_name
        if !is_dynamic_layer {
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

        // Dispatch to page:on_change() handlers
        // These allow Lua apps to respond to layer changes (including derivations)
        if let Err(e) = dispatch_page_handlers(
            &self.lua,
            &self.page_handlers,
            &layer_name,
            "updated",
            &full_data,
        ) {
            tracing::error!(
                page_id = %self.page_id,
                layer_name = %layer_name,
                error = %e,
                "Failed to dispatch page handlers"
            );
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
    /// **Calls**: Legacy `on_layer_discovered(layer_name)` and new `page:on_change` handlers
    fn handle_layer_discovered(
        &self,
        layer_name: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Call legacy on_layer_discovered function if defined
        if let Ok(func) = self.lua.globals().get::<mlua::Function>("on_layer_discovered") {
            tracing::info!(
                page_id = %self.page_id,
                layer = %layer_name,
                "Calling Lua on_layer_discovered"
            );

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
                }
            }
        }

        // Dispatch to page:on_change() handlers with "created" event type
        // Note: full_data is empty for layer discovery - handlers should query the layer
        if let Err(e) = dispatch_page_handlers(
            &self.lua,
            &self.page_handlers,
            layer_name,
            "created",
            &serde_json::json!({}),
        ) {
            tracing::error!(
                page_id = %self.page_id,
                layer = %layer_name,
                error = %e,
                "Failed to dispatch page handlers for layer discovery"
            );
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
                let value_json = lua_to_json(&value_lua)?;

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
                        let item = lua_to_json(&item_lua)?;
                        VecModelOp::Push { model_name, item }
                    }
                    "insert" => {
                        let model_name: String = model_entry.get("model")?;
                        let index: usize = model_entry.get("index")?;
                        let item_lua: LuaValue = model_entry.get("item")?;
                        let item = lua_to_json(&item_lua)?;
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
                        let item = lua_to_json(&item_lua)?;
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
            // VecModelOp is designed for arrays, so return empty and let fallback handle maps
            if !updated.is_empty() {
                tracing::debug!(
                    model = %model_name,
                    keys = updated.len(),
                    "Map delta - falling back to full_data"
                );
            }
            vec![]
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

// =============================================================================
// PageBindings - page:on_change() API
// =============================================================================

/// Registered page event handler
pub(crate) struct PageHandler {
    /// Pattern to match (e.g., "{page_id}/orders/*")
    pub pattern: String,
    /// Lua function reference (stored in registry)
    pub callback: mlua::RegistryKey,
}

/// Page bindings providing `page:on_change()` API
///
/// **Usage in Lua**:
/// ```lua
/// page:on_change("{page_id}/orders/*", function(event)
///     -- event.layer_name = "shop123/orders/did:key:customer"
///     -- event.event_type = "created" | "updated"
///     -- event.full_data = { ... }
///     local summary_layer = loro:get_layer("{page_id}/derived/orders_summary", "map")
///     summary_layer:set(event.full_data.id, transform(event.full_data))
/// end)
/// ```
pub struct PageBindings {
    /// Registered handlers (pattern → callback)
    pub handlers: Arc<std::sync::RwLock<Vec<PageHandler>>>,
    /// Page ID for pattern expansion
    pub page_id: String,
}

impl UserData for PageBindings {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        // page:on_change(pattern, callback)
        methods.add_method("on_change", |lua, this, (pattern, callback): (String, mlua::Function)| {
            // Expand {page_id} in pattern
            let expanded_pattern = pattern.replace("{page_id}", &this.page_id);

            // Store callback in Lua registry (keeps it alive)
            let registry_key = lua.create_registry_value(callback)
                .map_err(|e| LuaError::RuntimeError(format!("Failed to store callback: {}", e)))?;

            // Add handler
            let handler = PageHandler {
                pattern: expanded_pattern.clone(),
                callback: registry_key,
            };

            if let Ok(mut handlers) = this.handlers.write() {
                handlers.push(handler);
                tracing::info!(
                    pattern = %expanded_pattern,
                    handler_count = handlers.len(),
                    "Registered page:on_change handler"
                );
            }

            Ok(())
        });
    }
}

/// Dispatch page event to registered handlers
///
/// **Context**: Called from handle_loro_change after layer update
/// **We do**: Find handlers matching the layer name, call their callbacks
pub fn dispatch_page_handlers(
    lua: &Lua,
    handlers: &std::sync::RwLock<Vec<PageHandler>>,
    layer_name: &str,
    event_type: &str,
    full_data: &JsonValue,
) -> Result<(), LuaError> {
    let handlers_guard = handlers.read()
        .map_err(|e| LuaError::RuntimeError(format!("Lock error: {}", e)))?;

    let mut matched_count = 0;

    for handler in handlers_guard.iter() {
        if matches_layer_pattern(&handler.pattern, layer_name) {
            // Create event table
            let event_table = lua.create_table()?;
            event_table.set("layer_name", layer_name)?;
            event_table.set("event_type", event_type)?;
            event_table.set("full_data", json_to_lua(lua, full_data)?)?;

            // Get callback from registry
            let callback: mlua::Function = lua.registry_value(&handler.callback)?;

            // Call handler
            match callback.call::<()>(event_table) {
                Ok(()) => {
                    matched_count += 1;
                    tracing::debug!(
                        layer = %layer_name,
                        pattern = %handler.pattern,
                        "page:on_change handler executed"
                    );
                }
                Err(e) => {
                    tracing::error!(
                        layer = %layer_name,
                        pattern = %handler.pattern,
                        error = %e,
                        "page:on_change handler failed"
                    );
                }
            }
        }
    }

    if matched_count > 0 {
        tracing::debug!(
            layer = %layer_name,
            matched_count = matched_count,
            "Dispatched to page:on_change handlers"
        );
    }

    Ok(())
}
