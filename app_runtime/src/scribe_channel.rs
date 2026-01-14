//! Scribe Channel - Bridge between Lua scripts and Scribe actor
//!
//! Provides a bidirectional channel for Lua scripts to:
//! - Send commands to Scribe (list_push, map_insert, etc.)
//! - Receive events from Scribe (layer updates)

use mlua::{UserData, UserDataMethods, Value as LuaValue};
use serde_json::Value as JsonValue;
use tokio::sync::mpsc;
use crate::lua_to_json;

/// Commands that Lua scripts can send to Scribe
#[derive(Debug, Clone)]
pub enum ScribeCommand {
    /// Push an item to a list
    ListPush {
        layer: String,
        path: String,
        item: JsonValue,
    },
    /// Insert a key-value pair into a map
    MapInsert {
        layer: String,
        path: String,
        key: String,
        value: JsonValue,
    },
    /// Delete an item from a list by index
    ListDelete {
        layer: String,
        path: String,
        index: usize,
    },
    /// Delete a key from a map
    MapDelete {
        layer: String,
        path: String,
        key: String,
    },
    /// Set a value at a path
    Set {
        layer: String,
        path: String,
        value: JsonValue,
    },
}

/// Events from Scribe that are sent to Lua scripts
#[derive(Debug, Clone)]
pub enum ScribeEvent {
    /// A layer was updated with new data
    LayerUpdated { layer: String, data: JsonValue },
}

/// Lua UserData wrapper for sending commands to Scribe
#[derive(Clone)]
pub struct ScribeChannel {
    tx: mpsc::Sender<ScribeCommand>,
}

impl ScribeChannel {
    pub fn new(tx: mpsc::Sender<ScribeCommand>) -> Self {
        Self { tx }
    }
}

impl UserData for ScribeChannel {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        // scribe.list_push(layer, path, item)
        methods.add_method(
            "list_push",
            |_, this, (layer, path, item): (String, String, LuaValue)| {
                let json_item = lua_to_json(&item)?;
                tracing::info!(
                    layer = %layer,
                    path = %path,
                    item = ?json_item,
                    "Lua calling scribe.list_push"
                );
                match this.tx.try_send(ScribeCommand::ListPush {
                    layer: layer.clone(),
                    path: path.clone(),
                    item: json_item,
                }) {
                    Ok(_) => tracing::debug!("Command sent to channel"),
                    Err(e) => tracing::error!("Failed to send command: {:?}", e),
                }
                Ok(())
            },
        );

        // scribe.map_insert(layer, path, key, value)
        methods.add_method(
            "map_insert",
            |_, this, (layer, path, key, value): (String, String, String, LuaValue)| {
                let json_value = lua_to_json(&value)?;
                let _ = this.tx.try_send(ScribeCommand::MapInsert {
                    layer,
                    path,
                    key,
                    value: json_value,
                });
                Ok(())
            },
        );

        // scribe.list_delete(layer, path, index)
        methods.add_method(
            "list_delete",
            |_, this, (layer, path, index): (String, String, usize)| {
                let _ = this.tx.try_send(ScribeCommand::ListDelete { layer, path, index });
                Ok(())
            },
        );

        // scribe.map_delete(layer, path, key)
        methods.add_method(
            "map_delete",
            |_, this, (layer, path, key): (String, String, String)| {
                let _ = this.tx.try_send(ScribeCommand::MapDelete { layer, path, key });
                Ok(())
            },
        );

        // scribe.set(layer, path, value)
        methods.add_method(
            "set",
            |_, this, (layer, path, value): (String, String, LuaValue)| {
                let json_value = lua_to_json(&value)?;
                let _ = this.tx.try_send(ScribeCommand::Set {
                    layer,
                    path,
                    value: json_value,
                });
                Ok(())
            },
        );
    }
}
