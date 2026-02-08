//! Reconciliation handling for Scribe actor
//!
//! Handles periodic reconciliation with peers and sync target resolution.

use tracing::{debug, warn, instrument};

use super::broadcast::broadcast_update;
use crate::message::SyncEvent;
use crate::state::{ScribeState, SyncMode};

// Periodic Reconciliation

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

// Sync Target Resolution

/// Get user_dids that should receive updates for this page.
///
/// **Context**: Called during broadcast to find unsubscribed targets.
/// **Returns**: Vec<String> of user_dids - Coordinator handles device resolution.
#[instrument(skip(state), fields(page_id = %state.page_id))]
pub fn get_sync_targets(state: &ScribeState) -> Vec<String> {
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
            state.peer_resolver
                .as_ref()
                .map(|r| r.list_authorized_users())
                .unwrap_or_default()
        }
    }
}

/// Check if a user is currently subscribed (by any device).
///
/// **Context**: Called during broadcast to filter already-subscribed users.
pub fn is_user_subscribed(state: &ScribeState, user_did: &str) -> bool {
    state.subscribers.read()
        .map(|subs| subs.keys().any(|(did, _device)| did == user_did))
        .unwrap_or(false)
}

/// Emit EnsureSync for users that should be synced but aren't subscribed.
///
/// **Context**: Called after broadcasting to current subscribers.
/// **We do**: Get sync targets, filter out already-subscribed, emit EnsureSync.
#[instrument(skip(state), fields(page_id = %state.page_id))]
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
