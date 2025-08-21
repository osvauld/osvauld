use log::{error, info};
use osvauld_core::models::document::{YjsDocExt, create_doc};
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use yrs::Doc;

#[derive(Debug, Clone)]
struct Buffers {
    note_id: Option<String>,
    main_current_doc: Doc,
    main_previous_doc: Doc,
    images_current_doc: Doc,
    images_previous_doc: Doc,
    shared_users: Vec<String>,
    active_connections: HashSet<String>,
    inactive_connections: HashSet<String>,
}
impl Default for Buffers {
    fn default() -> Self {
        Self {
            note_id: None,
            main_current_doc: Doc::new(),
            main_previous_doc: Doc::new(),
            images_current_doc: Doc::new(),
            images_previous_doc: Doc::new(),
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
    pub fn set_current_note(&self, note_id: Option<String>) {
        let mut buffers = self.0.lock().unwrap();
        buffers.note_id = note_id.clone();

        // Move current docs to previous and create new current docs
        buffers.main_previous_doc = std::mem::replace(&mut buffers.main_current_doc, Doc::new());
        buffers.images_previous_doc =
            std::mem::replace(&mut buffers.images_current_doc, Doc::new());

        info!("Current note set to: {:?}", note_id);
    }
    pub fn reset_to_default(&self) {
        let mut buffers = self.0.lock().unwrap();
        *buffers = Buffers::default();
        info!("Reset note state to default - cleared all data");
    }

    pub fn get_current_note(&self) -> Option<String> {
        let buffers = self.0.lock().unwrap();
        buffers.note_id.clone()
    }

    pub fn move_current_to_previous(&self) {
        let mut buffers = self.0.lock().unwrap();

        // Move both current docs to their respective previous docs
        buffers.main_previous_doc = std::mem::replace(&mut buffers.main_current_doc, Doc::new());
        buffers.images_previous_doc =
            std::mem::replace(&mut buffers.images_current_doc, Doc::new());

        info!("Moved current docs to previous buffers for both main and images");
    }

    pub async fn merge_to_current(&self, new_updates: Vec<u8>, doc_type: &str) {
        if new_updates.is_empty() {
            return;
        }

        // Clone what we need outside the mutex
        let self_clone = self.clone();

        // Create temporary doc to avoid holding the lock during async operations
        let mut temp_doc = {
            let buffers = self_clone.0.lock().unwrap();
            match doc_type {
                "main" => buffers.main_current_doc.clone(),
                "images" => buffers.images_current_doc.clone(),
                _ => {
                    error!("Unknown doc type: {}", doc_type);
                    return;
                }
            }
        };

        // Apply updates to the temporary doc
        if let Err(e) = temp_doc.apply_update_v2(&new_updates).await {
            error!("Failed to apply updates to {} document: {}", doc_type, e);
            return;
        }

        // Now update the actual doc with the modified temp doc
        let mut buffers = self_clone.0.lock().unwrap();
        match doc_type {
            "main" => buffers.main_current_doc = temp_doc,
            "images" => buffers.images_current_doc = temp_doc,
            _ => {
                error!("Unknown doc type: {}", doc_type);
                return;
            }
        }

        info!(
            "Successfully applied updates to current {} document",
            doc_type
        );
    }

    /// Combine the current and previous documents for both types and return as JSON string
    pub async fn get_combined_updates(&self) -> Result<String, String> {
        // Clone the docs outside the mutex
        let (main_current, mut main_previous, images_current, mut images_previous) = {
            let buffers = self.0.lock().unwrap();
            (
                buffers.main_current_doc.clone(),
                buffers.main_previous_doc.clone(),
                buffers.images_current_doc.clone(),
                buffers.images_previous_doc.clone(),
            )
        };

        // Merge docs for each type
        let main_updates = main_current.get_state_as_update_v2().await;
        main_previous.apply_update_v2(&main_updates).await;
        let main_updates = main_previous.get_state_as_update_v2().await;

        let image_current_updates = images_current.get_state_as_update_v2().await;
        images_previous
            .apply_update_v2(&image_current_updates)
            .await?;
        let images_updates = images_previous.get_state_as_update_v2().await;

        // Convert to JSON format
        let mut result = serde_json::Map::new();

        // Add yjs_state (main) updates
        if !main_updates.is_empty() {
            let main_updates_array: Vec<serde_json::Value> = main_updates
                .iter()
                .map(|&b| serde_json::Value::Number(serde_json::Number::from(b)))
                .collect();

            let mut main_result = serde_json::Map::new();
            main_result.insert(
                "updates".to_string(),
                serde_json::Value::Array(main_updates_array),
            );
            result.insert(
                "main_doc".to_string(),
                serde_json::Value::Object(main_result),
            );
        }

        // Add image_state updates
        if !images_updates.is_empty() {
            let images_updates_array: Vec<serde_json::Value> = images_updates
                .iter()
                .map(|&b| serde_json::Value::Number(serde_json::Number::from(b)))
                .collect();

            let mut images_result = serde_json::Map::new();
            images_result.insert(
                "updates".to_string(),
                serde_json::Value::Array(images_updates_array),
            );
            result.insert(
                "image_state".to_string(),
                serde_json::Value::Object(images_result),
            );
        }

        let json_string = serde_json::to_string(&result).unwrap_or_else(|e| {
            error!("Failed to serialize combined updates: {}", e);
            "{}".to_string()
        });

        info!(
            "Combined updates: main={} bytes, images={} bytes",
            main_updates.len(),
            images_updates.len()
        );

        Ok(json_string)
    }

    pub fn set_shared_users(&self, users: Vec<String>) {
        let mut buffers = self.0.lock().unwrap();
        buffers.shared_users = users.clone();
        info!("Set shared users: {:?}", users);
    }

    pub fn get_shared_users(&self) -> Vec<String> {
        let buffers = self.0.lock().unwrap();
        buffers.shared_users.clone()
    }

    pub fn clear_shared_users(&self) {
        let mut buffers = self.0.lock().unwrap();
        buffers.shared_users.clear();
        info!("Cleared shared users for note");
    }

    pub fn add_active_connection(&self, connection_id: String) {
        let mut buffers = self.0.lock().unwrap();
        buffers.active_connections.insert(connection_id.clone());
        info!("Added active connection: {}", connection_id);
    }

    pub fn get_active_connections(&self) -> Vec<String> {
        let buffers = self.0.lock().unwrap();
        buffers.active_connections.iter().cloned().collect()
    }

    pub fn get_inactive_connections(&self) -> Vec<String> {
        let buffers = self.0.lock().unwrap();
        buffers.inactive_connections.iter().cloned().collect()
    }

    pub fn is_connection_active(&self, connection_id: &str) -> bool {
        let buffers = self.0.lock().unwrap();
        buffers.active_connections.contains(connection_id)
    }

    pub fn remove_active_connection(&self, connection_id: &str) {
        let mut buffers = self.0.lock().unwrap();
        if buffers.active_connections.remove(connection_id) {
            info!("Removed active connection: {}", connection_id);
        }
    }

    pub fn add_inactive_connection(&self, connection_id: &str) {
        let mut buffers = self.0.lock().unwrap();
        buffers
            .inactive_connections
            .insert(connection_id.to_string());
    }

    pub fn clear_active_connections(&self) {
        let mut buffers = self.0.lock().unwrap();
        buffers.active_connections.clear();
        info!("Cleared all active connections");
    }
    pub fn clear_inactive_connections(&self) {
        let mut buffers = self.0.lock().unwrap();
        buffers.inactive_connections.clear();
        info!("Cleared all inactive connections");
    }
}
