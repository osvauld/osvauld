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

use crate::layer_unit::LayerUnit;
use crate::state::ScribeState;
use crate::loro_observer;

// Typed CRDT Operation Handlers

/// Helper to ensure a layer exists, returns true if layer was newly created
fn ensure_layer_exists(state: &mut ScribeState, layer_name: &str) -> bool {
    let is_new = !state.units.contains_key(layer_name);
    if is_new {
        let mut unit = LayerUnit::new_empty();
        unit.set_local_only(!state.should_sync_layer(layer_name));
        state.units.insert(layer_name.to_string(), unit);
        state.track_new_sync_layer_in_meta(layer_name);
        debug!(layer = %layer_name, "Created new layer");
    }
    is_new
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
