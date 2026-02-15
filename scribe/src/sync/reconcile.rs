//! Reconciliation handling for Scribe actor
//!
//! Handles periodic reconciliation with peers and sync target resolution.

use tracing::{debug, warn, instrument};

use super::broadcast::broadcast_update;
use crate::message::SyncEvent;
use crate::state::{ScribeState, SyncMode};

// Periodic Reconciliation

/// Periodic reconciliation — check if any layer subscriber is behind our vector
#[instrument(skip(state), fields(page_id = %state.page_id))]
pub async fn handle_reconcile_with_peers(state: &mut ScribeState) {
    let layer_names: Vec<String> = state.units.keys().cloned().collect();

    for layer_name in layer_names {
        let needs_sync = match state.units.get(&layer_name) {
            Some(unit) => {
                let our_vector = unit.layer().version_vector();
                unit.subscribers().read()
                    .map(|subs| {
                        subs.values().any(|sub| {
                            if !sub.capabilities.read { return false; }
                            if sub.version_vector.is_empty() { return true; }
                            sub.version_vector != our_vector
                        })
                    })
                    .unwrap_or(false)
            }
            None => continue,
        };

        if needs_sync {
            debug!(layer_name = %layer_name, "Divergence detected, broadcasting");
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

/// Check if a user is connected (present in connections map)
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

            state.emit_sync_event_capture(&event);
            if let Err(e) = sync_event_tx.try_send(event) {
                warn!(user_did = %user_did, error = %e, "Failed to emit EnsureSync");
            } else {
                debug!(user_did = %user_did, "Emitted EnsureSync for unsubscribed target");
            }
        }
    }
}
