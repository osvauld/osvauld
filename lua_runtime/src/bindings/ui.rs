//! UI Bindings for Lua
//!
//! Provides ui:set, ui:get, ui:push, ui:subscribe, ui:emit and other UI operations.

use mlua::{Error as LuaError, Lua, RegistryKey, UserData, UserDataMethods, Value as LuaValue};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};

use crate::event_bus::{Event, EventBus, EventSource, SubscribeOptions};
use crate::ui_types::{PropertyUpdate, UiMutation, UiQuery, VecModelOp};

use super::{json_to_lua, lua_to_json};

/// Shared mutable state between UiBindings and runtime
///
/// Single lock for all state that needs interior mutability.
/// Everything runs on the same OS thread, so this is for interior mutability,
/// not thread safety.
pub struct UiSharedState {
    /// Event Bus for subscribe/emit
    pub event_bus: EventBus,
    /// Lua registry keys for event callbacks (subscriber_id -> registry_key)
    pub callback_keys: HashMap<u64, RegistryKey>,
    /// Pending property updates (accumulated during Lua callback, flushed at end)
    pub pending_properties: Vec<PropertyUpdate>,
    /// Pending model operations (accumulated during Lua callback, flushed at end)
    pub pending_model_ops: Vec<VecModelOp>,
}

impl UiSharedState {
    pub fn new() -> Self {
        Self {
            event_bus: EventBus::new(),
            callback_keys: HashMap::new(),
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

/// UI Bindings for Lua - allows setting properties, emitting events, subscribing
///
/// **Usage in Lua**:
/// - `ui:set("property_name", value)` - set property
/// - `ui:get("property_name")` - get property value
/// - `ui:subscribe("event_type", options, callback)` - subscribe to events
/// - `ui:emit("event_type", data)` - emit an event
///
/// **Threading**: Holds sender, sends mutations to UI thread
pub struct UiBindings {
    /// App identifier for mutations
    pub app_id: String,
    /// Channel sender for UI mutations (Sender is Clone+Send, no wrapper needed)
    pub ui_tx: mpsc::Sender<UiMutation>,
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

            let json_value =
                lua_to_json(&value).map_err(|e| LuaError::RuntimeError(e.to_string()))?;

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
        methods.add_method("push", |_lua, this, (model_name, item): (String, LuaValue)| {
            let item_json =
                lua_to_json(&item).map_err(|e| LuaError::RuntimeError(e.to_string()))?;

            this.shared.lock().pending_model_ops.push(VecModelOp::Push {
                model_name,
                item: item_json,
            });

            Ok(())
        });

        // ui:insert(model_name, index, item) - Insert item at index
        methods.add_method(
            "insert",
            |_lua, this, (model_name, index, item): (String, usize, LuaValue)| {
                let item_json =
                    lua_to_json(&item).map_err(|e| LuaError::RuntimeError(e.to_string()))?;

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
        methods.add_method("remove", |_lua, this, (model_name, index): (String, usize)| {
            this.shared
                .lock()
                .pending_model_ops
                .push(VecModelOp::Remove { model_name, index });

            Ok(())
        });

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
                let item_json =
                    lua_to_json(&item).map_err(|e| LuaError::RuntimeError(e.to_string()))?;

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

        // ui:subscribe(event_type, callback) or ui:subscribe(event_type, options, callback)
        // Returns subscriber_id for unsubscription
        methods.add_method("subscribe", |lua, this, args: mlua::MultiValue| {
            let args_vec: Vec<LuaValue> = args.into_vec();

            // Parse arguments: (event_type, callback) or (event_type, options, callback)
            let (event_type, options, callback) = match args_vec.len() {
                2 => {
                    let event_type: String = match &args_vec[0] {
                        LuaValue::String(s) => s.to_str()?.to_string(),
                        _ => {
                            return Err(LuaError::RuntimeError(
                                "event_type must be string".into(),
                            ))
                        }
                    };
                    let callback = match &args_vec[1] {
                        LuaValue::Function(f) => f.clone(),
                        _ => {
                            return Err(LuaError::RuntimeError(
                                "callback must be function".into(),
                            ))
                        }
                    };
                    (event_type, SubscribeOptions::default(), callback)
                }
                3 => {
                    let event_type: String = match &args_vec[0] {
                        LuaValue::String(s) => s.to_str()?.to_string(),
                        _ => {
                            return Err(LuaError::RuntimeError(
                                "event_type must be string".into(),
                            ))
                        }
                    };
                    let options = parse_subscribe_options(&args_vec[1])?;
                    let callback = match &args_vec[2] {
                        LuaValue::Function(f) => f.clone(),
                        _ => {
                            return Err(LuaError::RuntimeError(
                                "callback must be function".into(),
                            ))
                        }
                    };
                    (event_type, options, callback)
                }
                _ => {
                    return Err(LuaError::RuntimeError(
                        "subscribe takes 2 or 3 arguments: (event_type, [options], callback)"
                            .into(),
                    ))
                }
            };

            // Store callback in Lua registry
            let registry_key = lua
                .create_registry_value(callback)
                .map_err(|e| LuaError::RuntimeError(format!("Failed to store callback: {}", e)))?;

            let mut shared = this.shared.lock();

            // Subscribe to event bus (callback_key is placeholder, we use subscriber_id for lookup)
            let subscriber_id = shared.event_bus.subscribe(&event_type, options, 0_usize);

            // Store registry key for later lookup
            shared.callback_keys.insert(subscriber_id, registry_key);

            tracing::debug!(
                app_id = %this.app_id,
                event_type = %event_type,
                subscriber_id = subscriber_id,
                "Subscribed to UI event"
            );

            Ok(LuaValue::Integer(subscriber_id as i64))
        });

        // ui:unsubscribe(subscriber_id) - Remove a subscription
        methods.add_method("unsubscribe", |_lua, this, subscriber_id: i64| {
            let id = subscriber_id as u64;
            let mut shared = this.shared.lock();

            // Remove from event bus
            let removed = shared.event_bus.unsubscribe(id);

            // Remove callback key
            if let Some(key) = shared.callback_keys.remove(&id) {
                drop(key); // Let Lua GC the callback
            }

            Ok(removed)
        });

        // ui:emit(event_type, data) - Emit an event (for AI/test automation)
        methods.add_method("emit", |_lua, this, (event_type, data): (String, LuaValue)| {
            let data_json =
                lua_to_json(&data).map_err(|e| LuaError::RuntimeError(e.to_string()))?;

            // Extract target from data if present
            let target = if let serde_json::Value::Object(ref obj) = data_json {
                obj.get("target")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            } else {
                None
            };

            let mut event = Event::new(&event_type, EventSource::Ai);
            if let Some(t) = target {
                event = event.with_target(t);
            }
            event = event.with_data(data_json);

            // Emit to event bus - returns deliveries to process
            let deliveries = this.shared.lock().event_bus.emit(event.clone());

            tracing::debug!(
                app_id = %this.app_id,
                event_type = %event_type,
                deliveries = deliveries.len(),
                "Emitted UI event"
            );

            // Return the number of subscribers notified
            Ok(deliveries.len() as i64)
        });
    }
}

/// Convert Event to Lua table
pub fn event_to_lua(lua: &Lua, event: &Event) -> Result<LuaValue, LuaError> {
    let table = lua.create_table()?;
    table.set("type", event.event_type.clone())?;
    table.set("source", format!("{:?}", event.source).to_lowercase())?;
    table.set("timestamp", event.timestamp)?;

    if let Some(ref target) = event.target {
        table.set("target", target.clone())?;
    }

    // Merge event data into table
    if let serde_json::Value::Object(ref map) = event.data {
        for (key, value) in map {
            let lua_value = json_to_lua(lua, value)?;
            table.set(key.clone(), lua_value)?;
        }
    }

    Ok(LuaValue::Table(table))
}

/// Parse subscribe options from Lua table
pub fn parse_subscribe_options(value: &LuaValue) -> Result<SubscribeOptions, LuaError> {
    match value {
        LuaValue::Table(t) => {
            let mut options = SubscribeOptions::default();

            if let Ok(target) = t.get::<String>("target") {
                options.target = Some(target);
            }
            if let Ok(prefix) = t.get::<String>("target_prefix") {
                options.target_prefix = Some(prefix);
            }
            if let Ok(rate) = t.get::<u32>("sample_rate") {
                options.sample_rate = Some(rate);
            }
            if let Ok(batch) = t.get::<u32>("batch_ms") {
                options.batch_ms = Some(batch);
            }

            Ok(options)
        }
        LuaValue::Nil => Ok(SubscribeOptions::default()),
        _ => Err(LuaError::RuntimeError(
            "options must be table or nil".into(),
        )),
    }
}
