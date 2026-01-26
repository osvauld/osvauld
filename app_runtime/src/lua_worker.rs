//! Lua Worker Thread - Stateless computation engine
//!
//! OS thread per app, owns Lua VM, computes UI operations.
//!
//! **Threading**: Uses `std::thread::spawn` (mlua::Lua is NOT Send for tokio)
//! **Pattern**: Query Loro → Compute → Return operations (stateless)
//! **Communication**: mpsc channels with serializable messages

use mlua::{Lua, Value as LuaValue, Error as LuaError, Table, UserData, UserDataMethods, RegistryKey};
use tokio::sync::{mpsc, oneshot};
use ractor::ActorRef;
use butler::{ScribeMessage, LoroDelta, ListOp};
use serde_json::Value as JsonValue;
use std::sync::Arc;
use parking_lot::Mutex;
use crate::vecmodel_ops::{UiMutation, VecModelOp, PropertyUpdate, UiQuery};
use crate::butler_bindings::ButlerBindings;
use crate::event_bus::{EventBus, Event, EventSource, SubscribeOptions, EventDelivery};
// Re-exported from butler
use crate::{LoroBindings, PermitBindings, json_to_lua, lua_to_json, matches_layer_pattern};
use crate::layout_bindings::LayoutBindings;

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

    /// UI Event from Slint (for Event Bus)
    ///
    /// **Context**: Slint UI event (click, field_changed, etc.) routed to Event Bus
    /// **We do**: Dispatch to Lua subscribers
    UiEvent {
        event: Event,
    },

    /// Shutdown worker thread
    Shutdown,

    /// Game tick (called periodically for games/animations)
    ///
    /// **Context**: Timer fires, call Lua's `tick()` if defined
    /// **We do**: Call tick() and flush UI mutations
    Tick,

    /// Debug: Execute Lua code and return result
    ///
    /// **Context**: Called from debug server for introspection
    /// **Returns**: JSON-serialized result or error string
    DebugEval {
        code: String,
        response_tx: oneshot::Sender<Result<JsonValue, String>>,
    },

    /// Debug: Get current state snapshot
    ///
    /// **Context**: Called from debug server for state inspection
    /// **Returns**: Snapshot of Lua globals, registered handlers, etc.
    DebugGetState {
        response_tx: oneshot::Sender<DebugState>,
    },

    // ==================== Ephemeral Events (from peers via datagram) ====================

    /// Ephemeral data from peer (generic)
    ///
    /// **Context**: Peer sent ephemeral data (cursor, typing, etc.) via datagram
    /// **We do**: Call Lua's `on_ephemeral(user_did, payload)` if defined
    /// **Design**: Generic payload - Lua app interprets format (JSON with type/x/y, etc.)
    Ephemeral {
        user_did: String,
        /// Opaque payload bytes - Lua converts to string and parses as JSON
        payload: Vec<u8>,
    },

    // ==================== Peer Presence Events (from Scribe subscribe/unsubscribe) ====================

    /// Peer joined this page (subscribed to Scribe)
    ///
    /// **Context**: Remote peer subscribed to this page's Scribe
    /// **We do**: Call Lua's `on_peer_joined(user_did)` if defined
    /// **Use cases**: Online counters, presence indicators
    PeerJoined {
        user_did: String,
    },

    /// Peer left this page (unsubscribed from Scribe)
    ///
    /// **Context**: Remote peer unsubscribed from this page's Scribe
    /// **We do**: Call Lua's `on_peer_left(user_did)` if defined
    /// **Use cases**: Online counters, presence indicators
    PeerLeft {
        user_did: String,
    },

    // ==================== Asset Upload Events ====================

    /// Asset uploaded successfully
    ///
    /// **Context**: User picked a file via native dialog, it was uploaded to AssetStore
    /// **We do**: Call Lua's `on_asset_uploaded(asset)` if defined
    /// **Lua receives**: { hash, filename, mime_type, size }
    AssetUploaded {
        /// Content hash of the asset (for retrieval)
        hash: String,
        /// Original filename
        filename: String,
        /// MIME type (e.g., "image/png")
        mime_type: String,
        /// File size in bytes
        size: u64,
    },
}

/// Debug state snapshot for introspection
///
/// **Context**: Returned by DebugGetState command
#[derive(Debug, Clone, serde::Serialize)]
pub struct DebugState {
    /// Page/app identifier
    pub page_id: String,
    /// App name
    pub app_name: String,
    /// User's DID
    pub user_did: String,
    /// User's role
    pub user_role: String,
    /// Registered page:on_change handler patterns
    pub page_handler_patterns: Vec<String>,
    /// Global Lua variable names (excluding builtins)
    pub lua_globals: Vec<String>,
}

/// UI Bindings for Lua - allows setting properties, emitting events, subscribing
///
/// **Usage in Lua**:
/// - `ui:set("property_name", value)` - set property
/// - `ui:get("property_name")` - get property value
/// - `ui:subscribe("event_type", options, callback)` - subscribe to events
/// - `ui:emit("event_type", data)` - emit an event
///
/// **Threading**: Holds sender, sends mutations to Slint thread
struct UiBindings {
    /// App identifier for mutations
    app_id: String,
    /// Channel sender for UI mutations (wrapped for thread safety)
    ui_tx: Arc<Mutex<mpsc::Sender<UiMutation>>>,
    /// Channel sender for UI queries (for ui:get)
    query_tx: Arc<Mutex<mpsc::Sender<UiQuery>>>,
    /// Event Bus for subscribe/emit
    event_bus: Arc<Mutex<EventBus>>,
    /// Lua registry keys for event callbacks (subscriber_id -> registry_key)
    /// Stored separately to access in dispatch
    callback_keys: Arc<Mutex<std::collections::HashMap<u64, RegistryKey>>>,
    /// Pending property updates (accumulated during Lua callback, flushed at end)
    pending_properties: Arc<Mutex<Vec<PropertyUpdate>>>,
    /// Pending model operations (accumulated during Lua callback, flushed at end)
    pending_model_ops: Arc<Mutex<Vec<VecModelOp>>>,
}

impl UserData for UiBindings {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        // ui:set(key, value) - Set a property or replace model data
        // Note: Slint uses kebab-case for property names
        // Mutations are accumulated and flushed at end of Lua callback (batch mode)
        methods.add_method("set", |_lua, this, (key, value): (String, LuaValue)| {
            // Use property name as-is (Slint expects kebab-case)
            let prop_name = key;

            let json_value = lua_to_json(&value)
                .map_err(|e| LuaError::RuntimeError(e.to_string()))?;

            // Check if value is an array (for VecModel operations)
            if let JsonValue::Array(items) = json_value {
                // Array value: Clear model, then push all items
                let mut ops = vec![VecModelOp::Clear { model_name: prop_name.clone() }];
                for item in items {
                    ops.push(VecModelOp::Push {
                        model_name: prop_name.clone(),
                        item,
                    });
                }
                // Accumulate model ops for batch flush
                this.pending_model_ops.lock().extend(ops);
            } else {
                // Scalar value: Accumulate PropertyUpdate for batch flush
                this.pending_properties.lock().push(PropertyUpdate { key: prop_name, value: json_value });
            }

            Ok(())
        });

        // ui:push(model_name, item) - Add item to end of model
        // Mutations are accumulated and flushed at end of Lua callback (batch mode)
        methods.add_method("push", |_lua, this, (model_name, item): (String, LuaValue)| {
            let item_json = lua_to_json(&item)
                .map_err(|e| LuaError::RuntimeError(e.to_string()))?;

            // Accumulate model op for batch flush
            this.pending_model_ops.lock().push(VecModelOp::Push {
                model_name,
                item: item_json,
            });

            Ok(())
        });

        // ui:insert(model_name, index, item) - Insert item at index
        // Mutations are accumulated and flushed at end of Lua callback (batch mode)
        methods.add_method("insert", |_lua, this, (model_name, index, item): (String, usize, LuaValue)| {
            let item_json = lua_to_json(&item)
                .map_err(|e| LuaError::RuntimeError(e.to_string()))?;

            // Accumulate model op for batch flush
            this.pending_model_ops.lock().push(VecModelOp::Insert {
                model_name,
                index,
                item: item_json,
            });

            Ok(())
        });

        // ui:remove(model_name, index) - Remove item at index
        // Mutations are accumulated and flushed at end of Lua callback (batch mode)
        methods.add_method("remove", |_lua, this, (model_name, index): (String, usize)| {
            // Accumulate model op for batch flush
            this.pending_model_ops.lock().push(VecModelOp::Remove {
                model_name,
                index,
            });

            Ok(())
        });

        // ui:clear(model_name) - Remove all items from model
        // Mutations are accumulated and flushed at end of Lua callback (batch mode)
        methods.add_method("clear", |_lua, this, model_name: String| {
            // Accumulate model op for batch flush
            this.pending_model_ops.lock().push(VecModelOp::Clear { model_name });

            Ok(())
        });

        // ui:update(model_name, index, item) - Update single item at index (efficient for drag)
        // Triggers ModelNotify::row_changed(index) - only re-renders that one item
        // Mutations are accumulated and flushed at end of Lua callback (batch mode)
        methods.add_method("update", |_lua, this, (model_name, index, item): (String, usize, LuaValue)| {
            let item_json = lua_to_json(&item)
                .map_err(|e| LuaError::RuntimeError(e.to_string()))?;

            // Accumulate model op for batch flush
            this.pending_model_ops.lock().push(VecModelOp::Set {
                model_name,
                index,
                item: item_json,
            });

            Ok(())
        });

        // ui:get(key) -> value - Read a property from Slint
        methods.add_method("get", |lua, this, key: String| {
            // Create oneshot channel for response
            let (response_tx, response_rx) = oneshot::channel();

            let query = UiQuery {
                prop_name: key.clone(),
                response_tx,
            };

            // Send query to Slint thread
            if let Err(e) = this.query_tx.lock().try_send(query) {
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
                        _ => return Err(LuaError::RuntimeError("event_type must be string".into())),
                    };
                    let callback = match &args_vec[1] {
                        LuaValue::Function(f) => f.clone(),
                        _ => return Err(LuaError::RuntimeError("callback must be function".into())),
                    };
                    (event_type, SubscribeOptions::default(), callback)
                }
                3 => {
                    let event_type: String = match &args_vec[0] {
                        LuaValue::String(s) => s.to_str()?.to_string(),
                        _ => return Err(LuaError::RuntimeError("event_type must be string".into())),
                    };
                    let options = parse_subscribe_options(&args_vec[1])?;
                    let callback = match &args_vec[2] {
                        LuaValue::Function(f) => f.clone(),
                        _ => return Err(LuaError::RuntimeError("callback must be function".into())),
                    };
                    (event_type, options, callback)
                }
                _ => return Err(LuaError::RuntimeError(
                    "subscribe takes 2 or 3 arguments: (event_type, [options], callback)".into()
                )),
            };

            // Store callback in Lua registry
            let registry_key = lua.create_registry_value(callback)
                .map_err(|e| LuaError::RuntimeError(format!("Failed to store callback: {}", e)))?;

            // Use subscriber_id as the callback_key in event bus (we'll look up RegistryKey by subscriber_id)
            // We don't need to extract a raw key since we'll use the subscriber_id for lookup
            let callback_key_usize = 0_usize; // Placeholder, actual lookup uses subscriber_id

            // Subscribe to event bus
            let subscriber_id = this.event_bus.lock().subscribe(
                &event_type,
                options,
                callback_key_usize,
            );

            // Store registry key for later lookup
            this.callback_keys.lock().insert(subscriber_id, registry_key);

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

            // Remove from event bus
            let removed = this.event_bus.lock().unsubscribe(id);

            // Remove callback key
            if let Some(key) = this.callback_keys.lock().remove(&id) {
                drop(key); // Let Lua GC the callback
            }

            Ok(removed)
        });

        // ui:emit(event_type, data) - Emit an event (for AI/test automation)
        methods.add_method("emit", |_lua, this, (event_type, data): (String, LuaValue)| {
            let data_json = lua_to_json(&data)
                .map_err(|e| LuaError::RuntimeError(e.to_string()))?;

            // Extract target from data if present
            let target = if let JsonValue::Object(ref obj) = data_json {
                obj.get("target").and_then(|v| v.as_str()).map(|s| s.to_string())
            } else {
                None
            };

            let mut event = Event::new(&event_type, EventSource::Ai);
            if let Some(t) = target {
                event = event.with_target(t);
            }
            event = event.with_data(data_json);

            // Emit to event bus - returns deliveries to process
            let deliveries = this.event_bus.lock().emit(event.clone());

            tracing::debug!(
                app_id = %this.app_id,
                event_type = %event_type,
                deliveries = deliveries.len(),
                "Emitted UI event"
            );

            // Note: The actual callback dispatch happens in LuaWorker::handle_ui_event
            // For emit(), we dispatch immediately since we're already in Lua context
            // This is handled by the caller (they need to call dispatch_event_deliveries)

            // Return the number of subscribers notified
            Ok(deliveries.len() as i64)
        });
    }
}

/// Emoji bindings for Lua - lookup emojis by shortcode
///
/// **Usage in Lua**:
/// - `emoji:get("smile")` → "😄" (returns Unicode emoji string)
/// - `emoji:name("😄")` → "grinning face with smiling eyes"
///
/// **Note**: Requires emoji-capable fonts installed on system
struct EmojiBindings;

impl UserData for EmojiBindings {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        // emoji:get(shortcode) -> Unicode emoji string or nil
        methods.add_method("get", |_lua, _this, shortcode: String| {
            match emojis::get_by_shortcode(&shortcode) {
                Some(emoji) => Ok(LuaValue::String(_lua.create_string(emoji.as_str())?)),
                None => Ok(LuaValue::Nil),
            }
        });

        // emoji:name(unicode) -> emoji name string or nil
        methods.add_method("name", |_lua, _this, unicode: String| {
            match emojis::get(&unicode) {
                Some(emoji) => Ok(LuaValue::String(_lua.create_string(emoji.name())?)),
                None => Ok(LuaValue::Nil),
            }
        });

        // emoji:search(query) -> array of matching emoji objects
        // Returns up to 10 matches: [{ emoji = "😄", name = "...", shortcode = "..." }, ...]
        methods.add_method("search", |lua, _this, query: String| {
            let table = lua.create_table()?;
            let mut count = 0;

            for emoji in emojis::iter() {
                if count >= 10 {
                    break;
                }

                let name_matches = emoji.name().to_lowercase().contains(&query.to_lowercase());
                let shortcode_matches = emoji.shortcode()
                    .map(|s| s.to_lowercase().contains(&query.to_lowercase()))
                    .unwrap_or(false);

                if name_matches || shortcode_matches {
                    let entry = lua.create_table()?;
                    entry.set("emoji", emoji.as_str())?;
                    entry.set("name", emoji.name())?;
                    if let Some(shortcode) = emoji.shortcode() {
                        entry.set("shortcode", shortcode)?;
                    }
                    count += 1;
                    table.set(count, entry)?;
                }
            }

            Ok(LuaValue::Table(table))
        });
    }
}

/// Convert Event to Lua table
fn event_to_lua(lua: &Lua, event: &Event) -> Result<LuaValue, LuaError> {
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
fn parse_subscribe_options(value: &LuaValue) -> Result<SubscribeOptions, LuaError> {
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
        _ => Err(LuaError::RuntimeError("options must be table or nil".into())),
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

    /// User's display name (from signup)
    user_name: String,

    /// User's role from permit (e.g., "owner", "viewer")
    user_role: String,

    /// Channel to send UI mutations to Slint thread (wrapped for sharing with UiBindings)
    ui_tx: Arc<Mutex<mpsc::Sender<UiMutation>>>,

    /// Channel to send UI queries to Slint thread (for ui:get)
    query_tx: Arc<Mutex<mpsc::Sender<UiQuery>>>,

    /// Reference to Scribe actor (for Loro queries)
    scribe_ref: ActorRef<ScribeMessage>,

    /// Channel to receive commands from Slint thread
    cmd_rx: mpsc::Receiver<LuaWorkerCommand>,

    /// Registered page:on_change() handlers (shared with PageBindings)
    page_handlers: Arc<std::sync::RwLock<Vec<PageHandler>>>,

    /// Event Bus for UI event dispatching (shared with UiBindings)
    event_bus: Arc<Mutex<EventBus>>,

    /// Callback registry keys for event subscribers (subscriber_id -> registry_key)
    callback_keys: Arc<Mutex<std::collections::HashMap<u64, RegistryKey>>>,

    /// Pending property updates (accumulated during Lua callback, flushed at end)
    /// Shared with UiBindings for batching
    pending_properties: Arc<Mutex<Vec<PropertyUpdate>>>,

    /// Pending model operations (accumulated during Lua callback, flushed at end)
    /// Shared with UiBindings for batching
    pending_model_ops: Arc<Mutex<Vec<VecModelOp>>>,
}

impl LuaWorker {
    /// Spawn Lua worker thread
    ///
    /// **Parameters**:
    /// - `page_id`: Page UUID (e.g., "bf57890c-70ce-46e3-b205-f419163ab0f4")
    /// - `app_name`: App name (e.g., "Shop Customer") - for logging
    /// - `user_did`: User's DID (e.g., "did:key:z6Mk...") - for permit bindings
    /// - `user_name`: User's display name (from signup) - for permit bindings
    /// - `user_role`: User's role from permit (e.g., "owner", "viewer") - for permit bindings
    /// - `lua_code`: Lua source code to execute
    /// - `scribe_ref`: Reference to Scribe actor
    /// - `ui_tx`: Channel to send UI mutations
    /// - `query_tx`: Channel to send UI property queries
    ///
    /// **Returns**: (thread handle, command sender)
    /// **Thread**: Blocks on event loop until Shutdown
    pub fn spawn(
        page_id: String,
        app_name: String,
        user_did: String,
        user_name: String,
        user_role: String,
        lua_code: String,
        scribe_ref: ActorRef<ScribeMessage>,
        ui_tx: mpsc::Sender<UiMutation>,
        query_tx: mpsc::Sender<UiQuery>,
    ) -> Result<(std::thread::JoinHandle<()>, mpsc::Sender<LuaWorkerCommand>), Box<dyn std::error::Error>> {
        let (cmd_tx, cmd_rx) = mpsc::channel(32);

        let handle = std::thread::spawn(move || {
            // Create Lua VM on this thread (owned, not shared)
            let lua = Lua::new();

            // Load api module FIRST (before app code, which uses api.export())
            let api_lib = include_str!("lua_libs/api.lua");
            match lua.load(api_lib).eval::<mlua::Value>() {
                Ok(api_module) => {
                    if let Err(e) = lua.globals().set("api", api_module) {
                        tracing::error!(
                            page_id = %page_id,
                            app_name = %app_name,
                            error = %e,
                            "Failed to set api global"
                        );
                        return;
                    }
                }
                Err(e) => {
                    tracing::error!(
                        page_id = %page_id,
                        app_name = %app_name,
                        error = %e,
                        "Failed to load api module"
                    );
                    return;
                }
            }

            // Load app code (now has access to api module)
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
            let query_tx = Arc::new(Mutex::new(query_tx));

            // Create shared page handlers (shared between PageBindings and LuaWorker)
            let page_handlers = Arc::new(std::sync::RwLock::new(Vec::new()));

            // Create Event Bus and callback keys (shared with UiBindings)
            let event_bus = Arc::new(Mutex::new(EventBus::new()));
            let callback_keys = Arc::new(Mutex::new(std::collections::HashMap::new()));

            // Create pending mutation buffers (shared with UiBindings for batching)
            let pending_properties = Arc::new(Mutex::new(Vec::new()));
            let pending_model_ops = Arc::new(Mutex::new(Vec::new()));

            let mut worker = LuaWorker {
                lua,
                page_id,
                app_name,
                user_did,
                user_name,
                user_role,
                ui_tx,
                query_tx,
                scribe_ref,
                cmd_rx,
                page_handlers,
                event_bus,
                callback_keys,
                pending_properties,
                pending_model_ops,
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
                // Flush any UI mutations from on_init()
                worker.flush_mutations();
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
                    // Flush accumulated UI mutations after Lua callback completes
                    self.flush_mutations();
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
                    // Flush accumulated UI mutations after Lua callback completes
                    self.flush_mutations();
                }
                Some(LuaWorkerCommand::UiCallback { callback_name, args }) => {
                    if let Err(e) = self.handle_ui_callback(callback_name, args) {
                        tracing::error!(
                            page_id = %self.page_id,
                            error = %e,
                            "UI callback handler failed"
                        );
                    }
                    // Flush accumulated UI mutations after Lua callback completes
                    self.flush_mutations();
                }
                Some(LuaWorkerCommand::UiEvent { event }) => {
                    if let Err(e) = self.handle_ui_event(event) {
                        tracing::error!(
                            page_id = %self.page_id,
                            error = %e,
                            "UI event handler failed"
                        );
                    }
                    // Flush accumulated UI mutations after Lua callback completes
                    self.flush_mutations();
                }
                Some(LuaWorkerCommand::Shutdown) => {
                    tracing::info!(
                        page_id = %self.page_id,
                        "Lua worker shutting down"
                    );
                    break;
                }
                Some(LuaWorkerCommand::DebugEval { code, response_tx }) => {
                    let result = self.handle_debug_eval(&code);
                    // Flush accumulated UI mutations after debug eval completes
                    // This is critical for automation via debug socket
                    self.flush_mutations();
                    let _ = response_tx.send(result);
                }
                Some(LuaWorkerCommand::DebugGetState { response_tx }) => {
                    let state = self.get_debug_state();
                    let _ = response_tx.send(state);
                }

                // ==================== Game Tick ====================

                Some(LuaWorkerCommand::Tick) => {
                    self.handle_tick();
                    self.flush_mutations();
                }

                // ==================== Ephemeral Events ====================

                Some(LuaWorkerCommand::Ephemeral { user_did, payload }) => {
                    self.handle_ephemeral(&user_did, &payload);
                    // Flush accumulated UI mutations after ephemeral handler completes
                    self.flush_mutations();
                }

                Some(LuaWorkerCommand::PeerJoined { user_did }) => {
                    self.handle_peer_joined(&user_did);
                    // Flush accumulated UI mutations after peer presence handler completes
                    self.flush_mutations();
                }

                Some(LuaWorkerCommand::PeerLeft { user_did }) => {
                    self.handle_peer_left(&user_did);
                    // Flush accumulated UI mutations after peer presence handler completes
                    self.flush_mutations();
                }

                Some(LuaWorkerCommand::AssetUploaded { hash, filename, mime_type, size }) => {
                    self.handle_asset_uploaded(&hash, &filename, &mime_type, size);
                    // Flush accumulated UI mutations after asset handler completes
                    self.flush_mutations();
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
        // Shares pending buffers for batch mode (mutations accumulated during callback, flushed at end)
        let ui_binding = UiBindings {
            app_id: self.page_id.clone(),
            ui_tx: self.ui_tx.clone(),
            query_tx: self.query_tx.clone(),
            event_bus: self.event_bus.clone(),
            callback_keys: self.callback_keys.clone(),
            pending_properties: self.pending_properties.clone(),
            pending_model_ops: self.pending_model_ops.clone(),
        };
        globals.set("ui", ui_binding)?;

        // permit binding - provides identity and layer naming helpers
        // Uses real page_id (UUID), user_did, user_name, and role from permit
        let permit_binding = PermitBindings::new(
            self.page_id.clone(),
            self.user_did.clone(),
            self.user_name.clone(),
            self.user_role.clone(),
        );
        globals.set("permit", permit_binding)?;

        // page binding - provides page:on_change() for registering layer event handlers
        let page_binding = PageBindings {
            handlers: self.page_handlers.clone(),
            page_id: self.page_id.clone(),
        };
        globals.set("page", page_binding)?;

        // datetime binding - LuaDate library for date/time operations
        // Load the LuaDate library and expose as 'datetime' global
        // Note: LuaDate returns the module (doesn't set global), so we use eval()
        let date_lib = include_str!("lua_libs/date.lua");
        let datetime: mlua::Value = self.lua.load(date_lib).eval()?;
        globals.set("datetime", datetime)?;

        // layout binding - provides graph layout algorithms for canvas apps
        let layout_binding = LayoutBindings::new();
        globals.set("layout", layout_binding)?;

        // emoji binding - lookup emojis by shortcode
        let emoji_binding = EmojiBindings;
        globals.set("emoji", emoji_binding)?;

        // Note: api module is loaded earlier in spawn() before app code loads
        // (because app code uses api.export() at load time)

        tracing::info!(
            page_id = %self.page_id,
            user_did = %self.user_did,
            user_role = %self.user_role,
            app_name = %self.app_name,
            "Lua globals initialized (loro, butler, ui, permit, page, datetime, layout, emoji, api)"
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

    /// Handle UI event from Event Bus
    ///
    /// **Context**: Event from Slint or AI emit, dispatch to Lua subscribers
    fn handle_ui_event(
        &self,
        event: Event,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Emit event to bus and get deliveries
        let deliveries = self.event_bus.lock().emit(event.clone());

        tracing::debug!(
            page_id = %self.page_id,
            event_type = %event.event_type,
            deliveries = deliveries.len(),
            "Processing UI event"
        );

        // Dispatch to each subscriber
        for (subscriber_id, _callback_key, delivery) in deliveries {
            // Look up registry key by subscriber_id
            let callback_keys = self.callback_keys.lock();
            if let Some(registry_key) = callback_keys.get(&subscriber_id) {
                // Get callback from registry
                let callback: mlua::Function = match self.lua.registry_value(registry_key) {
                    Ok(cb) => cb,
                    Err(e) => {
                        tracing::warn!(
                            page_id = %self.page_id,
                            subscriber_id = subscriber_id,
                            error = %e,
                            "Failed to get callback from registry"
                        );
                        continue;
                    }
                };

                // Convert event(s) to Lua and call
                match delivery {
                    EventDelivery::Single(evt) => {
                        let event_lua = event_to_lua(&self.lua, &evt)?;
                        if let Err(e) = callback.call::<()>(event_lua) {
                            tracing::warn!(
                                page_id = %self.page_id,
                                subscriber_id = subscriber_id,
                                error = %e,
                                "Event handler failed"
                            );
                        }
                    }
                    EventDelivery::Batch(events) => {
                        let events_lua = self.lua.create_table()?;
                        for (i, evt) in events.iter().enumerate() {
                            let event_lua = event_to_lua(&self.lua, evt)?;
                            events_lua.set(i + 1, event_lua)?;
                        }
                        if let Err(e) = callback.call::<()>(events_lua) {
                            tracing::warn!(
                                page_id = %self.page_id,
                                subscriber_id = subscriber_id,
                                error = %e,
                                "Batch event handler failed"
                            );
                        }
                    }
                }
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
        tracing::trace!(
            page_id = %self.page_id,
            callback = %callback_name,
            "Looking up Lua callback"
        );

        // Get callback function from Lua (optional - not all callbacks need Lua handlers)
        let callback: mlua::Function = match self.lua.globals().get(callback_name.as_str()) {
            Ok(func) => {
                tracing::trace!(
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

    /// Flush accumulated UI mutations to Slint thread
    ///
    /// **Context**: Called after Lua code execution completes (callbacks, debug eval, etc.)
    /// **Purpose**: Batches all ui:set(), ui:push(), etc. calls into a single UiMutation
    /// **Benefit**: Prevents Slint recursion detection when multiple properties change together
    fn flush_mutations(&self) {
        // Take all pending mutations (swap with empty vecs)
        let properties = std::mem::take(&mut *self.pending_properties.lock());
        let model_ops = std::mem::take(&mut *self.pending_model_ops.lock());

        // Only send if there are actual mutations
        if properties.is_empty() && model_ops.is_empty() {
            return;
        }

        let mutation = UiMutation {
            app_id: self.page_id.clone(),
            properties,
            model_ops,
        };

        tracing::debug!(
            page_id = %self.page_id,
            prop_count = mutation.properties.len(),
            model_op_count = mutation.model_ops.len(),
            "Flushing batched UI mutations"
        );

        if let Err(e) = self.ui_tx.lock().try_send(mutation) {
            tracing::warn!(
                page_id = %self.page_id,
                error = %e,
                "Failed to send batched UI mutation"
            );
        }
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

    /// Debug: Execute arbitrary Lua code and return JSON result
    ///
    /// **Context**: Called from debug server for introspection
    /// **Security**: Only enabled when debug server is active
    fn handle_debug_eval(&self, code: &str) -> Result<JsonValue, String> {
        tracing::debug!(
            page_id = %self.page_id,
            code_len = code.len(),
            "Executing debug eval"
        );

        // Execute the Lua code
        let result: LuaValue = self.lua.load(code)
            .eval()
            .map_err(|e| format!("Lua error: {}", e))?;

        // Convert result to JSON
        lua_to_json(&result)
            .map_err(|e| format!("JSON conversion error: {}", e))
    }

    /// Debug: Get current state snapshot
    ///
    /// **Context**: Called from debug server for state inspection
    fn get_debug_state(&self) -> DebugState {
        // Collect page handler patterns
        let page_handler_patterns = self.page_handlers
            .read()
            .map(|handlers| handlers.iter().map(|h| h.pattern.clone()).collect())
            .unwrap_or_default();

        // Collect Lua global names (excluding builtins)
        let lua_globals = self.collect_lua_globals();

        DebugState {
            page_id: self.page_id.clone(),
            app_name: self.app_name.clone(),
            user_did: self.user_did.clone(),
            user_role: self.user_role.clone(),
            page_handler_patterns,
            lua_globals,
        }
    }

    /// Collect non-builtin Lua global variable names
    fn collect_lua_globals(&self) -> Vec<String> {
        let builtins = [
            "_G", "_VERSION", "assert", "collectgarbage", "dofile", "error",
            "getmetatable", "ipairs", "load", "loadfile", "next", "pairs",
            "pcall", "print", "rawequal", "rawget", "rawlen", "rawset",
            "require", "select", "setmetatable", "tonumber", "tostring",
            "type", "warn", "xpcall", "coroutine", "debug", "io", "math",
            "os", "package", "string", "table", "utf8",
            // Our bindings (also skip these)
            "loro", "butler", "ui", "permit", "page", "date", "datetime",
        ];

        let mut globals = Vec::new();

        if let Ok(g) = self.lua.globals().pairs::<String, LuaValue>().collect::<Result<Vec<_>, _>>() {
            for (name, _) in g {
                if !builtins.contains(&name.as_str()) {
                    globals.push(name);
                }
            }
        }

        globals.sort();
        globals
    }

    // ==================== Game Tick Handler ====================

    /// Handle game tick
    ///
    /// **Context**: Timer fires periodically for games/animations
    /// **Calls**: Lua `tick()` if defined
    fn handle_tick(&self) {
        if let Ok(func) = self.lua.globals().get::<mlua::Function>("tick") {
            if let Err(e) = func.call::<()>(()) {
                tracing::warn!(
                    page_id = %self.page_id,
                    error = %e,
                    "tick() callback failed"
                );
            }
        }
        // Silently ignore if tick() not defined - not all apps need game loop
    }

    // ==================== Ephemeral Event Handlers ====================

    /// Handle ephemeral data from peer (generic)
    ///
    /// **Context**: Peer sent ephemeral data (cursor, typing, etc.) via datagram
    /// **Calls**: Lua `on_ephemeral(user_did, payload_string)` if defined
    /// **Design**: Payload is passed as string - Lua parses as JSON with type/x/y, etc.
    fn handle_ephemeral(&self, user_did: &str, payload: &[u8]) {
        // Convert payload bytes to string (UTF-8 expected)
        let payload_str = match String::from_utf8(payload.to_vec()) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(
                    page_id = %self.page_id,
                    user_did = %user_did,
                    error = %e,
                    "Invalid UTF-8 in ephemeral payload"
                );
                return;
            }
        };

        if let Ok(func) = self.lua.globals().get::<mlua::Function>("on_ephemeral") {
            if let Err(e) = func.call::<()>((user_did.to_string(), payload_str)) {
                tracing::warn!(
                    page_id = %self.page_id,
                    user_did = %user_did,
                    error = %e,
                    "on_ephemeral callback failed"
                );
            }
        }
        // Silently ignore if callback not defined - not all apps need ephemeral data
    }

    // ==================== Peer Presence Handlers ====================

    /// Handle peer joined event
    ///
    /// **Context**: Remote peer subscribed to this page's Scribe
    /// **Calls**: Lua `on_peer_joined(user_did)` if defined
    /// **Use cases**: Online counters, presence indicators
    fn handle_peer_joined(&self, user_did: &str) {
        if let Ok(func) = self.lua.globals().get::<mlua::Function>("on_peer_joined") {
            if let Err(e) = func.call::<()>(user_did.to_string()) {
                tracing::warn!(
                    page_id = %self.page_id,
                    user_did = %user_did,
                    error = %e,
                    "on_peer_joined callback failed"
                );
            } else {
                tracing::info!(
                    page_id = %self.page_id,
                    user_did = %user_did,
                    "on_peer_joined callback executed"
                );
            }
        }
        // Silently ignore if callback not defined - not all apps need presence tracking
    }

    /// Handle peer left event
    ///
    /// **Context**: Remote peer unsubscribed from this page's Scribe
    /// **Calls**: Lua `on_peer_left(user_did)` if defined
    /// **Use cases**: Online counters, presence indicators
    fn handle_peer_left(&self, user_did: &str) {
        if let Ok(func) = self.lua.globals().get::<mlua::Function>("on_peer_left") {
            if let Err(e) = func.call::<()>(user_did.to_string()) {
                tracing::warn!(
                    page_id = %self.page_id,
                    user_did = %user_did,
                    error = %e,
                    "on_peer_left callback failed"
                );
            } else {
                tracing::info!(
                    page_id = %self.page_id,
                    user_did = %user_did,
                    "on_peer_left callback executed"
                );
            }
        }
        // Silently ignore if callback not defined - not all apps need presence tracking
    }

    // ==================== Asset Upload Handlers ====================

    /// Handle asset uploaded event
    ///
    /// **Context**: User selected a file via native picker, it was uploaded
    /// **Calls**: Lua `on_asset_uploaded(asset)` if defined
    /// **Lua receives**: { hash, filename, mime_type, size }
    fn handle_asset_uploaded(&self, hash: &str, filename: &str, mime_type: &str, size: u64) {
        if let Ok(func) = self.lua.globals().get::<mlua::Function>("on_asset_uploaded") {
            // Create asset table for Lua
            match self.lua.create_table() {
                Ok(table) => {
                    if let Err(e) = table.set("hash", hash) {
                        tracing::warn!(page_id = %self.page_id, error = %e, "Failed to set hash");
                        return;
                    }
                    if let Err(e) = table.set("filename", filename) {
                        tracing::warn!(page_id = %self.page_id, error = %e, "Failed to set filename");
                        return;
                    }
                    if let Err(e) = table.set("mime_type", mime_type) {
                        tracing::warn!(page_id = %self.page_id, error = %e, "Failed to set mime_type");
                        return;
                    }
                    if let Err(e) = table.set("size", size) {
                        tracing::warn!(page_id = %self.page_id, error = %e, "Failed to set size");
                        return;
                    }

                    if let Err(e) = func.call::<()>(table) {
                        tracing::warn!(
                            page_id = %self.page_id,
                            hash = %hash,
                            filename = %filename,
                            error = %e,
                            "on_asset_uploaded callback failed"
                        );
                    } else {
                        tracing::info!(
                            page_id = %self.page_id,
                            hash = %hash,
                            filename = %filename,
                            "on_asset_uploaded callback executed"
                        );
                    }
                }
                Err(e) => {
                    tracing::error!(
                        page_id = %self.page_id,
                        error = %e,
                        "Failed to create Lua table for asset"
                    );
                }
            }
        } else {
            tracing::debug!(
                page_id = %self.page_id,
                hash = %hash,
                filename = %filename,
                "on_asset_uploaded not defined in Lua (asset uploaded but not handled)"
            );
        }
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
