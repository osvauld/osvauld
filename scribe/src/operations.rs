//! Typed CRDT operation handlers
//!
//! Handles list, map, and counter operations for Scribe actor.
//! **Broadcast**: Handled automatically by Loro observer after commit()
//!
//! **Path convention**:
//! - Empty path ("") = use layer_name as container name directly (Lua bindings)
//! - Non-empty path = use root map pattern with nested containers (template actions)

use tracing::{debug, info, instrument};

use crate::{ScribeError, Result};
use domains::json_to_loro_value;

use crate::layer_unit::{LayerUnit, find_matching_dynamic_schema};
use crate::state::ScribeState;
use crate::loro_observer;

// Typed CRDT Operation Handlers

/// Helper to ensure a layer exists, returns true if layer was newly created.
///
/// **Sync-target auto-subscription**: When a new syncable layer is created and we have
/// a sync_target (owner/viewer mode), the sync_target is auto-added as subscriber.
/// This ensures new dynamic layers propagate to the node via the normal CRDT observer
/// broadcast path — no separate __sync_meta discovery needed.
fn ensure_layer_exists(state: &mut ScribeState, layer_name: &str) -> bool {
    let is_new = !state.units.contains_key(layer_name);
    if is_new {
        let mut unit = LayerUnit::new_empty();
        unit.set_local_only(!state.should_sync_layer(layer_name));

        // Mark as dynamic if it matches a dynamic schema (node-side detection)
        if let Some(ref permit) = state.our_permit {
            if find_matching_dynamic_schema(permit, layer_name, &state.page_id).is_some() {
                unit.is_dynamic = true;
            }
        }

        state.units.insert(layer_name.to_string(), unit);

        // Auto-subscribe sync_target to new syncable layers
        auto_subscribe_sync_target(state, layer_name);

        debug!(layer = %layer_name, "Created new layer");
    }

    is_new
}

/// Auto-subscribe the sync_target to a newly created syncable layer.
///
/// **Context**: Owner/viewer creates a new layer. The sync_target (node) needs to
/// receive data for it. By adding them as subscriber, the Loro observer broadcasts
/// will include them, and the PeerActor sends SyncOffer to the node.
pub(crate) fn auto_subscribe_sync_target(state: &ScribeState, layer_name: &str) {
    if !state.should_sync_layer(layer_name) {
        info!(layer = %layer_name, "auto_subscribe_sync_target: layer not syncable, skipping");
        return;
    }

    let sync_target_did = match state.sync_config.as_ref() {
        Some(cfg) => match cfg.sync_target.as_deref() {
            Some(did) => did.to_string(),
            None => {
                info!(layer = %layer_name, "auto_subscribe_sync_target: no sync_target in config");
                return;
            }
        },
        None => {
            info!(layer = %layer_name, "auto_subscribe_sync_target: no sync_config");
            return;
        }
    };

    let sub_count = state.subscribers.read().map(|s| s.len()).unwrap_or(0);
    let broadcast_tx = state.subscribers.read().ok().and_then(|subs| {
        subs.iter()
            .find(|((d, _), _)| d == &sync_target_did)
            .map(|(_, info)| info.broadcast_tx.clone())
    });

    if broadcast_tx.is_none() {
        info!(
            layer = %layer_name,
            sync_target = %sync_target_did,
            subscriber_count = sub_count,
            "auto_subscribe_sync_target: sync_target not found in subscribers"
        );
        // Log all subscriber DIDs for debugging
        if let Ok(subs) = state.subscribers.read() {
            for ((did, _), _) in subs.iter() {
                info!(layer = %layer_name, subscriber_did = %did, "  existing subscriber");
            }
        }
    }

    if let (Some(unit), Some(tx)) = (state.units.get(layer_name), broadcast_tx) {
        let caps = crate::layer_unit::Capabilities { read: true, write: true, sync: true };
        unit.add_subscriber(sync_target_did.clone(), caps, tx);
        info!(layer = %layer_name, target = %sync_target_did, "Auto-subscribed sync_target to new layer");
    }
}

/// Handle list push operation (append)
///
/// **Broadcast**: Loro observer triggers broadcast after commit()
/// **Path**: Empty = use layer_name as container; non-empty = root map pattern
#[instrument(skip(state, item), fields(page_id = %state.page_id, layer = %layer_name, path = %path))]
pub async fn handle_list_push(
    state: &mut ScribeState,
    layer_name: &str,
    path: &str,
    item: serde_json::Value,
) -> Result<()> {
    info!("ListPush operation");

    // Ensure layer exists
    let is_new_layer = ensure_layer_exists(state, layer_name);

    let unit = state.units.get_mut(layer_name)
        .ok_or_else(|| ScribeError::LayerNotFound(layer_name.to_string()))?;

    if path.is_empty() {
        // Lua pattern: use layer_name as container name directly
        let list = unit.layer().loro().get_list(layer_name);
        let loro_value = json_to_loro_value(&item);
        list.push(loro_value)
            .map_err(|e| ScribeError::CrdtError(format!("ListPush: {}", e)))?;
        unit.layer().commit();
    } else {
        // Template pattern: use root map with nested path
        unit.layer().list_push(path, &item)
            .map_err(|e| ScribeError::CrdtError(format!("Layer: {}", e)))?;
    }

    unit.mark_dirty();

    // Set up observer if new layer (after first write)
    if is_new_layer {
        loro_observer::setup_layer_observer(state, layer_name);
    }

    Ok(())
}

/// Handle list insert operation
///
/// **Broadcast**: Loro observer triggers broadcast after commit()
/// **Path**: Empty = use layer_name as container; non-empty = root map pattern
#[instrument(skip(state, item), fields(page_id = %state.page_id, layer = %layer_name, path = %path))]
pub async fn handle_list_insert(
    state: &mut ScribeState,
    layer_name: &str,
    path: &str,
    index: usize,
    item: serde_json::Value,
) -> Result<()> {
    info!("ListInsert operation");

    let is_new_layer = ensure_layer_exists(state, layer_name);

    let unit = state.units.get_mut(layer_name)
        .ok_or_else(|| ScribeError::LayerNotFound(layer_name.to_string()))?;

    if path.is_empty() {
        // Lua pattern: use layer_name as container name directly
        let list = unit.layer().loro().get_list(layer_name);
        let loro_value = json_to_loro_value(&item);
        list.insert(index, loro_value)
            .map_err(|e| ScribeError::CrdtError(format!("ListInsert: {}", e)))?;
        unit.layer().commit();
    } else {
        // Template pattern: use root map with nested path
        unit.layer().list_insert(path, index, &item)
            .map_err(|e| ScribeError::CrdtError(format!("Layer: {}", e)))?;
    }

    unit.mark_dirty();

    if is_new_layer {
        loro_observer::setup_layer_observer(state, layer_name);
    }

    Ok(())
}

/// Handle list delete operation
///
/// **Broadcast**: Loro observer triggers broadcast after commit()
/// **Path**: Empty = use layer_name as container; non-empty = root map pattern
#[instrument(skip(state), fields(page_id = %state.page_id, layer = %layer_name, path = %path))]
pub async fn handle_list_delete(
    state: &mut ScribeState,
    layer_name: &str,
    path: &str,
    index: usize,
) -> Result<()> {
    info!("ListDelete operation");

    let is_new_layer = ensure_layer_exists(state, layer_name);

    let unit = state.units.get_mut(layer_name)
        .ok_or_else(|| ScribeError::LayerNotFound(layer_name.to_string()))?;

    if path.is_empty() {
        // Lua pattern: use layer_name as container name directly
        let list = unit.layer().loro().get_list(layer_name);
        list.delete(index, 1)
            .map_err(|e| ScribeError::CrdtError(format!("ListDelete: {}", e)))?;
        unit.layer().commit();
    } else {
        // Template pattern: use root map with nested path
        unit.layer().list_delete(path, index)
            .map_err(|e| ScribeError::CrdtError(format!("Layer: {}", e)))?;
    }

    unit.mark_dirty();

    if is_new_layer {
        loro_observer::setup_layer_observer(state, layer_name);
    }

    Ok(())
}

/// Handle map insert operation
///
/// **Broadcast**: Loro observer triggers broadcast after commit()
/// **Path**: Empty = use layer_name as container; non-empty = root map pattern
#[instrument(skip(state, value), fields(page_id = %state.page_id, layer = %layer_name, path = %path))]
pub async fn handle_map_insert(
    state: &mut ScribeState,
    layer_name: &str,
    path: &str,
    key: &str,
    value: serde_json::Value,
) -> Result<()> {
    info!("MapInsert operation");

    let is_new_layer = ensure_layer_exists(state, layer_name);

    let unit = state.units.get_mut(layer_name)
        .ok_or_else(|| ScribeError::LayerNotFound(layer_name.to_string()))?;

    if path.is_empty() {
        // Lua pattern: use layer_name as container name directly
        let map = unit.layer().loro().get_map(layer_name);
        let loro_value = json_to_loro_value(&value);
        map.insert(key, loro_value)
            .map_err(|e| ScribeError::CrdtError(format!("MapInsert: {}", e)))?;
        unit.layer().commit();
    } else {
        // Template pattern: use root map with nested path
        unit.layer().map_insert(path, key, &value)
            .map_err(|e| ScribeError::CrdtError(format!("Layer: {}", e)))?;
    }

    unit.mark_dirty();

    if is_new_layer {
        loro_observer::setup_layer_observer(state, layer_name);
    }

    Ok(())
}

/// Handle map delete operation
///
/// **Broadcast**: Loro observer triggers broadcast after commit()
/// **Path**: Empty = use layer_name as container; non-empty = root map pattern
#[instrument(skip(state), fields(page_id = %state.page_id, layer = %layer_name, path = %path))]
pub async fn handle_map_delete(
    state: &mut ScribeState,
    layer_name: &str,
    path: &str,
    key: &str,
) -> Result<()> {
    info!("MapDelete operation");

    let is_new_layer = ensure_layer_exists(state, layer_name);

    let unit = state.units.get_mut(layer_name)
        .ok_or_else(|| ScribeError::LayerNotFound(layer_name.to_string()))?;

    if path.is_empty() {
        // Lua pattern: use layer_name as container name directly
        let map = unit.layer().loro().get_map(layer_name);
        map.delete(key).ok(); // Ignore if key doesn't exist
        unit.layer().commit();
    } else {
        // Template pattern: use root map with nested path
        unit.layer().map_delete(path, key)
            .map_err(|e| ScribeError::CrdtError(format!("Layer: {}", e)))?;
    }

    unit.mark_dirty();

    if is_new_layer {
        loro_observer::setup_layer_observer(state, layer_name);
    }

    Ok(())
}

/// Handle counter increment operation
///
/// **Broadcast**: Loro observer triggers broadcast after commit()
#[instrument(skip(state), fields(page_id = %state.page_id, layer = %layer_name, path = %path))]
pub async fn handle_counter_inc(
    state: &mut ScribeState,
    layer_name: &str,
    path: &str,
    amount: i64,
) -> Result<()> {
    info!("CounterInc operation");

    let is_new_layer = ensure_layer_exists(state, layer_name);

    let unit = state.units.get_mut(layer_name)
        .ok_or_else(|| ScribeError::LayerNotFound(layer_name.to_string()))?;

    unit.layer().counter_inc(path, amount)
        .map_err(|e| ScribeError::CrdtError(format!("Layer: {}", e)))?;

    unit.mark_dirty();

    if is_new_layer {
        loro_observer::setup_layer_observer(state, layer_name);
    }

    Ok(())
}
