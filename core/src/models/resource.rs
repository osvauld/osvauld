use std::collections::HashMap;
use loro::LoroDoc;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::models::document::{
    apply_updates, create_doc, export_shallow_snapshot, export_updates, import_snapshot,
    state_frontiers,
};

// UCAN parsing utilities
use crypto_utils::ucan_utils::{extract_doc_capabilities, extract_facts};

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

    /// Filter documents and export snapshots for sending to a peer
    ///
    /// Uses both our UCAN (to know what we're allowed to send) and peer's UCAN
    /// (to know what they're allowed to receive). Always exports shallow snapshots.
    ///
    /// # Arguments
    /// * `our_ucan` - Our UCAN token (checks dont_send_to_node in facts)
    /// * `peer_ucan` - Peer's UCAN token (checks their capabilities)
    ///
    /// # Returns
    /// HashMap of doc_name -> shallow snapshot bytes
    pub fn filter_to_send(
        &self,
        our_ucan: &str,
        peer_ucan: &str,
    ) -> Result<HashMap<String, Vec<u8>>, String> {
        // Parse our UCAN to check what we shouldn't send
        let our_facts = extract_facts(our_ucan)
            .map_err(|e| e.to_string())?
            .unwrap_or_default();
        let dont_send = our_facts
            .get("dont_send_to_node")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        // Parse peer's UCAN to check what they can access
        let peer_doc_caps = extract_doc_capabilities(peer_ucan)
            .map_err(|e| e.to_string())?;

        let mut filtered_snapshots = HashMap::new();

        // For each doc in peer's capabilities
        for (doc_name, _ability) in peer_doc_caps {
            // Skip if we're not allowed to send this doc
            if dont_send.contains(&doc_name) {
                continue;
            }

            // Skip if we don't have this doc
            if let Some(doc) = self.docs.get(&doc_name) {
                // Export as shallow snapshot (viewers don't need full history)
                let snapshot = export_shallow_snapshot(doc);
                filtered_snapshots.insert(doc_name, snapshot);
            }
        }

        Ok(filtered_snapshots)
    }

    /// Get state vectors for all documents
    ///
    /// Returns version vectors encoded as JSON for sync protocol
    ///
    /// # Returns
    /// JSON string: {"doc_name": {"state_vector": [1,2,3,...]}, ...}
    pub fn get_state_vectors(&self) -> Result<String, String> {
        let mut result = serde_json::Map::new();

        for (doc_name, doc) in &self.docs {
            // Get state frontiers for this doc
            let state_vector = state_frontiers(doc);

            // Convert to JSON array
            let vector_array: Vec<Value> = state_vector
                .iter()
                .map(|&b| Value::Number(serde_json::Number::from(b)))
                .collect();

            let mut doc_data = serde_json::Map::new();
            doc_data.insert("state_vector".to_string(), Value::Array(vector_array));

            result.insert(doc_name.clone(), Value::Object(doc_data));
        }

        serde_json::to_string(&result)
            .map_err(|e| format!("Failed to serialize state vectors: {}", e))
    }

    /// Get state vectors filtered by UCAN token capabilities
    ///
    /// Returns state vectors only for documents that the UCAN token grants access to.
    /// Respects dont_send_to_node rules for viewer tokens.
    ///
    /// # Arguments
    /// * `ucan_token` - UCAN token from initiator (contains capabilities)
    /// * `domain` - Domain for UCAN parsing (e.g., "sthalam")
    ///
    /// # Returns
    /// JSON string with state vectors for accessible docs only:
    /// ```json
    /// {
    ///   "doc_name": {
    ///     "state_vector": [1, 2, 3, ...]
    ///   }
    /// }
    /// ```
    pub fn get_state_vectors_for_ucan(&self, ucan_token: &str) -> Result<String, String> {
        // 1. Extract document capabilities from UCAN (domain is extracted from UCAN itself)
        let doc_capabilities = extract_doc_capabilities(ucan_token)
            .map_err(|e| format!("Failed to extract doc capabilities: {}", e))?;

        // 2. Extract facts to check for dont_send_to_node
        let facts = extract_facts(ucan_token)
            .map_err(|e| format!("Failed to extract facts: {}", e))?;

        // 3. Get dont_send_to_node list (if exists)
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

        // 4. Build result with filtered docs
        let mut result = serde_json::Map::new();

        for (doc_name, doc) in &self.docs {
            // Skip if not in capabilities
            if !doc_capabilities.contains_key(doc_name) {
                continue;
            }

            // Skip if in dont_send_to_node list
            if dont_send_to_node.contains(doc_name) {
                continue;
            }

            // Get state frontiers for this doc
            let state_vector = state_frontiers(doc);

            // Convert to JSON array
            let vector_array: Vec<Value> = state_vector
                .iter()
                .map(|&b| Value::Number(serde_json::Number::from(b)))
                .collect();

            let mut doc_data = serde_json::Map::new();
            doc_data.insert("state_vector".to_string(), Value::Array(vector_array));

            result.insert(doc_name.clone(), Value::Object(doc_data));
        }

        serde_json::to_string(&result)
            .map_err(|e| format!("Failed to serialize state vectors: {}", e))
    }

    /// Generate updates for a peer based on their state vectors
    ///
    /// Compares peer's state vectors with our current state and generates
    /// incremental updates for documents they're behind on. Respects peer's
    /// UCAN capabilities - only generates updates for docs they have access to.
    ///
    /// # Arguments
    /// * `peer_ucan` - Peer's UCAN token (checks their capabilities and dont_send_to_node)
    /// * `peer_state_vectors_json` - JSON with peer's current state vectors
    ///
    /// # Input Format
    /// ```json
    /// {
    ///   "doc_name": {
    ///     "state_vector": [1, 2, 3, ...]
    ///   }
    /// }
    /// ```
    ///
    /// # Returns
    /// JSON string with updates for each doc that peer is behind on:
    /// ```json
    /// {
    ///   "doc_name": {
    ///     "updates": [1, 2, 3, ...],
    ///     "state_vector": [4, 5, 6, ...]
    ///   }
    /// }
    /// ```
    pub fn generate_updates(
        &self,
        peer_ucan: &str,
        peer_state_vectors_json: &str,
    ) -> Result<String, String> {
        // Parse input
        let peer_vectors: HashMap<String, serde_json::Map<String, Value>> =
            serde_json::from_str(peer_state_vectors_json)
                .map_err(|e| format!("Failed to parse peer state vectors: {}", e))?;

        // Parse peer's UCAN to get capabilities
        let peer_doc_caps = extract_doc_capabilities(peer_ucan)
            .map_err(|e| e.to_string())?;

        // Check peer's dont_send_to_node rules (e.g., viewer doesn't want user_content_doc)
        let peer_facts = extract_facts(peer_ucan)
            .map_err(|e| e.to_string())?
            .unwrap_or_default();
        let dont_send_to_peer = peer_facts
            .get("dont_send_to_node")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let mut result = serde_json::Map::new();

        // For each doc in peer's state vectors
        for (doc_name, doc_data) in peer_vectors {
            // Skip if peer doesn't have capability for this doc
            if !peer_doc_caps.contains_key(&doc_name) {
                continue;
            }

            // Skip if in peer's dont_send_to_node list
            if dont_send_to_peer.contains(&doc_name) {
                continue;
            }

            // Skip if we don't have this doc
            if let Some(doc) = self.docs.get(&doc_name) {
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
                    .map_err(|e| format!("Failed to generate updates for {}: {}", doc_name, e))?;

                // Get our current state vector
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
            .map_err(|e| format!("Failed to serialize updates: {}", e))
    }

    /// Apply updates from a peer
    ///
    /// Validates that peer has permission to update each document based on their UCAN.
    ///
    /// # Arguments
    /// * `peer_ucan` - Peer's UCAN token (validates their capabilities)
    /// * `updates_json` - JSON with updates from peer
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
    pub fn apply_updates(&mut self, peer_ucan: &str, updates_json: &str) -> Result<(), String> {
        // Parse input
        let input: HashMap<String, serde_json::Map<String, Value>> =
            serde_json::from_str(updates_json)
                .map_err(|e| format!("Failed to parse updates JSON: {}", e))?;

        // Parse peer's UCAN
        let peer_doc_caps = extract_doc_capabilities(peer_ucan)
            .map_err(|e| e.to_string())?;

        // Apply updates for each doc
        for (doc_name, doc_data) in input {
            // Check peer's capability for this doc
            let ability = peer_doc_caps
                .get(&doc_name)
                .ok_or_else(|| format!("Peer does not have capability for doc: {}", doc_name))?;

            // Reject if peer only has readonly access
            if ability == "crud/readonly" {
                return Err(format!(
                    "Peer has readonly access to {}, cannot accept updates",
                    doc_name
                ));
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

            // Get or create the doc
            let doc = self.docs.entry(doc_name.clone()).or_insert_with(create_doc);

            // Apply the updates
            apply_updates(doc, &update_bytes)
                .map_err(|e| format!("Failed to apply updates to {}: {}", doc_name, e))?;
        }

        Ok(())
    }

    /// Apply updates from a peer with additional filtering based on our UCAN
    ///
    /// Used by viewers to respect no_update_from_node rules
    ///
    /// # Arguments
    /// * `our_ucan` - Our UCAN token (checks no_update_from_node in facts)
    /// * `peer_ucan` - Peer's UCAN token (validates their capabilities)
    /// * `updates_json` - JSON with updates from peer
    pub fn apply_updates_filtered(
        &mut self,
        our_ucan: &str,
        peer_ucan: &str,
        updates_json: &str,
    ) -> Result<(), String> {
        // Parse our UCAN to check what we shouldn't accept
        let our_facts = extract_facts(our_ucan)
            .map_err(|e| e.to_string())?
            .unwrap_or_default();
        let no_update = our_facts
            .get("no_update_from_node")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        // Parse input
        let input: HashMap<String, serde_json::Map<String, Value>> =
            serde_json::from_str(updates_json)
                .map_err(|e| format!("Failed to parse updates JSON: {}", e))?;

        // Filter out docs we shouldn't accept updates for
        let mut filtered_input = serde_json::Map::new();
        for (doc_name, doc_data) in input {
            if !no_update.contains(&doc_name) {
                filtered_input.insert(doc_name, Value::Object(doc_data));
            }
        }

        // Convert back to JSON and apply
        let filtered_json = serde_json::to_string(&filtered_input)
            .map_err(|e| format!("Failed to serialize filtered updates: {}", e))?;

        self.apply_updates(peer_ucan, &filtered_json)
    }

    /// Export all documents as JSON for encryption and storage
    ///
    /// # Returns
    /// JSON string with all document snapshots as byte arrays
    /// Format: {"doc_name": [1,2,3,...], ...}
    pub fn to_json(&self) -> Result<String, String> {
        let mut result = serde_json::Map::new();

        for (doc_name, doc) in &self.docs {
            // Export full snapshot (not shallow) for owner/node storage
            let snapshot = export_shallow_snapshot(doc);

            // Convert to JSON array
            let snapshot_array: Vec<Value> = snapshot
                .iter()
                .map(|&b| Value::Number(serde_json::Number::from(b)))
                .collect();

            result.insert(doc_name.clone(), Value::Array(snapshot_array));
        }

        serde_json::to_string(&result)
            .map_err(|e| format!("Failed to serialize to JSON: {}", e))
    }

    /// Extract state vectors from updates JSON
    ///
    /// Converts updates format to state vectors format by removing updates and keeping only state_vector.
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
    pub fn extract_state_vectors_from_updates(updates_json: &str) -> Result<String, String> {
        let input: HashMap<String, serde_json::Map<String, Value>> =
            serde_json::from_str(updates_json)
                .map_err(|e| format!("Failed to parse updates JSON: {}", e))?;

        let mut result = serde_json::Map::new();

        for (doc_name, doc_data) in input {
            if let Some(state_vector) = doc_data.get("state_vector") {
                let mut doc_result = serde_json::Map::new();
                doc_result.insert("state_vector".to_string(), state_vector.clone());
                result.insert(doc_name, Value::Object(doc_result));
            }
        }

        serde_json::to_string(&result)
            .map_err(|e| format!("Failed to serialize state vectors: {}", e))
    }
}


