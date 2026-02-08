//! Loro CRDT observer and delta handling
//!
//! Handles subscription to Loro changes, delta conversion, and reactive notifications.
//!
//! **Architecture**: Loro's `import()` triggers `subscribe_root` observers automatically.
//! We set up observers for all layers at startup so that both UI notifications and
//! peer broadcasts are handled through the observer pattern (no manual callbacks needed).

use loro::event::{Diff, DiffEvent, ListDiffItem};
use loro::{LoroValue, ValueOrContainer};
use tokio::sync::mpsc;
use tracing::{debug, info, instrument, warn};
use base64::Engine;

use crate::message::{BroadcastPayload, LoroDelta, ListOp, PageUpdate};
use crate::permit::Permissions;
use crate::state::ScribeState;
use domains::JsonOp;

/// Data sent through observer channel (delta + ops for reactive bindings)
struct ObserverSignal {
    delta: Option<LoroDelta>,
    ops: Vec<JsonOp>,
    /// Source of the change: None = local, Some = remote peer (user_did, device_id)
    /// Used for sender exclusion in broadcasts
    from_peer: Option<(String, String)>,
}

// Delta Conversion Helpers

/// Convert DiffEvent to structured JsonOps for reactive bindings
///
/// **Context**: Observer receives DiffEvent on Loro changes
/// **We do**: Extract structured ops (insert/update/delete) with keys/values
/// **Used by**: Lua's on_loro_change(layer, ops, full_data) for surgical UI updates
pub fn diff_event_to_ops(diff_event: &DiffEvent) -> Vec<JsonOp> {
    let mut ops = Vec::new();

    for container_diff in &diff_event.events {
        let path = container_diff.target.to_string();

        match &container_diff.diff {
            Diff::List(items) => {
                // Track position for list operations
                let mut pos = 0usize;
                for item in items.iter() {
                    match item {
                        ListDiffItem::Insert { insert, .. } => {
                            for (i, value) in insert.iter().enumerate() {
                                let json_value = value_or_container_to_json(value);
                                ops.push(JsonOp {
                                    op: "insert".to_string(),
                                    path: path.clone(),
                                    key: None,
                                    index: Some(pos + i),
                                    value: Some(json_value),
                                    old_value: None,
                                });
                            }
                            pos += insert.len();
                        }
                        ListDiffItem::Delete { delete } => {
                            // Each delete is a single op
                            for _ in 0..*delete {
                                ops.push(JsonOp {
                                    op: "delete".to_string(),
                                    path: path.clone(),
                                    key: None,
                                    index: Some(pos),
                                    value: None,
                                    old_value: None,
                                });
                            }
                            // Position doesn't advance for deletes
                        }
                        ListDiffItem::Retain { retain } => {
                            // Retain just moves position, no op
                            pos += retain;
                        }
                    }
                }
            }

            Diff::Map(map_delta) => {
                for (key, value) in map_delta.updated.iter() {
                    let json_value = value.as_ref()
                        .map(|v| value_or_container_to_json(v));

                    if let Some(val) = json_value {
                        // Could be insert or update - we use "set" for both
                        ops.push(JsonOp {
                            op: "set".to_string(),
                            path: path.clone(),
                            key: Some(key.to_string()),
                            index: None,
                            value: Some(val),
                            old_value: None,  // TODO: Could track old values if needed
                        });
                    } else {
                        // Value is None = deletion
                        ops.push(JsonOp {
                            op: "delete".to_string(),
                            path: path.clone(),
                            key: Some(key.to_string()),
                            index: None,
                            value: None,
                            old_value: None,
                        });
                    }
                }
            }

            Diff::Text(_text_delta) => {
                // Text deltas not yet supported for ops
                debug!("Text ops not yet implemented");
            }

            _ => {
                debug!("Unsupported diff type for ops: {:?}", container_diff.diff);
            }
        }
    }

    ops
}

/// Convert Loro's Diff to our LoroDelta format
///
/// **Context**: Loro observer provides Diff on changes, we convert to JSON-serializable format
/// **Returns**: LoroDelta for List/Map/Text containers, None for unsupported types
pub fn convert_loro_diff_to_delta(diff: &Diff) -> Option<LoroDelta> {
    match diff {
        Diff::List(items) => {
            let ops = items.iter().map(|item| {
                match item {
                    ListDiffItem::Insert { insert, .. } => {
                        // Convert ValueOrContainer to JSON values
                        let values = insert.iter()
                            .map(|v| value_or_container_to_json(v))
                            .collect();
                        ListOp::Insert { values }
                    }
                    ListDiffItem::Delete { delete } => {
                        ListOp::Delete { count: *delete }
                    }
                    ListDiffItem::Retain { retain } => {
                        ListOp::Retain { count: *retain }
                    }
                }
            }).collect();

            Some(LoroDelta::List { ops })
        }

        Diff::Map(map_delta) => {
            let updated = map_delta.updated.iter()
                .map(|(k, v)| {
                    let value = v.as_ref()
                        .map(|v| value_or_container_to_json(v));
                    (k.to_string(), value)
                })
                .collect();

            Some(LoroDelta::Map { updated })
        }

        Diff::Text(_text_delta) => {
            // TODO: Implement text delta conversion when needed
            // For now, return None and fall back to full data
            debug!("Text delta conversion not yet implemented");
            None
        }

        _ => {
            debug!("Unsupported diff type: {:?}", diff);
            None
        }
    }
}

/// Convert Loro ValueOrContainer to serde_json::Value
///
/// **Context**: Loro stores values as LoroValue or nested Containers
/// **We do**: Extract deep value and convert to JSON
fn value_or_container_to_json(value: &ValueOrContainer) -> serde_json::Value {
    // Use ValueOrContainer::get_deep_value() to resolve containers
    let deep_value = value.get_deep_value();
    loro_value_to_json(&deep_value)
}

/// Convert LoroValue to serde_json::Value
///
/// **Context**: Loro's native value type → JSON for Lua consumption
pub fn loro_value_to_json(value: &LoroValue) -> serde_json::Value {
    match value {
        LoroValue::Null => serde_json::Value::Null,
        LoroValue::Bool(b) => serde_json::Value::Bool(*b),
        LoroValue::I64(i) => serde_json::json!(*i),
        LoroValue::Double(f) => serde_json::json!(*f),
        LoroValue::String(s) => serde_json::Value::String(s.to_string()),
        LoroValue::List(arr) => {
            serde_json::Value::Array(
                arr.iter().map(loro_value_to_json).collect()
            )
        }
        LoroValue::Map(map) => {
            serde_json::Value::Object(
                map.iter()
                    .map(|(k, v)| (k.to_string(), loro_value_to_json(v)))
                    .collect()
            )
        }
        LoroValue::Binary(bytes) => {
            // Encode binary as base64 string
            serde_json::Value::String(base64::engine::general_purpose::STANDARD.encode(&**bytes))
        }
        LoroValue::Container(_) => {
            // Containers are resolved by get_deep_value, shouldn't appear in deltas
            debug!("Container in delta value (unexpected), converting to null");
            serde_json::Value::Null
        }
    }
}

// Loro Layer Handler

/// Handle layer modified by Lua via direct FFI access
///
/// **Context**: Lua called push/set/etc on a LoroList/LoroMap
/// **We do**: Check permission, commit transaction, let Loro observer handle notifications
///
/// **Note**: By the time this is called, the change is already in the Loro doc (via FFI).
/// We can't reject it here, but we log a warning if permission would be denied.
/// Future: Lua bindings should check can_local_write() before allowing the operation.
#[instrument(skip_all, fields(page_id = %state.page_id, layer = %layer_name))]
pub async fn handle_layer_modified_by_lua(state: &mut ScribeState, layer_name: String) {
    debug!(layer_name = %layer_name, "Layer modified by Lua, committing transaction");

    // Check if we have permission to write to this layer
    // Note: Changes are already made via FFI, this is defense-in-depth logging
    if !Permissions::can_local_write(state, &layer_name) {
        warn!(
            layer = %layer_name,
            our_did = %state.our_did,
            "Local write to layer without permission (changes already applied via FFI)"
        );
        // TODO: In future, Lua bindings should check permission before allowing write
        // For now, we still commit to avoid leaving Loro in inconsistent state
    }

    // Commit the Loro transaction to trigger native observer
    // This makes the observer fire immediately with delta information
    if let Some(layer) = state.layers.get(&layer_name) {
        layer.commit();
    }

    // Mark layer as dirty for periodic flush (every 10s)
    state.dirty_layers.insert(layer_name.clone());

    // NOTE: We don't manually notify observers here!
    // The commit() above triggers Loro's native observer (setup in handle_subscribe_loro_changes)
    // which will automatically:
    // - Send LoroChangeEvent with delta to UI
    // - Notify query subscribers
    // - Broadcast to peers
    //
    // Dirty layer will be flushed to storage by periodic timer (every 10s)
}

// Startup Observer Setup

/// Set up Loro observers for all layers at Scribe startup
///
/// **Context**: Called from `pre_start` to ensure observers are ready before any updates
/// **Why**: Loro's `import()` triggers observers automatically, so we need them set up
/// **Handles**: Peer broadcasts (to subscribers) + UI notifications (to loro_observers)
///
/// This ensures both local changes (via commit) and remote changes (via import)
/// go through the same observer-based notification path.
#[instrument(skip_all, fields(page_id = %state.page_id))]
pub fn setup_observers_for_all_layers(state: &mut ScribeState) {
    let layer_names: Vec<String> = state.layers.keys().cloned().collect();

    for layer_name in layer_names {
        setup_layer_observer(state, &layer_name);
    }

    info!(
        page_id = %state.page_id,
        layer_count = state.loro_subscriptions.len(),
        "Set up Loro observers for all layers"
    );
}

/// Set up Loro observer for a single layer
///
/// **Context**: Sets up the observer infrastructure for peer broadcasts and UI notifications
/// **Observer fires**: On commit() AND import() - both trigger subscribe_root
///
/// **Public**: Called from mod.rs when new layers are dynamically created
#[instrument(skip(state), fields(page_id = %state.page_id))]
pub fn setup_layer_observer(state: &mut ScribeState, layer_name: &str) {
    // Skip if already set up
    if state.loro_subscriptions.contains_key(layer_name) {
        return;
    }

    let layer = match state.layers.get(layer_name) {
        Some(l) => l,
        None => {
            warn!(layer_name = %layer_name, "Layer not found for observer setup");
            return;
        }
    };

    // Create internal channel for observer signals (includes ops for reactive bindings)
    let (signal_tx, mut signal_rx) = tokio::sync::mpsc::unbounded_channel::<ObserverSignal>();

    // Clone for the async task
    let layer_for_task = layer.clone();
    let layer_name_for_task = layer_name.to_string();
    let subscribers_for_task = state.subscribers.clone();
    let local_only_for_task = state.local_only_layers.clone();
    let page_id_for_task = state.page_id.clone();
    let page_update_subscribers_for_task = state.page_update_subscribers.clone();

    // Clone pending_update_source for the synchronous callback
    let pending_update_source = state.pending_update_source.clone();

    // Spawn async task to handle observer signals
    // Note: Derivation is handled in handle_apply_update using ops, not here
    tokio::spawn(async move {
        while let Some(signal) = signal_rx.recv().await {
            let state_vector = layer_for_task.version_vector();

            // Prepare ops for reactive bindings (Some if non-empty, None otherwise)
            let ops = if signal.ops.is_empty() { None } else { Some(signal.ops) };

            // Skip expensive get_content() when delta is available — consumers use surgical path
            let full_data = if signal.delta.is_some() {
                None
            } else {
                Some(layer_for_task.get_content(&layer_name_for_task))
            };

            let page_update = PageUpdate::LayerChanged {
                layer: layer_name_for_task.clone(),
                from_peer: None,  // Local change
                ops,
                delta: signal.delta,
                state_vector,
                full_data,
                created: false,
            };
            if let Ok(subs) = page_update_subscribers_for_task.read() {
                for tx in subs.iter() {
                    let _ = tx.try_send(page_update.clone());
                }
                if !subs.is_empty() {
                    debug!(
                        layer_name = %layer_name_for_task,
                        subscriber_count = subs.len(),
                        "Sent PageUpdate::LayerChanged"
                    );
                }
            }

            // Check if layer is local-only (sync: false in permit)
            // If so, skip peer broadcasting entirely
            if let Ok(local_only) = local_only_for_task.read() {
                if local_only.contains(&layer_name_for_task) {
                    debug!(
                        layer_name = %layer_name_for_task,
                        "Skipping peer broadcast for local-only layer (sync: false)"
                    );
                    continue;
                }
            }

            // Skip peer broadcast for remote updates (sender exclusion)
            // Remote updates are forwarded via broadcast_update() in apply.rs which has proper sender exclusion
            // Broadcasting here would send the update back to the original sender, causing a sync storm
            if signal.from_peer.is_some() {
                debug!(
                    layer_name = %layer_name_for_task,
                    "Skipping observer peer broadcast for remote update (handled by broadcast_update)"
                );
                continue;
            }

            // Broadcast incremental updates to peer subscribers
            // Uses per-subscriber export_updates(their_vector) instead of full snapshot
            let current_vector = layer_for_task.version_vector();

            if let Ok(subs) = subscribers_for_task.read() {
                debug!(
                    page_id = %page_id_for_task,
                    layer_name = %layer_name_for_task,
                    subscriber_count = subs.len(),
                    "Observer broadcasting to subscribers"
                );
                for ((user_did, device_id), info) in subs.iter() {
                    if !info.can_receive_layer(&layer_name_for_task, &page_id_for_task) {
                        continue;
                    }

                    // Incremental export: use subscriber's last known vector
                    // Falls back to full snapshot for first sync (no vector cached)
                    let update = if let Some(their_vector) = info.vectors.get(&*layer_name_for_task) {
                        layer_for_task.export_updates(their_vector)
                            .unwrap_or_else(|_| layer_for_task.export_snapshot())
                    } else {
                        layer_for_task.export_snapshot()
                    };

                    let update_size = update.len();
                    let payload = BroadcastPayload {
                        page_id: page_id_for_task.clone(),
                        layer_name: layer_name_for_task.clone(),
                        update,
                        state_vector: current_vector.clone(),
                        permit: None,
                    };

                    match info.broadcast_tx.try_send(payload) {
                        Ok(()) => {
                            debug!(
                                user_did = %user_did,
                                layer_name = %layer_name_for_task,
                                update_size = update_size,
                                "Sent incremental broadcast to subscriber"
                            );
                        }
                        Err(mpsc::error::TrySendError::Full(_)) => {
                            warn!(user_did = %user_did, device_id = %device_id, "Peer broadcast channel full");
                        }
                        Err(mpsc::error::TrySendError::Closed(_)) => {
                            debug!(user_did = %user_did, "Peer broadcast channel closed");
                        }
                    }
                }
            }
        }
        debug!(layer_name = %layer_name_for_task, "Observer task ended");
    });

    // Subscribe to Loro changes - extract ops and delta for reactive bindings
    let subscription = layer.subscribe_root(move |diff_event: loro::event::DiffEvent| {
        // Extract structured ops for reactive Lua bindings (surgical UI updates)
        let ops = diff_event_to_ops(&diff_event);

        // Extract delta for legacy VecModel operations
        let delta = diff_event.events.first()
            .and_then(|container_diff| convert_loro_diff_to_delta(&container_diff.diff));

        // Read pending_update_source to determine if this is a remote update
        // Remote updates should not trigger peer broadcast (handled by broadcast_update with sender exclusion)
        let from_peer = pending_update_source.lock().ok().and_then(|g| g.clone());

        let _ = signal_tx.send(ObserverSignal { delta, ops, from_peer });
    });

    // Store subscription
    state.loro_subscriptions.insert(layer_name.to_string(), subscription);
    debug!(layer_name = %layer_name, "Set up Loro observer for layer");
}

// Pending Update Source Helpers

/// Set pending update source before import()
///
/// **Context**: Called in handle_apply_update before import()
/// **Usage**: Observer reads this to populate `from_peer` in PageEvent
pub fn set_pending_update_source(state: &ScribeState, from_peer: Option<(String, String)>) {
    if let Ok(mut guard) = state.pending_update_source.lock() {
        *guard = from_peer;
    }
}

/// Clear pending update source after import()
///
/// **Context**: Called in handle_apply_update after import()
/// **Important**: Must be called to reset for next local operation
pub fn clear_pending_update_source(state: &ScribeState) {
    if let Ok(mut guard) = state.pending_update_source.lock() {
        *guard = None;
    }
}
