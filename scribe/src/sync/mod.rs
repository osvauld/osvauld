//! Sync handling for Scribe actor
//!
//! This module orchestrates sync operations by delegating to specialized sub-modules:
//! - `subscription`: Peer subscribe/unsubscribe and initial state delivery
//! - `apply`: Update application with validation and derivation
//! - `broadcast`: Layer update broadcasting and page update emission
//! - `reconcile`: Periodic reconciliation and sync target resolution

use tracing::{debug, instrument};

use crate::state::ScribeState;
use crate::{Result, ScribeError};

// Sub-modules

pub mod apply;
pub mod broadcast;
pub mod reconcile;
pub mod subscription;
pub mod sync_meta;

// Re-exports for backward compatibility

// Subscription handlers
pub use subscription::{handle_subscribe, handle_unsubscribe};

// Apply handlers
pub use apply::handle_apply_update;

// Broadcast handlers
pub use broadcast::{broadcast_update, notify_layer_discovered};

// Reconcile handlers
pub use reconcile::{emit_sync_events_for_missing_targets, handle_reconcile_with_peers};

// Sync Request Handlers (kept here - simple pass-through handlers)

/// Handle sync request - export updates since their version
#[instrument(skip(state, their_vector), fields(page_id = %state.page_id, layer = %layer_name))]
pub fn handle_sync_request(
    state: &ScribeState,
    layer_name: &str,
    their_vector: &[u8],
) -> Result<Vec<u8>> {
    let layer = state
        .units
        .get(layer_name)
        .map(|u| u.layer())
        .ok_or_else(|| ScribeError::LayerNotFound(layer_name.to_string()))?;

    layer
        .export_updates(their_vector)
        .map_err(|e| ScribeError::CrdtError(e.to_string()))
}

/// Handle export snapshot request
#[instrument(skip(state), fields(page_id = %state.page_id, layer = %layer_name))]
pub fn handle_export_snapshot(state: &ScribeState, layer_name: &str) -> Result<Vec<u8>> {
    let layer = state
        .units
        .get(layer_name)
        .map(|u| u.layer())
        .ok_or_else(|| ScribeError::LayerNotFound(layer_name.to_string()))?;

    Ok(layer.export_snapshot())
}

/// Handle get state vector request
#[instrument(skip(state), fields(page_id = %state.page_id, layer = %layer_name))]
pub fn handle_get_state_vector(state: &ScribeState, layer_name: &str) -> Result<Vec<u8>> {
    let layer = state
        .units
        .get(layer_name)
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
    let layer = state
        .units
        .get(layer_name)
        .map(|u| u.layer())
        .ok_or_else(|| ScribeError::LayerNotFound(layer_name.to_string()))?;

    layer
        .export_updates(their_vector)
        .map_err(|e| ScribeError::CrdtError(e.to_string()))
}

/// Handle ReplaceLayer - replace a layer entirely with an authoritative snapshot
///
/// **Context**: Authoritative snapshot replacement (SyncReset recovery).
/// Creates the layer if it doesn't exist, replaces its content, and sets up
/// observers and subscribers for new layers.
pub fn handle_replace_layer(
    state: &mut ScribeState,
    layer_name: &str,
    snapshot: &[u8],
    from_peer: Option<(String, String, Vec<u8>)>,
) -> std::result::Result<(), String> {
    let is_new = !state.units.contains_key(layer_name);

    if is_new {
        let mut unit = crate::layer_unit::LayerUnit::new_empty();
        unit.set_local_only(!state.should_sync_layer(layer_name));
        state.units.insert(layer_name.to_string(), unit);
        tracing::info!(layer = %layer_name, "Created new layer for snapshot replacement");
    }

    tracing::info!(layer = %layer_name, snapshot_len = snapshot.len(), "Replacing layer with authoritative snapshot");
    match domains::Layer::from_snapshot(snapshot) {
        Ok(layer) => {
            if let Some(unit) = state.units.get_mut(layer_name) {
                unit.replace_layer(layer);
                unit.mark_dirty();
            }
            tracing::info!(layer = %layer_name, "Layer replaced successfully");

            if let Some((did, device_id, state_vector)) = from_peer {
                if let Some(unit) = state.units.get(layer_name) {
                    unit.update_subscriber_vector(&did, &device_id, state_vector);
                }
            }

            if is_new {
                if let Ok(subs) = state.subscribers.read() {
                    for ((did, device_id), info) in subs.iter() {
                        if let Some(unit) = state.units.get(layer_name) {
                            unit.add_subscriber(
                                did.clone(),
                                device_id.clone(),
                                true,
                                info.broadcast_tx.clone(),
                            );
                        }
                    }
                }
                crate::loro_observer::setup_layer_observer(state, layer_name);
                broadcast::notify_layer_discovered(state, layer_name);
            }

            Ok(())
        }
        Err(e) => {
            tracing::error!(layer = %layer_name, error = %e, "Failed to parse snapshot for layer replacement");
            Err(format!("Failed to parse snapshot: {}", e))
        }
    }
}

/// Handle ExportLayerSnapshot - export snapshot and state vector for a layer
///
/// **Context**: Peer needs both snapshot and state vector for a layer (e.g., LayerSubscribeAck).
/// **We do**: Return (snapshot, state_vector) tuple or error if layer not found.
#[instrument(skip(state), fields(page_id = %state.page_id, layer = %layer_name))]
pub fn handle_export_layer_snapshot(
    state: &ScribeState,
    layer_name: &str,
) -> std::result::Result<(Vec<u8>, Vec<u8>), String> {
    if let Some(unit) = state.units.get(layer_name) {
        let snapshot = unit.layer().export_snapshot();
        let state_vector = unit.layer().version_vector();
        Ok((snapshot, state_vector))
    } else {
        Err(format!("Layer '{}' not found", layer_name))
    }
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

    // Collect all vectors from LayerUnit subscribers
    let mut peer_vectors: std::collections::HashMap<
        (String, String),
        std::collections::HashMap<String, Vec<u8>>,
    > = std::collections::HashMap::new();

    for (layer_name, unit) in state.units.iter() {
        if let Ok(subs) = unit.subscribers().read() {
            for ((user_did, device_id), sub) in subs.iter() {
                if !sub.version_vector.is_empty() {
                    peer_vectors
                        .entry((user_did.clone(), device_id.clone()))
                        .or_insert_with(std::collections::HashMap::new)
                        .insert(layer_name.clone(), sub.version_vector.clone());
                }
            }
        }
    }

    // Persist each peer's vectors
    for ((user_did, device_id), vectors) in peer_vectors {
        if let Err(e) = state
            .vector_storage
            .save_vectors(&user_did, &device_id, &vectors)
        {
            tracing::warn!(user_did = %user_did, device_id = %device_id, error = %e, "Failed to save peer vectors");
        }
    }
}
