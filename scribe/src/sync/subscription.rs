//! Subscription handling for Scribe actor
//!
//! Handles peer subscription, unsubscription, and initial state delivery.

use std::collections::HashMap;

use tokio::sync::mpsc;
use tracing::{debug, info, warn, instrument};

use crate::message::{BroadcastPayload, EphemeralOutbound};
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

    // Notify app subscribers about peer joining (lifecycle event)
    emit_peer_subscribed(state, &user_did);

    // Send initial state to new subscriber for layers they have access to
    send_initial_state_to_subscriber(
        state,
        &user_did,
        &device_id,
        &broadcast_tx,
        &permit,
        &permit_holder_did,
    ).await;
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

    // Notify app subscribers about peer leaving (lifecycle event)
    emit_peer_unsubscribed(state, user_did);
}

// Initial State Delivery

/// Send initial state to a newly subscribed peer
///
/// **Context**: Peer just subscribed, may have missed previous updates
/// **We do**: Send current snapshot for each layer they have permission for
/// **Checks**: Uses permit.can_read_layer() which handles both fixed and pattern-based
#[instrument(skip_all, fields(page_id = %state.page_id, user_did = %user_did))]
async fn send_initial_state_to_subscriber(
    state: &ScribeState,
    user_did: &str,
    device_id: &str,
    broadcast_tx: &mpsc::Sender<BroadcastPayload>,
    permit: &gurkha::Permit,
    subscriber_did: &str,
) {
    // Get peer's stored vectors (if any), normalizing keys to bare names
    let peer_vectors = match state.vector_storage.load_vectors(user_did, device_id) {
        Ok(Some(v)) => v.into_iter()
            .map(|(k, v)| (normalize_layer_name(&k, &state.page_id), v))
            .collect(),
        Ok(None) => HashMap::new(),
        Err(_) => HashMap::new(),
    };

    for (layer_name, layer) in &state.layers {
        // Skip layers in no_incoming_updates (from sync facts)
        if permit.sync_facts().no_incoming_updates.contains(&layer_name.to_string()) {
            continue;
        }

        // Check if they can receive this layer using permit's can_read_layer
        // This handles both fixed layers and pattern-based permissions
        if !permit.can_read_layer(layer_name, &state.page_id, subscriber_did) {
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
            layer_type: domains::LayerType::from_layer_name(&layer_name),
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

