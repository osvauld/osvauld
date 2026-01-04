//! Lua Worker Thread - Stateless computation engine
//!
//! OS thread per app, owns Lua VM, computes UI operations.
//!
//! **Threading**: Uses `std::thread::spawn` (mlua::Lua is NOT Send for tokio)
//! **Pattern**: Query Loro → Compute → Return operations (stateless)
//! **Communication**: mpsc channels with serializable messages

use mlua::{Lua, Value as LuaValue, Error as LuaError, Table};
use tokio::sync::mpsc;
use ractor::ActorRef;
use butler::ScribeMessage;
use serde_json::Value as JsonValue;
use crate::vecmodel_ops::{UiMutation, VecModelOp, PropertyUpdate};
use crate::loro_bindings::LoroBindings;
use crate::butler_bindings::ButlerBindings;

/// Commands sent to Lua worker thread
///
/// **Sender**: Main thread (Slint) or Scribe observer
/// **Receiver**: LuaWorker event loop
#[derive(Debug)]
pub enum LuaWorkerCommand {
    /// Loro layer changed (local write OR peer sync)
    LoroChanged {
        layer_name: String,
        // TODO: Add delta parameter when LoroChangeEvent is updated
        // delta: Option<JsonValue>,
        full_data: JsonValue,
    },

    /// UI callback (button click, input change, etc.)
    UiCallback {
        callback_name: String,
        args: Vec<JsonValue>,
    },

    /// Shutdown worker thread
    Shutdown,
}

/// Lua worker thread state
///
/// **Ownership**: Runs on OS thread, owns Lua VM (NOT Rc)
/// **Lifetime**: Lives until app window closes or Shutdown command
pub struct LuaWorker {
    /// Owned Lua VM (mlua::Lua is NOT Send, stays on this thread)
    lua: Lua,

    /// App/Page identifier
    page_id: String,

    /// Channel to send UI mutations to Slint thread
    ui_tx: mpsc::Sender<UiMutation>,

    /// Reference to Scribe actor (for Loro queries)
    scribe_ref: ActorRef<ScribeMessage>,

    /// Channel to receive commands from Slint thread
    cmd_rx: mpsc::Receiver<LuaWorkerCommand>,
}

impl LuaWorker {
    /// Spawn Lua worker thread
    ///
    /// **Returns**: (thread handle, command sender)
    /// **Thread**: Blocks on event loop until Shutdown
    pub fn spawn(
        page_id: String,
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
                    error = %e,
                    "Failed to load Lua code"
                );
                return;
            }

            let mut worker = LuaWorker {
                lua,
                page_id,
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
                Some(LuaWorkerCommand::LoroChanged { layer_name, full_data }) => {
                    if let Err(e) = self.handle_loro_change(layer_name, full_data) {
                        tracing::error!(
                            page_id = %self.page_id,
                            error = %e,
                            "Loro change handler failed"
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

        // ui helper (placeholder for set_property)
        // This will be used by Lua to construct property updates
        let ui_table = self.lua.create_table()?;
        globals.set("ui", ui_table)?;

        tracing::debug!(
            page_id = %self.page_id,
            "Lua globals initialized (loro, butler, ui)"
        );

        Ok(())
    }

    /// Handle Loro change: call Lua, parse operations, send to UI
    ///
    /// **Calls**: `on_loro_change(layer_name, delta, full_data)` in Lua
    /// **Returns**: Table with `properties` and `models` arrays
    fn handle_loro_change(
        &self,
        layer_name: String,
        full_data: JsonValue,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Get on_loro_change function from Lua
        let on_loro_change: mlua::Function = self.lua.globals().get("on_loro_change")?;

        // Convert full_data to Lua value
        let full_data_lua = json_to_lua(&self.lua, &full_data)?;

        // Call Lua function
        // TODO: Add delta parameter when LoroChangeEvent supports it
        let ops_table: Table = on_loro_change.call((
            layer_name.clone(),
            LuaValue::Nil,  // delta (placeholder)
            full_data_lua,
        ))?;

        // Parse operations table → UiMutation
        let mutation = self.parse_operations(ops_table)?;

        // Send to Slint thread (non-blocking send)
        if let Err(e) = self.ui_tx.try_send(mutation) {
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

        Ok(())
    }

    /// Handle UI callback: call Lua, parse operations, send to UI
    ///
    /// **Calls**: `on_<callback_name>(args...)` in Lua
    /// **Example**: Button click calls `on_send()` in Lua
    fn handle_ui_callback(
        &self,
        callback_name: String,
        args: Vec<JsonValue>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Get callback function from Lua
        let callback: mlua::Function = self.lua.globals().get(callback_name.as_str())?;

        // Convert args to Lua values
        let lua_args: Result<Vec<LuaValue>, _> = args.iter()
            .map(|arg| json_to_lua(&self.lua, arg))
            .collect();
        let lua_args = lua_args?;

        // Call Lua function
        let ops_table: Table = callback.call(mlua::MultiValue::from_vec(lua_args))?;

        // Parse operations table → UiMutation
        let mutation = self.parse_operations(ops_table)?;

        // Send to Slint thread
        if let Err(e) = self.ui_tx.try_send(mutation) {
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

            if is_array && max_index > 0 {
                // Convert as array
                let mut arr = Vec::with_capacity(max_index);
                for i in 1..=max_index {
                    let val: LuaValue = table.get(i)?;
                    arr.push(lua_to_json(lua, &val)?);
                }
                Ok(JsonValue::Array(arr))
            } else {
                // Convert as object
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
