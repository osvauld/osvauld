//! Broadcast handling for Scribe actor

use tokio::sync::mpsc;
use tracing::{debug, info, instrument, warn};

use crate::message::{BroadcastPayload, PageUpdate};
use crate::state::ScribeState;

/// Broadcast layer update to all layer subscribers (excludes sender)
///
/// Uses LayerUnit's per-subscriber state for authorization, version vectors,
/// and broadcast channels. Disconnected subscribers are removed from the unit.
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
    let layer = unit.layer().clone();
    let sender_did = from_peer.map(|(did, _)| did.as_str());

    let mut to_remove: Vec<(String, String)> = Vec::new();

    if let Ok(mut layer_subs) = unit.subscribers().write() {
        for ((user_did, device_id), sub) in layer_subs.iter_mut() {
            // Skip sender
            if sender_did == Some(user_did.as_str()) {
                state.emit_broadcast_decision_capture(layer_name, user_did, "skip_sender");
                continue;
            }

            // Presence in subscriber map = can read (no separate capability check)

            let update = if !sub.version_vector.is_empty() {
                layer
                    .export_updates(&sub.version_vector)
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

            match sub.broadcast_tx.try_send(payload) {
                Ok(()) => {
                    sub.version_vector = current_vector.clone();
                }
                Err(mpsc::error::TrySendError::Closed(_)) => {
                    info!(user_did = %user_did, device_id = %device_id, "Layer subscriber disconnected, removing");
                    to_remove.push((user_did.clone(), device_id.clone()));
                }
                Err(mpsc::error::TrySendError::Full(_)) => {
                    warn!(user_did = %user_did, device_id = %device_id, "Broadcast channel full, update dropped");
                }
            }
        }

        for key in &to_remove {
            layer_subs.remove(key);
            debug!(user_did = %key.0, device_id = %key.1, layer = %layer_name, "Removed disconnected layer subscriber");
        }
    }

    super::reconcile::emit_sync_events_for_missing_targets(state);
}

// Page Update Emission

/// Notify page event subscribers of new layer discovery
///
/// **Context**: A new layer was created (from peer sync or local creation)
/// **We do**: Send PageUpdate::LayerChanged with created=true to unified subscribers
/// **Filters**: Protocol layers (__sync_meta:*) are not sent to Lua/UI subscribers
#[instrument(skip(state), fields(page_id = %state.page_id, layer = %layer_name))]
pub fn notify_layer_discovered(state: &ScribeState, layer_name: &str) {
    // Filter protocol layers from Lua/UI notifications
    if crate::sync::sync_meta::is_protocol_layer(layer_name) {
        debug!(layer = %layer_name, "Skipping notify_layer_discovered for protocol layer");
        return;
    }
    let full_data = state
        .units
        .get(layer_name)
        .map(|unit| unit.layer().get_content(layer_name))
        .unwrap_or(serde_json::Value::Null);

    let state_vector = state
        .units
        .get(layer_name)
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
        dynamic_ref: state.dynamic_ref_for_layer(layer_name),
    };
    state.emit_page_update_capture(&update);
    if let Ok(subs) = state.page_update_subscribers.read() {
        for tx in subs.iter() {
            let _ = tx.try_send(update.clone());
        }
    }
}
