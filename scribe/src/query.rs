//! Query handling for Scribe actor
//!
//! Handles query execution, filtering, sorting, pagination, and subscriptions.

use tokio::sync::mpsc;
use tracing::{debug, info, instrument, warn};

use crate::message::PageUpdate;
use crate::permit::glob_match;
use crate::Result;
use crate::ScribeError;
use domains::{QueryDelta, QueryResult, QuerySpec, SortOrder, Sthithi};

use crate::state::{QuerySubscriberInfo, ScribeState};

// Page Update Subscription

/// Handle SubscribeToPageUpdates message
///
/// **Context**: App opens a page and wants to receive all page-level events.
/// **We do**: Send PeerSubscribed for all currently connected peers, then register the subscriber.
#[instrument(skip_all, fields(page_id = %state.page_id))]
pub fn handle_subscribe_to_page_updates(state: &mut ScribeState, tx: mpsc::Sender<PageUpdate>) {
    // Send PeerSubscribed for all currently subscribed peers
    // (peers that subscribed before this app opened)
    if let Ok(subs) = state.subscribers.read() {
        info!(page_id = %state.page_id, peer_count = subs.len(), "Sending existing peers as PeerSubscribed");
        for ((user_did, _device_id), _info) in subs.iter() {
            let _ = tx.try_send(PageUpdate::PeerSubscribed {
                did: user_did.clone(),
                username: None,
            });
        }
    }

    // Presence state is delivered via CRDT sync (presence layer snapshot)
    // No need to send OnlinePeers event — apps read from LayerChanged on {page_id}/presence

    if let Ok(mut subs) = state.page_update_subscribers.write() {
        subs.push(tx);
        info!(page_id = %state.page_id, subscriber_count = subs.len(), "PageUpdate subscriber added");
    }
}

// Layer Listing and Data

/// Handle ListLayers message
///
/// **Context**: Lua/UI wants to know which layers exist matching a glob pattern.
/// **We do**: Match layer names against glob pattern, excluding protocol layers.
pub fn handle_list_layers(state: &ScribeState, pattern: &str) -> Vec<String> {
    state
        .units
        .keys()
        .filter(|name| !crate::sync::sync_meta::is_protocol_layer(name))
        .filter(|name| glob_match(pattern, name))
        .cloned()
        .collect()
}

/// Handle GetLayerData message
///
/// **Context**: Lua/UI wants the full content of a specific layer.
/// **We do**: Return the layer's content as Sthithi, or error if not found.
pub fn handle_get_layer_data(state: &ScribeState, layer_name: &str) -> Result<Sthithi> {
    state
        .units
        .get(layer_name)
        .map(|unit| unit.layer().get_content_sthithi(layer_name))
        .ok_or_else(|| ScribeError::LayerNotFound(layer_name.to_string()))
}

// Query Handlers

/// Handle a query request
///
/// **Context**: HUML renderer wants filtered/sorted/paginated data
/// **We do**: Execute query against layer, apply filter/sort/limit
#[instrument(skip(state), fields(page_id = %state.page_id, layer = %spec.layer_name))]
pub fn handle_query(state: &ScribeState, spec: &QuerySpec) -> Result<QueryResult> {
    // Layer may not exist yet for new pages - return empty result
    let Some(unit) = state.units.get(&spec.layer_name) else {
        debug!(layer = %spec.layer_name, "Layer not found, returning empty result");
        return Ok(QueryResult {
            query_id: spec.query_id.clone(),
            items: Vec::new(),
            total_count: 0,
            has_more: false,
            version: 0,
        });
    };

    // Get the data at the specified path
    let layer_sthithi = unit.layer().to_sthithi();
    let layer_json = serde_json::Value::from(&layer_sthithi);
    let data = get_path_value(&layer_json, &spec.path);

    // Extract array items
    let items = match data {
        Some(serde_json::Value::Array(arr)) => arr,
        Some(other) => vec![other],
        None => Vec::new(),
    };

    let total_count = items.len();

    // Apply sort
    let sorted_items = apply_sort(state, &items, spec);

    // Apply pagination
    let offset = spec.offset;
    let limit = spec.limit.unwrap_or(usize::MAX);
    let paginated: Vec<serde_json::Value> =
        sorted_items.into_iter().skip(offset).take(limit).collect();

    let has_more = offset + paginated.len() < total_count;

    Ok(QueryResult {
        query_id: spec.query_id.clone(),
        items: paginated,
        total_count,
        has_more,
        version: 0,
    })
}

/// Handle query subscription
///
/// **Context**: HUML renderer wants incremental updates
/// **We do**: Store subscription, send initial result as Reset delta
#[instrument(skip_all, fields(page_id = %state.page_id, query_id = %spec.query_id))]
pub async fn handle_subscribe_query(
    state: &mut ScribeState,
    spec: QuerySpec,
    delta_tx: mpsc::Sender<QueryDelta>,
) {
    let query_id = spec.query_id.clone();
    info!(page_id = %state.page_id, query_id = %query_id, "Query subscription added");

    // Execute initial query
    let initial_result = match handle_query(state, &spec) {
        Ok(result) => result,
        Err(e) => {
            warn!(error = %e, query_id = %query_id, "Failed to execute initial query");
            QueryResult::empty(query_id.clone())
        }
    };

    // Send initial result as Reset delta
    let delta = QueryDelta::reset(query_id.clone(), initial_result.clone(), 1);
    if let Err(e) = delta_tx.try_send(delta) {
        warn!(error = %e, query_id = %query_id, "Failed to send initial query result");
    }

    // Store subscription
    state.query_subscribers.insert(
        query_id,
        QuerySubscriberInfo {
            spec,
            delta_tx,
            version: 1,
            last_result: Some(initial_result),
        },
    );
}

/// Build a Sthithi context from all layers for rendering
///
/// **Context**: renderer needs all layer data for CEL evaluation
/// **We do**: Export each layer's Sthithi and merge into a single map
#[instrument(skip(state), fields(page_id = %state.page_id))]
pub fn build_context_sthithi(state: &ScribeState) -> Sthithi {
    let mut context = Vec::new();

    for (layer_name, unit) in &state.units {
        let layer_sthithi = unit.layer().to_sthithi();

        // If layer value is a map, flatten its fields into context
        // Otherwise, store under layer name
        if let Sthithi::Map(map) = layer_sthithi {
            for (key, value) in map {
                context.push((key, value));
            }
        } else {
            context.push((layer_name.clone(), layer_sthithi));
        }
    }

    Sthithi::Map(context)
}

// Helper Functions

/// Get value at a JSON path (simple dot notation)
///
/// **Example**: "messages" → data["root"]["messages"] (Loro stores under "root" container)
/// **Example**: "users.active" → data["root"]["users"]["active"]
fn get_path_value(data: &serde_json::Value, path: &str) -> Option<serde_json::Value> {
    if path.is_empty() {
        return Some(data.clone());
    }

    // Loro layers store data under a "root" container, so prepend "root" to the path
    let full_path = format!("root.{}", path);
    let parts: Vec<&str> = full_path.split('.').collect();
    let mut current = data;

    for part in parts {
        match current {
            serde_json::Value::Object(map) => {
                current = map.get(part)?;
            }
            serde_json::Value::Array(arr) => {
                // If path is a number, index into array
                if let Ok(idx) = part.parse::<usize>() {
                    current = arr.get(idx)?;
                } else {
                    return None;
                }
            }
            _ => return None,
        }
    }

    Some(current.clone())
}

/// Apply sorting to items
///
/// **Note**: Basic implementation - sorts by string comparison
/// Full sorting with type awareness will be in QueryBridge
fn apply_sort(state: &ScribeState, items: &[serde_json::Value], spec: &QuerySpec) -> Vec<serde_json::Value> {
    let (sort_by, desc) = resolve_sort_config(state, spec);
    let Some(sort_by) = sort_by else {
        return items.to_vec();
    };
    let sort_by_ref = sort_by.as_str();

    let mut sorted = items.to_vec();

    sorted.sort_by(|a, b| {
        let a_val = a.get(sort_by_ref);
        let b_val = b.get(sort_by_ref);

        let cmp = match (a_val, b_val) {
            (Some(serde_json::Value::Number(a)), Some(serde_json::Value::Number(b))) => a
                .as_f64()
                .unwrap_or(0.0)
                .partial_cmp(&b.as_f64().unwrap_or(0.0))
                .unwrap_or(std::cmp::Ordering::Equal),
            (Some(serde_json::Value::String(a)), Some(serde_json::Value::String(b))) => a.cmp(b),
            (Some(a), Some(b)) => a.to_string().cmp(&b.to_string()),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        };

        if desc {
            cmp.reverse()
        } else {
            cmp
        }
    });

    sorted
}

fn resolve_sort_config(state: &ScribeState, spec: &QuerySpec) -> (Option<String>, bool) {
    resolve_sort_config_with_policy(spec, state.validation_artifact.as_ref())
}

fn resolve_sort_config_with_policy(
    spec: &QuerySpec,
    validation: Option<&gurkha::ValidationArtifact>,
) -> (Option<String>, bool) {
    if let Some(sort_by) = &spec.sort_by {
        let desc = matches!(spec.sort_order, Some(SortOrder::Desc));
        return (Some(sort_by.clone()), desc);
    }

    let Some(validation) = validation else {
        return (None, false);
    };
    let Some(order) = validation.orderings.get(&spec.layer_name) else {
        return (None, false);
    };

    let desc = matches!(order.direction, gurkha::SortDirection::Descending);
    (Some(order.field.clone()), desc)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_sort_overrides_policy_default() {
        let spec = QuerySpec {
            query_id: "q1".to_string(),
            layer_name: "orders/{customer}/entries".to_string(),
            path: "entries".to_string(),
            filter: None,
            sort_by: Some("created_at".to_string()),
            sort_order: Some(SortOrder::Asc),
            offset: 0,
            limit: None,
        };

        let mut orderings = std::collections::HashMap::new();
        orderings.insert(
            "orders/{customer}/entries".to_string(),
            gurkha::OrderPolicy {
                field: "updated_at".to_string(),
                direction: gurkha::SortDirection::Descending,
            },
        );
        let validation = gurkha::ValidationArtifact {
            lua_validators: std::collections::HashMap::new(),
            broadcasts: std::collections::HashMap::new(),
            orderings,
            dynamic_layers: std::collections::HashMap::new(),
        };

        let (field, desc) = resolve_sort_config_with_policy(&spec, Some(&validation));
        assert_eq!(field.as_deref(), Some("created_at"));
        assert!(!desc);
    }

    #[test]
    fn policy_default_sort_applies_when_query_unspecified() {
        let spec = QuerySpec {
            query_id: "q1".to_string(),
            layer_name: "orders/{customer}/entries".to_string(),
            path: "entries".to_string(),
            filter: None,
            sort_by: None,
            sort_order: None,
            offset: 0,
            limit: None,
        };

        let mut orderings = std::collections::HashMap::new();
        orderings.insert(
            "orders/{customer}/entries".to_string(),
            gurkha::OrderPolicy {
                field: "updated_at".to_string(),
                direction: gurkha::SortDirection::Descending,
            },
        );
        let validation = gurkha::ValidationArtifact {
            lua_validators: std::collections::HashMap::new(),
            broadcasts: std::collections::HashMap::new(),
            orderings,
            dynamic_layers: std::collections::HashMap::new(),
        };

        let (field, desc) = resolve_sort_config_with_policy(&spec, Some(&validation));
        assert_eq!(field.as_deref(), Some("updated_at"));
        assert!(desc);
    }
}
