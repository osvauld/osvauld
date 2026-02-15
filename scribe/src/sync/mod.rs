//! Sync handling for Scribe actor
//!
//! This module orchestrates sync operations by delegating to specialized sub-modules:
//! - `subscription`: Peer subscribe/unsubscribe and initial state delivery
//! - `apply`: Update application with validation and derivation
//! - `broadcast`: Layer update broadcasting and page update emission
//! - `reconcile`: Periodic reconciliation and sync target resolution

use tracing::{debug, instrument};

use crate::{Result, ScribeError};
use crate::state::ScribeState;

// Sub-modules

pub mod subscription;
pub mod apply;
pub mod broadcast;
pub mod reconcile;
pub mod sync_meta;

// Re-exports for backward compatibility

// Subscription handlers
pub use subscription::{handle_subscribe, handle_unsubscribe};

// Apply handlers
pub use apply::handle_apply_update;

// Broadcast handlers
pub use broadcast::{broadcast_update, notify_layer_discovered};

// Reconcile handlers
pub use reconcile::{handle_reconcile_with_peers, emit_sync_events_for_missing_targets};

// Sync Request Handlers (kept here - simple pass-through handlers)

/// Handle sync request - export updates since their version
#[instrument(skip(state, their_vector), fields(page_id = %state.page_id, layer = %layer_name))]
pub fn handle_sync_request(
    state: &ScribeState,
    layer_name: &str,
    their_vector: &[u8],
) -> Result<Vec<u8>> {
    let layer = state.units.get(layer_name)
        .map(|u| u.layer())
        .ok_or_else(|| ScribeError::LayerNotFound(layer_name.to_string()))?;

    layer.export_updates(their_vector)
        .map_err(|e| ScribeError::CrdtError(e.to_string()))
}

/// Handle export snapshot request
#[instrument(skip(state), fields(page_id = %state.page_id, layer = %layer_name))]
pub fn handle_export_snapshot(
    state: &ScribeState,
    layer_name: &str,
) -> Result<Vec<u8>> {
    let layer = state.units.get(layer_name)
        .map(|u| u.layer())
        .ok_or_else(|| ScribeError::LayerNotFound(layer_name.to_string()))?;

    Ok(layer.export_snapshot())
}

/// Handle get state vector request
#[instrument(skip(state), fields(page_id = %state.page_id, layer = %layer_name))]
pub fn handle_get_state_vector(
    state: &ScribeState,
    layer_name: &str,
) -> Result<Vec<u8>> {
    let layer = state.units.get(layer_name)
        .map(|u| u.layer())
        .ok_or_else(|| ScribeError::LayerNotFound(layer_name.to_string()))?;

    Ok(layer.version_vector())
}

/// Handle get updates since request (for resync after divergence)
#[instrument(skip(state, their_vector), fields(page_id = %state.page_id, layer = %layer_name))]
pub fn handle_get_updates_since(
    state: &ScribeState,
    layer_name: &str,
    their_vector: &[u8],
) -> Result<Vec<u8>> {
    let layer = state.units.get(layer_name)
        .map(|u| u.layer())
        .ok_or_else(|| ScribeError::LayerNotFound(layer_name.to_string()))?;

    layer.export_updates(their_vector)
        .map_err(|e| ScribeError::CrdtError(e.to_string()))
}

/// Handle flush - save only dirty layers
#[instrument(skip(state), fields(page_id = %state.page_id))]
pub async fn handle_flush(state: &mut ScribeState) {
    let has_dirty = state.units.values().any(|u| u.is_dirty());
    if !has_dirty {
        return;
    }

    let dirty_count = state.units.values().filter(|u| u.is_dirty()).count();
    debug!(page_id = %state.page_id, dirty_count = dirty_count, "Flushing layers");

    for (name, unit) in state.units.iter_mut() {
        if let Some(snapshot) = unit.take_dirty_snapshot() {
            if let Err(e) = state.layer_storage.save_layer(name, &snapshot) {
                tracing::error!(layer = %name, error = %e, "Failed to save layer");
            }
        }
    }

    // Save all peer vectors
    if let Ok(subs) = state.subscribers.read() {
        for ((user_did, device_id), info) in subs.iter() {
            if let Err(e) = state.vector_storage.save_vectors(user_did, device_id, &info.vectors) {
                tracing::warn!(user_did = %user_did, error = %e, "Failed to save peer vectors");
            }
        }
    }
}
