//! Typed CRDT operation handlers
//!
//! Handles list, map, and counter operations for Scribe actor.
//! **Broadcast**: Handled automatically by Loro observer after commit()

use tracing::{info, instrument};

use crate::error::{ButlerError, Result};
use crate::models::Layer;

use super::state::ScribeState;

// =============================================================================
// Typed CRDT Operation Handlers
// =============================================================================

/// Handle list push operation (append)
///
/// **Broadcast**: Loro observer triggers broadcast after commit()
#[instrument(skip(state, item), fields(page_id = %state.page_id, layer = %layer_name, path = %path))]
pub async fn handle_list_push(
    state: &mut ScribeState,
    layer_name: &str,
    path: &str,
    item: serde_json::Value,
) -> Result<()> {
    info!("ListPush operation");

    let layer = state.layers.entry(layer_name.to_string())
        .or_insert_with(Layer::new);

    layer.list_push(path, &item)
        .map_err(|e| ButlerError::Layer(e.to_string()))?;

    // Commit triggers Loro observer which broadcasts to peers
    layer.commit();
    state.dirty_layers.insert(layer_name.to_string());

    Ok(())
}

/// Handle list insert operation
///
/// **Broadcast**: Loro observer triggers broadcast after commit()
#[instrument(skip(state, item), fields(page_id = %state.page_id, layer = %layer_name, path = %path))]
pub async fn handle_list_insert(
    state: &mut ScribeState,
    layer_name: &str,
    path: &str,
    index: usize,
    item: serde_json::Value,
) -> Result<()> {
    info!("ListInsert operation");

    let layer = state.layers.entry(layer_name.to_string())
        .or_insert_with(Layer::new);

    layer.list_insert(path, index, &item)
        .map_err(|e| ButlerError::Layer(e.to_string()))?;

    // Commit triggers Loro observer which broadcasts to peers
    layer.commit();
    state.dirty_layers.insert(layer_name.to_string());

    Ok(())
}

/// Handle list delete operation
///
/// **Broadcast**: Loro observer triggers broadcast after commit()
#[instrument(skip(state), fields(page_id = %state.page_id, layer = %layer_name, path = %path))]
pub async fn handle_list_delete(
    state: &mut ScribeState,
    layer_name: &str,
    path: &str,
    index: usize,
) -> Result<()> {
    info!("ListDelete operation");

    let layer = state.layers.entry(layer_name.to_string())
        .or_insert_with(Layer::new);

    layer.list_delete(path, index)
        .map_err(|e| ButlerError::Layer(e.to_string()))?;

    // Commit triggers Loro observer which broadcasts to peers
    layer.commit();
    state.dirty_layers.insert(layer_name.to_string());

    Ok(())
}

/// Handle map insert operation
///
/// **Broadcast**: Loro observer triggers broadcast after commit()
#[instrument(skip(state, value), fields(page_id = %state.page_id, layer = %layer_name, path = %path))]
pub async fn handle_map_insert(
    state: &mut ScribeState,
    layer_name: &str,
    path: &str,
    key: &str,
    value: serde_json::Value,
) -> Result<()> {
    info!("MapInsert operation");

    let layer = state.layers.entry(layer_name.to_string())
        .or_insert_with(Layer::new);

    layer.map_insert(path, key, &value)
        .map_err(|e| ButlerError::Layer(e.to_string()))?;

    // Commit triggers Loro observer which broadcasts to peers
    layer.commit();
    state.dirty_layers.insert(layer_name.to_string());

    Ok(())
}

/// Handle map delete operation
///
/// **Broadcast**: Loro observer triggers broadcast after commit()
#[instrument(skip(state), fields(page_id = %state.page_id, layer = %layer_name, path = %path))]
pub async fn handle_map_delete(
    state: &mut ScribeState,
    layer_name: &str,
    path: &str,
    key: &str,
) -> Result<()> {
    info!("MapDelete operation");

    let layer = state.layers.entry(layer_name.to_string())
        .or_insert_with(Layer::new);

    layer.map_delete(path, key)
        .map_err(|e| ButlerError::Layer(e.to_string()))?;

    // Commit triggers Loro observer which broadcasts to peers
    layer.commit();
    state.dirty_layers.insert(layer_name.to_string());

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

    let layer = state.layers.entry(layer_name.to_string())
        .or_insert_with(Layer::new);

    layer.counter_inc(path, amount)
        .map_err(|e| ButlerError::Layer(e.to_string()))?;

    // Commit triggers Loro observer which broadcasts to peers
    layer.commit();
    state.dirty_layers.insert(layer_name.to_string());

    Ok(())
}
