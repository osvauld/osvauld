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

use mlua::{
    Error as LuaError, Function, Lua, Result as LuaResult, Table, UserData, UserDataMethods,
    Value as LuaValue,
};
use parking_lot::Mutex;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

use crate::scribe_handle::ActorScribeHandle;

use super::binding::{
    expand_pattern, is_wildcard_pattern, process_binding_data, BindingManager, BindingOptions,
};
use super::convert::lua_to_json;
use super::loro::{LuaLoroList, LuaLoroMap};
use crate::ui_types::UiMutation;

/// Unified Scribe bindings for Lua apps
///
/// Provides identity, layers, contacts, binding, and ephemeral functionality.
pub struct ScribeBindings {
    scribe: Arc<ActorScribeHandle>,
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
        scribe: Arc<ActorScribeHandle>,
        page_id: String,
        our_did: String,
        our_name: Option<String>,
    ) -> Self {
        Self {
            scribe,
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
        scribe: Arc<ActorScribeHandle>,
        page_id: String,
        our_did: String,
        our_name: Option<String>,
        ui_tx: mpsc::Sender<UiMutation>,
    ) -> Self {
        Self {
            scribe,
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
        methods.add_method("my_name", |_, this, ()| Ok(this.our_name.clone()));

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

            let payload_bytes = serde_json::to_vec(&payload).map_err(|e| {
                LuaError::RuntimeError(format!("Failed to serialize ephemeral: {}", e))
            })?;

            this.scribe
                .send_ephemeral(payload_bytes)
                .map_err(|e| LuaError::RuntimeError(format!("Failed to send ephemeral: {}", e)))?;

            Ok(())
        });

        // Create a dynamic layer from a schema pattern
        //
        // Creates a dynamic layer matching a schema defined in the permit's dynamic_layer_schemas.
        // The schema_key identifies the pattern (e.g., "channels/{id}/messages"),
        // and layer_id fills the {id} placeholder (e.g., "general").
        //
        // Returns a table with:
        //   - layer_name: The full layer name (e.g., "{page_id}/channels/{did}/general/messages")
        //
        // # Examples
        // ```lua
        // local result = scribe:create_layer("channels/{id}/messages", "general")
        // print(result.layer_name) -- "page1/channels/did:key:alice/general/messages"
        // local messages = scribe:list(result.layer_name)
        // messages:push({text = "Hello!"})
        // ```
        // Add a DID as participant to an explicit dynamic layer
        //
        // Grants access to a specific peer for a dynamic layer with explicit grant type.
        // The peer will receive a layer permit on their next subscription (or immediately if connected).
        //
        // # Examples
        // ```lua
        // scribe:add_layer_access("page1/dms/did:key:alice/dm1/messages", "did:key:bob")
        // ```
        methods.add_method(
            "add_layer_access",
            |_, this, (layer_name, dids): (String, mlua::Value)| {
                // Accept either a single DID string or a table of DIDs
                let dids_vec: Vec<String> = match dids {
                    mlua::Value::String(s) => vec![s
                        .to_str()
                        .map_err(|e| LuaError::RuntimeError(e.to_string()))?
                        .to_string()],
                    mlua::Value::Table(t) => {
                        let mut v = Vec::new();
                        for pair in t.sequence_values::<String>() {
                            v.push(pair.map_err(|e| LuaError::RuntimeError(e.to_string()))?);
                        }
                        v
                    }
                    _ => {
                        return Err(LuaError::RuntimeError(
                            "Expected string or table of DIDs".to_string(),
                        ))
                    }
                };
                this.scribe
                    .add_layer_access(&layer_name, &dids_vec)
                    .map_err(|e| {
                        LuaError::RuntimeError(format!("Failed to add layer access: {}", e))
                    })?;
                Ok(())
            },
        );

        // Remove a DID from an explicit dynamic layer
        //
        // Revokes access for a specific peer from a dynamic layer with explicit grant type.
        //
        // # Examples
        // ```lua
        methods.add_method("create_layer", |lua, this, args: mlua::MultiValue| {
            let mut args_iter = args.into_iter();
            let schema_key: String = match args_iter.next() {
                Some(mlua::Value::String(s)) => s
                    .to_str()
                    .map_err(|e| LuaError::RuntimeError(e.to_string()))?
                    .to_string(),
                _ => {
                    return Err(LuaError::RuntimeError(
                        "Expected schema_key string as first argument".to_string(),
                    ))
                }
            };
            let second_arg = args_iter.next();
            let (placeholders, used_legacy_id): (std::collections::HashMap<String, String>, bool) =
                match second_arg {
                    Some(mlua::Value::String(s)) => {
                        let layer_id = s
                            .to_str()
                            .map_err(|e| LuaError::RuntimeError(e.to_string()))?
                            .to_string();
                        let mut map = std::collections::HashMap::new();
                        map.insert("id".to_string(), layer_id);
                        (map, true)
                    }
                    Some(mlua::Value::Table(t)) => {
                        let mut map = std::collections::HashMap::new();
                        for pair in t.pairs::<String, String>() {
                            let (k, v) = pair.map_err(|e| LuaError::RuntimeError(e.to_string()))?;
                            map.insert(k, v);
                        }
                        if map.is_empty() {
                            return Err(LuaError::RuntimeError(
                                "Expected non-empty placeholders table as second argument"
                                    .to_string(),
                            ));
                        }
                        (map, false)
                    }
                    _ => {
                        return Err(LuaError::RuntimeError(
                            "Expected layer_id string or placeholders table as second argument"
                                .to_string(),
                        ))
                    }
                };

            // Optional third argument: authorized_peers table
            let authorized_arg = args_iter.next();
            let authorized_peers: Option<Vec<String>> = match authorized_arg {
                Some(mlua::Value::Table(t)) => {
                    let mut v = Vec::new();
                    for pair in t.sequence_values::<String>() {
                        v.push(pair.map_err(|e| LuaError::RuntimeError(e.to_string()))?);
                    }
                    Some(v)
                }
                Some(mlua::Value::Nil) | None => None,
                _ => {
                    return Err(LuaError::RuntimeError(
                        "Expected table or nil for authorized_peers".to_string(),
                    ))
                }
            };

            let layer_name = if used_legacy_id {
                let layer_id = placeholders.get("id").cloned().ok_or_else(|| {
                    LuaError::RuntimeError("Missing 'id' placeholder".to_string())
                })?;
                this.scribe
                    .create_layer_with_id(&schema_key, &layer_id, authorized_peers)
                    .map_err(|e| {
                        LuaError::RuntimeError(format!("Failed to create dynamic layer: {}", e))
                    })?
            } else {
                this.scribe
                    .create_layer(&schema_key, placeholders, authorized_peers)
                    .map_err(|e| {
                        LuaError::RuntimeError(format!("Failed to create dynamic layer: {}", e))
                    })?
            };

            let result = lua.create_table()?;
            result.set("layer_name", layer_name)?;
            Ok(result)
        });

        // Get or create a list layer
        //
        // Returns a handle to the specified list layer, creating it if it doesn't exist.
        // The layer name can include the page_id, e.g., `scribe:list(page_id .. "/messages")`
        methods.add_method("list", |_, this, name: String| {
            this.scribe
                .ensure_list(&name)
                .map_err(|e| LuaError::RuntimeError(e))?;
            Ok(LuaLoroList::new(this.scribe.clone(), name))
        });

        // Get or create a map layer
        //
        // Returns a handle to the specified map layer, creating it if it doesn't exist.
        methods.add_method("map", |_, this, name: String| {
            this.scribe
                .ensure_map(&name)
                .map_err(|e| LuaError::RuntimeError(e))?;
            Ok(LuaLoroMap::new(this.scribe.clone(), name))
        });

        // List layers matching a pattern
        //
        // Returns a table of layer names matching the glob pattern.
        // Example: scribe:list_layers("*:orders") returns all order layers
        methods.add_method("list_layers", |lua, this, pattern: String| {
            let names = this
                .scribe
                .list_layers(&pattern)
                .map_err(|e| LuaError::RuntimeError(e))?;

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
                        .map_err(|e| {
                            LuaError::RuntimeError(format!("Failed to store transform: {}", e))
                        })?;

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
                    let aggregated_json =
                        aggregate_wildcard_layers(&this.scribe, &expanded_pattern)?;
                    sync_binding_to_ui(lua, this, &ui_property, &aggregated_json, None)?;
                } else {
                    // For exact patterns, fetch and sync single layer
                    if let Ok(data) = fetch_layer_data(&this.scribe, &expanded_pattern) {
                        sync_binding_to_ui(lua, this, &ui_property, &data, None)?;
                    }
                }

                Ok(())
            },
        );

        // Rebind an existing binding to a different layer
        //
        // Switches which layer backs a UI property while preserving
        // transform/key/max_items options. Clears caches and performs
        // a full initial sync from the new layer.
        //
        // # Arguments
        // * `ui_property` - Name of the existing binding to rebind
        // * `layer_pattern` - New layer pattern (page_id auto-prepended)
        //
        // # Examples
        // ```lua
        // -- Switch messages binding to a different channel
        // scribe:rebind("messages", "channels/random/messages")
        // ```
        methods.add_method(
            "rebind",
            |lua, this, (ui_property, layer_pattern): (String, String)| {
                if !this.ui_enabled {
                    debug!(
                        ui_property = %ui_property,
                        layer_pattern = %layer_pattern,
                        "scribe:rebind() called but UI not enabled, skipping"
                    );
                    return Ok(());
                }

                let expanded_pattern = expand_pattern(&layer_pattern, &this.page_id, &this.our_did);
                let is_wildcard = is_wildcard_pattern(&layer_pattern);

                info!(
                    ui_property = %ui_property,
                    raw_pattern = %layer_pattern,
                    expanded_pattern = %expanded_pattern,
                    is_wildcard = is_wildcard,
                    "scribe:rebind() - switching binding layer"
                );

                // Rebind in the manager (preserves transform/key/max_items)
                {
                    let mut manager = this.binding_manager.lock();
                    if !manager.rebind(
                        &ui_property,
                        expanded_pattern.clone(),
                        layer_pattern.clone(),
                        is_wildcard,
                    ) {
                        return Err(LuaError::RuntimeError(format!(
                            "scribe:rebind() - no existing binding for '{}'",
                            ui_property
                        )));
                    }
                }

                // Fetch and sync new layer's data
                if is_wildcard {
                    let aggregated_json =
                        aggregate_wildcard_layers(&this.scribe, &expanded_pattern)?;
                    sync_binding_to_ui(lua, this, &ui_property, &aggregated_json, None)?;
                } else {
                    if let Ok(data) = fetch_layer_data(&this.scribe, &expanded_pattern) {
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
    scribe: &Arc<ActorScribeHandle>,
    layer_name: &str,
) -> Result<serde_json::Value, String> {
    match scribe.get_layer_sthithi(layer_name)? {
        Some(data) => Ok(serde_json::Value::from(&data)),
        None => Ok(serde_json::Value::Array(vec![])),
    }
}

fn aggregate_wildcard_layers(
    scribe: &Arc<ActorScribeHandle>,
    expanded_pattern: &str,
) -> LuaResult<serde_json::Value> {
    let layer_names = scribe
        .list_layers(expanded_pattern)
        .map_err(LuaError::RuntimeError)?;

    let mut aggregated_data = Vec::new();
    for name in &layer_names {
        if let Ok(data) = fetch_layer_data(scribe, name) {
            match data {
                serde_json::Value::Array(items) => aggregated_data.extend(items),
                serde_json::Value::Object(map) => {
                    for value in map.into_values() {
                        aggregated_data.push(value);
                    }
                }
                _ => {}
            }
        }
    }

    aggregated_data.sort_by(|a, b| {
        let ta = a
            .get("timestamp")
            .and_then(|value| value.as_i64())
            .unwrap_or(0);
        let tb = b
            .get("timestamp")
            .and_then(|value| value.as_i64())
            .unwrap_or(0);
        ta.cmp(&tb)
    });

    Ok(serde_json::Value::Array(aggregated_data))
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
            if let Some(mutation) =
                super::binding::data_to_ui_mutation(&this.page_id, ui_property, processed)
            {
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
