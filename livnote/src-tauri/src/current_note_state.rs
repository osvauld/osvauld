use log::{error, info};
use osvauld_core::models::document::YjsDocExt;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::Mutex;
use yrs::Doc;

#[derive(Debug, Clone)]
struct Buffers {
    note_id: Option<String>,
    main_doc: Doc,
    image_doc: Doc,
    comment_doc: Doc,
    shared_users: Vec<String>,
    active_connections: HashSet<String>,
    inactive_connections: HashSet<String>,
}

impl Default for Buffers {
    fn default() -> Self {
        Self {
            note_id: None,
            main_doc: Doc::new(),
            image_doc: Doc::new(),
            comment_doc: Doc::new(),
            shared_users: Vec::new(),
            active_connections: HashSet::new(),
            inactive_connections: HashSet::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CurrentNoteState(Arc<Mutex<Buffers>>);

impl Default for CurrentNoteState {
    fn default() -> Self {
        Self(Arc::new(Mutex::new(Buffers::default())))
    }
}

impl CurrentNoteState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the current note and load its document states
    /// This will create new docs and populate them with the note's state
    pub async fn set_current_note(
        &self,
        note_id: Option<String>,
        main_doc_state: Option<Vec<u8>>,
        image_state: Option<Vec<u8>>,
        comment_state: Option<Vec<u8>>,
    ) {
        // Create new documents
        let mut new_main_doc = Doc::new();
        let mut new_image_doc = Doc::new();
        let mut new_comment_doc = Doc::new();
        // Load main document state if provided
        if let Some(main_state) = main_doc_state {
            if !main_state.is_empty() {
                if let Err(e) = new_main_doc.apply_update_v2(&main_state).await {
                    error!("Failed to load main document state: {}", e);
                }
            }
        }

        // Load image state if provided
        if let Some(img_state) = image_state {
            if !img_state.is_empty() {
                if let Err(e) = new_image_doc.apply_update_v2(&img_state).await {
                    error!("Failed to load image state: {}", e);
                }
            }
        }
        if let Some(comment_state) = comment_state {
            if !comment_state.is_empty() {
                if let Err(e) = new_comment_doc.apply_update_v2(&comment_state).await {
                    error!("Failed to load comment state: {}", e);
                }
            }
        }
        // Update buffers with new documents
        let mut buffers = self.0.lock().await;
        buffers.note_id = note_id.clone();
        buffers.main_doc = new_main_doc;
        buffers.image_doc = new_image_doc;
        buffers.comment_doc = new_comment_doc;
        info!("Current note set to: {:?}", note_id);
    }

    pub async fn reset_to_default(&self) {
        let mut buffers = self.0.lock().await;
        *buffers = Buffers::default();
        info!("Reset note state to default - cleared all data");
    }

    pub async fn get_current_note(&self) -> Option<String> {
        let buffers = self.0.lock().await;
        buffers.note_id.clone()
    }

    /// Apply updates to the appropriate document - optimized for live updates
    /// Uses RwLock pattern for minimal lock contention during live collaboration
    pub async fn apply_update(&self, new_updates: Vec<u8>, doc_type: &str) {
        if new_updates.is_empty() {
            return;
        }

        // Determine which doc to update based on doc_type
        let doc_selection = match doc_type {
            "images" | "image_state" => 1,
            "comments" | "comment_state" => 2, // ADD THIS
            "main" | "main_doc" | _ => 0,
        };

        let mut temp_doc = {
            let buffers = self.0.lock().await;
            match doc_selection {
                1 => buffers.image_doc.clone(),
                2 => buffers.comment_doc.clone(), // ADD THIS
                _ => buffers.main_doc.clone(),
            }
        };

        if let Err(e) = temp_doc.apply_update_v2(&new_updates).await {
            error!("Failed to apply updates to {} document: {}", doc_type, e);
            return;
        }

        {
            let mut buffers = self.0.lock().await;
            match doc_selection {
                1 => buffers.image_doc = temp_doc,
                2 => buffers.comment_doc = temp_doc, // ADD THIS
                _ => buffers.main_doc = temp_doc,
            }
        }
    }
    async fn get_doc_state_vector(
        doc: &Doc,
        doc_name: &str,
        result: &mut serde_json::Map<String, serde_json::Value>,
    ) -> Result<(), String> {
        let state_vector = doc.get_state_vector_v2().await;
        if !state_vector.is_empty() {
            result.insert(
                doc_name.to_string(),
                serde_json::Value::Object(Self::create_doc_result(vec![], state_vector)),
            );
        }
        Ok(())
    }

    // Simplified get_state_vectors
    pub async fn get_state_vectors(&self) -> Result<String, String> {
        let (main_doc, image_doc, comment_doc) = {
            let buffers = self.0.lock().await;
            (
                buffers.main_doc.clone(),
                buffers.image_doc.clone(),
                buffers.comment_doc.clone(),
            ) // ADD comment_doc
        };

        let mut result = serde_json::Map::new();

        Self::get_doc_state_vector(&main_doc, "main_doc", &mut result).await?;
        Self::get_doc_state_vector(&image_doc, "image_state", &mut result).await?;
        Self::get_doc_state_vector(&comment_doc, "comment_state", &mut result).await?; // ADD THIS

        serde_json::to_string(&result)
            .map_err(|e| format!("Failed to serialize state vectors: {}", e))
    }
    // Helper function to parse state vector from JSON value
    fn parse_byte_array(value: Option<&serde_json::Value>) -> Vec<u8> {
        value
            .and_then(|v| v.as_array())
            .map(|array| {
                array
                    .iter()
                    .filter_map(|v| v.as_u64().map(|n| n as u8))
                    .collect()
            })
            .unwrap_or_default()
    }

    // Helper function to convert bytes to JSON array
    fn bytes_to_json_array(bytes: &[u8]) -> Vec<serde_json::Value> {
        bytes
            .iter()
            .map(|&b| serde_json::Value::Number(serde_json::Number::from(b)))
            .collect()
    }

    // Helper function to create document result with updates and state vector
    fn create_doc_result(
        updates: Vec<u8>,
        state_vector: Vec<u8>,
    ) -> serde_json::Map<String, serde_json::Value> {
        let mut doc_result = serde_json::Map::new();
        doc_result.insert(
            "updates".to_string(),
            serde_json::Value::Array(Self::bytes_to_json_array(&updates)),
        );
        doc_result.insert(
            "state_vector".to_string(),
            serde_json::Value::Array(Self::bytes_to_json_array(&state_vector)),
        );
        doc_result
    }

    // Helper function to process a single document's state vectors
    async fn process_doc_state_vectors(
        doc: &Doc,
        doc_data: Option<&serde_json::Value>,
        doc_name: &str,
        result: &mut serde_json::Map<String, serde_json::Value>,
    ) -> Result<(), String> {
        if let Some(data) = doc_data {
            let peer_state_vector = Self::parse_byte_array(data.get("state_vector"));

            if !peer_state_vector.is_empty() {
                let updates_for_peer = doc
                    .get_diff_update_v2(&peer_state_vector)
                    .await
                    .map_err(|e| format!("Failed to generate {} diff: {}", doc_name, e))?;

                let our_state_vector = doc.get_state_vector_v2().await;

                result.insert(
                    doc_name.to_string(),
                    serde_json::Value::Object(Self::create_doc_result(
                        updates_for_peer,
                        our_state_vector,
                    )),
                );
            }
        }
        Ok(())
    }

    // Refactored generate_updates_for_peer
    pub async fn generate_updates_for_peer(
        &self,
        peer_state_vectors_json: &str,
    ) -> Result<String, String> {
        let peer_data: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(peer_state_vectors_json)
                .map_err(|e| format!("Failed to parse peer state vectors: {}", e))?;

        let (main_doc, image_doc) = {
            let buffers = self.0.lock().await;
            (buffers.main_doc.clone(), buffers.image_doc.clone())
        };

        let mut result = serde_json::Map::new();

        // Process both documents using the helper
        Self::process_doc_state_vectors(
            &main_doc,
            peer_data.get("main_doc"),
            "main_doc",
            &mut result,
        )
        .await?;

        Self::process_doc_state_vectors(
            &image_doc,
            peer_data.get("image_state"),
            "image_state",
            &mut result,
        )
        .await?;

        serde_json::to_string(&result).map_err(|e| format!("Failed to serialize updates: {}", e))
    }

    // Helper for processing updates and generating diffs
    async fn process_doc_updates_and_diff(
        doc: Doc,
        doc_data: Option<&serde_json::Value>,
        doc_name: &str,
        result: &mut serde_json::Map<String, serde_json::Value>,
    ) -> Result<Option<Doc>, String> {
        if let Some(data) = doc_data {
            let peer_updates = Self::parse_byte_array(data.get("updates"));
            let peer_state_vector = Self::parse_byte_array(data.get("state_vector"));

            let mut doc = doc;

            // Apply peer updates if any
            if !peer_updates.is_empty() {
                doc.apply_update_v2(&peer_updates)
                    .await
                    .map_err(|e| format!("Failed to apply {} updates: {}", doc_name, e))?;
            }

            // Generate diff based on peer's state vector
            if !peer_state_vector.is_empty() {
                let updates_for_peer = doc
                    .get_diff_update_v2(&peer_state_vector)
                    .await
                    .map_err(|e| format!("Failed to generate {} diff: {}", doc_name, e))?;

                let our_state_vector = doc.get_state_vector_v2().await;

                result.insert(
                    doc_name.to_string(),
                    serde_json::Value::Object(Self::create_doc_result(
                        updates_for_peer,
                        our_state_vector,
                    )),
                );

                return Ok(Some(doc));
            }
        }
        Ok(None)
    }

    // Refactored apply_updates_and_generate_diff
    pub async fn apply_updates_and_generate_diff(
        &self,
        peer_updates_json: &str,
    ) -> Result<String, String> {
        let peer_data: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(peer_updates_json)
                .map_err(|e| format!("Failed to parse peer updates: {}", e))?;

        let mut result = serde_json::Map::new();

        // Process main_doc
        let main_doc = {
            let buffers = self.0.lock().await;
            buffers.main_doc.clone()
        };

        if let Some(updated_doc) = Self::process_doc_updates_and_diff(
            main_doc,
            peer_data.get("main_doc"),
            "main_doc",
            &mut result,
        )
        .await?
        {
            let mut buffers = self.0.lock().await;
            buffers.main_doc = updated_doc;
        }

        // Process image_doc
        let image_doc = {
            let buffers = self.0.lock().await;
            buffers.image_doc.clone()
        };

        if let Some(updated_doc) = Self::process_doc_updates_and_diff(
            image_doc,
            peer_data.get("image_state"),
            "image_state",
            &mut result,
        )
        .await?
        {
            let mut buffers = self.0.lock().await;
            buffers.image_doc = updated_doc;
        }

        serde_json::to_string(&result)
            .map_err(|e| format!("Failed to serialize diff updates: {}", e))
    }
    // Apply updates from peer without generating any response
    pub async fn apply_peer_updates(&self, peer_updates_json: &str) -> Result<(), String> {
        let peer_data: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(peer_updates_json)
                .map_err(|e| format!("Failed to parse peer updates: {}", e))?;

        // Process main_doc updates
        if let Some(main_data) = peer_data.get("main_doc") {
            let peer_updates = Self::parse_byte_array(main_data.get("updates"));

            if !peer_updates.is_empty() {
                let mut main_doc = {
                    let buffers = self.0.lock().await;
                    buffers.main_doc.clone()
                };

                main_doc
                    .apply_update_v2(&peer_updates)
                    .await
                    .map_err(|e| format!("Failed to apply main_doc updates: {}", e))?;

                // Update the stored doc
                let mut buffers = self.0.lock().await;
                buffers.main_doc = main_doc;
            }
        }

        // Process image_state updates
        if let Some(image_data) = peer_data.get("image_state") {
            let peer_updates = Self::parse_byte_array(image_data.get("updates"));

            if !peer_updates.is_empty() {
                let mut image_doc = {
                    let buffers = self.0.lock().await;
                    buffers.image_doc.clone()
                };

                image_doc
                    .apply_update_v2(&peer_updates)
                    .await
                    .map_err(|e| format!("Failed to apply image_state updates: {}", e))?;

                // Update the stored doc
                let mut buffers = self.0.lock().await;
                buffers.image_doc = image_doc;
            }
        }

        Ok(())
    }
    // Connection management methods remain the same
    pub async fn set_shared_users(&self, users: Vec<String>) {
        let mut buffers = self.0.lock().await;
        buffers.shared_users = users.clone();
        info!("Set shared users: {:?}", users);
    }

    pub async fn get_shared_users(&self) -> Vec<String> {
        let buffers = self.0.lock().await;
        buffers.shared_users.clone()
    }

    pub async fn clear_shared_users(&self) {
        let mut buffers = self.0.lock().await;
        buffers.shared_users.clear();
        info!("Cleared shared users for note");
    }

    pub async fn add_active_connection(&self, connection_id: String) {
        let mut buffers = self.0.lock().await;
        buffers.active_connections.insert(connection_id.clone());
        info!("Added active connection: {}", connection_id);
    }

    pub async fn get_active_connections(&self) -> Vec<String> {
        let buffers = self.0.lock().await;
        buffers.active_connections.iter().cloned().collect()
    }

    pub async fn get_inactive_connections(&self) -> Vec<String> {
        let buffers = self.0.lock().await;
        buffers.inactive_connections.iter().cloned().collect()
    }

    pub async fn is_connection_active(&self, connection_id: &str) -> bool {
        let buffers = self.0.lock().await;
        buffers.active_connections.contains(connection_id)
    }

    pub async fn remove_active_connection(&self, connection_id: &str) {
        let mut buffers = self.0.lock().await;
        if buffers.active_connections.remove(connection_id) {
            info!("Removed active connection: {}", connection_id);
        }
    }

    pub async fn add_inactive_connection(&self, connection_id: &str) {
        let mut buffers = self.0.lock().await;
        buffers
            .inactive_connections
            .insert(connection_id.to_string());
        info!("Added inactive connection: {}", connection_id);
    }

    pub async fn clear_active_connections(&self) {
        let mut buffers = self.0.lock().await;
        buffers.active_connections.clear();
        info!("Cleared all active connections");
    }

    pub async fn clear_inactive_connections(&self) {
        let mut buffers = self.0.lock().await;
        buffers.inactive_connections.clear();
        info!("Cleared all inactive connections");
    }
}
