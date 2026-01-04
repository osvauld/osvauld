//! Butler API Bindings for Lua
//!
//! Provides high-level Butler methods to Lua scripts.
//! This is simpler than direct Loro FFI - Lua just calls Butler methods.

use mlua::{UserData, UserDataMethods, Value as LuaValue, Error as LuaError};
use tokio::sync::oneshot;
use butler::ScribeMessage;
use ractor::ActorRef;

/// Butler bindings - main entry point for Lua to access Butler/Scribe
///
/// **Usage in Lua**:
/// ```lua
/// local messages = butler.get_layer("messages")
/// butler.update_layer("messages", messages)
/// ```
pub struct ButlerBindings {
    scribe_ref: ActorRef<ScribeMessage>,
    page_id: String,
}

impl ButlerBindings {
    pub fn new(scribe_ref: ActorRef<ScribeMessage>, page_id: String) -> Self {
        Self { scribe_ref, page_id }
    }
}

impl UserData for ButlerBindings {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        // butler.get_layer(layer_name) - Get layer data as Lua table
        methods.add_method("get_layer", |lua, this, layer_name: String| {
            // Create oneshot channel for reply
            let (tx, rx) = oneshot::channel();

            // Send message to Scribe
            this.scribe_ref.cast(ScribeMessage::GetLayerJson {
                layer_name: layer_name.clone(),
                reply: tx,
            }).map_err(|e| LuaError::RuntimeError(format!("Failed to send GetLayerJson: {}", e)))?;

            // Block waiting for response
            // Handle case where we might not be in a Tokio runtime context
            let json_value = match tokio::runtime::Handle::try_current() {
                Ok(handle) => {
                    tokio::task::block_in_place(|| {
                        handle.block_on(async {
                            rx.await
                                .map_err(|e| LuaError::RuntimeError(format!("Scribe dropped reply: {}", e)))?
                                .ok_or_else(|| LuaError::RuntimeError(format!("Layer not found: {}", layer_name)))
                        })
                    })?
                }
                Err(_) => {
                    let rt = tokio::runtime::Runtime::new()
                        .map_err(|e| LuaError::RuntimeError(format!("Failed to create runtime: {}", e)))?;
                    rt.block_on(async {
                        rx.await
                            .map_err(|e| LuaError::RuntimeError(format!("Scribe dropped reply: {}", e)))?
                            .ok_or_else(|| LuaError::RuntimeError(format!("Layer not found: {}", layer_name)))
                    })?
                }
            };

            // Convert serde_json::Value to Lua value
            json_to_lua(lua, &json_value)
        });

        // butler.get_list(layer_name) - Get list items from a layer
        // Returns just the items array, not the full Loro structure
        methods.add_method("get_list", |lua, this, layer_name: String| {
            // Create oneshot channel for reply
            let (tx, rx) = oneshot::channel();

            // Send message to Scribe
            this.scribe_ref.cast(ScribeMessage::GetLayerJson {
                layer_name: layer_name.clone(),
                reply: tx,
            }).map_err(|e| LuaError::RuntimeError(format!("Failed to send GetLayerJson: {}", e)))?;

            // Block waiting for response
            // Handle case where we might not be in a Tokio runtime context
            let json_value = match tokio::runtime::Handle::try_current() {
                Ok(handle) => {
                    tokio::task::block_in_place(|| {
                        handle.block_on(async {
                            rx.await
                                .map_err(|e| LuaError::RuntimeError(format!("Scribe dropped reply: {}", e)))?
                                .ok_or_else(|| LuaError::RuntimeError(format!("Layer not found: {}", layer_name)))
                        })
                    })?
                }
                Err(_) => {
                    let rt = tokio::runtime::Runtime::new()
                        .map_err(|e| LuaError::RuntimeError(format!("Failed to create runtime: {}", e)))?;
                    rt.block_on(async {
                        rx.await
                            .map_err(|e| LuaError::RuntimeError(format!("Scribe dropped reply: {}", e)))?
                            .ok_or_else(|| LuaError::RuntimeError(format!("Layer not found: {}", layer_name)))
                    })?
                }
            };

            // Extract layer_name array from Loro structure
            // Loro exports as: {"messages": [...]} where "messages" is the layer name
            // If the list doesn't exist yet, return an empty Lua table
            match json_value.get(&layer_name) {
                Some(items) => json_to_lua(lua, items),
                None => {
                    // Layer list doesn't exist yet, return empty Lua table
                    Ok(LuaValue::Table(lua.create_table()?))
                }
            }
        });
    }
}

/// Convert serde_json::Value to Lua value
fn json_to_lua(lua: &mlua::Lua, value: &serde_json::Value) -> Result<LuaValue, LuaError> {
    match value {
        serde_json::Value::Null => Ok(LuaValue::Nil),
        serde_json::Value::Bool(b) => Ok(LuaValue::Boolean(*b)),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(LuaValue::Integer(i))
            } else if let Some(f) = n.as_f64() {
                Ok(LuaValue::Number(f))
            } else {
                Err(LuaError::RuntimeError(format!("Invalid number: {}", n)))
            }
        }
        serde_json::Value::String(s) => Ok(LuaValue::String(lua.create_string(s)?)),
        serde_json::Value::Array(arr) => {
            let table = lua.create_table()?;
            for (i, item) in arr.iter().enumerate() {
                table.set(i + 1, json_to_lua(lua, item)?)?;
            }
            Ok(LuaValue::Table(table))
        }
        serde_json::Value::Object(obj) => {
            let table = lua.create_table()?;
            for (key, val) in obj {
                table.set(key.as_str(), json_to_lua(lua, val)?)?;
            }
            Ok(LuaValue::Table(table))
        }
    }
}
