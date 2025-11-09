use std::collections::HashMap;
use loro::LoroDoc;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::models::document::{export_shallow_snapshot, import_snapshot};

// ============================================================================
// Core Structs
// ============================================================================

/// EncryptedResource - Database and network representation
/// Contains encrypted data and metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedResource {
    pub id: String,
    pub folder_id: String,
    pub created_at: i64,
    pub updated_at: i64,

    /// Encrypted JSON containing document snapshots
    pub encrypted_data: String,

    /// AES key encrypted with recipient's public key
    pub encrypted_key: String,

    /// UCAN token defining capabilities and sync rules
    pub ucan_token: String,

    /// Unencrypted metadata (title, type, search config)
    /// Format: {"title": "...", "type": "notes", "search": {"docs": [...]}}
    pub metadata: Value,
}

impl EncryptedResource {
    /// Create a re-encrypted version for a different recipient
    ///
    /// Preserves id, folder_id, metadata, timestamps while replacing
    /// encrypted data, key, and UCAN token.
    ///
    /// # Arguments
    /// * `new_encrypted_data` - Resource data encrypted for new recipient
    /// * `new_encrypted_key` - AES key encrypted for new recipient's public key
    /// * `new_ucan_token` - UCAN token for new recipient
    ///
    /// # Returns
    /// New EncryptedResource instance for the recipient
    pub fn re_encrypt_for_recipient(
        &self,
        new_encrypted_data: String,
        new_encrypted_key: String,
        new_ucan_token: String,
    ) -> Self {
        Self {
            id: self.id.clone(),
            folder_id: self.folder_id.clone(),
            created_at: self.created_at,
            updated_at: self.updated_at,
            encrypted_data: new_encrypted_data,
            encrypted_key: new_encrypted_key,
            ucan_token: new_ucan_token,
            metadata: self.metadata.clone(),
        }
    }
}

/// Resource - Runtime representation with decrypted Loro documents
/// Always contains decrypted data when in memory
#[derive(Debug)]
pub struct Resource {
    pub id: String,
    pub folder_id: String,
    pub ucan_token: String,
    pub metadata: Value,

    /// Loaded Loro documents, keyed by document name
    pub docs: HashMap<String, LoroDoc>,
}

// ============================================================================
// Helper Structs
// ============================================================================

/// Document update with both updates and state vector
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocUpdate {
    pub updates: Vec<u8>,
    pub state_vector: Vec<u8>,
}


// ============================================================================
// Resource Implementation
// ============================================================================

impl Resource {
    /// Create a new Resource from decrypted data
    ///
    /// # Arguments
    /// * `id` - Resource ID
    /// * `folder_id` - Folder ID
    /// * `ucan_token` - UCAN token string
    /// * `metadata` - Metadata JSON value
    /// * `decrypted_data_json` - Decrypted JSON string containing document snapshots
    ///
    /// # Decrypted Data Format
    /// ```json
    /// {
    ///   "main_doc": [1, 2, 3, ...],
    ///   "image_state": [4, 5, 6, ...],
    ///   "comment_state": [7, 8, 9, ...]
    /// }
    /// ```
    pub fn from_decrypted_data(
        id: String,
        folder_id: String,
        ucan_token: String,
        metadata: Value,
        decrypted_data_json: &str,
    ) -> Result<Self, String> {
        // Parse the decrypted data JSON
        let data: HashMap<String, Vec<Value>> = serde_json::from_str(decrypted_data_json)
            .map_err(|e| format!("Failed to parse decrypted data JSON: {}", e))?;

        let mut docs = HashMap::new();

        // Load each document
        for (doc_name, snapshot_array) in data {
            // Convert JSON array to Vec<u8>
            let snapshot_bytes: Vec<u8> = snapshot_array
                .iter()
                .filter_map(|v| v.as_u64().map(|n| n as u8))
                .collect();

            if !snapshot_bytes.is_empty() {
                // Import the snapshot into a new LoroDoc
                let doc = import_snapshot(&snapshot_bytes)
                    .map_err(|e| format!("Failed to import snapshot for {}: {}", doc_name, e))?;
                docs.insert(doc_name, doc);
            }
        }

        Ok(Resource {
            id,
            folder_id,
            ucan_token,
            metadata,
            docs,
        })
    }

    // ============================================================================
    // Document Accessor Methods
    // ============================================================================

    /// Get immutable reference to a document by name
    pub fn get_doc(&self, doc_name: &str) -> Option<&LoroDoc> {
        self.docs.get(doc_name)
    }

    /// Get mutable reference to a document by name
    pub fn get_doc_mut(&mut self, doc_name: &str) -> Option<&mut LoroDoc> {
        self.docs.get_mut(doc_name)
    }

    /// Insert or replace a document
    pub fn insert_doc(&mut self, doc_name: String, doc: LoroDoc) {
        self.docs.insert(doc_name, doc);
    }

    /// Get all document names
    pub fn doc_names(&self) -> Vec<String> {
        self.docs.keys().cloned().collect()
    }

    /// Check if document exists
    pub fn has_doc(&self, doc_name: &str) -> bool {
        self.docs.contains_key(doc_name)
    }

    // ============================================================================
    // Serialization Methods
    // ============================================================================

    /// Export all documents as JSON for encryption and storage
    ///
    /// # Returns
    /// JSON string with all document snapshots as byte arrays
    /// Format: {"doc_name": [1,2,3,...], ...}
    pub fn to_json(&self) -> Result<String, String> {
        let mut result = serde_json::Map::new();

        for (doc_name, doc) in &self.docs {
            let snapshot = export_shallow_snapshot(doc);

            let snapshot_array: Vec<Value> = snapshot
                .iter()
                .map(|&b| Value::Number(serde_json::Number::from(b)))
                .collect();

            result.insert(doc_name.clone(), Value::Array(snapshot_array));
        }

        serde_json::to_string(&result)
            .map_err(|e| format!("Failed to serialize to JSON: {}", e))
    }
}


