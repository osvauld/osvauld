//! Subscription handling for Scribe actor
//!
//! Handles peer subscription, unsubscription, and initial state delivery.

use std::collections::HashMap;

use tokio::sync::mpsc;
use tracing::{debug, info, warn, instrument};

use crate::message::{BroadcastPayload, EphemeralOutbound, SyncEvent};
use crate::state::{ScribeState, SubscriberInfo, normalize_layer_name};
use crate::ephemeral::{emit_peer_subscribed, emit_peer_unsubscribed};

// Helper Functions

/// Extract permit holder's DID from the permit's audience field
///
/// **Context**: The permit holder is the one who can create layers matching their patterns
/// **Note**: The UCAN audience may be stored as base64-encoded public key or DID format.
/// Layer names use DID format (did:key:...), so we normalize to DID for pattern matching.
fn extract_permit_holder_did(permit: &str, fallback_did: &str) -> String {
    match gurkha::Permit::from_token(permit) {
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
            warn!(error = %e, "Failed to extract audience from permit, falling back to provided DID");
            fallback_did.to_string()
        }
    }
}

// Subscription Handlers

/// Handle peer subscription
///
/// **Context**: PeerActor subscribes to receive updates for this page
/// **We do**: Parse permit once with gurkha::Permit, store it directly
///
/// **Pattern expansion**: For patterns like `{page_id}/orders/{aud}`, we use the permit's
/// audience field (the permit holder's DID) for expansion, not the subscriber's DID.
/// This ensures layers created by the permit holder match correctly.
#[instrument(skip(state, broadcast_tx, ephemeral_tx, permit_token), fields(page_id = %state.page_id))]
pub async fn handle_subscribe(
    state: &mut ScribeState,
    user_did: String,
    device_id: String,
    broadcast_tx: mpsc::Sender<BroadcastPayload>,
    ephemeral_tx: Option<mpsc::Sender<EphemeralOutbound>>,
    permit_token: String,
) {
    info!(user_did = %user_did, device_id = %device_id, permit_len = permit_token.len(), "Peer subscribing");

    // Parse permit once - store directly, no extraction needed
    let permit = match gurkha::Permit::from_token(&permit_token) {
        Ok(p) => p,
        Err(e) => {
            warn!(error = ?e, "Failed to parse permit, rejecting subscription");
            return;
        }
    };

    // Extract permit holder's DID for pattern expansion
    // The UCAN audience may be base64-encoded public key or DID format
    let permit_holder_did = extract_permit_holder_did(&permit_token, &user_did);

    // Load stored state vectors for this peer, normalizing keys to bare names
    let vectors = match state.vector_storage.load_vectors(&user_did, &device_id) {
        Ok(Some(v)) => v.into_iter()
            .map(|(k, v)| (normalize_layer_name(&k, &state.page_id), v))
            .collect(),
        Ok(None) => HashMap::new(),
        Err(e) => {
            warn!(error = %e, "Failed to load peer vectors");
            HashMap::new()
        }
    };

    // Extract presence info from permit
    let is_visible = permit.is_visible();
    let can_see_others = permit.can_see_others();
    let display_name = permit.display_name().map(String::from);

    info!(
        user_did = %user_did,
        permit_holder_did = %permit_holder_did,
        is_visible = is_visible,
        display_name = ?display_name,
        layers = ?permit.layers().keys().collect::<Vec<_>>(),
        patterns = ?permit.layer_patterns().keys().collect::<Vec<_>>(),
        "Subscription with permit stored directly"
    );

    if let Ok(mut subs) = state.subscribers.write() {
        subs.insert(
            (user_did.clone(), device_id.clone()),
            SubscriberInfo {
                permit: permit.clone(),
                subscriber_did: permit_holder_did.clone(),
                is_visible,
                can_see_others,
                display_name: display_name.clone(),
                broadcast_tx: broadcast_tx.clone(),
                ephemeral_tx,
                vectors,
            },
        );
        debug!(subscriber_count = subs.len(), "Subscriber added");
    }

    // Populate authorized_dids on existing LayerUnits for this subscriber
    authorize_subscriber_for_layers(state, &user_did, &permit, &permit_holder_did, can_see_others);

    // Notify app subscribers about peer joining (lifecycle event)
    emit_peer_subscribed(state, &user_did);

    // Send initial state to new subscriber for layers they have access to
    send_initial_state_to_subscriber(
        state,
        &user_did,
        &device_id,
        &broadcast_tx,
    ).await;

    // Issue pending layer permits for explicit dynamic layers (node mode only)
    issue_pending_layer_permits(state, &user_did, &device_id).await;

    // Emit subscriber state snapshot for observability
    let authorized_layers: Vec<String> = state.units.iter()
        .filter(|(_, unit)| unit.is_authorized(&user_did))
        .map(|(name, _)| name.clone())
        .collect();
    state.emit_subscriber_state_capture(
        &user_did,
        &authorized_layers,
        state.units.len(),
        can_see_others,
        is_visible,
    );
}

/// Populate authorized_dids on all existing LayerUnits for a newly subscribed peer
///
/// **Context**: Peer just subscribed with a page permit. Check each layer
/// against their permit and authorize accordingly.
fn authorize_subscriber_for_layers(
    state: &ScribeState,
    user_did: &str,
    permit: &gurkha::Permit,
    permit_holder_did: &str,
    can_see_others: bool,
) {
    let sync_target = state
        .sync_config
        .as_ref()
        .and_then(|cfg| cfg.sync_target.as_deref());

    for (layer_name, unit) in &state.units {
        if unit.is_local_only() { continue; }

        if layer_name.starts_with("__sync_meta/") {
            let did_in_layer = layer_name.trim_start_matches("__sync_meta/");
            if user_did == did_in_layer || sync_target == Some(user_did) {
                unit.authorize_did(user_did);
                state.emit_layer_auth_capture(layer_name, user_did, "authorized_protocol_sync_meta");
            }
            continue;
        }

        // Presence layer: only if can_see_others
        if (layer_name == "presence" || layer_name.ends_with("/presence")) && !can_see_others {
            continue;
        }

        // no_incoming_updates check
        if permit.sync_facts().no_incoming_updates.contains(&layer_name.to_string()) {
            continue;
        }

        // Check if page permit covers this layer
        if permit.can_read_layer(layer_name, &state.page_id, permit_holder_did) {
            unit.authorize_did(user_did);
            state.emit_layer_auth_capture(layer_name, user_did, "authorized_page_permit");
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
            if let Err(e) = state.vector_storage.save_vectors(user_did, device_id, &info.vectors) {
                warn!(error = %e, "Failed to save peer vectors on unsubscribe");
            }
        }

        subs.remove(&(user_did.to_string(), device_id.to_string()));
        debug!(subscriber_count = subs.len(), "Subscriber removed");
    }

    // Remove DID from all LayerUnits' authorized_dids
    for (_, unit) in &state.units {
        unit.revoke_did(user_did);
    }

    // Notify app subscribers about peer leaving (lifecycle event)
    emit_peer_unsubscribed(state, user_did);
}

// Initial State Delivery

/// Send initial state to a newly subscribed peer
///
/// **Context**: Peer just subscribed, may have missed previous updates
/// **We do**: Send current snapshot for each layer they have permission for
/// **Checks**: Uses LayerUnit.is_authorized() (populated by authorize_subscriber_for_layers)
#[instrument(skip_all, fields(page_id = %state.page_id, user_did = %user_did))]
async fn send_initial_state_to_subscriber(
    state: &ScribeState,
    user_did: &str,
    device_id: &str,
    broadcast_tx: &mpsc::Sender<BroadcastPayload>,
) {
    // Get peer's stored vectors (if any), normalizing keys to bare names
    let peer_vectors = match state.vector_storage.load_vectors(user_did, device_id) {
        Ok(Some(v)) => v.into_iter()
            .map(|(k, v)| (normalize_layer_name(&k, &state.page_id), v))
            .collect(),
        Ok(None) => HashMap::new(),
        Err(_) => HashMap::new(),
    };

    for (layer_name, unit) in &state.units {
        let layer = unit.layer();

        // Check if they're authorized for this layer (populated at subscribe time)
        if !unit.is_authorized(user_did) {
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

        // Skip empty data or empty state vectors (no operations).
        // Protocol sync metadata must still be sent as initial snapshot so the
        // remote side materializes the layer for pairwise sync state.
        let is_protocol_sync_meta = layer_name.starts_with("__sync_meta/");
        if data.is_empty() || (!is_protocol_sync_meta && state_vector.len() <= 1) {
            debug!(user_did = %user_did, layer = %layer_name, "Skipping empty layer in initial state");
            continue;
        }

        let payload = BroadcastPayload {
            page_id: state.page_id.clone(),
            layer_name: layer_name.clone(),
            layer_type: domains::LayerType::from_layer_name(layer_name),
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

/// Issue pending layer permits for a newly subscribed peer (node mode only)
///
/// **Context**: Peer just subscribed. Compute missing access permits from
/// authority permits and issue fresh layer permits.
#[instrument(skip_all, fields(page_id = %state.page_id, user_did = %user_did))]
async fn issue_pending_layer_permits(
    state: &ScribeState,
    user_did: &str,
    _device_id: &str,
) {
    let Some(ref issuer) = state.permit_issuer else { return; };
    let Some(ref sync_event_tx) = state.sync_event_tx else { return; };

    let authority_permits = match issuer.list_layer_authority_permits_for_audience(user_did) {
        Ok(permits) => permits,
        Err(e) => {
            warn!(error = %e, "Failed to list layer authority permits for subscriber");
            return;
        }
    };

    for (layer_name, _version, authority_token) in authority_permits {
        let already_issued = match issuer.has_layer_access_permit(user_did, &layer_name) {
            Ok(v) => v,
            Err(e) => {
                warn!(error = %e, layer = %layer_name, "Failed to check existing layer permit");
                continue;
            }
        };
        if already_issued {
            continue;
        }

        let Some(config) = authority_layer_config(&authority_token, &layer_name, &state.page_id) else {
            warn!(layer = %layer_name, "Authority permit missing layer config for subscriber");
            continue;
        };

        let intent_cid = match gurkha::crypto::get_permit_cid(&authority_token) {
            Ok(cid) => cid,
            Err(e) => {
                warn!(error = %e, layer = %layer_name, "Failed to compute authority permit CID");
                continue;
            }
        };

        match issuer.issue_layer_permit(user_did, &layer_name, config, Some(&intent_cid)) {
            Ok((token, _cid)) => {
                let bare_name = normalize_layer_name(&layer_name, &state.page_id);
                if let Some(unit) = state.units.get(&bare_name) {
                    unit.authorize_did(user_did);
                    state.emit_layer_auth_capture(&bare_name, user_did, "authorized_layer_permit");
                }

                let event = SyncEvent::LayerAccessChanged {
                    page_id: state.page_id.clone(),
                    layer_name: layer_name.to_string(),
                    permits: vec![(user_did.to_string(), token)],
                };
                state.emit_sync_event_capture(&event);
                if let Err(e) = sync_event_tx.send(event).await {
                    warn!(error = %e, layer = %layer_name, "Failed to emit pending layer permit");
                }
            }
            Err(e) => warn!(error = %e, layer = %layer_name, "Failed to issue pending layer permit"),
        }
    }
}

fn authority_layer_config(
    authority_token: &str,
    layer_name: &str,
    page_id: &str,
) -> Option<gurkha::LayerConfig> {
    let parsed = gurkha::Permit::from_token(authority_token).ok()?;

    if let Some(config) = parsed.layers().get(layer_name) {
        return Some(config.clone());
    }

    let bare = normalize_layer_name(layer_name, page_id);
    if let Some(config) = parsed.layers().get(&bare) {
        return Some(config.clone());
    }

    None
}
