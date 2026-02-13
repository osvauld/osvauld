//! Broadcast handling for Scribe actor
//!
//! Handles layer update broadcasting to subscribers and page update emission.

use tokio::sync::mpsc;
use tracing::{info, instrument, warn};

use crate::message::{BroadcastPayload, PageUpdate};
use crate::state::ScribeState;

// Layer Broadcasting

/// Broadcast layer update to all eligible subscribers
///
/// **Context**: Layer has been updated, notify all subscribers
/// **We do**: Send incremental or full snapshot to each subscriber
/// **Cleanup**: Remove subscribers whose channels are closed (disconnected)
#[instrument(skip_all, fields(page_id = %state.page_id, layer = %layer_name))]
pub async fn broadcast_update(
    state: &mut ScribeState,
    layer_name: &str,
    from_peer: Option<&(String, String)>,
) {
    let Some(unit) = state.units.get(layer_name) else {
        return;
    };

    let current_vector = unit.layer().version_vector();
    let layer = unit.layer().clone(); // Clone for use inside subscriber lock

    // Read authorized_dids once before the subscriber loop
    let authorized = unit.authorized_dids().read().unwrap_or_else(|e| e.into_inner()).clone();

    // Track subscribers to remove (channel closed = peer disconnected)
    let mut to_remove: Vec<(String, String)> = Vec::new();

    if let Ok(mut subs) = state.subscribers.write() {
        for ((user_did, device_id), info) in subs.iter_mut() {
            // Skip sender
            if from_peer == Some(&(user_did.clone(), device_id.clone())) {
                state.emit_broadcast_decision_capture(layer_name, user_did, "skip_sender");
                continue;
            }

            // Check authorization via LayerUnit
            if !authorized.contains(user_did) {
                state.emit_broadcast_decision_capture(layer_name, user_did, "not_authorized");
                continue;
            }

            // Incremental export: use subscriber's last known vector
            // Falls back to full snapshot for first sync (no vector cached)
            let update = if let Some(their_vector) = info.vectors.get(layer_name) {
                layer.export_updates(their_vector)
                    .unwrap_or_else(|_| layer.export_snapshot())
            } else {
                layer.export_snapshot()
            };

            let payload = BroadcastPayload {
                page_id: state.page_id.clone(),
                layer_name: layer_name.to_string(),
                layer_type: domains::LayerType::from_layer_name(layer_name),
                update,
                state_vector: current_vector.clone(),
            };

            match info.broadcast_tx.try_send(payload) {
                Ok(()) => {
                    // Update their vector on successful send
                    info.vectors.insert(layer_name.to_string(), current_vector.clone());
                }
                Err(mpsc::error::TrySendError::Closed(_)) => {
                    info!(user_did = %user_did, device_id = %device_id, "Subscriber disconnected, removing");
                    to_remove.push((user_did.clone(), device_id.clone()));
                }
                Err(mpsc::error::TrySendError::Full(_)) => {
                    warn!(user_did = %user_did, "Broadcast channel full, update dropped");
                }
            }
        }

        // Remove disconnected subscribers
        for key in to_remove {
            if let Some(info) = subs.remove(&key) {
                if let Err(e) = state.vector_storage.save_vectors(&key.0, &key.1, &info.vectors) {
                    warn!(error = %e, user_did = %key.0, device_id = %key.1, "Failed to save peer vector on disconnect");
                }
                info!(user_did = %key.0, device_id = %key.1, "Cleaned up disconnected subscriber");
            }
        }
    }

    // Emit EnsureSync for any sync targets not currently subscribed
    super::reconcile::emit_sync_events_for_missing_targets(state);
}

// Page Update Emission

/// Notify page event subscribers of new layer discovery
///
/// **Context**: A new layer was created (from peer sync or local creation)
/// **We do**: Send PageUpdate::LayerChanged with created=true to unified subscribers
#[instrument(skip(state), fields(page_id = %state.page_id, layer = %layer_name))]
pub fn notify_layer_discovered(state: &ScribeState, layer_name: &str) {
    let full_data = state.units.get(layer_name)
        .map(|unit| unit.layer().get_content(layer_name))
        .unwrap_or(serde_json::Value::Null);

    let state_vector = state.units.get(layer_name)
        .map(|unit| unit.layer().version_vector())
        .unwrap_or_default();

    let update = PageUpdate::LayerChanged {
        layer: layer_name.to_string(),
        from_peer: None,
        ops: None,
        delta: None,
        state_vector: state_vector.clone(),
        full_data: Some(full_data.clone()),
        created: true,
    };
    state.emit_page_update_capture(&update);
    if let Ok(subs) = state.page_update_subscribers.read() {
        for tx in subs.iter() {
            let _ = tx.try_send(update.clone());
        }
    }
}
