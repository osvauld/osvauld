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

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::sync::atomic::AtomicU64;

use ractor::{Actor, ActorProcessingErr, ActorRef};
use tokio::time::{interval, Duration};
use tracing::{debug, error, info, instrument, trace, warn};

use domains::Layer;

use crate::layer_unit::LayerUnit;
use crate::message;
use crate::state::{ScribeArgs, ScribeState, SyncMode, normalize_layer_name};
use crate::permit::glob_match;
use crate::ephemeral::{
    route_remote_ephemeral, emit_raw_ephemeral, emit_structured_ephemeral,
    broadcast_ephemeral_to_subscribers,
};
use crate::{
    ScribeMessage, ScribeError, Result,
    sync, query, operations, loro_observer,
};

// Scribe Actor

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

    #[instrument(skip_all, fields(page_id = %args.page_id))]
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

        // Note: Validation is now handled by kunki/ValidationService via ValidationHandle
        // Derivation is handled by kunki/LuaRuntime
        info!(
            page_id = %args.page_id,
            is_node = args.is_node,
            has_validation_handle = args.validation_handle.is_some(),
            "Scribe initialized (validation delegated to kunki)"
        );

        // Parse our permit using gurkha::Permit directly
        let (our_permit, static_layers) = if let Some(ref permit_token) = args.our_permit {
            match gurkha::Permit::from_token(permit_token) {
                Ok(permit) => {
                    let static_layers = permit.static_layers(&args.page_id, &args.our_did);
                    info!(
                        page_id = %args.page_id,
                        role = %permit.relationship().unwrap_or("peer"),
                        static_layer_count = static_layers.len(),
                        "Parsed our permit via gurkha::Permit"
                    );
                    (Some(permit), static_layers)
                }
                Err(e) => {
                    warn!(page_id = %args.page_id, error = ?e, "Failed to parse our permit");
                    (None, Vec::new())
                }
            }
        } else {
            debug!(page_id = %args.page_id, "No our_permit, defaulting to full access");
            (None, Vec::new())
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
        // Only for Node mode (Broadcast) - User mode (ToSource) relies on Node to initiate sync
        // This prevents bilateral reconciliation attempts that can cause sync storms
        let is_user_mode = args.sync_config.as_ref().map(|c| c.mode) == Some(SyncMode::ToSource);
        if !is_user_mode {
            let myself_reconcile = myself.clone();
            tokio::spawn(async move {
                let mut reconcile_interval = interval(Duration::from_secs(30));
                loop {
                    reconcile_interval.tick().await;
                    let _ = myself_reconcile.cast(ScribeMessage::ReconcileWithPeers);
                }
            });
        }

        // Convert args.layers → LayerUnit instances
        let mut units: HashMap<String, LayerUnit> = args.layers
            .into_iter()
            .map(|(name, layer)| (name, LayerUnit::new(layer)))
            .collect();

        let mut state = ScribeState {
            page_id: args.page_id,
            units: HashMap::new(), // Populated below
            subscribers: Arc::new(std::sync::RwLock::new(HashMap::new())),
            query_subscribers: HashMap::new(),
            // Storage traits
            layer_storage: args.layer_storage,
            vector_storage: args.vector_storage,
            peer_resolver: args.peer_resolver,
            permit_issuer: args.permit_issuer,
            // Sync configuration
            sync_config: args.sync_config,
            sync_event_tx: args.sync_event_tx,
            capture_tx: args.capture_tx,
            capture_seq: AtomicU64::new(0),
            page_update_subscribers: Arc::new(std::sync::RwLock::new(Vec::new())),
            pending_update_source: Arc::new(Mutex::new(None)),
            validation_handle: args.validation_handle,
            our_permit,
            our_did: args.our_did.clone(),
            node_script_shutdown: None,
            pending_layer_authorizations: HashMap::new(),
        };

        // Pre-create static layers from permit (already extracted via PermitContext)
        // Static layers are fully known after expanding {page_id} and {aud}
        // Dynamic layers (with wildcards) are created on-demand during sync
        for layer_name in static_layers {
            if !units.contains_key(&layer_name) {
                info!(
                    page_id = %state.page_id,
                    layer = %layer_name,
                    "Pre-creating static layer from permit"
                );
                units.insert(layer_name, LayerUnit::new_empty());
            }
        }

        // Set is_local_only on each unit based on permit
        for (name, unit) in units.iter_mut() {
            unit.set_local_only(!state.should_sync_layer(name));
        }

        // Move units into state
        state.units = units;

        // Set up Loro observers for all layers (including pre-created ones)
        // This ensures import() triggers broadcasts via observer pattern
        loro_observer::setup_observers_for_all_layers(&mut state);

        // Emit EnsureSync on page open to trigger remote subscription
        // Flow: EnsureSync → Coordinator → (connect if needed) → RefreshSubscriptions → remote subscribes
        // Works for both ToSource mode (single sync_target) and Broadcast mode (all authorized users)
        if let Some(ref tx) = state.sync_event_tx {
            let sync_targets = sync::reconcile::get_sync_targets(&state);
            if sync_targets.is_empty() {
                debug!(page_id = %state.page_id, "No sync targets on page open");
            } else {
                info!(page_id = %state.page_id, targets = ?sync_targets, "Emitting EnsureSync for {} targets on page open", sync_targets.len());
                for user_did in sync_targets {
                    let event = message::SyncEvent::EnsureSync {
                        user_did: user_did.clone(),
                    };
                    state.emit_sync_event_capture(&event);
                    match tx.try_send(event) {
                        Ok(()) => info!(user_did = %user_did, page_id = %state.page_id, "Emitted EnsureSync on page open"),
                        Err(e) => warn!(user_did = %user_did, error = %e, "Failed to emit EnsureSync on page open"),
                    }
                }
            }
        }

        // Note: Node script spawning is handled by the caller (e.g., kunki's NodeRuntimeManager)
        // This keeps the Scribe actor storage-agnostic and focused on CRDT sync

        Ok(state)
    }

    #[instrument(level = "trace", skip_all, fields(page_id = %state.page_id))]
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

            ScribeMessage::SubscribeToPageUpdates { tx } => {
                // Send PeerSubscribed for all currently subscribed peers
                // (peers that subscribed before this app opened)
                if let Ok(subs) = state.subscribers.read() {
                    info!(page_id = %state.page_id, peer_count = subs.len(), "Sending existing peers as PeerSubscribed");
                    for ((user_did, _device_id), _info) in subs.iter() {
                        let _ = tx.try_send(message::PageUpdate::PeerSubscribed {
                            did: user_did.clone(),
                            username: None,
                        });
                    }
                }

                // Presence state is delivered via CRDT sync (presence layer snapshot)
                // No need to send OnlinePeers event — apps read from LayerChanged on {page_id}/presence

                if let Ok(mut subs) = state.page_update_subscribers.write() {
                    subs.push(tx);
                    info!(page_id = %state.page_id, subscriber_count = subs.len(), "PageUpdate subscriber added");
                }
            }

            ScribeMessage::ApplyUpdate {
                layer_name,
                update,
                from_peer,
                permit,
            } => {
                let layer_name = normalize_layer_name(&layer_name, &state.page_id);
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
                let layer_name = normalize_layer_name(&layer_name, &state.page_id);
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
                let layer_name = normalize_layer_name(&layer_name, &state.page_id);
                let result = sync::handle_sync_request(state, &layer_name, &their_vector);
                let _ = reply.send(result);
            }

            ScribeMessage::ExportSnapshot { layer_name, reply } => {
                let layer_name = normalize_layer_name(&layer_name, &state.page_id);
                let result = sync::handle_export_snapshot(state, &layer_name);
                let _ = reply.send(result);
            }

            ScribeMessage::GetStateVector { layer_name, reply } => {
                let layer_name = normalize_layer_name(&layer_name, &state.page_id);
                let result = sync::handle_get_state_vector(state, &layer_name);
                let _ = reply.send(result);
            }

            ScribeMessage::GetUpdatesSince {
                layer_name,
                state_vector,
                reply,
            } => {
                let layer_name = normalize_layer_name(&layer_name, &state.page_id);
                let result = sync::handle_get_updates_since(state, &layer_name, &state_vector);
                let _ = reply.send(result);
            }

            ScribeMessage::ReplaceLayer {
                layer_name,
                snapshot,
                reply,
            } => {
                let layer_name = normalize_layer_name(&layer_name, &state.page_id);
                // Replace layer entirely with authoritative snapshot (SyncReset recovery)
                info!(layer = %layer_name, snapshot_len = snapshot.len(), "Replacing layer with authoritative snapshot");
                let result = match Layer::from_snapshot(&snapshot) {
                    Ok(layer) => {
                        let unit = state.units.entry(layer_name.clone()).or_insert_with(LayerUnit::new_empty);
                        unit.replace_layer(layer);
                        unit.mark_dirty();
                        info!(layer = %layer_name, "Layer replaced successfully");
                        Ok(())
                    }
                    Err(e) => {
                        error!(layer = %layer_name, error = %e, "Failed to parse snapshot for layer replacement");
                        Err(format!("Failed to parse snapshot: {}", e))
                    }
                };
                let _ = reply.send(result);
            }

            ScribeMessage::Flush => {
                sync::handle_flush(state).await;
            }

            ScribeMessage::EnsureLoroList { layer_name, reply } => {
                let layer_name = normalize_layer_name(&layer_name, &state.page_id);
                // Get existing layer or create new one (for list access)
                let is_new_layer = !state.units.contains_key(&layer_name);
                if is_new_layer {
                    let mut unit = LayerUnit::new_empty();
                    unit.set_local_only(!state.should_sync_layer(&layer_name));
                    unit.mark_dirty();
                    state.units.insert(layer_name.clone(), unit);
                    debug!(layer = %layer_name, "Created new layer for Lua list");
                    // Set up Loro observer for new layer to enable sync broadcasts
                    loro_observer::setup_layer_observer(state, &layer_name);
                    // Notify UI subscribers about the new layer
                    sync::notify_layer_discovered(state, &layer_name);
                }
                // Just confirm the layer exists, don't return handle
                let _ = reply.send(Ok(()));
            }

            ScribeMessage::EnsureLoroMap { layer_name, reply } => {
                let layer_name = normalize_layer_name(&layer_name, &state.page_id);
                // Get existing layer or create new one (for map access)
                let is_new_layer = !state.units.contains_key(&layer_name);
                if is_new_layer {
                    let mut unit = LayerUnit::new_empty();
                    unit.set_local_only(!state.should_sync_layer(&layer_name));
                    unit.mark_dirty();
                    state.units.insert(layer_name.clone(), unit);
                    debug!(layer = %layer_name, "Created new layer for Lua map");
                    // Set up Loro observer for new layer to enable sync broadcasts
                    loro_observer::setup_layer_observer(state, &layer_name);
                }
                // Just confirm the layer exists, don't return handle
                let _ = reply.send(Ok(()));
            }

            ScribeMessage::LayerExists { layer_name, reply } => {
                let layer_name = normalize_layer_name(&layer_name, &state.page_id);
                let exists = state.units.contains_key(&layer_name);
                let _ = reply.send(exists);
            }

            // Typed read operations (for Lua bindings - avoids stale handles)

            ScribeMessage::ListGet { layer_name, index, reply } => {
                let layer_name = normalize_layer_name(&layer_name, &state.page_id);
                let result = state.units.get(&layer_name)
                    .map(|unit| unit.layer())
                    .ok_or_else(|| ScribeError::LayerNotFound(layer_name.clone()))
                    .map(|layer| {
                        let list = layer.loro().get_list(layer_name.clone());
                        list.get(index).map(|v| {
                            let deep = v.get_deep_value();
                            domains::loro_value_to_json(deep)
                        })
                    });
                let _ = reply.send(result);
            }

            ScribeMessage::ListLength { layer_name, reply } => {
                let layer_name = normalize_layer_name(&layer_name, &state.page_id);
                let result = state.units.get(&layer_name)
                    .map(|unit| unit.layer())
                    .ok_or_else(|| ScribeError::LayerNotFound(layer_name.clone()))
                    .map(|layer| {
                        let list = layer.loro().get_list(layer_name.clone());
                        list.len()
                    });
                let _ = reply.send(result);
            }

            ScribeMessage::MapGet { layer_name, key, reply } => {
                let layer_name = normalize_layer_name(&layer_name, &state.page_id);
                let result = state.units.get(&layer_name)
                    .map(|unit| unit.layer())
                    .ok_or_else(|| ScribeError::LayerNotFound(layer_name.clone()))
                    .map(|layer| {
                        let map = layer.loro().get_map(layer_name.clone());
                        map.get(&key).map(|v| {
                            let deep = v.get_deep_value();
                            domains::loro_value_to_json(deep)
                        })
                    });
                let _ = reply.send(result);
            }

            ScribeMessage::MapLength { layer_name, reply } => {
                let layer_name = normalize_layer_name(&layer_name, &state.page_id);
                let result = state.units.get(&layer_name)
                    .map(|unit| unit.layer())
                    .ok_or_else(|| ScribeError::LayerNotFound(layer_name.clone()))
                    .map(|layer| {
                        let map = layer.loro().get_map(layer_name.clone());
                        map.len()
                    });
                let _ = reply.send(result);
            }

            ScribeMessage::MapKeys { layer_name, reply } => {
                let layer_name = normalize_layer_name(&layer_name, &state.page_id);
                let result = state.units.get(&layer_name)
                    .map(|unit| unit.layer())
                    .ok_or_else(|| ScribeError::LayerNotFound(layer_name.clone()))
                    .map(|layer| {
                        let map = layer.loro().get_map(layer_name.clone());
                        map.keys().map(|k| k.to_string()).collect()
                    });
                let _ = reply.send(result);
            }

            ScribeMessage::ListLayers { pattern, reply } => {
                let pattern = normalize_layer_name(&pattern, &state.page_id);
                // Match layer names against glob pattern
                let matching: Vec<String> = state
                    .units
                    .keys()
                    .filter(|name| glob_match(&pattern, name))
                    .cloned()
                    .collect();
                let _ = reply.send(matching);
            }


            ScribeMessage::GetLayerData { layer_name, reply } => {
                let layer_name = normalize_layer_name(&layer_name, &state.page_id);
                let result = state.units.get(&layer_name)
                    .map(|unit| unit.layer().get_content(&layer_name))
                    .ok_or_else(|| ScribeError::LayerNotFound(layer_name.to_string()));
                let _ = reply.send(result);
            }

            ScribeMessage::UpdatePeerVector {
                user_did,
                device_id,
                layer_name,
                state_vector,
            } => {
                let layer_name = normalize_layer_name(&layer_name, &state.page_id);
                if let Ok(mut subs) = state.subscribers.write() {
                    if let Some(info) = subs.get_mut(&(user_did.clone(), device_id.clone())) {
                        info.vectors.insert(layer_name.clone(), state_vector);
                        trace!(user_did = %user_did, device_id = %device_id, layer_name = %layer_name, "Updated in-memory peer vector");
                    }
                }
            }

            ScribeMessage::ReconcileWithPeers => {
                sync::handle_reconcile_with_peers(state).await;
            }

            ScribeMessage::GetSnapshot { layer_name, reply } => {
                let layer_name = normalize_layer_name(&layer_name, &state.page_id);
                let result = state.units.get(&layer_name).map(|unit| unit.layer().export_snapshot());
                let _ = reply.send(result);
            }

            ScribeMessage::GetLayerJson { layer_name, reply } => {
                let layer_name = normalize_layer_name(&layer_name, &state.page_id);
                let result = state.units.get(&layer_name).map(|unit| unit.layer().get_content(&layer_name));
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
                let layer_name = normalize_layer_name(&layer_name, &state.page_id);
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
                let layer_name = normalize_layer_name(&layer_name, &state.page_id);
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
                let layer_name = normalize_layer_name(&layer_name, &state.page_id);
                if let Err(e) = operations::handle_list_insert(state, &layer_name, &path, index, item).await {
                    warn!(error = %e, "ListInsert failed");
                }
            }

            ScribeMessage::ListDelete {
                layer_name,
                path,
                index,
            } => {
                let layer_name = normalize_layer_name(&layer_name, &state.page_id);
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
                let layer_name = normalize_layer_name(&layer_name, &state.page_id);
                if let Err(e) = operations::handle_map_insert(state, &layer_name, &path, &key, value).await {
                    warn!(error = %e, "MapInsert failed");
                }
            }

            ScribeMessage::MapDelete {
                layer_name,
                path,
                key,
            } => {
                let layer_name = normalize_layer_name(&layer_name, &state.page_id);
                if let Err(e) = operations::handle_map_delete(state, &layer_name, &path, &key).await {
                    warn!(error = %e, "MapDelete failed");
                }
            }

            ScribeMessage::CounterInc {
                layer_name,
                path,
                amount,
            } => {
                let layer_name = normalize_layer_name(&layer_name, &state.page_id);
                if let Err(e) = operations::handle_counter_inc(state, &layer_name, &path, amount).await {
                    warn!(error = %e, "CounterInc failed");
                }
            }

            ScribeMessage::CreateDynamicLayer { schema_key, layer_id, reply } => {
                let result = crate::layer_unit::handle_create_dynamic_layer(
                    state, &schema_key, &layer_id,
                );
                let _ = reply.send(result);
            }

            ScribeMessage::AddLayerAccess { layer_name, did, reply } => {
                let result = crate::layer_unit::handle_add_layer_access(state, &layer_name, &did).await;
                if let Err(ref e) = result {
                    warn!(
                        page_id = %state.page_id,
                        layer = %layer_name,
                        did = %did,
                        error = %e,
                        "AddLayerAccess failed"
                    );
                }
                let _ = reply.send(result);
            }

            ScribeMessage::RemoveLayerAccess { layer_name, did, reply } => {
                let result = crate::layer_unit::handle_remove_layer_access(state, &layer_name, &did);
                let _ = reply.send(result);
            }

            ScribeMessage::AuthorizeLayerSubscriber { layer_name, subscriber_did } => {
                let bare = normalize_layer_name(&layer_name, &state.page_id);
                if let Some(unit) = state.units.get(&bare) {
                    unit.authorize_did(&subscriber_did);
                    state.emit_layer_auth_capture(&bare, &subscriber_did, "authorized");
                    info!(layer = %bare, did = %subscriber_did, "Authorized subscriber for layer");
                } else {
                    // Layer doesn't exist yet — store pending authorization
                    state.pending_layer_authorizations
                        .entry(bare.clone())
                        .or_insert_with(Vec::new)
                        .push(subscriber_did.clone());
                    // Emit with pending_count showing queue size
                    let pending_count = state.pending_layer_authorizations
                        .get(&bare).map(|v| v.len()).unwrap_or(0);
                    if let Some(ref tx) = state.capture_tx {
                        let ts = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_millis())
                            .unwrap_or(0);
                        if let Ok(json) = serde_json::to_string(&serde_json::json!({
                            "type": "layer_auth",
                            "ts": ts,
                            "seq": state.capture_seq.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
                            "page_id": &state.page_id,
                            "layer": &bare,
                            "layer_short": crate::state::layer_short(&bare),
                            "did": &subscriber_did,
                            "short_did": crate::state::short_did(&subscriber_did),
                            "action": "pending",
                            "pending_count": pending_count,
                        })) {
                            let _ = tx.send(json);
                        }
                    }
                    info!(layer = %bare, did = %subscriber_did, pending_count = pending_count, "Stored pending layer authorization (layer not yet created)");
                }
            }

            ScribeMessage::Shutdown => {
                info!(page_id = %state.page_id, "Scribe shutting down");
                sync::handle_flush(state).await;
                myself.stop(Some("shutdown".to_string()));
            }

            // Ephemeral events (from datagrams via PeerActor)
            ScribeMessage::RemoteEphemeral { user_did, device_id, payload } => {
                // Log payload content for debugging
                let payload_str = std::str::from_utf8(&payload).unwrap_or("<binary>");
                debug!(
                    page_id = %state.page_id,
                    user_did = ?user_did,
                    device_id = ?device_id,
                    payload_len = payload.len(),
                    payload = %payload_str,
                    our_did = %state.our_did,
                    "Scribe received RemoteEphemeral"
                );
                // 1. Forward to local app layer (if we have sender info)
                if let Some(ref did) = user_did {
                    let handled = route_remote_ephemeral(state, did, &payload);
                    debug!(page_id = %state.page_id, handled = handled, our_did = %state.our_did, "route_remote_ephemeral returned");
                    if !handled {
                        let dev_id = device_id.as_deref().unwrap_or_default();
                        emit_raw_ephemeral(state, did, dev_id, &payload);
                    }
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

            ScribeMessage::SendStructuredEphemeral { func, args } => {
                // Log the incoming request
                let has_permit = state.our_permit.is_some();
                let ephemeral_funcs = state.our_permit
                    .as_ref()
                    .map(|p| p.ephemeral_funcs().to_vec())
                    .unwrap_or_default();
                debug!(
                    page_id = %state.page_id,
                    func = %func,
                    has_permit = has_permit,
                    ephemeral_funcs = ?ephemeral_funcs,
                    our_did = %state.our_did,
                    "SendStructuredEphemeral received"
                );

                // Validate func against our permit's ephemeral_funcs (if set)
                // Empty ephemeral_funcs list means all funcs allowed
                let can_send = state.our_permit
                    .as_ref()
                    .map(|p| p.can_send_ephemeral(&func))
                    .unwrap_or(true); // No permit = allow (local mode)

                if !can_send {
                    warn!(page_id = %state.page_id, func = %func, "Structured ephemeral func not allowed by permit");
                    return Ok(());
                }

                // Broadcast as StructuredEphemeral to app subscribers (local UI)
                let our_did = state.our_did.clone();
                emit_structured_ephemeral(state, &our_did, &func, &args);

                // Also broadcast via ephemeral channel for remote peers
                // PeerActor should be subscribed to this Scribe with ephemeral_tx
                // Include from_did so relayed messages preserve original sender identity
                let payload = serde_json::json!({
                    "type": "structured",
                    "from_did": our_did,
                    "func": func,
                    "args": args
                });
                if let Ok(bytes) = serde_json::to_vec(&payload) {
                    debug!(
                        page_id = %state.page_id,
                        func = %func,
                        "Broadcasting structured ephemeral to PeerActor subscribers"
                    );
                    broadcast_ephemeral_to_subscribers(state, &bytes, None);
                }
            }

            ScribeMessage::RemoteStructuredEphemeral { from_did, device_id, func, args } => {
                // Validate func against sender's permit
                let can_send = {
                    let subs = state.subscribers.read().ok();
                    subs.and_then(|s| {
                        s.get(&(from_did.clone(), device_id.clone()))
                            .map(|info| info.permit.can_send_ephemeral(&func))
                    }).unwrap_or(false) // Unknown sender = reject
                };

                if !can_send {
                    warn!(
                        page_id = %state.page_id,
                        from_did = %from_did,
                        func = %func,
                        "Remote structured ephemeral rejected - func not allowed"
                    );
                    return Ok(());
                }

                // Emit to local app subscribers
                emit_structured_ephemeral(state, &from_did, &func, &args);

                debug!(
                    page_id = %state.page_id,
                    from_did = %from_did,
                    func = %func,
                    "Processed remote structured ephemeral"
                );
            }

            // Note: Ephemeral events now go through SubscribeToPageUpdates (PageUpdate::Ephemeral)

            // Derivation operations (now handled by kunki/LuaRuntime externally)
            ScribeMessage::RebuildDerived { target: _, reply } => {
                // Derivation is now handled by kunki, not Scribe
                let _ = reply.send(Err("Derivation is handled externally by kunki".to_string()));
            }

            ScribeMessage::RebuildAllDerived { reply } => {
                // Derivation is now handled by kunki, not Scribe
                let _ = reply.send(Err("Derivation is handled externally by kunki".to_string()));
            }

            ScribeMessage::IsDerivationEnabled { reply } => {
                // Derivation is now external, Scribe always returns false
                let _ = reply.send(false);
            }

            ScribeMessage::CreateDerivedLayer { target_layer } => {
                let target_layer = normalize_layer_name(&target_layer, &state.page_id);
                // Create the derived layer (empty) so it exists for subscribers
                if !state.units.contains_key(&target_layer) {
                    let unit = LayerUnit::new_empty();
                    let _ = unit.layer().loro().get_map(target_layer.clone());
                    unit.layer().commit();
                    state.units.insert(target_layer.clone(), unit);
                    info!(target = %target_layer, "Created empty derived layer");

                    // Set up observer for the new layer
                    loro_observer::setup_layer_observer(state, &target_layer);
                }
            }

            ScribeMessage::GetAppFiles { app_name, reply } => {
                // Use bare layer name (Scribe uses bare names, no page_id/ prefix)
                let layer_name = format!("app:{}", app_name);
                let result = if let Some(unit) = state.units.get(&layer_name) {
                    Ok(unit.layer().get_all_files())
                } else {
                    Err(format!("App layer '{}' not found", layer_name))
                };
                let _ = reply.send(result);
            }

            ScribeMessage::GetSubscriberCount { reply } => {
                let count = state.subscribers.read()
                    .map(|s| s.len())
                    .unwrap_or(0);
                let _ = reply.send(count);
            }

            ScribeMessage::RefreshApp { app_name: _, app_dir: _, reply } => {
                // RefreshApp requires filesystem access which is handled by butler's refresh service
                // This stub returns an error - callers should use butler's refresh functionality instead
                let _ = reply.send(Err("RefreshApp not supported in base Scribe - use butler's refresh service".to_string()));
            }
        }

        Ok(())
    }

    #[instrument(skip_all, fields(page_id = %state.page_id))]
    async fn post_stop(
        &self,
        _myself: ActorRef<Self::Msg>,
        state: &mut Self::State,
    ) -> std::result::Result<(), ActorProcessingErr> {
        // Flush any dirty layers before stopping
        let has_dirty = state.units.values().any(|u| u.is_dirty());
        if has_dirty {
            let dirty_count = state.units.values().filter(|u| u.is_dirty()).count();
            info!(page_id = %state.page_id, dirty_count = dirty_count, "Flushing dirty layers on shutdown");
            sync::handle_flush(state).await;
        }

        // Stop node script if running
        if let Some(tx) = state.node_script_shutdown.take() {
            info!(page_id = %state.page_id, "Stopping node script tick loop");
            let _ = tx.send(()).await;
        }

        info!(page_id = %state.page_id, "Scribe stopped");
        Ok(())
    }
}

// Helper Methods

impl Scribe {
    /// Handle JSON update from UI (CEL commit())
    ///
    /// **Broadcast**: Handled automatically by Loro observer after commit()
    #[instrument(skip_all, fields(page_id = %state.page_id, layer = %layer_name, path = %path))]
    async fn handle_update_from_json(
        &self,
        state: &mut ScribeState,
        layer_name: &str,
        path: &str,
        value: serde_json::Value,
    ) -> Result<()> {
        info!(layer = %layer_name, path = %path, "Updating layer from JSON");

        // Get or create the layer unit
        let unit = state
            .units
            .entry(layer_name.to_string())
            .or_insert_with(LayerUnit::new_empty);

        // Convert JSON to Loro and update
        unit.layer()
            .set_from_json(path, &value)
            .map_err(|e| ScribeError::CrdtError(format!("Layer: {}", e)))?;

        // Commit to trigger Loro observer (which broadcasts to peers + notifies UI)
        unit.layer().commit();

        // Mark as dirty for persistence
        unit.mark_dirty();

        debug!("Layer update from JSON complete, observer will broadcast");
        Ok(())
    }
}

// Tests

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixtures::node_scribe_args;
    use std::time::Duration;

    /// Test that Scribe with no subscribers doesn't crash on ephemeral send
    #[tokio::test]
    async fn test_ephemeral_with_no_subscribers() {
        let args = node_scribe_args("test-page-456");
        let (scribe_ref, _handle) = ractor::Actor::spawn(
            Some("test-scribe-no-subs".to_string()),
            Scribe,
            args,
        ).await.expect("Failed to spawn Scribe");

        // Send ephemeral with no subscribers - should not crash
        scribe_ref.cast(ScribeMessage::SendStructuredEphemeral {
            func: "typing".to_string(),
            args: serde_json::json!({"user": "TestUser"}),
        }).expect("Failed to send ephemeral");

        // Small delay to ensure message is processed
        tokio::time::sleep(Duration::from_millis(50)).await;

        println!("✓ Ephemeral with no subscribers handled gracefully");
        scribe_ref.stop(Some("test complete".to_string()));
    }
}
