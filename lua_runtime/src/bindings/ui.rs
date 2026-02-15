//! UI Bindings for Lua
//!
//! Provides ui:set, ui:get, ui:push and other UI operations.

use mlua::{UserData, UserDataMethods, Value as LuaValue};
use parking_lot::Mutex;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};

use crate::ui_types::{PropertyUpdate, UiQuery, VecModelOp};

use super::convert::{json_to_lua, lua_to_json_err};

/// Shared mutable state between UiBindings and runtime
///
/// Single lock for all state that needs interior mutability.
/// Everything runs on the same OS thread, so this is for interior mutability,
/// not thread safety.
pub struct UiSharedState {
    /// Pending property updates (accumulated during Lua callback, flushed at end)
    pub pending_properties: Vec<PropertyUpdate>,
    /// Pending model operations (accumulated during Lua callback, flushed at end)
    pub pending_model_ops: Vec<VecModelOp>,
}

impl UiSharedState {
    pub fn new() -> Self {
        Self {
            pending_properties: Vec::new(),
            pending_model_ops: Vec::new(),
        }
    }
}

impl Default for UiSharedState {
    fn default() -> Self {
        Self::new()
    }
}

/// UI Bindings for Lua - allows setting and querying UI state
///
/// **Usage in Lua**:
/// - `ui:set("property_name", value)` - set property
/// - `ui:get("property_name")` - get property value
/// **Threading**: Holds sender, sends mutations to UI thread
pub struct UiBindings {
    /// App identifier for mutations
    pub app_id: String,
    /// Channel sender for UI queries (for ui:get)
    pub query_tx: mpsc::Sender<UiQuery>,
    /// Shared mutable state (single lock for all)
    pub shared: Arc<Mutex<UiSharedState>>,
}

impl UserData for UiBindings {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        // ui:set(key, value) - Set a property or replace model data
        // Note: Slint uses kebab-case for property names
        // Mutations are accumulated and flushed at end of Lua callback (batch mode)
        methods.add_method("set", |_lua, this, (key, value): (String, LuaValue)| {
            let prop_name = key;

            let json_value = lua_to_json_err(&value)?;

            let mut shared = this.shared.lock();

            // Check if value is an array (for VecModel operations)
            if let serde_json::Value::Array(items) = json_value {
                // Array value: Use Replace for atomic update (no flickering)
                shared.pending_model_ops.push(VecModelOp::Replace {
                    model_name: prop_name,
                    items,
                });
            } else {
                // Scalar value: Accumulate PropertyUpdate for batch flush
                shared.pending_properties.push(PropertyUpdate {
                    key: prop_name,
                    value: json_value,
                });
            }

            Ok(())
        });

        // ui:push(model_name, item) - Add item to end of model
        methods.add_method(
            "push",
            |_lua, this, (model_name, item): (String, LuaValue)| {
                let item_json = lua_to_json_err(&item)?;

                this.shared.lock().pending_model_ops.push(VecModelOp::Push {
                    model_name,
                    item: item_json,
                });

                Ok(())
            },
        );

        // ui:insert(model_name, index, item) - Insert item at index
        methods.add_method(
            "insert",
            |_lua, this, (model_name, index, item): (String, usize, LuaValue)| {
                let item_json = lua_to_json_err(&item)?;

                this.shared
                    .lock()
                    .pending_model_ops
                    .push(VecModelOp::Insert {
                        model_name,
                        index,
                        item: item_json,
                    });

                Ok(())
            },
        );

        // ui:remove(model_name, index) - Remove item at index
        methods.add_method(
            "remove",
            |_lua, this, (model_name, index): (String, usize)| {
                this.shared
                    .lock()
                    .pending_model_ops
                    .push(VecModelOp::Remove { model_name, index });

                Ok(())
            },
        );

        // ui:clear(model_name) - Remove all items from model
        methods.add_method("clear", |_lua, this, model_name: String| {
            this.shared
                .lock()
                .pending_model_ops
                .push(VecModelOp::Clear { model_name });

            Ok(())
        });

        // ui:update(model_name, index, item) - Update single item at index (efficient for drag)
        // Triggers ModelNotify::row_changed(index) - only re-renders that one item
        methods.add_method(
            "update",
            |_lua, this, (model_name, index, item): (String, usize, LuaValue)| {
                let item_json = lua_to_json_err(&item)?;

                this.shared.lock().pending_model_ops.push(VecModelOp::Set {
                    model_name,
                    index,
                    item: item_json,
                });

                Ok(())
            },
        );

        // ui:get(key) -> value - Read a property from UI
        methods.add_method("get", |lua, this, key: String| {
            // Create oneshot channel for response
            let (response_tx, response_rx) = oneshot::channel();

            let query = UiQuery {
                prop_name: key.clone(),
                response_tx,
            };

            // Send query to UI thread
            if let Err(e) = this.query_tx.try_send(query) {
                tracing::warn!(
                    app_id = %this.app_id,
                    key = %key,
                    error = %e,
                    "Failed to send UI query from Lua"
                );
                return Ok(LuaValue::Nil);
            }

            // Block waiting for response (should be fast)
            match response_rx.blocking_recv() {
                Ok(Some(value)) => json_to_lua(lua, &value),
                Ok(None) => Ok(LuaValue::Nil),
                Err(_) => {
                    tracing::warn!(
                        app_id = %this.app_id,
                        key = %key,
                        "UI query response channel closed"
                    );
                    Ok(LuaValue::Nil)
                }
            }
        });
    }
}
