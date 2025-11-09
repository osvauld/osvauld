//! Merge Service - UCAN-aware CRDT synchronization orchestration
//!
//! This service handles document filtering, state vector generation, and update
//! application based on UCAN capabilities and facts. It orchestrates between
//! ucan_service (for permissions) and document operations (for CRDT sync).
//!
//! Architecture:
//! - Calls ucan_service for UCAN parsing (capabilities, facts)
//! - Calls document.rs for pure CRDT operations (export_updates, apply_updates, etc.)
//! - Uses Resource accessors to get/modify documents
//! - Resource is just a data container - no UCAN logic

use crate::errors::{ResourceServiceError, ServiceResult};
use osvauld_core::models::{
    document::{apply_updates, create_doc, export_shallow_snapshot, export_updates, state_frontiers},
    resource::Resource,
};
use serde_json::Value;
use std::collections::HashMap;

/// Filter documents for sending to a peer based on UCAN permissions
///
/// Checks both our UCAN (dont_send_to_node) and peer's UCAN (capabilities)
/// to determine which documents to send as shallow snapshots.
///
/// # Arguments
/// * `resource` - The resource containing documents
/// * `our_ucan` - Our UCAN token (checks dont_send_to_node)
/// * `peer_ucan` - Peer's UCAN token (checks their capabilities)
///
/// # Returns
/// HashMap of doc_name -> shallow snapshot bytes
pub async fn filter_documents_to_send(
    resource: &Resource,
    our_ucan: &str,
    peer_ucan: &str,
) -> ServiceResult<HashMap<String, Vec<u8>>> {
    // 1. Extract our facts to check dont_send_to_node
    let our_facts = crate::ucan_service::extract_facts(our_ucan)
        .await?
        .unwrap_or_default();

    let dont_send: Vec<String> = our_facts
        .get("dont_send_to_node")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    // 2. Extract peer's document capabilities
    let peer_doc_caps = crate::ucan_service::extract_doc_capabilities(peer_ucan).await?;

    // 3. Filter and export snapshots
    let mut filtered_snapshots = HashMap::new();

    for (doc_name, _ability) in peer_doc_caps {
        // Skip if we're not allowed to send this doc
        if dont_send.contains(&doc_name) {
            continue;
        }

        // Skip if we don't have this doc
        if let Some(doc) = resource.get_doc(&doc_name) {
            // Export as shallow snapshot (viewers don't need full history)
            let snapshot = export_shallow_snapshot(doc);
            filtered_snapshots.insert(doc_name, snapshot);
        }
    }

    Ok(filtered_snapshots)
}

/// Get state vectors filtered by UCAN capabilities
///
/// Returns state vectors only for documents the UCAN grants access to,
/// respecting dont_send_to_node rules.
///
/// # Arguments
/// * `resource` - The resource containing documents
/// * `ucan_token` - UCAN token (contains capabilities)
///
/// # Returns
/// JSON string with state vectors for accessible docs
pub async fn get_state_vectors_for_ucan(
    resource: &Resource,
    ucan_token: &str,
) -> ServiceResult<String> {
    // 1. Extract document capabilities
    let doc_capabilities = crate::ucan_service::extract_doc_capabilities(ucan_token).await?;

    // 2. Extract facts for dont_send_to_node
    let facts = crate::ucan_service::extract_facts(ucan_token).await?;

    let dont_send_to_node: Vec<String> = facts
        .as_ref()
        .and_then(|f| f.get("dont_send_to_node"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    // 3. Build filtered state vectors
    let mut result = serde_json::Map::new();

    for doc_name in resource.doc_names() {
        // Skip if not in capabilities
        if !doc_capabilities.contains_key(&doc_name) {
            continue;
        }

        // Skip if in dont_send_to_node
        if dont_send_to_node.contains(&doc_name) {
            continue;
        }

        // Get state vector for this doc
        if let Some(doc) = resource.get_doc(&doc_name) {
            let state_vector = state_frontiers(doc);

            let vector_array: Vec<Value> = state_vector
                .iter()
                .map(|&b| Value::Number(serde_json::Number::from(b)))
                .collect();

            let mut doc_data = serde_json::Map::new();
            doc_data.insert("state_vector".to_string(), Value::Array(vector_array));

            result.insert(doc_name, Value::Object(doc_data));
        }
    }

    serde_json::to_string(&result)
        .map_err(|e| ResourceServiceError::InvalidResourceData(e.to_string()).into())
}

/// Generate updates for a peer based on UCAN permissions and state vectors
///
/// # Arguments
/// * `resource` - The resource containing documents
/// * `peer_ucan` - Peer's UCAN token
/// * `peer_state_vectors_json` - Peer's current state vectors
///
/// # Returns
/// JSON string with updates for docs peer is behind on
pub async fn generate_updates_for_peer(
    resource: &Resource,
    peer_ucan: &str,
    peer_state_vectors_json: &str,
) -> ServiceResult<String> {
    // 1. Parse peer state vectors
    let peer_vectors: HashMap<String, serde_json::Map<String, Value>> =
        serde_json::from_str(peer_state_vectors_json)
            .map_err(|e| ResourceServiceError::InvalidResourceData(e.to_string()))?;

    // 2. Extract peer capabilities
    let peer_doc_caps = crate::ucan_service::extract_doc_capabilities(peer_ucan).await?;

    // 3. Check peer's dont_send_to_node
    let peer_facts = crate::ucan_service::extract_facts(peer_ucan)
        .await?
        .unwrap_or_default();

    let dont_send_to_peer: Vec<String> = peer_facts
        .get("dont_send_to_node")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    // 4. Generate updates
    let mut result = serde_json::Map::new();

    for (doc_name, doc_data) in peer_vectors {
        // Skip if peer doesn't have capability
        if !peer_doc_caps.contains_key(&doc_name) {
            continue;
        }

        // Skip if in peer's dont_send_to_node
        if dont_send_to_peer.contains(&doc_name) {
            continue;
        }

        // Generate updates if we have this doc
        if let Some(doc) = resource.get_doc(&doc_name) {
            // Extract peer's state vector
            let peer_state_vector: Vec<u8> = doc_data
                .get("state_vector")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_u64().map(|n| n as u8))
                        .collect()
                })
                .unwrap_or_default();

            // Generate updates from peer's version
            let updates = export_updates(doc, &peer_state_vector)
                .map_err(|e| ResourceServiceError::Loro(e.to_string()))?;

            // Get current state vector
            let current_state_vector = state_frontiers(doc);

            // Build response
            let updates_array: Vec<Value> = updates
                .iter()
                .map(|&b| Value::Number(serde_json::Number::from(b)))
                .collect();

            let state_vector_array: Vec<Value> = current_state_vector
                .iter()
                .map(|&b| Value::Number(serde_json::Number::from(b)))
                .collect();

            let mut doc_result = serde_json::Map::new();
            doc_result.insert("updates".to_string(), Value::Array(updates_array));
            doc_result.insert("state_vector".to_string(), Value::Array(state_vector_array));

            result.insert(doc_name, Value::Object(doc_result));
        }
    }

    serde_json::to_string(&result)
        .map_err(|e| ResourceServiceError::InvalidResourceData(e.to_string()).into())
}

/// Apply updates from a peer after validating UCAN permissions
///
/// # Arguments
/// * `resource` - Mutable resource to apply updates to
/// * `peer_ucan` - Peer's UCAN token
/// * `updates_json` - Updates from peer
pub async fn apply_peer_updates(
    resource: &mut Resource,
    peer_ucan: &str,
    updates_json: &str,
) -> ServiceResult<()> {
    // 1. Parse updates
    let input: HashMap<String, serde_json::Map<String, Value>> =
        serde_json::from_str(updates_json)
            .map_err(|e| ResourceServiceError::InvalidResourceData(e.to_string()))?;

    // 2. Extract peer capabilities
    let peer_doc_caps = crate::ucan_service::extract_doc_capabilities(peer_ucan).await?;

    // 3. Apply updates for each doc
    for (doc_name, doc_data) in input {
        // Check peer capability
        let ability = peer_doc_caps
            .get(&doc_name)
            .ok_or_else(|| {
                ResourceServiceError::UcanError(
                    format!("Peer does not have capability for doc: {}", doc_name)
                )
            })?;

        // Reject if peer only has readonly
        if ability == "crud/readonly" {
            return Err(ResourceServiceError::UcanError(
                format!("Peer has readonly access to {}, cannot accept updates", doc_name)
            ).into());
        }

        // Extract updates
        let update_bytes: Vec<u8> = doc_data
            .get("updates")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_u64().map(|n| n as u8))
                    .collect()
            })
            .unwrap_or_default();

        if update_bytes.is_empty() {
            continue;
        }

        // Get or create doc
        if let Some(doc) = resource.get_doc_mut(&doc_name) {
            // Apply updates to existing doc
            apply_updates(doc, &update_bytes)
                .map_err(|e| ResourceServiceError::Loro(e.to_string()))?;
        } else {
            // Create new doc and apply updates
            let mut doc = create_doc();
            apply_updates(&mut doc, &update_bytes)
                .map_err(|e| ResourceServiceError::Loro(e.to_string()))?;
            resource.insert_doc(doc_name, doc);
        }
    }

    Ok(())
}

/// Apply updates with additional filtering based on our UCAN
///
/// Used by viewers to respect no_update_from_node rules.
///
/// # Arguments
/// * `resource` - Mutable resource to apply updates to
/// * `our_ucan` - Our UCAN token (checks no_update_from_node)
/// * `peer_ucan` - Peer's UCAN token
/// * `updates_json` - Updates from peer
pub async fn apply_peer_updates_filtered(
    resource: &mut Resource,
    our_ucan: &str,
    peer_ucan: &str,
    updates_json: &str,
) -> ServiceResult<()> {
    // 1. Extract our facts for no_update_from_node
    let our_facts = crate::ucan_service::extract_facts(our_ucan)
        .await?
        .unwrap_or_default();

    let no_update: Vec<String> = our_facts
        .get("no_update_from_node")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    // 2. Parse and filter updates
    let input: HashMap<String, serde_json::Map<String, Value>> =
        serde_json::from_str(updates_json)
            .map_err(|e| ResourceServiceError::InvalidResourceData(e.to_string()))?;

    let mut filtered_input = serde_json::Map::new();
    for (doc_name, doc_data) in input {
        if !no_update.contains(&doc_name) {
            filtered_input.insert(doc_name, Value::Object(doc_data));
        }
    }

    // 3. Apply filtered updates
    let filtered_json = serde_json::to_string(&filtered_input)
        .map_err(|e| ResourceServiceError::InvalidResourceData(e.to_string()))?;

    apply_peer_updates(resource, peer_ucan, &filtered_json).await
}

/// Extract state vectors from updates JSON
///
/// Utility function to convert updates format to state vectors format
/// by removing updates and keeping only state_vector.
///
/// # Input Format
/// ```json
/// {
///   "doc_name": {
///     "updates": [1, 2, 3, ...],
///     "state_vector": [4, 5, 6, ...]
///   }
/// }
/// ```
///
/// # Output Format
/// ```json
/// {
///   "doc_name": {
///     "state_vector": [4, 5, 6, ...]
///   }
/// }
/// ```
pub fn extract_state_vectors_from_updates(updates_json: &str) -> ServiceResult<String> {
    let input: HashMap<String, serde_json::Map<String, Value>> =
        serde_json::from_str(updates_json)
            .map_err(|e| ResourceServiceError::InvalidResourceData(e.to_string()))?;

    let mut result = serde_json::Map::new();

    for (doc_name, doc_data) in input {
        if let Some(state_vector) = doc_data.get("state_vector") {
            let mut doc_result = serde_json::Map::new();
            doc_result.insert("state_vector".to_string(), state_vector.clone());
            result.insert(doc_name, Value::Object(doc_result));
        }
    }

    serde_json::to_string(&result)
        .map_err(|e| ResourceServiceError::InvalidResourceData(e.to_string()).into())
}
