//! Scribe - Document actor for reactive CRDT synchronization
//!
//! **Context**: Each open page gets its own Scribe that owns LoroDoc layers
//! and handles bidirectional sync with permission-aware merge logic.
//!
//! **Key responsibilities:**
//! - Own LoroDoc layers for a page
//! - Handle permission checks (viewer/submitter/collaborator)
//! - Apply sync policies (local_only, no_incoming_updates, send_full_snapshot)
//! - Broadcast updates to subscribed PeerActors
//! - Notify UI subscribers of changes

pub mod message;
pub mod state;
pub mod permit;  // Merged from permissions.rs + permit_parser.rs
pub mod sync;
pub mod query;
pub mod operations;
pub mod loro_observer;
pub mod validation;
pub mod lua_runtime;

// Re-exports
pub use message::{
    BroadcastPayload, EphemeralBroadcast, EphemeralEvent, EphemeralOutbound, ListOp, LoroDelta,
    PageEvent, PageEventType, ScribeMessage, SyncEvent, TextOp,
};
pub use state::{
    LayerWritePermission, ListAuthorizedUsersFn, LoadPeerVectorFn, LoadUserPermitFn, PatternRule,
    SaveLayerFn, SavePeerVectorFn, ScribeArgs, ScribeState, SubscriberInfo, SyncConfig, SyncPolicy,
};
pub use validation::{LuaValidator, JsonOp};
pub use lua_runtime::ScribeLuaRuntime;

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{Arc, Mutex};

use ractor::{Actor, ActorProcessingErr, ActorRef};
use tokio::time::{interval, Duration};
use tracing::{debug, info, warn};
use walkdir::WalkDir;

use crate::error::{ButlerError, Result};
use crate::models::Layer;

// =============================================================================
// Scribe Actor
// =============================================================================

/// Scribe - Document sync actor
///
/// One Scribe per open page. Owns layers, handles sync, broadcasts to peers.
pub struct Scribe;

impl Scribe {
    pub fn new() -> Self {
        Self
    }
}

impl Default for Scribe {
    fn default() -> Self {
        Self::new()
    }
}

impl Actor for Scribe {
    type Msg = ScribeMessage;
    type State = ScribeState;
    type Arguments = ScribeArgs;

    async fn pre_start(
        &self,
        myself: ActorRef<Self::Msg>,
        args: Self::Arguments,
    ) -> std::result::Result<Self::State, ActorProcessingErr> {
        let mode_info = args
            .sync_config
            .as_ref()
            .map(|c| format!("mode={:?}, target={:?}", c.mode, c.sync_target))
            .unwrap_or_else(|| "no sync config".to_string());
        info!(page_id = %args.page_id, %mode_info, "Scribe started");

        // Initialize Lua runtime (validation + derivation)
        // - All Scribes: validation.lua for incoming update validation
        // - Node Scribes: also init.lua for derivation rules
        info!(
            page_id = %args.page_id,
            is_node = args.is_node,
            has_validation = args.validation_code.is_some(),
            has_init = args.init_code.is_some(),
            "Creating Scribe Lua runtime"
        );

        let lua_runtime = match (&args.validation_code, &args.init_code, args.is_node) {
            // Node with both validation and derivation
            (Some(validation_code), Some(init_code), true) => {
                info!(page_id = %args.page_id, init_code_len = init_code.len(), "Attempting derivation setup");
                match ScribeLuaRuntime::new_with_derivation(&args.page_id, validation_code, init_code) {
                    Ok(runtime) => {
                        info!(
                            page_id = %args.page_id,
                            derivation_enabled = runtime.is_derivation_enabled(),
                            rule_count = runtime.derivation_rules().len(),
                            "Lua runtime initialized with derivation"
                        );
                        Some(runtime)
                    }
                    Err(e) => {
                        warn!(page_id = %args.page_id, error = %e, "Failed to initialize Lua runtime with derivation");
                        // Fall back to validation only
                        match ScribeLuaRuntime::new_validation_only(&args.page_id, validation_code) {
                            Ok(runtime) => Some(runtime),
                            Err(_) => None,
                        }
                    }
                }
            }
            // Validation only (non-node or node without init_code)
            (Some(validation_code), _, _) => {
                match ScribeLuaRuntime::new_validation_only(&args.page_id, validation_code) {
                    Ok(runtime) => {
                        info!(page_id = %args.page_id, "Lua runtime initialized (validation only)");
                        Some(runtime)
                    }
                    Err(e) => {
                        warn!(page_id = %args.page_id, error = %e, "Failed to initialize Lua validator");
                        None
                    }
                }
            }
            // No validation code provided
            _ => None,
        };

        // Parse our permit for local write authorization
        let (our_layer_permissions, our_writable_patterns, our_role) = if let Some(ref permit) = args.our_permit {
            let (layer_perms, _sync_policy) = permit::parse_permit_for_layers(permit)
                .unwrap_or_default();
            let (_readable, writable) = permit::extract_patterns_from_permit(permit, &args.page_id);
            let role = permit::extract_role_from_permit(permit);
            info!(page_id = %args.page_id, role = %role, layer_count = layer_perms.len(), pattern_count = writable.len(), "Parsed our permit");
            (layer_perms, writable, role)
        } else {
            // No permit = full access, derive role from sync mode
            let role = args.sync_config.as_ref()
                .map(|c| match c.mode {
                    state::SyncMode::Broadcast => "node".to_string(),
                    state::SyncMode::ToSource => "owner".to_string(),
                })
                .unwrap_or_else(|| "owner".to_string());
            debug!(page_id = %args.page_id, role = %role, "No our_permit, defaulting to full access");
            (HashMap::new(), Vec::new(), role)
        };

        // Spawn periodic flush timer - saves dirty layers every 10 seconds
        // This prevents blocking the actor on every single operation
        let myself_clone = myself.clone();
        tokio::spawn(async move {
            let mut flush_interval = interval(Duration::from_secs(10));
            loop {
                flush_interval.tick().await;
                let _ = myself_clone.cast(ScribeMessage::Flush);
            }
        });

        // Spawn periodic reconciliation timer - catches missed syncs every 30 seconds
        // Useful for reconnection scenarios where state vectors may have diverged
        let myself_reconcile = myself.clone();
        tokio::spawn(async move {
            let mut reconcile_interval = interval(Duration::from_secs(30));
            loop {
                reconcile_interval.tick().await;
                let _ = myself_reconcile.cast(ScribeMessage::ReconcileWithPeers);
            }
        });

        let mut state = ScribeState {
            page_id: args.page_id,
            layers: args.layers,
            subscribers: Arc::new(std::sync::RwLock::new(HashMap::new())),
            local_only_layers: Arc::new(std::sync::RwLock::new(HashSet::new())),
            query_subscribers: HashMap::new(),
            dirty_layers: HashSet::new(),
            save_layer: args.save_layer,
            load_peer_vector: args.load_peer_vector,
            save_peer_vector: args.save_peer_vector,
            sync_config: args.sync_config,
            sync_event_tx: args.sync_event_tx,
            list_authorized_users: args.list_authorized_users,
            load_user_permit: args.load_user_permit,
            loro_subscriptions: HashMap::new(),
            // Unified page event channel subscribers
            page_event_subscribers: Arc::new(std::sync::RwLock::new(Vec::new())),
            // Pending update source for observer context (tracks who caused the change)
            pending_update_source: Arc::new(Mutex::new(None)),
            // Ephemeral event subscribers (cursor, typing, presence)
            ephemeral_subscribers: Arc::new(std::sync::RwLock::new(Vec::new())),
            // Note: ephemeral_broadcast_tx removed - now uses SubscriberInfo.ephemeral_tx
            // Unified Lua runtime for validation + derivation
            lua_runtime,
            // Our permit for local write authorization
            our_permit: args.our_permit,
            our_did: args.our_did,
            our_role,
            our_layer_permissions,
            our_writable_patterns,
        };

        // Pre-create static layers from permit
        // Static layers are fully known after expanding {page_id} and {aud}
        // Dynamic layers (with wildcards) are created on-demand during sync
        if let Some(ref permit) = state.our_permit {
            let static_layers = permit::extract_static_layers(permit, &state.page_id, &state.our_did);
            for layer_name in static_layers {
                if !state.layers.contains_key(&layer_name) {
                    info!(
                        page_id = %state.page_id,
                        layer = %layer_name,
                        "Pre-creating static layer from permit"
                    );
                    state.layers.insert(layer_name, Layer::new());
                }
            }
        }

        // Update local_only_layers based on permit before setting up observers
        state.update_local_only_layers();

        // Set up Loro observers for all layers (including pre-created ones)
        // This ensures import() triggers broadcasts via observer pattern
        loro_observer::setup_observers_for_all_layers(&mut state);

        // Emit EnsureSync on page open to trigger remote subscription
        // Flow: EnsureSync → Coordinator → (connect if needed) → RefreshSubscriptions → remote subscribes
        if let Some(ref sync_config) = state.sync_config {
            if let Some(ref sync_target) = sync_config.sync_target {
                if let Some(ref tx) = state.sync_event_tx {
                    let event = SyncEvent::EnsureSync {
                        user_did: sync_target.clone(),
                    };
                    match tx.try_send(event) {
                        Ok(()) => info!(sync_target = %sync_target, page_id = %state.page_id, "Emitted EnsureSync on page open"),
                        Err(e) => warn!(sync_target = %sync_target, error = %e, "Failed to emit EnsureSync on page open"),
                    }
                }
            }
        }

        Ok(state)
    }

    async fn handle(
        &self,
        myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        state: &mut Self::State,
    ) -> std::result::Result<(), ActorProcessingErr> {
        match message {
            ScribeMessage::Subscribe {
                user_did,
                device_id,
                broadcast_tx,
                ephemeral_tx,
                permit,
            } => {
                sync::handle_subscribe(state, user_did, device_id, broadcast_tx, ephemeral_tx, permit).await;
            }

            ScribeMessage::Unsubscribe { user_did, device_id } => {
                sync::handle_unsubscribe(state, &user_did, &device_id).await;
            }

            ScribeMessage::SubscribeToPageEvents { event_tx } => {
                if let Ok(mut subs) = state.page_event_subscribers.write() {
                    subs.push(event_tx);
                    info!(page_id = %state.page_id, subscriber_count = subs.len(), "Page event subscriber added");
                }
            }

            ScribeMessage::ApplyUpdate {
                layer_name,
                update,
                from_peer,
                permit,
            } => {
                // Loro observer handles broadcast automatically after import()
                let _ = sync::handle_apply_update(
                    state,
                    &layer_name,
                    &update,
                    from_peer,
                    permit.as_deref(),
                )
                .await;
            }

            ScribeMessage::ApplyUpdateWithResult {
                layer_name,
                update,
                from_peer,
                permit,
                reply,
            } => {
                // Loro observer handles broadcast automatically after import()
                let result = sync::handle_apply_update(
                    state,
                    &layer_name,
                    &update,
                    from_peer,
                    permit.as_deref(),
                )
                .await;
                let _ = reply.send(result);
            }

            ScribeMessage::SyncRequest {
                layer_name,
                their_vector,
                reply,
            } => {
                let result = sync::handle_sync_request(state, &layer_name, &their_vector);
                let _ = reply.send(result);
            }

            ScribeMessage::ExportSnapshot { layer_name, reply } => {
                let result = sync::handle_export_snapshot(state, &layer_name);
                let _ = reply.send(result);
            }

            ScribeMessage::GetStateVector { layer_name, reply } => {
                let result = sync::handle_get_state_vector(state, &layer_name);
                let _ = reply.send(result);
            }

            ScribeMessage::GetUpdatesSince {
                layer_name,
                state_vector,
                reply,
            } => {
                let result = sync::handle_get_updates_since(state, &layer_name, &state_vector);
                let _ = reply.send(result);
            }

            ScribeMessage::Flush => {
                sync::handle_flush(state).await;
            }

            ScribeMessage::GetLoroList { layer_name, reply } => {
                let result = state
                    .layers
                    .get(&layer_name)
                    .ok_or_else(|| ButlerError::LayerNotFound(layer_name.clone()))
                    .map(|layer| layer.loro().get_list(layer_name.clone()));
                let _ = reply.send(result);
            }

            ScribeMessage::GetLoroMap { layer_name, reply } => {
                let result = state
                    .layers
                    .get(&layer_name)
                    .ok_or_else(|| ButlerError::LayerNotFound(layer_name.clone()))
                    .map(|layer| layer.loro().get_map(layer_name.clone()));
                let _ = reply.send(result);
            }

            ScribeMessage::GetOrCreateLoroList { layer_name, reply } => {
                // Get existing layer or create new one
                let is_new_layer = !state.layers.contains_key(&layer_name);
                if is_new_layer {
                    state.layers.insert(layer_name.clone(), Layer::new());
                    state.dirty_layers.insert(layer_name.clone());
                    debug!(layer = %layer_name, "Created new layer for Lua");
                    // Check if layer is local-only (sync: false in permit)
                    state.mark_layer_local_only_if_needed(&layer_name);
                    // Set up Loro observer for new layer to enable sync broadcasts
                    loro_observer::setup_layer_observer(state, &layer_name);
                    // Notify UI subscribers about the new layer (so they can subscribe to changes)
                    sync::notify_layer_discovered(state, &layer_name);
                }
                let result = state
                    .layers
                    .get(&layer_name)
                    .ok_or_else(|| ButlerError::LayerNotFound(layer_name.clone()))
                    .map(|layer| layer.loro().get_list(layer_name.clone()));
                let _ = reply.send(result);
            }

            ScribeMessage::GetOrCreateLoroMap { layer_name, reply } => {
                // Get existing layer or create new one
                let is_new_layer = !state.layers.contains_key(&layer_name);
                if is_new_layer {
                    state.layers.insert(layer_name.clone(), Layer::new());
                    state.dirty_layers.insert(layer_name.clone());
                    debug!(layer = %layer_name, "Created new layer for Lua");
                    // Check if layer is local-only (sync: false in permit)
                    state.mark_layer_local_only_if_needed(&layer_name);
                    // Set up Loro observer for new layer to enable sync broadcasts
                    loro_observer::setup_layer_observer(state, &layer_name);
                }
                let result = state
                    .layers
                    .get(&layer_name)
                    .ok_or_else(|| ButlerError::LayerNotFound(layer_name.clone()))
                    .map(|layer| layer.loro().get_map(layer_name.clone()));
                let _ = reply.send(result);
            }

            ScribeMessage::ListLayers { pattern, reply } => {
                // Match layer names against glob pattern
                let matching: Vec<String> = state
                    .layers
                    .keys()
                    .filter(|name| permit::glob_match(&pattern, name))
                    .cloned()
                    .collect();
                let _ = reply.send(matching);
            }

            ScribeMessage::LayerModifiedByLua { layer_name } => {
                loro_observer::handle_layer_modified_by_lua(state, layer_name).await;
            }

            ScribeMessage::CommitLayer { layer_name } => {
                // Commit changes and trigger sync via Loro observer
                info!(layer = %layer_name, "CommitLayer received");
                let subscriber_count = state.subscribers.read().map(|s| s.len()).unwrap_or(0);
                info!(layer = %layer_name, subscriber_count, "CommitLayer: subscriber count");
                if let Some(layer) = state.layers.get(&layer_name) {
                    info!(layer = %layer_name, "Layer found, calling commit()");
                    layer.commit();
                    state.dirty_layers.insert(layer_name.clone());
                    info!(layer = %layer_name, "Layer committed successfully");
                } else {
                    warn!(layer = %layer_name, "CommitLayer: layer not found in state.layers!");
                }
            }

            ScribeMessage::GetLayerData { layer_name, reply } => {
                let result = state.layers.get(&layer_name)
                    .map(|layer| layer.to_json_value())
                    .ok_or_else(|| crate::error::ButlerError::LayerNotFound(layer_name));
                let _ = reply.send(result);
            }

            ScribeMessage::UpdatePeerVector {
                user_did,
                device_id,
                layer_name,
                state_vector,
            } => {
                if let Ok(mut subs) = state.subscribers.write() {
                    if let Some(info) = subs.get_mut(&(user_did.clone(), device_id.clone())) {
                        info.vectors.insert(layer_name.clone(), state_vector);
                        debug!(user_did = %user_did, device_id = %device_id, layer_name = %layer_name, "Updated in-memory peer vector");
                    }
                }
            }

            ScribeMessage::ReconcileWithPeers => {
                sync::handle_reconcile_with_peers(state).await;
            }

            ScribeMessage::GetSnapshot { layer_name, reply } => {
                let result = state.layers.get(&layer_name).map(|layer| layer.export_snapshot());
                let _ = reply.send(result);
            }

            ScribeMessage::GetLayerJson { layer_name, reply } => {
                let result = state.layers.get(&layer_name).map(|layer| layer.to_json());
                let _ = reply.send(result);
            }

            ScribeMessage::GetContext { reply } => {
                let result = query::build_context_json(state);
                let _ = reply.send(result);
            }

            ScribeMessage::Query { spec, reply } => {
                let result = query::handle_query(state, &spec);
                let _ = reply.send(result);
            }

            ScribeMessage::SubscribeQuery { spec, delta_tx } => {
                query::handle_subscribe_query(state, spec, delta_tx).await;
            }

            ScribeMessage::UnsubscribeQuery { query_id } => {
                state.query_subscribers.remove(&query_id);
                debug!(page_id = %state.page_id, query_id = %query_id, "Query subscription removed");
            }

            ScribeMessage::UpdateFromJson {
                layer_name,
                path,
                value,
                reply,
            } => {
                let result = self
                    .handle_update_from_json(state, &layer_name, &path, value)
                    .await;
                if let Some(tx) = reply {
                    let _ = tx.send(result);
                }
            }

            // Typed CRDT operations
            // NOTE: Broadcast handled by Loro observer after commit() in each operation
            ScribeMessage::ListPush {
                layer_name,
                path,
                item,
            } => {
                if let Err(e) = operations::handle_list_push(state, &layer_name, &path, item).await {
                    warn!(error = %e, "ListPush failed");
                }
            }

            ScribeMessage::ListInsert {
                layer_name,
                path,
                index,
                item,
            } => {
                if let Err(e) = operations::handle_list_insert(state, &layer_name, &path, index, item).await {
                    warn!(error = %e, "ListInsert failed");
                }
            }

            ScribeMessage::ListDelete {
                layer_name,
                path,
                index,
            } => {
                if let Err(e) = operations::handle_list_delete(state, &layer_name, &path, index).await {
                    warn!(error = %e, "ListDelete failed");
                }
            }

            ScribeMessage::MapInsert {
                layer_name,
                path,
                key,
                value,
            } => {
                if let Err(e) = operations::handle_map_insert(state, &layer_name, &path, &key, value).await {
                    warn!(error = %e, "MapInsert failed");
                }
            }

            ScribeMessage::MapDelete {
                layer_name,
                path,
                key,
            } => {
                if let Err(e) = operations::handle_map_delete(state, &layer_name, &path, &key).await {
                    warn!(error = %e, "MapDelete failed");
                }
            }

            ScribeMessage::CounterInc {
                layer_name,
                path,
                amount,
            } => {
                if let Err(e) = operations::handle_counter_inc(state, &layer_name, &path, amount).await {
                    warn!(error = %e, "CounterInc failed");
                }
            }

            ScribeMessage::Shutdown => {
                info!(page_id = %state.page_id, "Scribe shutting down");
                sync::handle_flush(state).await;
                myself.stop(Some("shutdown".to_string()));
            }

            // Ephemeral events (from datagrams via PeerActor)
            ScribeMessage::RemoteEphemeral { user_did, device_id, payload } => {
                // 1. Forward to local app layer - app interprets the payload format
                if let Some(ref did) = user_did {
                    emit_ephemeral_event(state, message::EphemeralEvent::Data {
                        user_did: did.clone(),
                        payload: payload.clone(),
                    }).await;
                } else {
                    debug!(page_id = %state.page_id, "RemoteEphemeral without user_did - not emitting to local app");
                }

                // 2. Relay to other subscribers (node mode relay) - exclude sender
                let exclude = match (&user_did, &device_id) {
                    (Some(d), Some(dev)) => Some((d.as_str(), dev.as_str())),
                    _ => None,
                };
                broadcast_ephemeral_to_subscribers(state, &payload, exclude);
            }

            ScribeMessage::SendEphemeral { payload } => {
                // Broadcast ephemeral data directly to all subscribed PeerActors
                // No exclusion - this is local user action, send to everyone
                debug!(page_id = %state.page_id, payload_len = payload.len(), "Broadcasting ephemeral to all subscribers");
                broadcast_ephemeral_to_subscribers(state, &payload, None);
            }

            ScribeMessage::SubscribeEphemeral { tx } => {
                if let Ok(mut subscribers) = state.ephemeral_subscribers.write() {
                    subscribers.push(tx);
                    debug!(page_id = %state.page_id, "Added ephemeral subscriber");
                }
            }

            // Note: SetEphemeralBroadcast removed - ephemeral now goes directly via
            // SubscriberInfo.ephemeral_tx (Scribe → PeerActor channels)

            // Derivation operations (using ScribeLuaRuntime)
            ScribeMessage::RebuildDerived { target, reply } => {
                let result = if let Some(ref runtime) = state.lua_runtime {
                    runtime.rebuild_derived(&target, &mut state.layers)
                } else {
                    Err("No Lua runtime available".to_string())
                };
                let _ = reply.send(result);
            }

            ScribeMessage::RebuildAllDerived { reply } => {
                let result = if let Some(ref runtime) = state.lua_runtime {
                    runtime.rebuild_all_derived(&mut state.layers)
                } else {
                    Err("No Lua runtime available".to_string())
                };
                let _ = reply.send(result);
            }

            ScribeMessage::IsDerivationEnabled { reply } => {
                let enabled = state.lua_runtime
                    .as_ref()
                    .map(|r| r.is_derivation_enabled())
                    .unwrap_or(false);
                let _ = reply.send(enabled);
            }

            ScribeMessage::CreateDerivedLayer { target_layer } => {
                // Create the derived layer (empty) so it exists for subscribers
                if !state.layers.contains_key(&target_layer) {
                    let layer = Layer::new();
                    let _ = layer.loro().get_map(target_layer.clone());
                    layer.commit();
                    state.layers.insert(target_layer.clone(), layer);
                    info!(target = %target_layer, "Created empty derived layer");

                    // Set up observer for the new layer
                    loro_observer::setup_layer_observer(state, &target_layer);
                }
            }

            ScribeMessage::RefreshApp { app_name, app_dir, reply } => {
                let result = handle_refresh_app(state, &app_name, &app_dir).await;
                let _ = reply.send(result);
            }

            ScribeMessage::GetAppFiles { app_name, reply } => {
                let layer_name = format!("app:{}", app_name);
                let result = if let Some(layer) = state.layers.get(&layer_name) {
                    Ok(layer.get_all_files())
                } else {
                    Err(format!("App layer '{}' not found", layer_name))
                };
                let _ = reply.send(result);
            }
        }

        Ok(())
    }

    async fn post_stop(
        &self,
        _myself: ActorRef<Self::Msg>,
        state: &mut Self::State,
    ) -> std::result::Result<(), ActorProcessingErr> {
        info!(page_id = %state.page_id, "Scribe stopped");
        Ok(())
    }
}

// =============================================================================
// Ephemeral Event Helpers
// =============================================================================

/// Emit an ephemeral event to all local app subscribers
///
/// **Context**: Called when receiving remote cursor/typing/presence updates
/// **Design**: Non-blocking - drops events if subscribers are slow
async fn emit_ephemeral_event(state: &ScribeState, event: message::EphemeralEvent) {
    if let Ok(subscribers) = state.ephemeral_subscribers.read() {
        for tx in subscribers.iter() {
            let _ = tx.try_send(event.clone());
        }
    }
}

/// Broadcast ephemeral data to all subscribed PeerActors (via their ephemeral channels)
///
/// **Context**: Called for:
/// - SendEphemeral from Lua (local user action) - no exclusion
/// - RemoteEphemeral (relay to other peers) - exclude sender
///
/// **exclude_key**: (user_did, device_id) to skip (the sender for relay)
fn broadcast_ephemeral_to_subscribers(
    state: &ScribeState,
    payload: &[u8],
    exclude_key: Option<(&str, &str)>,
) {
    if let Ok(subscribers) = state.subscribers.read() {
        for ((did, device), info) in subscribers.iter() {
            // Skip sender (for relay - don't echo back)
            if let Some((ex_did, ex_device)) = exclude_key {
                if did == ex_did && device == ex_device {
                    continue;
                }
            }
            // Send via ephemeral channel if available
            if let Some(ref tx) = info.ephemeral_tx {
                let _ = tx.try_send(message::EphemeralOutbound {
                    page_id: state.page_id.clone(),
                    payload: payload.to_vec(),
                });
            }
        }
    }
}

// =============================================================================
// Helper Methods
// =============================================================================

impl Scribe {
    /// Handle JSON update from UI (CEL commit())
    ///
    /// **Broadcast**: Handled automatically by Loro observer after commit()
    async fn handle_update_from_json(
        &self,
        state: &mut ScribeState,
        layer_name: &str,
        path: &str,
        value: serde_json::Value,
    ) -> Result<()> {
        info!(layer = %layer_name, path = %path, "Updating layer from JSON");

        // Get or create the layer
        let layer = state
            .layers
            .entry(layer_name.to_string())
            .or_insert_with(Layer::new);

        // Convert JSON to Loro and update
        layer
            .set_from_json(path, &value)
            .map_err(|e| ButlerError::Layer(e.to_string()))?;

        // Commit to trigger Loro observer (which broadcasts to peers + notifies UI)
        layer.commit();

        // Mark as dirty for persistence
        state.dirty_layers.insert(layer_name.to_string());

        debug!("Layer update from JSON complete, observer will broadcast");
        Ok(())
    }
}

// =============================================================================
// App Refresh Handler
// =============================================================================

/// Refresh app from filesystem (owner only)
///
/// **Context**: Owner edited files on disk, wants to reload into Scribe
/// **We do**: Read files, compare with current layer, update LoroMap, commit
/// **Observer**: Loro observer broadcasts to peers + emits PageEvent
async fn handle_refresh_app(
    state: &mut ScribeState,
    app_name: &str,
    app_dir: &Path,
) -> std::result::Result<Vec<String>, String> {
    let layer_name = format!("app:{}", app_name);

    info!(
        page_id = %state.page_id,
        app_name = %app_name,
        app_dir = %app_dir.display(),
        "Refreshing app from directory"
    );

    // 1. Collect new files from disk
    let new_files = collect_app_files_for_refresh(app_dir)
        .map_err(|e| format!("Failed to read app files: {}", e))?;

    // 2. Get or create the app layer
    let layer = state.layers
        .entry(layer_name.clone())
        .or_insert_with(Layer::new);

    // 3. Get current files to compare
    let old_files = layer.get_all_files();

    // 4. Find changed files
    let mut changed_files = Vec::new();
    for (path, content) in &new_files {
        if old_files.get(path) != Some(content) {
            changed_files.push(path.clone());
        }
    }
    // Track deleted files
    for path in old_files.keys() {
        if !new_files.contains_key(path) {
            changed_files.push(path.clone());
        }
    }

    if changed_files.is_empty() {
        info!(
            page_id = %state.page_id,
            app_name = %app_name,
            "No changes detected"
        );
        return Ok(changed_files);
    }

    // 5. Update layer with new files
    layer.set_all_files(&new_files)
        .map_err(|e| format!("Failed to update app layer: {}", e))?;

    // 6. Commit to trigger Loro observer (broadcasts to peers + emits PageEvent)
    layer.commit();

    // 7. Mark as dirty for persistence
    state.dirty_layers.insert(layer_name.clone());

    info!(
        page_id = %state.page_id,
        app_name = %app_name,
        changed_count = changed_files.len(),
        files = ?changed_files,
        "App refreshed, observer will broadcast"
    );

    Ok(changed_files)
}

/// Collect app files from a directory
///
/// Returns HashMap of relative_path -> content for text files.
fn collect_app_files_for_refresh(app_dir: &Path) -> std::result::Result<std::collections::HashMap<String, String>, String> {
    let mut files = std::collections::HashMap::new();

    for entry in WalkDir::new(app_dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if !["slint", "lua", "json"].contains(&ext) {
            continue;
        }

        if let Ok(relative_path) = path.strip_prefix(app_dir) {
            let relative_str = relative_path.to_string_lossy().to_string();
            if let Ok(content) = std::fs::read_to_string(path) {
                files.insert(relative_str, content);
            }
        }
    }

    Ok(files)
}
