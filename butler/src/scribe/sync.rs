//! Sync handling for Scribe actor
//!
//! Handles peer subscription, broadcasting, reconciliation, and sync target resolution.

use std::collections::HashMap;

use logging_utils::ShortLayer;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn, instrument};

use crate::error::{ButlerError, Result};
use crate::Layer;

use super::message::{BroadcastPayload, PageEvent, PageEventType, SyncEvent};
use super::loro_observer::{set_pending_update_source, clear_pending_update_source};
use super::permit::{extract_patterns_from_permit, extract_role_from_permit, parse_permit_for_layers, Permissions};
use super::state::{LayerWritePermission, ScribeState, SubscriberInfo, SyncPolicy};

// =============================================================================
// Subscription Handlers
// =============================================================================

/// Handle peer subscription
///
/// **Context**: PeerActor subscribes to receive updates for this page
/// **We do**: Parse permit, extract permissions, load stored vectors
///
/// **Pattern expansion**: For patterns like `{page_id}/orders/{aud}`, we use the permit's
/// audience field (the permit holder's DID) for expansion, not the subscriber's DID.
/// This ensures layers created by the permit holder match correctly.
#[instrument(skip(state, broadcast_tx, ephemeral_tx, permit), fields(page_id = %state.page_id))]
pub async fn handle_subscribe(
    state: &mut ScribeState,
    user_did: String,
    device_id: String,
    broadcast_tx: mpsc::Sender<BroadcastPayload>,
    ephemeral_tx: Option<mpsc::Sender<super::message::EphemeralOutbound>>,
    permit: String,
) {
    info!(user_did = %user_did, device_id = %device_id, permit_len = permit.len(), "Peer subscribing");

    // Parse permit to extract layer permissions and sync policies
    let (raw_layer_permissions, sync_policy) = match parse_permit_for_layers(&permit) {
        Ok(result) => result,
        Err(e) => {
            warn!(error = %e, "Failed to parse permit, using empty permissions");
            (HashMap::new(), SyncPolicy::default())
        }
    };

    // Expand {page_id} placeholders in layer permission keys
    // The permit has keys like "{page_id}/products" but actual layer names are expanded
    let layer_permissions: HashMap<String, LayerWritePermission> = raw_layer_permissions
        .into_iter()
        .map(|(key, value)| {
            let expanded_key = key.replace("{page_id}", &state.page_id);
            (expanded_key, value)
        })
        .collect();

    // Extract permit holder's DID from the permit's audience field
    // This is used for pattern expansion (e.g., {aud} in layer patterns)
    // The permit holder is the one who can create layers matching their patterns
    //
    // Note: The UCAN audience may be stored as base64-encoded public key or DID format.
    // Layer names use DID format (did:key:...), so we normalize to DID for pattern matching.
    let permit_holder_did = match gurkha::Permit::from_token(&permit) {
        Ok(parsed) => {
            let raw_aud = parsed.parsed().audience().to_string();
            // If already a DID, use as-is; otherwise convert from base64
            if raw_aud.starts_with("did:key:") {
                raw_aud
            } else {
                // Assume base64-encoded public key, convert to DID
                match herald::Identity::did_from_base64_pubkey(&raw_aud) {
                    Ok(did) => did,
                    Err(e) => {
                        warn!(error = %e, raw_aud = %raw_aud, "Failed to convert audience to DID, using raw value");
                        raw_aud
                    }
                }
            }
        }
        Err(e) => {
            warn!(error = %e, "Failed to extract audience from permit, falling back to user_did");
            user_did.clone()
        }
    };

    // Load stored state vectors for this peer
    let vectors = match (state.load_peer_vector)(&user_did, &device_id) {
        Ok(Some(v)) => v,
        Ok(None) => HashMap::new(),
        Err(e) => {
            warn!(error = %e, "Failed to load peer vectors");
            HashMap::new()
        }
    };

    // Extract pattern rules from permit for role-agnostic access control
    let (readable_patterns, writable_patterns) = extract_patterns_from_permit(&permit, &state.page_id);

    let pattern_strs: Vec<_> = readable_patterns.iter()
        .map(|p| format!("{}(sync={})", p.pattern, p.sync))
        .collect();
    info!(
        user_did = %user_did,
        permit_holder_did = %permit_holder_did,
        layer_permissions = ?layer_permissions.keys().collect::<Vec<_>>(),
        readable_patterns = ?pattern_strs,
        "Subscription permissions parsed"
    );

    if let Ok(mut subs) = state.subscribers.write() {
        subs.insert(
            (user_did.clone(), device_id.clone()),
            SubscriberInfo {
                broadcast_tx: broadcast_tx.clone(),
                ephemeral_tx,  // Passed from PeerActor for direct ephemeral routing
                vectors,
                layer_permissions: layer_permissions.clone(),
                sync_policy: sync_policy.clone(),
                // Use permit holder's DID for pattern expansion, not subscriber's DID
                // This ensures patterns like {page_id}/orders/{aud} match correctly
                subscriber_did: permit_holder_did,
                readable_patterns,
                writable_patterns,
            },
        );
        debug!(subscriber_count = subs.len(), "Subscriber added");
    }

    // Notify app subscribers about peer joining (for online counters, presence)
    if let Ok(subs) = state.ephemeral_subscribers.read() {
        for tx in subs.iter() {
            let _ = tx.try_send(super::message::EphemeralEvent::PeerJoined {
                user_did: user_did.clone(),
            });
        }
        if !subs.is_empty() {
            info!(user_did = %user_did, "Emitted PeerJoined event to {} local subscribers", subs.len());
        }
    }

    // Broadcast PeerJoined to other connected peers so their client apps can update online count
    // Also send peer_count to the joining peer so they know how many are already online
    // Format: {"type":"peer_joined","user_did":"..."} or {"type":"peer_count","count":N}
    let join_payload = format!(r#"{{"type":"peer_joined","user_did":"{}"}}"#, user_did);
    if let Ok(subs) = state.subscribers.read() {
        let total_count = subs.len();

        for ((sub_did, sub_device), info) in subs.iter() {
            if let Some(ref eph_tx) = info.ephemeral_tx {
                if sub_did == &user_did && sub_device == &device_id {
                    // Send current peer count to the joining peer
                    let count_payload = format!(r#"{{"type":"peer_count","count":{}}}"#, total_count);
                    let _ = eph_tx.try_send(super::message::EphemeralOutbound {
                        page_id: state.page_id.clone(),
                        payload: count_payload.as_bytes().to_vec(),
                    });
                } else {
                    // Send join notification to other peers
                    let _ = eph_tx.try_send(super::message::EphemeralOutbound {
                        page_id: state.page_id.clone(),
                        payload: join_payload.as_bytes().to_vec(),
                    });
                }
            }
        }
        info!(user_did = %user_did, total_peers = total_count, "Broadcast PeerJoined/Count to peers");
    }

    // Send initial state to new subscriber for layers they have access to
    // Read subscriber info to get patterns for access check
    let subscriber_info = if let Ok(subs) = state.subscribers.read() {
        subs.get(&(user_did.clone(), device_id.clone())).map(|info| {
            (info.subscriber_did.clone(), info.readable_patterns.clone())
        })
    } else {
        None
    };

    if let Some((subscriber_did, readable_patterns)) = subscriber_info {
        send_initial_state_to_subscriber(
            state,
            &user_did,
            &device_id,
            &broadcast_tx,
            &layer_permissions,
            &sync_policy,
            &subscriber_did,
            &readable_patterns,
        ).await;
    }
}

/// Send initial state to a newly subscribed peer
///
/// **Context**: Peer just subscribed, may have missed previous updates
/// **We do**: Send current snapshot for each layer they have permission for
/// **Checks**: Both fixed layer_permissions AND pattern-based readable_patterns
async fn send_initial_state_to_subscriber(
    state: &ScribeState,
    user_did: &str,
    device_id: &str,
    broadcast_tx: &mpsc::Sender<BroadcastPayload>,
    layer_permissions: &HashMap<String, LayerWritePermission>,
    sync_policy: &SyncPolicy,
    subscriber_did: &str,
    readable_patterns: &[super::state::PatternRule],
) {
    // Get peer's stored vectors (if any)
    let peer_vectors = match (state.load_peer_vector)(user_did, device_id) {
        Ok(Some(v)) => v,
        Ok(None) => HashMap::new(),
        Err(_) => HashMap::new(),
    };

    for (layer_name, layer) in &state.layers {
        // Skip layers in no_incoming_updates
        if sync_policy.no_incoming_updates.contains(layer_name) {
            continue;
        }

        // Check if they can receive this layer (fixed permission OR pattern match)
        let has_fixed_permission = layer_permissions.contains_key(layer_name);
        let has_pattern_permission = readable_patterns.iter().any(|pattern| {
            pattern.sync && pattern.matches(layer_name, &state.page_id, subscriber_did)
        });

        if !has_fixed_permission && !has_pattern_permission {
            continue;
        }

        // Determine what to send: incremental or full snapshot
        let data = if let Some(their_vector) = peer_vectors.get(layer_name) {
            if their_vector.is_empty() {
                layer.export_snapshot()
            } else {
                match layer.export_updates(their_vector) {
                    Ok(updates) if updates.is_empty() => {
                        // No new updates since their last sync
                        debug!(user_did = %user_did, layer = %layer_name, "No new updates for subscriber");
                        continue;
                    }
                    Ok(updates) => updates,
                    Err(e) => {
                        warn!(error = %e, layer = %layer_name, "Failed to export incremental, using snapshot");
                        layer.export_snapshot()
                    }
                }
            }
        } else {
            // No stored vector - send full snapshot
            layer.export_snapshot()
        };

        // Get our current state vector for 3-step sync protocol
        let state_vector = layer.version_vector();

        // Skip empty data or empty state vectors (no operations)
        // Empty LoroDoc has state_vector [0] (single byte indicating 0 entries)
        if data.is_empty() || state_vector.len() <= 1 {
            debug!(user_did = %user_did, layer = %layer_name, "Skipping empty layer in initial state");
            continue;
        }

        let payload = BroadcastPayload {
            page_id: state.page_id.clone(),
            layer_name: layer_name.clone(),
            update: data,
            state_vector,
        };

        if let Err(e) = broadcast_tx.try_send(payload) {
            warn!(user_did = %user_did, layer = %layer_name, error = %e, "Failed to send initial state");
        } else {
            debug!(user_did = %user_did, layer = %layer_name, "Sent initial state to subscriber");
        }
    }
}

/// Handle peer unsubscription
#[instrument(skip(state), fields(page_id = %state.page_id))]
pub async fn handle_unsubscribe(
    state: &mut ScribeState,
    user_did: &str,
    device_id: &str,
) {
    info!(user_did = %user_did, device_id = %device_id, "Peer unsubscribing");

    if let Ok(mut subs) = state.subscribers.write() {
        // Save their vectors before removing
        if let Some(info) = subs.get(&(user_did.to_string(), device_id.to_string())) {
            if let Err(e) = (state.save_peer_vector)(user_did, device_id, &info.vectors) {
                warn!(error = %e, "Failed to save peer vectors on unsubscribe");
            }
        }

        subs.remove(&(user_did.to_string(), device_id.to_string()));
        debug!(subscriber_count = subs.len(), "Subscriber removed");
    }

    // Notify app subscribers about peer leaving (for online counters, presence)
    if let Ok(subs) = state.ephemeral_subscribers.read() {
        for tx in subs.iter() {
            let _ = tx.try_send(super::message::EphemeralEvent::PeerLeft {
                user_did: user_did.to_string(),
            });
        }
        if !subs.is_empty() {
            info!(user_did = %user_did, "Emitted PeerLeft event to {} local subscribers", subs.len());
        }
    }

    // Broadcast PeerLeft to other connected peers so their client apps can update online count
    // This is sent via ephemeral datagram - format: {"type":"peer_left","user_did":"..."}
    let leave_payload = format!(r#"{{"type":"peer_left","user_did":"{}"}}"#, user_did);
    if let Ok(subs) = state.subscribers.read() {
        for (_, info) in subs.iter() {
            if let Some(ref eph_tx) = info.ephemeral_tx {
                let _ = eph_tx.try_send(super::message::EphemeralOutbound {
                    page_id: state.page_id.clone(),
                    payload: leave_payload.as_bytes().to_vec(),
                });
            }
        }
        info!(user_did = %user_did, "Broadcast PeerLeft to {} remaining peers", subs.len());
    }
}

// =============================================================================
// Update Handlers
// =============================================================================

/// Handle layer update (local or remote)
///
/// **Context**: Edit from UI or sync push from peer
/// **We do**: Permission check, validation, CRDT merge
/// **Broadcast**: Handled automatically by Loro observer (set up at startup)
/// **Returns**: Ok(()) on success, Err(message) on failure
#[instrument(skip(state, update, permit), fields(page_id = %state.page_id, layer = %ShortLayer(layer_name)))]
pub async fn handle_apply_update(
    state: &mut ScribeState,
    layer_name: &str,
    update: &[u8],
    from_peer: Option<(String, String)>,
    permit: Option<&str>,
) -> std::result::Result<(), String> {
    let _ = permit; // Permit is used for subscription in Courier, not here

    // Skip local_only layers for remote updates
    if from_peer.is_some() && Permissions::is_local_only(layer_name) {
        warn!("Rejected update for local_only layer from remote peer");
        return Err("local_only layer cannot be updated by remote peer".to_string());
    }

    // Permission check for remote updates
    if let Some(ref peer) = from_peer {
        if !Permissions::can_write(state, peer, layer_name) {
            warn!(user_did = %peer.0, "Permission denied: cannot write to layer");
            return Err("Permission denied: cannot write to layer".to_string());
        }
        // Note: submitter namespace validation is client-side (trusted)
    }

    // Extract ops from update for validation and derivation
    // Flow: Extract ops → Validate with Lua → Apply → Run derivation
    //
    // We extract ops once and use for both:
    // 1. Validation (before apply) - reject invalid updates
    // 2. Derivation (after apply) - transform changed entries only
    let extracted_ops = if from_peer.is_some() {
        // For existing layers, extract ops by comparing current state to updated state
        // For new layers, extract ops by comparing empty state to updated state (all inserts)
        let is_new_layer = !state.layers.contains_key(layer_name);
        let layer_for_extraction = state.layers.get(layer_name)
            .cloned()
            .unwrap_or_else(|| crate::models::Layer::new());

        match super::lua_runtime::extract_ops_from_update(&layer_for_extraction, update) {
            Ok(ops) => {
                debug!(layer = %layer_name, op_count = ops.len(), is_new_layer = is_new_layer, "Extracted ops from update");
                Some(ops)
            }
            Err(e) => {
                warn!(layer = %layer_name, error = %e, "Failed to extract ops for validation");
                // Continue anyway - extraction failure shouldn't block sync
                None
            }
        }
    } else {
        None // Local updates don't need validation
    };

    // Business logic validation (Lua) for remote updates
    if from_peer.is_some() {
        if let Some(ref lua_runtime) = state.lua_runtime {
            if let Some(ref ops) = extracted_ops {
                if !ops.is_empty() {
                    let from_did = from_peer.as_ref().map(|(d, _)| d.as_str()).unwrap_or("unknown");

                    // Get role from peer's stored permit (if node mode) or default to "peer"
                    let role = get_peer_role(state, from_did);

                    match lua_runtime.validate_ops(layer_name, ops, from_did, &role) {
                        Ok((true, _)) => {
                            debug!(layer = %layer_name, from_did = %from_did, role = %role, "Validation passed");
                        }
                        Ok((false, error_msg)) => {
                            let msg = error_msg.unwrap_or_else(|| "Validation failed".to_string());
                            warn!(layer = %layer_name, from_did = %from_did, role = %role, error = %msg, "Validation rejected update");
                            return Err(format!("Validation failed: {}", msg));
                        }
                        Err(e) => {
                            warn!(layer = %layer_name, error = %e, "Validation error, rejecting update");
                            return Err(format!("Validation error: {}", e));
                        }
                    }
                }
            }
        }
    }

    // Get or create layer for CRDT merge
    // For peer updates, we auto-create the layer if it doesn't exist
    // This enables P2P layer discovery (e.g., customer creates orders layer, owner sees it)
    let is_new_layer = !state.layers.contains_key(layer_name);
    if is_new_layer {
        if from_peer.is_some() {
            state.layers.insert(layer_name.to_string(), crate::models::Layer::new());
            state.dirty_layers.insert(layer_name.to_string());
            info!(layer = %layer_name, "Created new layer from peer sync");
            // Set up Loro observer for new layer to enable sync broadcasts
            super::loro_observer::setup_layer_observer(state, layer_name);
            // Notify UI subscribers about the new layer
            notify_layer_discovered(state, layer_name);
        } else {
            // Local updates should only go to existing layers
            warn!("Layer not found for local update");
            return Err("Layer not found".to_string());
        }
    }

    let Some(layer) = state.layers.get(layer_name) else {
        warn!("Layer not found");
        return Err("Layer not found".to_string());
    };

    // Set pending update source BEFORE apply() so observer knows who caused this change
    set_pending_update_source(state, from_peer.clone());

    if let Err(e) = layer.apply(update) {
        // Clear on error too
        clear_pending_update_source(state);
        error!(error = %e, "Failed to apply update to layer");
        return Err(format!("Failed to apply update: {}", e));
    }

    // Clear pending update source AFTER apply() (observer already captured it)
    clear_pending_update_source(state);

    state.dirty_layers.insert(layer_name.to_string());
    debug!("Update applied successfully to layer {}", layer_name);

    // Manually notify page event subscribers after remote update applied
    // (subscribe_root may not fire for apply(), only for commit())
    if from_peer.is_some() {
        // For app layers, flush immediately to storage so restart loads new code
        // (app restart reads from storage, not in-memory layer)
        if layer_name.starts_with("app:") {
            if let Some(layer) = state.layers.get(layer_name) {
                let snapshot = layer.export_snapshot();
                if let Err(e) = (state.save_layer)(layer_name, &snapshot) {
                    error!(layer = %layer_name, error = %e, "Failed to save app layer immediately");
                } else {
                    info!(layer = %layer_name, "Saved app layer immediately for restart");
                    // Remove from dirty set since we just saved it
                    state.dirty_layers.remove(layer_name);
                }
            }
        }

        if let Some(layer) = state.layers.get(layer_name) {
            let page_event = PageEvent {
                layer_name: layer_name.to_string(),
                from_peer: from_peer.clone(),
                event_type: PageEventType::Updated,
                delta: None,  // Delta not available after apply
                full_data: layer.to_json_value(),
            };
            if let Ok(subs) = state.page_event_subscribers.read() {
                for tx in subs.iter() {
                    let _ = tx.try_send(page_event.clone());
                }
                if !subs.is_empty() {
                    debug!(
                        layer_name = %layer_name,
                        subscriber_count = subs.len(),
                        from_peer = ?from_peer,
                        "Sent PageEvent after apply (manual notification for remote update)"
                    );
                }
            }
        }
    }

    // Update sender's vector to reflect what they've sent us
    if let Some(ref peer) = from_peer {
        if let Ok(mut subs) = state.subscribers.write() {
            if let Some(info) = subs.get_mut(peer) {
                let current_vector = layer.version_vector();
                info.vectors.insert(layer_name.to_string(), current_vector);
                debug!(user_did = %peer.0, "Updated sender's peer vector");
            }
        }
    }

    // Explicitly broadcast to eligible subscribers (targeted delivery)
    // Note: Observers are for UI reactivity; peer sync uses explicit broadcast
    // The broadcast_update function checks can_receive_layer for each subscriber,
    // so orders only go to owner (who has permission), not to other customers
    if from_peer.is_some() {
        broadcast_update(state, layer_name, from_peer.as_ref()).await;
    }

    // Run derivation for source layer changes (node only)
    //
    // **Flow**: After update applied, if this layer matches a derivation source pattern,
    // transform only the changed entries (from ops) and insert into derived layer.
    // This is efficient because we only process what actually changed.
    //
    // **Note**: Derived layers are just regular layers. After derivation modifies them
    // and calls commit(), the Loro observer fires and broadcasts automatically (same as
    // any other layer). We ensure observers are set up BEFORE derivation so commit() triggers.

    // First, collect derivation targets (to avoid borrow conflicts)
    let derivation_targets: Option<Vec<String>> = state.lua_runtime.as_ref()
        .filter(|r| r.is_derivation_enabled())
        .map(|r| r.derivation_rules().iter().map(|(_, t)| t.clone()).collect());

    if let Some(targets) = derivation_targets {
        if let Some(ops) = extracted_ops {
            if !ops.is_empty() {
                // Pre-create derived layers with observers BEFORE derivation runs
                // This ensures commit() inside derivation triggers the observer broadcast
                for target in &targets {
                    if !state.layers.contains_key(target) {
                        let layer = Layer::new();
                        let _ = layer.loro().get_map(target.clone());
                        layer.commit();
                        state.layers.insert(target.clone(), layer);
                        debug!(target = %target, "Pre-created derived layer");
                    }
                    // Set up observer (no-op if already exists)
                    super::loro_observer::setup_layer_observer(state, target);
                }

                // Now run derivation - commit() will trigger observer broadcast
                if let Some(ref lua_runtime) = state.lua_runtime {
                    lua_runtime.on_source_ops(layer_name, &ops, &mut state.layers);
                }

                // Mark derived layers as dirty for persistence
                for target in &targets {
                    if state.layers.contains_key(target) {
                        state.dirty_layers.insert(target.clone());
                    }
                }
            }
        }
    }

    // Dirty layer will be flushed by periodic timer (every 10s)
    Ok(())
}

/// Broadcast layer update to all eligible subscribers
///
/// **Context**: Layer has been updated, notify all subscribers
/// **We do**: Send incremental or full snapshot to each subscriber
/// **Cleanup**: Remove subscribers whose channels are closed (disconnected)
pub async fn broadcast_update(
    state: &mut ScribeState,
    layer_name: &str,
    from_peer: Option<&(String, String)>,
) {
    let Some(layer) = state.layers.get(layer_name) else {
        return;
    };

    // Collect version vector and snapshot once (same for all peers)
    let current_vector = layer.version_vector();
    let snapshot = layer.export_snapshot();

    // Track subscribers to remove (channel closed = peer disconnected)
    let mut to_remove: Vec<(String, String)> = Vec::new();

    if let Ok(mut subs) = state.subscribers.write() {
        for ((user_did, device_id), info) in subs.iter_mut() {
            // Skip sender
            if from_peer == Some(&(user_did.clone(), device_id.clone())) {
                continue;
            }

            // Check can receive (fixed layers OR pattern-based)
            if !info.can_receive_layer(layer_name, &state.page_id) {
                continue;
            }

            // Send same snapshot to all (CRDT handles ordering)
            let payload = BroadcastPayload {
                page_id: state.page_id.clone(),
                layer_name: layer_name.to_string(),
                update: snapshot.clone(),
                state_vector: current_vector.clone(),
            };

            match info.broadcast_tx.try_send(payload) {
                Ok(()) => {
                    // Update their vector on successful send
                    info.vectors.insert(layer_name.to_string(), current_vector.clone());
                }
                Err(mpsc::error::TrySendError::Closed(_)) => {
                    // Channel closed = peer disconnected, mark for removal
                    info!(user_did = %user_did, device_id = %device_id, "Subscriber disconnected, removing");
                    to_remove.push((user_did.clone(), device_id.clone()));
                }
                Err(mpsc::error::TrySendError::Full(_)) => {
                    // Channel full - log warning but don't remove (temporary backpressure)
                    warn!(user_did = %user_did, "Broadcast channel full, update dropped");
                }
            }
        }

        // Remove disconnected subscribers
        for key in to_remove {
            if let Some(info) = subs.remove(&key) {
                // Save their final vectors before removal
                if let Err(e) = (state.save_peer_vector)(&key.0, &key.1, &info.vectors) {
                    warn!(error = %e, user_did = %key.0, device_id = %key.1, "Failed to save peer vector on disconnect");
                }
                info!(user_did = %key.0, device_id = %key.1, "Cleaned up disconnected subscriber");
            }
        }
    }

    // Emit EnsureSync for any sync targets not currently subscribed
    emit_sync_events_for_missing_targets(state);
}

/// Notify page event subscribers of new layer discovery
///
/// **Context**: A new layer was created (from peer sync or local creation)
/// **We do**: Send PageEvent with Created type to unified page event subscribers
pub fn notify_layer_discovered(state: &ScribeState, layer_name: &str) {
    // Get layer data (may be empty for new layers before apply)
    let full_data = state.layers.get(layer_name)
        .map(|layer| layer.to_json_value())
        .unwrap_or(serde_json::Value::Null);

    let event = PageEvent {
        layer_name: layer_name.to_string(),
        from_peer: None,
        event_type: PageEventType::Created,
        delta: None,
        full_data,
    };

    // Send to unified page event subscribers
    if let Ok(subs) = state.page_event_subscribers.read() {
        for tx in subs.iter() {
            let _ = tx.try_send(event.clone());
        }
    }
}

/// Emit unified page event to all subscribers
///
/// **Context**: Layer was modified (local or remote), send to all page event consumers
/// **Consumers**: UI/Lua callbacks, Broadcast (filtered by sender), Derivation engine
/// **Design**: All layer changes flow through this single channel
pub fn emit_page_event(
    state: &ScribeState,
    layer_name: &str,
    from_peer: Option<(String, String)>,
    event_type: PageEventType,
) {
    // Get layer data as JSON
    let full_data = state.layers.get(layer_name)
        .map(|layer| layer.to_json_value())
        .unwrap_or(serde_json::Value::Null);

    let event = PageEvent {
        layer_name: layer_name.to_string(),
        from_peer,
        event_type,
        delta: None, // TODO: Extract delta from Loro event when available
        full_data,
    };

    // Send to all page event subscribers (fan-out)
    let Ok(subs) = state.page_event_subscribers.read() else {
        warn!(layer = %layer_name, "Failed to acquire page event subscribers lock");
        return;
    };

    let subscriber_count = subs.len();
    let mut sent_count = 0;
    for tx in subs.iter() {
        if tx.try_send(event.clone()).is_ok() {
            sent_count += 1;
        }
    }

    debug!(
        layer = %layer_name,
        subscriber_count = subscriber_count,
        sent_count = sent_count,
        event_type = ?event.event_type,
        "Emitted page event"
    );
}

// =============================================================================
// Sync Request Handlers
// =============================================================================

/// Handle sync request - export updates since their version
pub fn handle_sync_request(
    state: &ScribeState,
    layer_name: &str,
    their_vector: &[u8],
) -> Result<Vec<u8>> {
    let layer = state.layers.get(layer_name)
        .ok_or_else(|| ButlerError::NotFound(format!("Layer {} not found", layer_name)))?;

    layer.export_updates(their_vector)
        .map_err(|e| ButlerError::Layer(e.to_string()))
}

/// Handle export snapshot request
pub fn handle_export_snapshot(
    state: &ScribeState,
    layer_name: &str,
) -> Result<Vec<u8>> {
    let layer = state.layers.get(layer_name)
        .ok_or_else(|| ButlerError::NotFound(format!("Layer {} not found", layer_name)))?;

    Ok(layer.export_snapshot())
}

/// Handle get state vector request
pub fn handle_get_state_vector(
    state: &ScribeState,
    layer_name: &str,
) -> Result<Vec<u8>> {
    let layer = state.layers.get(layer_name)
        .ok_or_else(|| ButlerError::NotFound(format!("Layer {} not found", layer_name)))?;

    Ok(layer.version_vector())
}

/// Handle get updates since request (for resync after divergence)
pub fn handle_get_updates_since(
    state: &ScribeState,
    layer_name: &str,
    their_vector: &[u8],
) -> Result<Vec<u8>> {
    let layer = state.layers.get(layer_name)
        .ok_or_else(|| ButlerError::NotFound(format!("Layer {} not found", layer_name)))?;

    layer.export_updates(their_vector)
        .map_err(|e| ButlerError::Layer(e.to_string()))
}

/// Handle flush - save only dirty layers
pub async fn handle_flush(state: &mut ScribeState) {
    if state.dirty_layers.is_empty() {
        return;
    }

    debug!(page_id = %state.page_id, dirty_count = state.dirty_layers.len(), "Flushing layers");

    for layer_name in state.dirty_layers.drain() {
        if let Some(layer) = state.layers.get(&layer_name) {
            let snapshot = layer.export_snapshot();
            if let Err(e) = (state.save_layer)(&layer_name, &snapshot) {
                error!(layer = %layer_name, error = %e, "Failed to save layer");
            }
        }
    }

    // Save all peer vectors
    if let Ok(subs) = state.subscribers.read() {
        for ((user_did, device_id), info) in subs.iter() {
            if let Err(e) = (state.save_peer_vector)(user_did, device_id, &info.vectors) {
                warn!(user_did = %user_did, error = %e, "Failed to save peer vectors");
            }
        }
    }
}

/// Handle periodic reconciliation with all subscribed peers
///
/// **Context**: Timer-driven check for state divergence (every 30s)
/// **We do**: For each layer, check if any subscriber is behind our vector
/// **If diverged**: Re-broadcast update (triggers 3-step sync to catch them up)
#[instrument(skip(state), fields(page_id = %state.page_id))]
pub async fn handle_reconcile_with_peers(state: &mut ScribeState) {
    // Check if any subscribers exist (read lock)
    let has_subscribers = state.subscribers.read()
        .map(|s| !s.is_empty())
        .unwrap_or(false);

    if !has_subscribers {
        return; // No subscribers to reconcile with
    }

    let sub_count = state.subscribers.read().map(|s| s.len()).unwrap_or(0);
    debug!(subscriber_count = sub_count, layer_count = state.layers.len(), "Starting periodic reconciliation");

    // For each layer, check if any subscriber needs sync
    for layer_name in state.layers.keys().cloned().collect::<Vec<_>>() {
        let our_vector = match state.layers.get(&layer_name) {
            Some(layer) => layer.version_vector(),
            None => continue,
        };

        // Check if any subscriber has a different vector (may be behind)
        let needs_sync = if let Ok(subs) = state.subscribers.read() {
            subs.iter().any(|(_, info)| {
                // Skip if they can't receive this layer (checks fixed AND pattern permissions)
                if !info.can_receive_layer(&layer_name, &state.page_id) {
                    return false;
                }

                // Compare vectors - if different or missing, they may need sync
                info.vectors.get(&layer_name)
                    .map(|v| v != &our_vector)
                    .unwrap_or(true) // No vector = never synced = needs sync
            })
        } else {
            false
        };

        if needs_sync {
            debug!(layer_name = %layer_name, "Divergence detected during reconciliation, broadcasting");
            broadcast_update(state, &layer_name, None).await;
        }
    }
}

// =============================================================================
// Sync Target Resolution
// =============================================================================

/// Get user_dids that should receive updates for this page.
///
/// **Context**: Called during broadcast to find unsubscribed targets.
/// **Returns**: Vec<String> of user_dids - Coordinator handles device resolution.
fn get_sync_targets(state: &ScribeState) -> Vec<String> {
    use super::state::SyncMode;

    match state.sync_config.as_ref().map(|c| c.mode) {
        Some(SyncMode::ToSource) => {
            // User mode: single target (the node's DID)
            state.sync_config.as_ref()
                .and_then(|c| c.sync_target.as_ref())
                .map(|node_did| vec![node_did.clone()])
                .unwrap_or_default()
        }
        Some(SyncMode::Broadcast) | None => {
            // Node mode: query storage for authorized user_dids
            state.list_authorized_users
                .as_ref()
                .map(|f| f(&state.page_id))
                .unwrap_or_default()
        }
    }
}

/// Check if a user is currently subscribed (by any device).
///
/// **Context**: Called during broadcast to filter already-subscribed users.
fn is_user_subscribed(state: &ScribeState, user_did: &str) -> bool {
    state.subscribers.read()
        .map(|subs| subs.keys().any(|(did, _device)| did == user_did))
        .unwrap_or(false)
}

/// Emit EnsureSync for users that should be synced but aren't subscribed.
///
/// **Context**: Called after broadcasting to current subscribers.
/// **We do**: Get sync targets, filter out already-subscribed, emit EnsureSync.
pub fn emit_sync_events_for_missing_targets(state: &ScribeState) {
    let sync_targets = get_sync_targets(state);

    for user_did in sync_targets {
        if is_user_subscribed(state, &user_did) {
            // Already subscribed, broadcast handled normally
            continue;
        }

        // User not subscribed - emit EnsureSync
        // Coordinator will handle: device resolution, connection, subscription
        if let Some(ref sync_event_tx) = state.sync_event_tx {
            let event = SyncEvent::EnsureSync { user_did: user_did.clone() };

            if let Err(e) = sync_event_tx.try_send(event) {
                warn!(user_did = %user_did, error = %e, "Failed to emit EnsureSync");
            } else {
                debug!(user_did = %user_did, "Emitted EnsureSync for unsubscribed target");
            }
        }
    }
}

/// Get role for a peer from their stored permit
///
/// **Context**: Determining role for Lua validation
/// **Checks**:
/// 1. Sync target (owner/viewer mode) → our role's counterpart
/// 2. Stored permit (node mode) → extract role from permit
/// 3. Default → "peer"
fn get_peer_role(state: &ScribeState, peer_did: &str) -> String {
    // If peer is our sync target, they are the "node" from our perspective
    if let Some(ref config) = state.sync_config {
        if let Some(ref sync_target) = config.sync_target {
            if sync_target == peer_did {
                return "node".to_string();
            }
        }
    }

    // Node mode: look up peer's stored permit and extract role
    if let Some(ref load_fn) = state.load_user_permit {
        if let Some(permit_token) = load_fn(&state.page_id, peer_did) {
            let role = extract_role_from_permit(&permit_token);
            debug!(peer_did = %peer_did, role = %role, "Got peer role from stored permit");
            return role;
        }
    }

    // Default role
    "peer".to_string()
}
