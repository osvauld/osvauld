//! Headless Lua runtime for node and AI
//!
//! **Context**: Same Lua as apps, no UI bindings
//! **All access via Scribe actor (read + write)**
//!
//! Used by:
//! - kunki (node server) for server-side validation and logic
//! - AI runtime for LLM-driven operations

pub mod bindings;

use mlua::{Lua, Function, ObjectLike, Value};
use ractor::ActorRef;
use tokio::sync::mpsc;
use tracing::{debug, info};

use crate::scribe::{ScribeMessage, PageEvent};

pub use bindings::{
    LoroBindings, PermitBindings, DerivationBindings, LuaLoroList, LuaLoroMap,
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
pub struct HeadlessRuntime {
    lua: Lua,
    page_id: String,
    scribe_ref: ActorRef<ScribeMessage>,
    /// Receiver for page events from Scribe (unified event channel)
    page_event_rx: mpsc::Receiver<PageEvent>,
}

impl HeadlessRuntime {
    /// Create runtime for a page
    ///
    /// **Subscribes to Scribe for Loro change events**
    pub async fn new(
        page_id: &str,
        scribe_ref: ActorRef<ScribeMessage>,
        our_did: &str,
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
            our_role.to_string(),
        );
        lua.globals().set("permit", permit)
            .map_err(|e| format!("Failed to set permit global: {}", e))?;

        // Setup derivation bindings (node-only operations)
        let derivation = DerivationBindings::new(scribe_ref.clone());
        lua.globals().set("derivation", derivation)
            .map_err(|e| format!("Failed to set derivation global: {}", e))?;

        // Subscribe to page events from Scribe (unified event channel)
        let (page_event_tx, page_event_rx) = mpsc::channel(32);
        scribe_ref.cast(ScribeMessage::SubscribeToPageEvents {
            event_tx: page_event_tx,
        }).map_err(|e| format!("Failed to subscribe to page events: {}", e))?;

        info!(page_id = %page_id, "HeadlessRuntime created");

        Ok(Self {
            lua,
            page_id: page_id.to_string(),
            scribe_ref,
            page_event_rx,
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
