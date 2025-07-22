use core::fmt;

use crate::models::{ResourceKey, ResourceVectorClock, ShareRecord, document};
use chrono::Local;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resource {
    pub id: String,
    pub resource_type: ResourceType,
    pub data: String,
    pub folder_id: String,
    pub signature: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub favourite: bool,
    pub last_accessed: i64,
    pub deleted: bool,
    pub deleted_at: Option<i64>,
}

impl Resource {
    pub fn new(resource_type: String, data: String, folder_id: String, signature: String) -> Self {
        let now = Local::now().timestamp_millis();

        Self {
            id: uuid::Uuid::new_v4().to_string(),
            resource_type: ResourceType::from_str(&resource_type),
            data,
            folder_id,
            signature,
            created_at: now,
            updated_at: now,
            favourite: false,
            last_accessed: now,
            deleted: false,
            deleted_at: None,
        }
    }
    /// Check if this resource supports CRDT operations
    pub fn supports_crdt(&self) -> bool {
        self.resource_type.has_crdt()
    }

    /// Get the document state keys for this resource
    pub fn get_document_state_keys(&self) -> Vec<&'static str> {
        self.resource_type.document_state_keys()
    }

    /// Get the primary state key for sync operations
    pub fn get_primary_state_key(&self) -> Option<&'static str> {
        self.resource_type.primary_state_key()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DecryptedResource {
    pub id: String,
    pub resource_type: ResourceType,
    pub data: Value,
    pub last_accessed: i64,
    pub favourite: bool,
    pub folder_id: String,
}

impl DecryptedResource {
    /// Check if this resource supports CRDT operations
    pub fn supports_crdt(&self) -> bool {
        self.resource_type.has_crdt()
    }

    /// Get a specific document state by key
    pub fn get_document_state(&self, state_key: &str) -> Option<Vec<u8>> {
        self.data
            .get(state_key)
            .and_then(|state| state.as_array())
            .map(|array| {
                array
                    .iter()
                    .filter_map(|v| v.as_u64().map(|n| n as u8))
                    .collect()
            })
    }

    /// Get all available document states for this resource
    pub fn get_all_document_states(&self) -> std::collections::HashMap<String, Vec<u8>> {
        let mut states = std::collections::HashMap::new();

        for state_key in self.resource_type.document_state_keys() {
            if let Some(state_data) = self.get_document_state(state_key) {
                states.insert(state_key.to_string(), state_data);
            }
        }

        states
    }

    /// Get the primary document state (main content for sync)
    pub fn get_primary_document_state(&self) -> Option<Vec<u8>> {
        self.resource_type
            .primary_state_key()
            .and_then(|key| self.get_document_state(key))
    }
    pub async fn get_state_vectors(&self) -> Result<String, String> {
        let mut state_vectors = serde_json::Map::new();

        for state_key in self.resource_type.document_state_keys() {
            if let Some(state_data) = self.get_document_state(state_key) {
                match document::get_state_vector(&state_data).await {
                    Ok(state_vector) => {
                        // Convert Vec<u8> to JSON array
                        let vector_array: Vec<serde_json::Value> = state_vector
                            .iter()
                            .map(|&b| serde_json::Value::Number(serde_json::Number::from(b)))
                            .collect();

                        state_vectors.insert(
                            format!("{}_vector", state_key),
                            serde_json::Value::Array(vector_array),
                        );
                    }
                    Err(e) => {
                        return Err(format!(
                            "Failed to get state vector for {}: {}",
                            state_key, e
                        ));
                    }
                }
            }
        }

        serde_json::to_string(&state_vectors)
            .map_err(|e| format!("Failed to serialize state vectors: {}", e))
    }
    /// Generate updates for a peer based on their state vectors
    /// Takes peer state vectors as JSON string, returns updates as JSON string
    pub async fn generate_updates_for_peer(
        &self,
        peer_state_vectors: &str,
    ) -> Result<String, String> {
        // Parse peer state vectors JSON
        let peer_vectors: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(peer_state_vectors)
                .map_err(|e| format!("Invalid peer state vectors JSON: {}", e))?;

        let mut result_updates = serde_json::Map::new();

        // Process each document state type for this resource
        for state_key in self.resource_type.document_state_keys() {
            // Get our document state
            if let Some(our_state_data) = self.get_document_state(state_key) {
                // Get peer's state vector for this document type
                let peer_vector_key = format!("{}_vector", state_key);
                if let Some(peer_vector_json) = peer_vectors.get(&peer_vector_key) {
                    // Convert peer's state vector from JSON array to Vec<u8>
                    let peer_state_vector: Vec<u8> = match peer_vector_json.as_array() {
                        Some(array) => array
                            .iter()
                            .filter_map(|v| v.as_u64().map(|n| n as u8))
                            .collect(),
                        None => {
                            return Err(format!(
                                "Peer state vector for {} is not an array",
                                state_key
                            ));
                        }
                    };

                    // Generate updates for this document type
                    match document::generate_updates_for_peer(&our_state_data, &peer_state_vector)
                        .await
                    {
                        Ok((updates, current_state_vector)) => {
                            // Convert updates to JSON array
                            let updates_array: Vec<serde_json::Value> = updates
                                .iter()
                                .map(|&b| serde_json::Value::Number(serde_json::Number::from(b)))
                                .collect();

                            // Convert current state vector to JSON array
                            let state_vector_array: Vec<serde_json::Value> = current_state_vector
                                .iter()
                                .map(|&b| serde_json::Value::Number(serde_json::Number::from(b)))
                                .collect();

                            // Add to result
                            let mut document_result = serde_json::Map::new();
                            document_result.insert(
                                "updates".to_string(),
                                serde_json::Value::Array(updates_array),
                            );
                            document_result.insert(
                                "state_vector".to_string(),
                                serde_json::Value::Array(state_vector_array),
                            );

                            result_updates.insert(
                                state_key.to_string(),
                                serde_json::Value::Object(document_result),
                            );
                        }
                        Err(e) => {
                            return Err(format!(
                                "Failed to generate updates for {}: {}",
                                state_key, e
                            ));
                        }
                    }
                }
            }
        }

        serde_json::to_string(&result_updates)
            .map_err(|e| format!("Failed to serialize updates: {}", e))
    }

    /// Apply updates from peer and generate updates to send back to peer
    /// Takes peer updates with state vectors as JSON string, mutates self with applied updates, returns peer updates as JSON string
    pub async fn apply_updates_and_get_peer_updates(
        &mut self,
        peer_updates_with_vectors: &str,
    ) -> Result<String, String> {
        // Parse peer updates JSON (contains both updates and state vectors)
        let updates_json: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(peer_updates_with_vectors)
                .map_err(|e| format!("Invalid peer updates JSON: {}", e))?;

        let mut result_updates = serde_json::Map::new();

        // Process each document state type for this resource
        for state_key in self.resource_type.document_state_keys() {
            // Get our current document state
            if let Some(our_state_data) = self.get_document_state(state_key) {
                // Extract peer updates and state vector from the JSON structure
                let (peer_updates_for_doc, peer_state_vector) =
                    if let Some(peer_doc_data) = updates_json.get(state_key) {
                        let updates = if let Some(updates_array) =
                            peer_doc_data.get("updates").and_then(|u| u.as_array())
                        {
                            updates_array
                                .iter()
                                .filter_map(|v| v.as_u64().map(|n| n as u8))
                                .collect::<Vec<u8>>()
                        } else {
                            Vec::new()
                        };

                        let state_vector = if let Some(vector_array) = peer_doc_data
                            .get("state_vector")
                            .and_then(|sv| sv.as_array())
                        {
                            vector_array
                                .iter()
                                .filter_map(|v| v.as_u64().map(|n| n as u8))
                                .collect::<Vec<u8>>()
                        } else {
                            Vec::new()
                        };

                        (updates, state_vector)
                    } else {
                        (Vec::new(), Vec::new()) // No data for this document type
                    };

                // Apply peer updates, then compare our new state with their state vector to generate updates for them
                match document::apply_updates_and_generate_for_peer(
                    &our_state_data,
                    &peer_updates_for_doc,
                    &peer_state_vector,
                )
                .await
                {
                    Ok((new_state, updates_for_peer, _current_state_vector)) => {
                        // Mutate our resource data with the new merged state
                        let state_array: Vec<serde_json::Value> = new_state
                            .iter()
                            .map(|&b| serde_json::Value::Number(serde_json::Number::from(b)))
                            .collect();

                        if let Some(data_obj) = self.data.as_object_mut() {
                            data_obj.insert(
                                state_key.to_string(),
                                serde_json::Value::Array(state_array),
                            );
                        }

                        // Convert updates for peer to JSON array
                        let updates_array: Vec<serde_json::Value> = updates_for_peer
                            .iter()
                            .map(|&b| serde_json::Value::Number(serde_json::Number::from(b)))
                            .collect();

                        // Add to result - only return updates for peer (they don't need our state vector)
                        let mut document_result = serde_json::Map::new();
                        document_result.insert(
                            "updates".to_string(),
                            serde_json::Value::Array(updates_array),
                        );

                        result_updates.insert(
                            state_key.to_string(),
                            serde_json::Value::Object(document_result),
                        );
                    }
                    Err(e) => {
                        return Err(format!(
                            "Failed to apply updates and generate for peer for {}: {}",
                            state_key, e
                        ));
                    }
                }
            }
        }
        serde_json::to_string(&result_updates)
            .map_err(|e| format!("Failed to serialize peer updates: {}", e))
    }
    /// Apply updates from peer (final step - just apply, no response needed)
    /// Takes peer updates as JSON string and mutates self with applied updates
    pub async fn apply_updates(&mut self, peer_updates: &str) -> Result<(), String> {
        // Parse peer updates JSON
        let updates_json: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(peer_updates)
                .map_err(|e| format!("Invalid peer updates JSON: {}", e))?;

        // Process each document state type for this resource
        for state_key in self.resource_type.document_state_keys() {
            // Get our current document state
            if let Some(our_state_data) = self.get_document_state(state_key) {
                // Extract peer updates from the JSON structure
                let peer_updates_for_doc = if let Some(peer_doc_data) = updates_json.get(state_key)
                {
                    if let Some(updates_array) =
                        peer_doc_data.get("updates").and_then(|u| u.as_array())
                    {
                        updates_array
                            .iter()
                            .filter_map(|v| v.as_u64().map(|n| n as u8))
                            .collect::<Vec<u8>>()
                    } else {
                        Vec::new()
                    }
                } else {
                    Vec::new() // No updates for this document type
                };

                // Apply the updates to get new merged state
                if !peer_updates_for_doc.is_empty() {
                    match document::apply_updates_and_generate_for_peer(
                        &our_state_data,
                        &peer_updates_for_doc,
                        &[], // Empty state vector since we don't need to generate updates
                    )
                    .await
                    {
                        Ok((new_state, _updates_for_peer, _current_state_vector)) => {
                            // Mutate our resource data with the new merged state
                            let state_array: Vec<serde_json::Value> = new_state
                                .iter()
                                .map(|&b| serde_json::Value::Number(serde_json::Number::from(b)))
                                .collect();

                            if let Some(data_obj) = self.data.as_object_mut() {
                                data_obj.insert(
                                    state_key.to_string(),
                                    serde_json::Value::Array(state_array),
                                );
                            }
                        }
                        Err(e) => {
                            return Err(format!(
                                "Failed to apply updates for {}: {}",
                                state_key, e
                            ));
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct ResourceWithKey {
    pub resource: Resource,
    pub encrypted_key: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceKeyPair {
    pub resource: Resource,
    pub key: ResourceKey,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceManifestData {
    pub resource_id: String,
    pub share_record_ids: Vec<String>,
    pub vector_clocks: Vec<ResourceVectorClock>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceSyncData {
    pub resource: Resource,
    pub resource_keys: Vec<ResourceKey>,
    pub share_records: Vec<ShareRecord>,
    pub vector_clocks: Vec<ResourceVectorClock>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ResourceType {
    Notes,
    Default,
}

impl ResourceType {
    /// Returns whether this resource type supports CRDT operations
    pub fn has_crdt(&self) -> bool {
        match self {
            ResourceType::Notes => true,
            ResourceType::Default => true,
        }
    }

    /// Returns the document state keys that this resource type uses
    pub fn document_state_keys(&self) -> Vec<&'static str> {
        match self {
            ResourceType::Notes => vec!["yjs_state", "image_state"],
            ResourceType::Default => vec!["yjs_state"],
        }
    }

    /// Returns the primary document state key (used for main content sync)
    pub fn primary_state_key(&self) -> Option<&'static str> {
        match self {
            ResourceType::Notes => Some("yjs_state"),
            ResourceType::Default => Some("yjs_state"),
        }
    }

    /// Convert from string representation with fallback to Default
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "notes" => ResourceType::Notes,
            _ => ResourceType::Default, // All unknown types default to Default
        }
    }

    /// Convert to string representation
    pub fn to_string(&self) -> String {
        match self {
            ResourceType::Notes => "notes".to_string(),
            ResourceType::Default => "default".to_string(),
        }
    }
}
impl fmt::Display for ResourceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_string())
    }
}
