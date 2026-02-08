//! Unified Scribe Bindings for Lua
//!
//! Provides a single `scribe` binding that replaces multiple separate bindings (loro, permit, peers).
//! Apps interact with Scribe via this unified API for identity, layers, contacts, and ephemerals.
//!
//! ## API Reference
//!
//! ### Identity
//! - `scribe:my_did()` - Get our DID
//! - `scribe:my_name()` - Get our display name (from permit presence)
//! - `scribe:page_id()` - Get current page ID
//!
//! ### Layers (CRDT)
//! - `scribe:list(name)` - Get or create a list layer
//! - `scribe:map(name)` - Get or create a map layer
//!
//! ### Bindings (Declarative Layer → UI Sync)
//! - `scribe:bind(ui_property, layer_pattern, options?)` - Auto-sync layer to UI
//!
//! ### Contacts
//! - `scribe:get_contacts()` - Get contents of /users layer (persistent contacts)
//!
//! ### Ephemerals
//! - `scribe:send(func, args)` - Send structured ephemeral (permission-validated)

use mlua::{Error as LuaError, Function, Lua, Result as LuaResult, Table, UserData, UserDataMethods, Value as LuaValue};
use parking_lot::Mutex;
use ractor::ActorRef;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};
use tracing::{debug, info, warn};

use scribe::ScribeMessage;

use super::binding::{
    expand_pattern, is_wildcard_pattern, process_binding_data, BindingManager, BindingOptions,
};
use super::block_on_async;
use super::convert::lua_to_json;
use super::loro::{LuaLoroList, LuaLoroMap};
use crate::ui_types::UiMutation;

/// Unified Scribe bindings for Lua apps
///
/// Provides identity, layers, contacts, binding, and ephemeral functionality.
pub struct ScribeBindings {
    scribe_ref: ActorRef<ScribeMessage>,
    page_id: String,
    our_did: String,
    our_name: Option<String>,
    /// Binding manager for declarative layer → UI sync
    binding_manager: Arc<Mutex<BindingManager>>,
    /// UI mutation sender (if UI enabled)
    ui_tx: Option<mpsc::Sender<UiMutation>>,
    /// Whether UI is enabled
    ui_enabled: bool,
}

impl ScribeBindings {
    /// Create new ScribeBindings (no UI)
    pub fn new(
        scribe_ref: ActorRef<ScribeMessage>,
        page_id: String,
        our_did: String,
        our_name: Option<String>,
    ) -> Self {
        Self {
            scribe_ref,
            page_id,
            our_did,
            our_name,
            binding_manager: Arc::new(Mutex::new(BindingManager::new())),
            ui_tx: None,
            ui_enabled: false,
        }
    }

    /// Create ScribeBindings with UI support
    pub fn with_ui(
        scribe_ref: ActorRef<ScribeMessage>,
        page_id: String,
        our_did: String,
        our_name: Option<String>,
        ui_tx: mpsc::Sender<UiMutation>,
    ) -> Self {
        Self {
            scribe_ref,
            page_id,
            our_did,
            our_name,
            binding_manager: Arc::new(Mutex::new(BindingManager::new())),
            ui_tx: Some(ui_tx),
            ui_enabled: true,
        }
    }

    /// Get reference to the binding manager
    pub fn binding_manager(&self) -> Arc<Mutex<BindingManager>> {
        self.binding_manager.clone()
    }
}

impl UserData for ScribeBindings {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {

        // Get our DID
        //
        // Returns the DID (Decentralized Identifier) of the current user.
        // This is the unique identifier used in permit patterns and layer names.
        methods.add_method("my_did", |_, this, ()| Ok(this.our_did.clone()));

        // Get the current page ID
        //
        // Returns the page ID this scribe instance is associated with.
        methods.add_method("page_id", |_, this, ()| Ok(this.page_id.clone()));

        // Get our display name
        //
        // Returns the display name if available, or nil.
        methods.add_method("my_name", |_, this, ()| {
            Ok(this.our_name.clone())
        });

        // Send structured ephemeral data
        //
        // Broadcasts ephemeral (non-persistent) data to all peers.
        // Used for typing indicators, cursor positions, game state, etc.
        methods.add_method("send", |_, this, (func, args): (String, LuaValue)| {
            let json_args = lua_to_json(&args)?;

            let payload = serde_json::json!({
                "func": func,
                "args": json_args,
            });

            let payload_bytes = serde_json::to_vec(&payload)
                .map_err(|e| LuaError::RuntimeError(format!("Failed to serialize ephemeral: {}", e)))?;

            this.scribe_ref
                .cast(ScribeMessage::SendEphemeral {
                    payload: payload_bytes,
                })
                .map_err(|e| LuaError::RuntimeError(format!("Failed to send ephemeral: {}", e)))?;

            Ok(())
        });

        // Get or create a list layer
        //
        // Returns a handle to the specified list layer, creating it if it doesn't exist.
        // The layer name can include the page_id, e.g., `scribe:list(page_id .. "/messages")`
        methods.add_method("list", |_, this, name: String| {
            let (tx, rx) = oneshot::channel();
            this.scribe_ref
                .cast(ScribeMessage::EnsureLoroList {
                    layer_name: name.clone(),
                    reply: tx,
                })
                .map_err(|e| LuaError::RuntimeError(format!("Failed to ensure list: {}", e)))?;

            block_on_async(async {
                rx.await
                    .map_err(|e| LuaError::RuntimeError(format!("Channel error: {}", e)))?
                    .map_err(|e| LuaError::RuntimeError(format!("Scribe error: {}", e)))
            })??;

            Ok(LuaLoroList::new(this.scribe_ref.clone(), name))
        });

        // Get or create a map layer
        //
        // Returns a handle to the specified map layer, creating it if it doesn't exist.
        methods.add_method("map", |_, this, name: String| {
            let (tx, rx) = oneshot::channel();
            this.scribe_ref
                .cast(ScribeMessage::EnsureLoroMap {
                    layer_name: name.clone(),
                    reply: tx,
                })
                .map_err(|e| LuaError::RuntimeError(format!("Failed to ensure map: {}", e)))?;

            block_on_async(async {
                rx.await
                    .map_err(|e| LuaError::RuntimeError(format!("Channel error: {}", e)))?
                    .map_err(|e| LuaError::RuntimeError(format!("Scribe error: {}", e)))
            })??;

            Ok(LuaLoroMap::new(this.scribe_ref.clone(), name))
        });

        // List layers matching a pattern
        //
        // Returns a table of layer names matching the glob pattern.
        // Example: scribe:list_layers("*:orders") returns all order layers
        methods.add_method("list_layers", |lua, this, pattern: String| {
            let (tx, rx) = oneshot::channel();
            this.scribe_ref
                .cast(ScribeMessage::ListLayers { pattern, reply: tx })
                .map_err(|e| LuaError::RuntimeError(format!("Failed to list layers: {}", e)))?;

            let names = block_on_async(async {
                rx.await
                    .map_err(|e| LuaError::RuntimeError(format!("Channel error: {}", e)))
            })?;

            let table = lua.create_table()?;
            for (i, name) in names.iter().enumerate() {
                table.set(i + 1, name.clone())?;
            }
            Ok(table)
        });

        // Bind a layer pattern to a UI property (declarative sync)
        //
        // Automatically syncs layer data to UI property whenever the layer changes.
        // Supports transform, sort, and filter options.
        //
        // # Arguments
        // * `ui_property` - Name of the UI property/model to sync to
        // * `layer_pattern` - Layer pattern (page_id auto-prepended)
        // * `options` (optional) - Table with transform, sort, filter functions
        //
        // # Placeholders
        // * `{me}` - Expands to user's DID
        //
        // # Examples
        // ```lua
        // -- Simple binding
        // scribe:bind("products", "products")
        //
        // -- With transform and sort
        // scribe:bind("orders", "derived/orders_summary", {
        //     transform = function(item) return {id=item.id, name=item.name} end,
        //     sort = function(a, b) return a.created_at > b.created_at end
        // })
        //
        // -- {me} placeholder
        // scribe:bind("my_orders", "orders/{me}")
        //
        // -- Wildcard (aggregates matching layers)
        // scribe:bind("all_orders", "orders/*")
        // ```
        methods.add_method(
            "bind",
            |lua, this, (ui_property, layer_pattern, options): (String, String, Option<Table>)| {
                if !this.ui_enabled {
                    debug!(
                        ui_property = %ui_property,
                        layer_pattern = %layer_pattern,
                        "scribe:bind() called but UI not enabled, skipping"
                    );
                    return Ok(());
                }

                // Expand placeholders in pattern
                let expanded_pattern = expand_pattern(&layer_pattern, &this.page_id, &this.our_did);
                let is_wildcard = is_wildcard_pattern(&layer_pattern);

                info!(
                    ui_property = %ui_property,
                    raw_pattern = %layer_pattern,
                    expanded_pattern = %expanded_pattern,
                    is_wildcard = is_wildcard,
                    "scribe:bind() - registering binding"
                );

                // Parse options
                // NOTE: sort is NOT supported - sorting is a Slint view concern
                // This enables surgical updates with stable indices
                let binding_options = if let Some(opts) = options {
                    let transform = opts
                        .get::<Function>("transform")
                        .ok()
                        .map(|f| lua.create_registry_value(f))
                        .transpose()
                        .map_err(|e| LuaError::RuntimeError(format!("Failed to store transform: {}", e)))?;

                    let key = opts.get::<String>("key").ok();
                    let max_items = opts.get::<usize>("max_items").ok();

                    BindingOptions {
                        transform,
                        key,
                        max_items,
                    }
                } else {
                    BindingOptions::default()
                };

                // Register binding
                {
                    let mut manager = this.binding_manager.lock();
                    manager.register(
                        ui_property.clone(),
                        expanded_pattern.clone(),
                        layer_pattern.clone(),
                        is_wildcard,
                        binding_options,
                    );
                }

                // Perform initial sync
                if is_wildcard {
                    // For wildcards, list matching layers and aggregate data
                    let (tx, rx) = oneshot::channel();
                    this.scribe_ref
                        .cast(ScribeMessage::ListLayers {
                            pattern: expanded_pattern.clone(),
                            reply: tx,
                        })
                        .map_err(|e| LuaError::RuntimeError(format!("Failed to list layers: {}", e)))?;

                    let layer_names: Vec<String> = block_on_async(async {
                        rx.await
                            .map_err(|e| LuaError::RuntimeError(format!("Channel error: {}", e)))
                    })??;

                    let mut aggregated_data = Vec::new();
                    for name in &layer_names {
                        if let Ok(data) = fetch_layer_data(&this.scribe_ref, name) {
                            if let serde_json::Value::Array(items) = data {
                                for item in items {
                                    aggregated_data.push(item);
                                }
                            }
                        }
                    }

                    let aggregated_json = serde_json::Value::Array(aggregated_data);
                    sync_binding_to_ui(lua, this, &ui_property, &aggregated_json, None)?;
                } else {
                    // For exact patterns, fetch and sync single layer
                    if let Ok(data) = fetch_layer_data(&this.scribe_ref, &expanded_pattern) {
                        sync_binding_to_ui(lua, this, &ui_property, &data, None)?;
                    }
                }

                Ok(())
            },
        );
    }
}

/// Fetch layer data as JSON from Scribe
fn fetch_layer_data(
    scribe_ref: &ActorRef<ScribeMessage>,
    layer_name: &str,
) -> Result<serde_json::Value, String> {
    let (tx, rx) = oneshot::channel();
    scribe_ref
        .cast(ScribeMessage::GetLayerJson {
            layer_name: layer_name.to_string(),
            reply: tx,
        })
        .map_err(|e| format!("Failed to request layer data: {}", e))?;

    let result = block_on_async(async {
        rx.await
            .map_err(|e| LuaError::RuntimeError(format!("Channel error: {}", e)))
    })
    .map_err(|e| format!("Async error: {}", e))?;

    match result {
        Ok(Some(data)) => Ok(data),
        Ok(None) => Ok(serde_json::Value::Array(vec![])),
        Err(e) => Err(format!("Scribe error: {}", e)),
    }
}

/// Sync binding data to UI after processing through transform/sort/filter
fn sync_binding_to_ui(
    lua: &Lua,
    this: &ScribeBindings,
    ui_property: &str,
    data: &serde_json::Value,
    layer_name: Option<&str>,
) -> LuaResult<()> {
    let manager = this.binding_manager.lock();

    if let Some(binding) = manager.get_binding(ui_property) {
        // Process through binding options
        let processed = process_binding_data(lua, binding, data, layer_name)?;

        // Send to UI (skip if data_to_ui_mutation returns None for empty/null data)
        if let Some(ref ui_tx) = this.ui_tx {
            if let Some(mutation) = super::binding::data_to_ui_mutation(&this.page_id, ui_property, processed) {
                if let Err(e) = ui_tx.try_send(mutation) {
                    warn!(
                        ui_property = %ui_property,
                        error = %e,
                        "Failed to send binding UI mutation"
                    );
                } else {
                    debug!(
                        ui_property = %ui_property,
                        "Binding synced to UI"
                    );
                }
            }
        }
    }

    Ok(())
}

