//! Query handling for Scribe actor
//!
//! Handles query execution, filtering, sorting, pagination, and subscriptions.

use tokio::sync::mpsc;
use tracing::{debug, info, instrument, warn};

use crate::Result;
use domains::{QuerySpec, QueryResult, QueryDelta, SortOrder};

use crate::state::{ScribeState, QuerySubscriberInfo};

// Query Handlers

/// Handle a query request
///
/// **Context**: HUML renderer wants filtered/sorted/paginated data
/// **We do**: Execute query against layer, apply filter/sort/limit
#[instrument(skip(state), fields(page_id = %state.page_id, layer = %spec.layer_name))]
pub fn handle_query(state: &ScribeState, spec: &QuerySpec) -> Result<QueryResult> {
    // Layer may not exist yet for new pages - return empty result
    let Some(layer) = state.layers.get(&spec.layer_name) else {
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
    let layer_json = layer.to_json();
    let data = get_path_value(&layer_json, &spec.path);

    // Extract array items
    let items = match data {
        Some(serde_json::Value::Array(arr)) => arr,
        Some(other) => vec![other],
        None => Vec::new(),
    };

    // Apply CEL filter if specified
    let filtered_items = apply_filter(items, spec.filter.as_deref());
    let total_count = filtered_items.len();

    // Apply sort
    let sorted_items = apply_sort(&filtered_items, spec);

    // Apply pagination
    let offset = spec.offset;
    let limit = spec.limit.unwrap_or(usize::MAX);
    let paginated: Vec<serde_json::Value> = sorted_items
        .into_iter()
        .skip(offset)
        .take(limit)
        .collect();

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

/// Notify query subscribers when a layer changes
///
/// **Context**: Layer was updated, check if any query results changed
/// **We do**: Re-execute affected queries, send deltas for changed results
#[instrument(skip(state), fields(page_id = %state.page_id))]
pub async fn notify_query_subscribers(state: &mut ScribeState, layer_name: &str) {
    // Find queries that depend on this layer
    let affected_queries: Vec<String> = state.query_subscribers
        .iter()
        .filter(|(_, info)| info.spec.layer_name == layer_name)
        .map(|(id, _)| id.clone())
        .collect();

    for query_id in affected_queries {
        // Clone spec to avoid borrow issues
        let spec = match state.query_subscribers.get(&query_id) {
            Some(info) => info.spec.clone(),
            None => continue,
        };

        // Re-execute query with cloned spec
        let new_result = match handle_query(state, &spec) {
            Ok(result) => result,
            Err(e) => {
                warn!(error = %e, query_id = %query_id, "Failed to re-execute query");
                continue;
            }
        };

        // Now get mutable reference to update info
        if let Some(info) = state.query_subscribers.get_mut(&query_id) {
            // Increment version
            info.version += 1;

            // For MVP, send full Reset delta
            // TODO: Compute incremental diffs for Insert/Update/Remove
            let delta = QueryDelta::reset(query_id.clone(), new_result.clone(), info.version);

            match info.delta_tx.try_send(delta) {
                Ok(()) => {
                    info.last_result = Some(new_result);
                }
                Err(mpsc::error::TrySendError::Closed(_)) => {
                    // Subscriber disconnected, will be cleaned up
                    debug!(query_id = %query_id, "Query subscriber disconnected");
                }
                Err(mpsc::error::TrySendError::Full(_)) => {
                    warn!(query_id = %query_id, "Query delta channel full");
                }
            }
        }
    }

    // Clean up disconnected subscribers
    state.query_subscribers.retain(|id, info| {
        if info.delta_tx.is_closed() {
            debug!(query_id = %id, "Removing disconnected query subscriber");
            false
        } else {
            true
        }
    });
}

/// Build a JSON context from all layers for egui rendering
///
/// **Context**: egui window needs all layer data for CEL evaluation
/// **We do**: Export each layer's JSON and merge into a single object
#[instrument(skip(state), fields(page_id = %state.page_id))]
pub fn build_context_json(state: &ScribeState) -> serde_json::Value {
    let mut context = serde_json::Map::new();

    for (layer_name, layer) in &state.layers {
        let layer_json = layer.to_json();

        // If layer JSON is an object, flatten its fields into context
        // Otherwise, store under layer name
        if let serde_json::Value::Object(map) = layer_json {
            for (key, value) in map {
                context.insert(key, value);
            }
        } else {
            context.insert(layer_name.clone(), layer_json);
        }
    }

    serde_json::Value::Object(context)
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

/// Apply filter to items
///
/// **Context**: Filter expression is evaluated against each item
/// **Note**: CEL is being replaced by Rune runtime. Currently stubbed to return all items.
fn apply_filter(items: Vec<serde_json::Value>, filter: Option<&str>) -> Vec<serde_json::Value> {
    let Some(filter_expr) = filter else {
        return items;
    };

    // Skip empty filter expressions
    if filter_expr.trim().is_empty() {
        return items;
    }

    // TODO: Replace with Rune-based filter evaluation
    // For now, return all items (filter is a no-op)
    debug!(filter = %filter_expr, "Filter stubbed - returning all items");
    items
}

/// Apply sorting to items
///
/// **Note**: Basic implementation - sorts by string comparison
/// Full sorting with type awareness will be in QueryBridge
fn apply_sort(items: &[serde_json::Value], spec: &QuerySpec) -> Vec<serde_json::Value> {
    let Some(sort_by) = &spec.sort_by else {
        return items.to_vec();
    };

    let mut sorted = items.to_vec();
    let desc = matches!(spec.sort_order, Some(SortOrder::Desc));

    sorted.sort_by(|a, b| {
        let a_val = a.get(sort_by);
        let b_val = b.get(sort_by);

        let cmp = match (a_val, b_val) {
            (Some(serde_json::Value::Number(a)), Some(serde_json::Value::Number(b))) => {
                a.as_f64().unwrap_or(0.0).partial_cmp(&b.as_f64().unwrap_or(0.0))
                    .unwrap_or(std::cmp::Ordering::Equal)
            }
            (Some(serde_json::Value::String(a)), Some(serde_json::Value::String(b))) => {
                a.cmp(b)
            }
            (Some(a), Some(b)) => {
                a.to_string().cmp(&b.to_string())
            }
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        };

        if desc { cmp.reverse() } else { cmp }
    });

    sorted
}
