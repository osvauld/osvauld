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
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex};

use ractor::{Actor, ActorProcessingErr, ActorRef};
use tokio::time::{interval, Duration};
use tracing::{debug, info, instrument, trace, warn};

use domains::Layer;

use crate::ephemeral::{
    broadcast_ephemeral_to_subscribers, emit_raw_ephemeral, handle_remote_structured_ephemeral,
    handle_send_structured_ephemeral, route_remote_ephemeral,
};
use crate::layer_unit::LayerUnit;
use crate::message;
use crate::state::{normalize_layer_name, ScribeArgs, ScribeState, SyncMode};
use crate::{loro_observer, operations, query, sync, Result, ScribeError, ScribeMessage};

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
        let (our_permit, static_layers) = parse_permit(&args);
        spawn_timers(&args, &myself);
        let mut state = build_initial_state(args, our_permit, static_layers);
        setup_observers(&mut state);
        emit_ensure_sync_on_open(&state);
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
                sync::handle_subscribe(
                    state,
                    user_did,
                    device_id,
                    broadcast_tx,
                    ephemeral_tx,
                    permit,
                )
                .await;
            }

            ScribeMessage::Unsubscribe {
                user_did,
                device_id,
            } => {
                sync::handle_unsubscribe(state, &user_did, &device_id).await;
            }

            ScribeMessage::SubscribeToPageUpdates { tx } => {
                query::handle_subscribe_to_page_updates(state, tx);
            }

            ScribeMessage::ApplyUpdate {
                layer_name,
                update,
                from_peer,
                permit,
            } => {
                let layer_name = normalize_layer_name(&layer_name, &state.page_id);
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
                let _ = reply.send(sync::handle_sync_request(state, &layer_name, &their_vector));
            }

            ScribeMessage::ExportSnapshot { layer_name, reply } => {
                let layer_name = normalize_layer_name(&layer_name, &state.page_id);
                let _ = reply.send(sync::handle_export_snapshot(state, &layer_name));
            }

            ScribeMessage::GetStateVector { layer_name, reply } => {
                let layer_name = normalize_layer_name(&layer_name, &state.page_id);
                let _ = reply.send(sync::handle_get_state_vector(state, &layer_name));
            }

            ScribeMessage::GetUpdatesSince {
                layer_name,
                state_vector,
                reply,
            } => {
                let layer_name = normalize_layer_name(&layer_name, &state.page_id);
                let _ = reply.send(sync::handle_get_updates_since(
                    state,
                    &layer_name,
                    &state_vector,
                ));
            }

            ScribeMessage::ReplaceLayer {
                layer_name,
                snapshot,
                reply,
            } => {
                let layer_name = normalize_layer_name(&layer_name, &state.page_id);
                let result = sync::handle_replace_layer(state, &layer_name, &snapshot);
                let _ = reply.send(result);
            }

            ScribeMessage::Flush => {
                sync::handle_flush(state).await;
            }

            ScribeMessage::EnsureLoroList { layer_name, reply } => {
                let layer_name = normalize_layer_name(&layer_name, &state.page_id);
                ensure_layer_for_lua(state, &layer_name, "list");
                let _ = reply.send(Ok(()));
            }

            ScribeMessage::EnsureLoroMap { layer_name, reply } => {
                let layer_name = normalize_layer_name(&layer_name, &state.page_id);
                ensure_layer_for_lua(state, &layer_name, "map");
                let _ = reply.send(Ok(()));
            }

            ScribeMessage::LayerExists { layer_name, reply } => {
                let layer_name = normalize_layer_name(&layer_name, &state.page_id);
                let _ = reply.send(state.units.contains_key(&layer_name));
            }

            ScribeMessage::ListGet {
                layer_name,
                index,
                reply,
            } => {
                let layer_name = normalize_layer_name(&layer_name, &state.page_id);
                let result = state
                    .units
                    .get(&layer_name)
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
                let result = state
                    .units
                    .get(&layer_name)
                    .map(|unit| unit.layer())
                    .ok_or_else(|| ScribeError::LayerNotFound(layer_name.clone()))
                    .map(|layer| {
                        let list = layer.loro().get_list(layer_name.clone());
                        list.len()
                    });
                let _ = reply.send(result);
            }

            ScribeMessage::MapGet {
                layer_name,
                key,
                reply,
            } => {
                let layer_name = normalize_layer_name(&layer_name, &state.page_id);
                let result = state
                    .units
                    .get(&layer_name)
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
                let result = state
                    .units
                    .get(&layer_name)
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
                let result = state
                    .units
                    .get(&layer_name)
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
                let _ = reply.send(query::handle_list_layers(state, &pattern));
            }

            ScribeMessage::GetLayerData { layer_name, reply } => {
                let layer_name = normalize_layer_name(&layer_name, &state.page_id);
                let _ = reply.send(query::handle_get_layer_data(state, &layer_name));
            }

            ScribeMessage::UpdatePeerVector {
                user_did,
                device_id,
                layer_name,
                state_vector,
            } => {
                let layer_name = normalize_layer_name(&layer_name, &state.page_id);
                if let Some(unit) = state.units.get(&layer_name) {
                    unit.update_subscriber_vector(&user_did, &device_id, state_vector);
                    trace!(user_did = %user_did, device_id = %device_id, layer_name = %layer_name, "Updated in-memory peer vector");
                }
            }

            ScribeMessage::ReconcileWithPeers => {
                sync::handle_reconcile_with_peers(state).await;
            }

            ScribeMessage::GetSnapshot { layer_name, reply } => {
                let layer_name = normalize_layer_name(&layer_name, &state.page_id);
                let _ = reply.send(
                    state
                        .units
                        .get(&layer_name)
                        .map(|unit| unit.layer().export_snapshot()),
                );
            }

            ScribeMessage::GetLayerJson { layer_name, reply } => {
                let layer_name = normalize_layer_name(&layer_name, &state.page_id);
                let _ = reply.send(
                    state
                        .units
                        .get(&layer_name)
                        .map(|unit| unit.layer().get_content(&layer_name)),
                );
            }

            ScribeMessage::GetContext { reply } => {
                let _ = reply.send(query::build_context_json(state));
            }

            ScribeMessage::Query { spec, reply } => {
                let _ = reply.send(query::handle_query(state, &spec));
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
                let result = handle_update_from_json(state, &layer_name, &path, value).await;
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
                if let Err(e) = operations::handle_list_push(state, &layer_name, &path, item).await
                {
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
                if let Err(e) =
                    operations::handle_list_insert(state, &layer_name, &path, index, item).await
                {
                    warn!(error = %e, "ListInsert failed");
                }
            }

            ScribeMessage::ListDelete {
                layer_name,
                path,
                index,
            } => {
                let layer_name = normalize_layer_name(&layer_name, &state.page_id);
                if let Err(e) =
                    operations::handle_list_delete(state, &layer_name, &path, index).await
                {
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
                if let Err(e) =
                    operations::handle_map_insert(state, &layer_name, &path, &key, value).await
                {
                    warn!(error = %e, "MapInsert failed");
                }
            }

            ScribeMessage::MapDelete {
                layer_name,
                path,
                key,
            } => {
                let layer_name = normalize_layer_name(&layer_name, &state.page_id);
                if let Err(e) = operations::handle_map_delete(state, &layer_name, &path, &key).await
                {
                    warn!(error = %e, "MapDelete failed");
                }
            }

            ScribeMessage::CounterInc {
                layer_name,
                path,
                amount,
            } => {
                let layer_name = normalize_layer_name(&layer_name, &state.page_id);
                if let Err(e) =
                    operations::handle_counter_inc(state, &layer_name, &path, amount).await
                {
                    warn!(error = %e, "CounterInc failed");
                }
            }

            ScribeMessage::CreateDynamicLayer {
                schema_key,
                layer_id,
                authorized_peers,
                reply,
            } => {
                let result = crate::layer_unit::handle_create_dynamic_layer(
                    state,
                    &schema_key,
                    &layer_id,
                    authorized_peers.as_deref(),
                );
                let _ = reply.send(result);
            }

            ScribeMessage::AddLayerAccess {
                layer_name,
                dids,
                reply,
            } => {
                let result = crate::layer_unit::handle_add_layer_access(state, &layer_name, &dids);
                if let Err(ref e) = result {
                    warn!(
                        page_id = %state.page_id, layer = %layer_name,
                        dids = ?dids, error = %e, "AddLayerAccess failed"
                    );
                }
                let _ = reply.send(result);
            }

            ScribeMessage::ExportLayerSnapshot { layer_name, reply } => {
                let layer_name = normalize_layer_name(&layer_name, &state.page_id);
                let _ = reply.send(sync::handle_export_layer_snapshot(state, &layer_name));
            }

            ScribeMessage::AuthorizeLayerSubscriber {
                layer_name,
                subscriber_did,
            } => {
                sync::subscription::handle_authorize_layer_subscriber(
                    state,
                    &layer_name,
                    &subscriber_did,
                );
            }

            ScribeMessage::GetUnsyncedSyncMeta { reply } => {
                let entries = sync::sync_meta::read_unsynced_entries(state, &state.our_did.clone());
                let _ = reply.send(entries);
            }

            ScribeMessage::HandleLayerSubscribe {
                layer_name,
                peer_did,
                reply,
            } => {
                let result = sync::subscription::handle_layer_subscribe(
                    state,
                    &layer_name,
                    &peer_did,
                    crate::permit::issue_layer_permit_for_subscribe,
                );
                let _ = reply.send(result);
            }

            ScribeMessage::MarkSyncMetaSynced { layer_name } => {
                let our_did = state.our_did.clone();
                sync::sync_meta::mark_entry_synced(state, &our_did, &layer_name);
            }

            ScribeMessage::StoreLayerAuthority {
                layer_name,
                creator_did,
                authority_token,
                layer_data,
                state_vector: _sv,
            } => {
                sync::sync_meta::handle_store_layer_authority(
                    state,
                    &layer_name,
                    &creator_did,
                    &authority_token,
                    &layer_data,
                );
            }

            ScribeMessage::FanOutLayerToUsers {
                layer_name,
                authorized_peers,
            } => {
                sync::sync_meta::handle_fan_out_layer_to_users(
                    state,
                    &layer_name,
                    authorized_peers.as_deref(),
                );
            }

            ScribeMessage::Shutdown => {
                info!(page_id = %state.page_id, "Scribe shutting down");
                sync::handle_flush(state).await;
                myself.stop(Some("shutdown".to_string()));
            }

            ScribeMessage::RemoteEphemeral {
                user_did,
                device_id,
                payload,
            } => {
                handle_remote_ephemeral(state, user_did.as_deref(), device_id.as_deref(), &payload);
            }

            ScribeMessage::SendEphemeral { payload } => {
                debug!(page_id = %state.page_id, payload_len = payload.len(), "Broadcasting ephemeral to all subscribers");
                broadcast_ephemeral_to_subscribers(state, &payload, None);
            }

            ScribeMessage::SendStructuredEphemeral { func, args } => {
                handle_send_structured_ephemeral(state, &func, &args);
            }

            ScribeMessage::RemoteStructuredEphemeral {
                from_did,
                device_id,
                func,
                args,
            } => {
                handle_remote_structured_ephemeral(state, &from_did, &device_id, &func, &args);
            }

            ScribeMessage::CreateDerivedLayer { target_layer } => {
                let target_layer = normalize_layer_name(&target_layer, &state.page_id);
                if !state.units.contains_key(&target_layer) {
                    let unit = LayerUnit::new_empty();
                    let _ = unit.layer().loro().get_map(target_layer.clone());
                    unit.layer().commit();
                    state.units.insert(target_layer.clone(), unit);
                    info!(target = %target_layer, "Created empty derived layer");
                    loro_observer::setup_layer_observer(state, &target_layer);
                }
            }

            ScribeMessage::GetAppFiles { app_name, reply } => {
                let layer_name = format!("app:{}", app_name);
                let result = if let Some(unit) = state.units.get(&layer_name) {
                    Ok(unit.layer().get_all_files())
                } else {
                    Err(format!("App layer '{}' not found", layer_name))
                };
                let _ = reply.send(result);
            }

            ScribeMessage::GetSubscriberCount { reply } => {
                let count = state.subscribers.read().map(|s| s.len()).unwrap_or(0);
                let _ = reply.send(count);
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
        let has_dirty = state.units.values().any(|u| u.is_dirty());
        if has_dirty {
            let dirty_count = state.units.values().filter(|u| u.is_dirty()).count();
            info!(page_id = %state.page_id, dirty_count = dirty_count, "Flushing dirty layers on shutdown");
            sync::handle_flush(state).await;
        }

        if let Some(tx) = state.node_script_shutdown.take() {
            info!(page_id = %state.page_id, "Stopping node script tick loop");
            let _ = tx.send(()).await;
        }

        info!(page_id = %state.page_id, "Scribe stopped");
        Ok(())
    }
}

// pre_start Helpers

/// Parse our permit from args, returning the parsed permit and static layer names
fn parse_permit(args: &ScribeArgs) -> (Option<gurkha::Permit>, Vec<String>) {
    if let Some(ref permit_token) = args.our_permit {
        match gurkha::Permit::from_token(permit_token) {
            Ok(permit) => {
                let static_layers = permit.static_layers(&args.page_id, &args.our_did);
                info!(
                    page_id = %args.page_id,
                    token_type = %permit.token_type().unwrap_or("peer"),
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
    }
}

/// Spawn periodic flush and reconciliation timers
fn spawn_timers(args: &ScribeArgs, myself: &ActorRef<ScribeMessage>) {
    // Flush timer - saves dirty layers every 10 seconds
    let myself_flush = myself.clone();
    tokio::spawn(async move {
        let mut flush_interval = interval(Duration::from_secs(10));
        loop {
            flush_interval.tick().await;
            let _ = myself_flush.cast(ScribeMessage::Flush);
        }
    });

    // Reconciliation timer - catches missed syncs every 30 seconds
    // Only for Node mode (Broadcast) - User mode (ToSource) relies on Node to initiate sync
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
}

/// Convert args.layers into LayerUnit instances
fn build_layer_units(layers: HashMap<String, Layer>) -> HashMap<String, LayerUnit> {
    layers
        .into_iter()
        .map(|(name, layer)| (name, LayerUnit::new(layer)))
        .collect()
}

/// Build initial ScribeState from args and permit
fn build_initial_state(
    args: ScribeArgs,
    our_permit: Option<gurkha::Permit>,
    static_layers: Vec<String>,
) -> ScribeState {
    let mode_info = args
        .sync_config
        .as_ref()
        .map(|c| format!("mode={:?}, target={:?}", c.mode, c.sync_target))
        .unwrap_or_else(|| "no sync config".to_string());
    info!(page_id = %args.page_id, %mode_info, "Scribe started");

    info!(
        page_id = %args.page_id,
        is_node = args.is_node,
        has_validation_handle = args.validation_handle.is_some(),
        "Scribe initialized (validation delegated to kunki)"
    );

    let mut units = build_layer_units(args.layers);
    precreate_static_layers(&mut units, &static_layers);

    let state = ScribeState {
        page_id: args.page_id.clone(),
        units: HashMap::new(),
        subscribers: Arc::new(std::sync::RwLock::new(HashMap::new())),
        query_subscribers: HashMap::new(),
        layer_storage: args.layer_storage,
        vector_storage: args.vector_storage,
        peer_resolver: args.peer_resolver,
        permit_issuer: args.permit_issuer,
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
    };

    // Set is_local_only on each unit based on permit
    for (name, unit) in units.iter_mut() {
        unit.set_local_only(!state.should_sync_layer(name));
    }

    let mut state = state;
    state.units = units;
    state
}

/// Pre-create static layers from permit that don't already exist
fn precreate_static_layers(units: &mut HashMap<String, LayerUnit>, static_layers: &[String]) {
    for layer_name in static_layers {
        if !units.contains_key(layer_name) {
            info!(layer = %layer_name, "Pre-creating static layer from permit");
            units.insert(layer_name.clone(), LayerUnit::new_empty());
        }
    }
}

/// Setup Loro observers for all existing layers
fn setup_observers(state: &mut ScribeState) {
    loro_observer::setup_observers_for_all_layers(state);
}

/// Emit EnsureSync on page open to trigger remote subscription
fn emit_ensure_sync_on_open(state: &ScribeState) {
    let Some(ref tx) = state.sync_event_tx else {
        return;
    };

    let sync_targets = sync::reconcile::get_sync_targets(state);
    if sync_targets.is_empty() {
        debug!(page_id = %state.page_id, "No sync targets on page open");
        return;
    }

    info!(page_id = %state.page_id, targets = ?sync_targets, "Emitting EnsureSync for {} targets on page open", sync_targets.len());
    for user_did in sync_targets {
        let event = message::SyncEvent::EnsureSync {
            user_did: user_did.clone(),
        };
        state.emit_sync_event_capture(&event);
        match tx.try_send(event) {
            Ok(()) => {
                info!(user_did = %user_did, page_id = %state.page_id, "Emitted EnsureSync on page open")
            }
            Err(e) => {
                warn!(user_did = %user_did, error = %e, "Failed to emit EnsureSync on page open")
            }
        }
    }
}

// Message Handler Helpers

/// Handle JSON update from UI (CEL commit())
///
/// **Broadcast**: Handled automatically by Loro observer after commit()
#[instrument(skip_all, fields(page_id = %state.page_id, layer = %layer_name, path = %path))]
async fn handle_update_from_json(
    state: &mut ScribeState,
    layer_name: &str,
    path: &str,
    value: serde_json::Value,
) -> Result<()> {
    info!(layer = %layer_name, path = %path, "Updating layer from JSON");

    let unit = state
        .units
        .entry(layer_name.to_string())
        .or_insert_with(LayerUnit::new_empty);

    unit.layer()
        .set_from_json(path, &value)
        .map_err(|e| ScribeError::CrdtError(format!("Layer: {}", e)))?;

    unit.layer().commit();
    unit.mark_dirty();

    debug!("Layer update from JSON complete, observer will broadcast");
    Ok(())
}

/// Ensure a layer exists for Lua EnsureLoroList/EnsureLoroMap
fn ensure_layer_for_lua(state: &mut ScribeState, layer_name: &str, kind: &str) {
    if state.units.contains_key(layer_name) {
        return;
    }

    let mut unit = LayerUnit::new_empty();
    unit.set_local_only(!state.should_sync_layer(layer_name));
    unit.mark_dirty();
    state.units.insert(layer_name.to_string(), unit);
    debug!(layer = %layer_name, kind = %kind, "Created new layer for Lua");
    loro_observer::setup_layer_observer(state, layer_name);
    operations::auto_subscribe_sync_target(state, layer_name);
    sync::notify_layer_discovered(state, layer_name);
}

/// Handle RemoteEphemeral message
fn handle_remote_ephemeral(
    state: &mut ScribeState,
    user_did: Option<&str>,
    device_id: Option<&str>,
    payload: &[u8],
) {
    let payload_str = std::str::from_utf8(payload).unwrap_or("<binary>");
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
    if let Some(did) = user_did {
        let handled = route_remote_ephemeral(state, did, payload);
        debug!(page_id = %state.page_id, handled = handled, our_did = %state.our_did, "route_remote_ephemeral returned");
        if !handled {
            let dev_id = device_id.unwrap_or_default();
            emit_raw_ephemeral(state, did, dev_id, payload);
        }
    } else {
        debug!(page_id = %state.page_id, "RemoteEphemeral without user_did - not emitting to local app");
    }

    // 2. Relay to other subscribers (node mode relay) - exclude sender
    let exclude = match (user_did, device_id) {
        (Some(d), Some(dev)) => Some((d, dev)),
        _ => None,
    };
    broadcast_ephemeral_to_subscribers(state, payload, exclude);
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
        let (scribe_ref, _handle) =
            ractor::Actor::spawn(Some("test-scribe-no-subs".to_string()), Scribe, args)
                .await
                .expect("Failed to spawn Scribe");

        // Send ephemeral with no subscribers - should not crash
        scribe_ref
            .cast(ScribeMessage::SendStructuredEphemeral {
                func: "typing".to_string(),
                args: serde_json::json!({"user": "TestUser"}),
            })
            .expect("Failed to send ephemeral");

        // Small delay to ensure message is processed
        tokio::time::sleep(Duration::from_millis(50)).await;

        println!("✓ Ephemeral with no subscribers handled gracefully");
        scribe_ref.stop(Some("test complete".to_string()));
    }
}
