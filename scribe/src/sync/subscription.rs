//! Subscription handling for Scribe actor

use std::collections::HashMap;

use tokio::sync::mpsc;
use tracing::{debug, info, warn, instrument};

use crate::layer_unit::Capabilities;
use crate::message::{BroadcastPayload, EphemeralOutbound};
use crate::state::{ScribeState, SubscriberInfo, normalize_layer_name};
use crate::ephemeral::{emit_peer_subscribed, emit_peer_unsubscribed};

/// Extract permit holder's DID from the permit's audience field
fn extract_permit_holder_did(permit: &str, fallback_did: &str) -> String {
    match gurkha::Permit::from_token(permit) {
        Ok(parsed) => {
            let raw_aud = parsed.parsed().audience().to_string();
            if raw_aud.starts_with("did:key:") {
                raw_aud
            } else {
                match herald::Identity::did_from_base64_pubkey(&raw_aud) {
                    Ok(did) => did,
                    Err(e) => {
                        warn!(error = %e, raw_aud = %raw_aud, "Failed to convert audience to DID");
                        raw_aud
                    }
                }
            }
        }
        Err(e) => {
            warn!(error = %e, "Failed to extract audience from permit");
            fallback_did.to_string()
        }
    }
}

/// Handle peer subscription
///
/// Parses permit, stores connection info, adds subscriber to each accessible
/// LayerUnit, sends initial state, and issues pending layer permits.
#[instrument(skip(state, broadcast_tx, ephemeral_tx, permit_token), fields(page_id = %state.page_id))]
pub async fn handle_subscribe(
    state: &mut ScribeState,
    user_did: String,
    device_id: String,
    broadcast_tx: mpsc::Sender<BroadcastPayload>,
    ephemeral_tx: Option<mpsc::Sender<EphemeralOutbound>>,
    permit_token: String,
) {
    info!(user_did = %user_did, device_id = %device_id, "Peer subscribing");

    let permit = match gurkha::Permit::from_token(&permit_token) {
        Ok(p) => p,
        Err(e) => {
            warn!(error = ?e, "Failed to parse permit, rejecting subscription");
            return;
        }
    };

    let permit_holder_did = extract_permit_holder_did(&permit_token, &user_did);

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

    let is_visible = permit.is_visible();
    let can_see_others = permit.can_see_others();
    let display_name = permit.display_name().map(String::from);

    info!(
        user_did = %user_did,
        permit_holder_did = %permit_holder_did,
        is_visible = is_visible,
        layers = ?permit.layers().keys().collect::<Vec<_>>(),
        "Subscription with permit"
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
    }

    // Add subscriber to each accessible LayerUnit
    add_subscriber_to_layers(state, &user_did, &permit, &permit_holder_did, can_see_others, &broadcast_tx);

    emit_peer_subscribed(state, &user_did);

    send_initial_state_to_subscriber(state, &user_did, &device_id, &broadcast_tx).await;

    // __sync_meta bootstrap (node-side only): create and populate the peer's discovery catalog
    if state.permit_issuer.is_some() {
        bootstrap_sync_meta(state, &permit_holder_did, &permit, &broadcast_tx);
    }

    // Observability: list which layers this subscriber was added to
    let subscribed_layers: Vec<String> = state.units.iter()
        .filter(|(_, unit)| unit.can_push_to(&user_did))
        .map(|(name, _)| name.clone())
        .collect();
    state.emit_subscriber_state_capture(
        &user_did,
        &subscribed_layers,
        state.units.len(),
        can_see_others,
        is_visible,
    );
}

/// Add subscriber to all LayerUnits they have access to (from page permit)
fn add_subscriber_to_layers(
    state: &ScribeState,
    user_did: &str,
    permit: &gurkha::Permit,
    permit_holder_did: &str,
    can_see_others: bool,
    broadcast_tx: &mpsc::Sender<BroadcastPayload>,
) {
    for (layer_name, unit) in &state.units {
        if unit.is_local_only() { continue; }

        let is_presence = layer_name == "presence" || layer_name.ends_with("/presence");
        if is_presence && !can_see_others {
            continue;
        }

        if permit.sync_facts().no_incoming_updates.contains(&layer_name.to_string()) {
            continue;
        }

        // Presence layer: can_see_others is sufficient authorization (not in layers list)
        if is_presence && can_see_others {
            let caps = Capabilities { read: true, write: true, sync: true };
            unit.add_subscriber(user_did.to_string(), caps, broadcast_tx.clone());
            state.emit_layer_auth_capture(layer_name, user_did, "subscriber_added_presence");
            continue;
        }

        if permit.can_read_layer(layer_name, &state.page_id, permit_holder_did) {
            let can_write = permit.can_write_layer(layer_name, &state.page_id, permit_holder_did);
            let caps = Capabilities { read: true, write: can_write, sync: true };
            unit.add_subscriber(user_did.to_string(), caps, broadcast_tx.clone());
            state.emit_layer_auth_capture(layer_name, user_did, "subscriber_added_page_permit");
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

    // Save vectors from LayerUnit subscribers before removing
    let mut vectors: HashMap<String, Vec<u8>> = HashMap::new();
    for (layer_name, unit) in &state.units {
        if let Some(sub) = unit.remove_subscriber(user_did) {
            if !sub.version_vector.is_empty() {
                vectors.insert(layer_name.clone(), sub.version_vector);
            }
        }
    }
    if !vectors.is_empty() {
        if let Err(e) = state.vector_storage.save_vectors(user_did, device_id, &vectors) {
            warn!(error = %e, "Failed to save peer vectors on unsubscribe");
        }
    }

    if let Ok(mut subs) = state.subscribers.write() {
        subs.remove(&(user_did.to_string(), device_id.to_string()));
        debug!(subscriber_count = subs.len(), "Subscriber removed");
    }

    emit_peer_unsubscribed(state, user_did);
}

/// Send initial state to a newly subscribed peer for layers they have access to
#[instrument(skip_all, fields(page_id = %state.page_id, user_did = %user_did))]
async fn send_initial_state_to_subscriber(
    state: &ScribeState,
    user_did: &str,
    device_id: &str,
    broadcast_tx: &mpsc::Sender<BroadcastPayload>,
) {
    let peer_vectors = match state.vector_storage.load_vectors(user_did, device_id) {
        Ok(Some(v)) => v.into_iter()
            .map(|(k, v)| (normalize_layer_name(&k, &state.page_id), v))
            .collect(),
        Ok(None) => HashMap::new(),
        Err(_) => HashMap::new(),
    };

    for (layer_name, unit) in &state.units {
        // Only send to layers where subscriber was added
        if !unit.can_push_to(user_did) {
            continue;
        }

        let layer = unit.layer();

        let data = if let Some(their_vector) = peer_vectors.get(layer_name) {
            if their_vector.is_empty() {
                layer.export_snapshot()
            } else {
                match layer.export_updates(their_vector) {
                    Ok(updates) if updates.is_empty() => {
                        debug!(user_did = %user_did, layer = %layer_name, "No new updates");
                        continue;
                    }
                    Ok(updates) => updates,
                    Err(e) => {
                        warn!(error = %e, layer = %layer_name, "Failed incremental export, using snapshot");
                        layer.export_snapshot()
                    }
                }
            }
        } else {
            layer.export_snapshot()
        };

        let state_vector = layer.version_vector();

        if data.is_empty() || state_vector.len() <= 1 {
            debug!(user_did = %user_did, layer = %layer_name, "Skipping empty layer");
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
            debug!(user_did = %user_did, layer = %layer_name, "Sent initial state");
        }
    }
}

/// Bootstrap __sync_meta for a newly subscribed peer (node-side)
///
/// **Context**: After a peer subscribes, the node creates their __sync_meta layer,
/// populates it with static layer entries, adds existing dynamic layer entries
/// for late joiners, and subscribes the peer to their __sync_meta layer.
#[instrument(skip(state, broadcast_tx), fields(page_id = %state.page_id, peer_did = %peer_did))]
fn bootstrap_sync_meta(
    state: &mut ScribeState,
    peer_did: &str,
    peer_permit: &gurkha::Permit,
    broadcast_tx: &mpsc::Sender<BroadcastPayload>,
) {
    use super::sync_meta;

    // 1. Create __sync_meta:{peer_did} layer
    let meta_layer_name = sync_meta::ensure_sync_meta_layer(state, peer_did);

    // 2. Populate with static layer entries
    let is_owner = peer_permit.relationship() == Some("owner");
    sync_meta::populate_static_layers(state, peer_did, peer_permit, is_owner);

    // 3. Populate dynamic layers for late joiners
    sync_meta::populate_dynamic_layers_for_late_joiner(state, peer_did);

    // 4. Subscribe the peer to their __sync_meta layer so it syncs to them
    if let Some(unit) = state.units.get(&meta_layer_name) {
        let caps = Capabilities { read: true, write: true, sync: true };
        unit.add_subscriber(peer_did.to_string(), caps, broadcast_tx.clone());
        info!(
            layer = %meta_layer_name,
            peer_did = %peer_did,
            "Subscribed peer to their __sync_meta layer"
        );
    }
}
