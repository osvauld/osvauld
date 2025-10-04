use core::fmt;

use crate::models::{
    ResourceKey, ResourceVectorClock, ShareRecord,
    document::{YjsDocExt, create_doc},
};
use chrono::Local;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resource {
    pub id: String,
    pub resource_type: ResourceType,
    pub data: String,
    pub folder_id: String,
    pub created_folder_id: String,
    pub signature: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub favourite: bool,
    pub created_by: String,
    pub last_accessed: i64,
    pub deleted: bool,
    pub deleted_at: Option<i64>,
}

impl Resource {
    pub fn new(
        resource_type: String,
        data: String,
        folder_id: String,
        signature: String,
        created_by: String,
    ) -> Self {
        let now = Local::now().timestamp_millis();

        Self {
            id: uuid::Uuid::new_v4().to_string(),
            resource_type: ResourceType::from_str(&resource_type),
            data,
            created_folder_id: folder_id.clone(),
            folder_id: folder_id,
            signature,
            created_at: now,
            updated_at: now,
            favourite: false,
            created_by,
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

    pub async fn get_state_vectors(&self) -> Result<String, String> {
        let mut result = serde_json::Map::new();

        for state_key in self.resource_type.document_state_keys() {
            if let Some(state_data) = self.get_document_state(state_key) {
                let mut doc = create_doc();
                doc.apply_update_v2(&state_data).await?;
                let state_vector = doc.get_state_vector_v2().await;

                let vector_array: Vec<serde_json::Value> = state_vector
                    .iter()
                    .map(|&b| serde_json::Value::Number(serde_json::Number::from(b)))
                    .collect();

                let mut doc_data = serde_json::Map::new();
                doc_data.insert("updates".to_string(), serde_json::Value::Array(vec![])); // Empty updates
                doc_data.insert(
                    "state_vector".to_string(),
                    serde_json::Value::Array(vector_array),
                );

                result.insert(state_key.to_string(), serde_json::Value::Object(doc_data));
            }
        }

        serde_json::to_string(&result)
            .map_err(|e| format!("Failed to serialize state vectors: {}", e))
    }
    /// Input: {"yjs_state": {"updates": [1,2,3], "state_vector": [4,5,6]}, "image_state": {...}}
    /// Logic:
    /// - If updates empty + state_vector present → generate updates for peer
    /// - If updates present + state_vector empty → apply updates only  
    /// - If both present → apply updates AND generate updates for peer
    /// - If both empty → do nothing
    /// /// Returns: {"yjs_state": {"updates": [1,2,3], "state_vector": [4,5,6]}, "image_state": {"updates": [7,8,9], "state_vector": [10,11,12]}}
    pub async fn sync_updates(&mut self, input_data: &str) -> Result<String, String> {
        let input_json: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(input_data).map_err(|e| format!("Invalid input JSON: {}", e))?;

        let mut result_updates = serde_json::Map::new();

        for state_key in self.resource_type.document_state_keys() {
            if let Some(our_state_data) = self.get_document_state(state_key) {
                if let Some(doc_data) = input_json.get(state_key) {
                    let mut doc = create_doc();
                    doc.apply_update_v2(&our_state_data).await?;

                    // Parse input updates and state vector
                    let input_updates: Vec<u8> = doc_data
                        .get("updates")
                        .and_then(|u| u.as_array())
                        .map(|array| {
                            array
                                .iter()
                                .filter_map(|v| v.as_u64().map(|n| n as u8))
                                .collect()
                        })
                        .unwrap_or_default();

                    let input_state_vector: Vec<u8> = doc_data
                        .get("state_vector")
                        .and_then(|sv| sv.as_array())
                        .map(|array| {
                            array
                                .iter()
                                .filter_map(|v| v.as_u64().map(|n| n as u8))
                                .collect()
                        })
                        .unwrap_or_default();

                    let has_updates = !input_updates.is_empty();
                    let has_state_vector = !input_state_vector.is_empty();
                    let mut doc_was_updated = false;

                    // Apply updates if present
                    if has_updates {
                        doc.apply_update_v2(&input_updates).await?;
                        doc_was_updated = true;
                    }

                    // Generate updates for peer if state vector present
                    if has_state_vector {
                        let updates_for_peer = doc.get_diff_update_v2(&input_state_vector).await?;
                        let current_state_vector = doc.get_state_vector_v2().await;

                        // Always return both updates and current state vector
                        let updates_array: Vec<serde_json::Value> = updates_for_peer
                            .iter()
                            .map(|&b| serde_json::Value::Number(serde_json::Number::from(b)))
                            .collect();

                        let state_vector_array: Vec<serde_json::Value> = current_state_vector
                            .iter()
                            .map(|&b| serde_json::Value::Number(serde_json::Number::from(b)))
                            .collect();

                        let mut doc_result = serde_json::Map::new();
                        doc_result.insert(
                            "updates".to_string(),
                            serde_json::Value::Array(updates_array),
                        );
                        doc_result.insert(
                            "state_vector".to_string(),
                            serde_json::Value::Array(state_vector_array),
                        );
                        result_updates
                            .insert(state_key.to_string(), serde_json::Value::Object(doc_result));
                    }

                    // Update our resource data if doc was modified
                    if doc_was_updated {
                        let new_state = doc.get_state_as_update_v2().await;
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
                }
            }
        }

        serde_json::to_string(&result_updates)
            .map_err(|e| format!("Failed to serialize result: {}", e))
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
    Chat,
    Default,
}

impl ResourceType {
    /// Returns whether this resource type supports CRDT operations
    pub fn has_crdt(&self) -> bool {
        match self {
            ResourceType::Notes => true,
            ResourceType::Chat => true,
            ResourceType::Default => true,
        }
    }

    /// Returns the document state keys that this resource type uses
    pub fn document_state_keys(&self) -> Vec<&'static str> {
        match self {
            ResourceType::Notes => vec!["main_doc", "image_state", "comment_state"],
            ResourceType::Chat => vec!["chat", "image_state"],
            ResourceType::Default => vec!["yjs_state"],
        }
    }

    /// Returns the primary document state key (used for main content sync)
    pub fn primary_state_key(&self) -> Option<&'static str> {
        match self {
            ResourceType::Notes => Some("main_doc"),
            ResourceType::Chat => Some("chat"),
            ResourceType::Default => Some("yjs_state"),
        }
    }

    /// Convert from string representation with fallback to Default
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "notes" => ResourceType::Notes,
            "chat" => ResourceType::Chat,
            _ => ResourceType::Default, // All unknown types default to Default
        }
    }

    /// Convert to string representation
    pub fn to_string(&self) -> String {
        match self {
            ResourceType::Notes => "notes".to_string(),
            ResourceType::Chat => "chat".to_string(),
            ResourceType::Default => "default".to_string(),
        }
    }
}
impl fmt::Display for ResourceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_string())
    }
}
