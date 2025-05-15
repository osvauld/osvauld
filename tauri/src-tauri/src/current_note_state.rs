use log::{error, info};
use osvauld_core::models::document::{apply_update_to_doc, merge_docs_as_update};
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use yrs::Doc;

#[derive(Debug, Clone)]
struct Buffers {
    note_id: Option<String>,
    current_doc: Doc,
    previous_doc: Doc,
    shared_users: Vec<String>,
    active_connections: HashSet<String>,
}
impl Default for Buffers {
    fn default() -> Self {
        Self {
            note_id: None,
            current_doc: Doc::new(),
            previous_doc: Doc::new(),
            shared_users: Vec::new(),
            active_connections: HashSet::new(),
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
        buffers.previous_doc = std::mem::replace(&mut buffers.current_doc, Doc::new());
        info!("Current note set to: {:?}", note_id);
    }

    pub fn get_current_note(&self) -> Option<String> {
        let buffers = self.0.lock().unwrap();
        buffers.note_id.clone()
    }

    pub fn move_current_to_previous(&self) {
        let mut buffers = self.0.lock().unwrap();
        buffers.previous_doc = std::mem::replace(&mut buffers.current_doc, Doc::new());

        info!("Moved current Yjs state to previous buffer");
    }

    pub async fn merge_to_current(&self, new_updates: Vec<u8>) {
        if new_updates.is_empty() {
            return;
        }
        // Clone what we need outside the mutex
        let self_clone = self.clone();
        // Create temporary doc to avoid holding the lock during async operations
        let mut temp_doc = {
            let buffers = self_clone.0.lock().unwrap();
            buffers.current_doc.clone()
        };
        // Apply updates to the temporary doc
        if let Err(e) = apply_update_to_doc(&mut temp_doc, &new_updates).await {
            error!("Failed to apply updates to Yjs document: {}", e);
            return;
        }
        // Now update the actual doc with the modified temp doc
        let mut buffers = self_clone.0.lock().unwrap();
        buffers.current_doc = temp_doc;
        info!("Successfully applied updates to current Yjs document");
    }

    /// Combine the current and previous documents and return their merged state as an update array
    pub async fn get_combined_updates(&self) -> Vec<u8> {
        // Clone the docs outside the mutex
        let (current_doc, previous_doc) = {
            let buffers = self.0.lock().unwrap();
            (buffers.current_doc.clone(), buffers.previous_doc.clone())
        };

        // Merge the cloned docs
        let updates = merge_docs_as_update(&current_doc, &previous_doc).await;

        info!(
            "Combined current and previous docs into {} bytes of updates",
            updates.len()
        );
        updates
    }

    pub fn clear_buffers(&self) {
        let mut buffers = self.0.lock().unwrap();
        buffers.current_doc = Doc::new();
        buffers.previous_doc = Doc::new();
        info!("Yjs state buffers cleared");
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

    pub fn clear_active_connections(&self) {
        let mut buffers = self.0.lock().unwrap();
        buffers.active_connections.clear();
        info!("Cleared all active connections");
    }
}
