use log::info;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Default)]
struct Buffers {
    note_id: Option<String>,
    current_yjs_state: Option<Vec<u8>>,
    previous_yjs_state: Option<Vec<u8>>,
    shared_users: Vec<String>,
    active_connections: HashSet<String>,
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
        buffers.current_yjs_state = None;
        buffers.previous_yjs_state = None;
        info!("Current note set to: {:?}", note_id);
    }

    pub fn get_current_note(&self) -> Option<String> {
        let buffers = self.0.lock().unwrap();
        buffers.note_id.clone()
    }

    pub fn move_current_to_previous(&self) {
        let mut buffers = self.0.lock().unwrap();

        // Atomic operation - move current to previous and clear current
        buffers.previous_yjs_state = buffers.current_yjs_state.take();

        info!("Moved current Yjs state to previous buffer");
    }

    pub fn update_yjs_state(&self, new_state: Vec<u8>) {
        let mut buffers = self.0.lock().unwrap();

        // If there's an existing buffer, append to it, otherwise create new
        if let Some(ref mut current_data) = buffers.current_yjs_state {
            // Append the new state to the existing buffer
            current_data.extend_from_slice(&new_state);
            info!(
                "Appended to current Yjs state buffer, new size: {} bytes",
                current_data.len()
            );
        } else {
            // No existing buffer, create new one
            buffers.current_yjs_state = Some(new_state);
            info!("Created new current Yjs state buffer");
        }
    }

    pub fn get_current_yjs_state(&self) -> Option<Vec<u8>> {
        let buffers = self.0.lock().unwrap();
        buffers.current_yjs_state.clone()
    }

    pub fn get_previous_yjs_state(&self) -> Option<Vec<u8>> {
        let buffers = self.0.lock().unwrap();
        buffers.previous_yjs_state.clone()
    }

    pub fn clear_buffers(&self) {
        let mut buffers = self.0.lock().unwrap();
        buffers.current_yjs_state = None;
        buffers.previous_yjs_state = None;
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
