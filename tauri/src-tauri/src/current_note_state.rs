use log::info;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct CurrentNoteState {
    note_id: Arc<Mutex<Option<String>>>,
    current_yjs_state: Arc<Mutex<Option<Vec<u8>>>>,
    previous_yjs_state: Arc<Mutex<Option<Vec<u8>>>>,
    shared_users: Arc<Mutex<Vec<String>>>,
    active_connections: Arc<Mutex<HashSet<String>>>,
}

impl Default for CurrentNoteState {
    fn default() -> Self {
        Self {
            note_id: Arc::new(Mutex::new(None)),
            current_yjs_state: Arc::new(Mutex::new(None)),
            previous_yjs_state: Arc::new(Mutex::new(None)),
            shared_users: Arc::new(Mutex::new(Vec::new())),
            active_connections: Arc::new(Mutex::new(HashSet::new())),
        }
    }
}

impl CurrentNoteState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_current_note(&self, note_id: Option<String>) {
        let mut current = self.note_id.lock().unwrap();
        *current = note_id;
        self.clear_buffers();
        info!("Current note set to: {:?}", current);
    }

    pub fn get_current_note(&self) -> Option<String> {
        let current = self.note_id.lock().unwrap();
        current.clone()
    }

    pub fn move_current_to_previous(&self) {
        let current = self.current_yjs_state.lock().unwrap().clone();
        let mut previous = self.previous_yjs_state.lock().unwrap();
        *previous = current;

        // Clear the current buffer
        let mut current = self.current_yjs_state.lock().unwrap();
        *current = None;

        info!("Moved current Yjs state to previous buffer");
    }

    pub fn update_yjs_state(&self, new_state: Vec<u8>) {
        // Append to the current state instead of overwriting
        let mut current_lock = self.current_yjs_state.lock().unwrap();

        // If there's an existing buffer, append to it, otherwise create new
        if let Some(ref mut current_data) = *current_lock {
            // Append the new state to the existing buffer
            current_data.extend_from_slice(&new_state);
            info!(
                "Appended to current Yjs state buffer, new size: {} bytes",
                current_data.len()
            );
        } else {
            // No existing buffer, create new one
            *current_lock = Some(new_state);
            info!("Created new current Yjs state buffer");
        }
    }

    pub fn get_current_yjs_state(&self) -> Option<Vec<u8>> {
        let state = self.current_yjs_state.lock().unwrap();
        state.clone()
    }

    pub fn get_previous_yjs_state(&self) -> Option<Vec<u8>> {
        let state = self.previous_yjs_state.lock().unwrap();
        state.clone()
    }

    pub fn clear_buffers(&self) {
        {
            let mut current = self.current_yjs_state.lock().unwrap();
            *current = None;
        }
        {
            let mut previous = self.previous_yjs_state.lock().unwrap();
            *previous = None;
        }
        info!("Yjs state buffers cleared");
    }
    pub fn set_shared_users(&self, users: Vec<String>) {
        let mut shared_users = self.shared_users.lock().unwrap();
        *shared_users = users.clone();
        info!("Set shared users: {:?}", users);
    }
    pub fn get_shared_users(&self) -> Vec<String> {
        let shared_users = self.shared_users.lock().unwrap();
        shared_users.clone()
    }
    pub fn clear_shared_users(&self) {
        let mut shared_users = self.shared_users.lock().unwrap();
        shared_users.clear();
        info!("Cleared shared users for note");
    }

    pub fn add_active_connection(&self, connection_id: String) {
        let mut connections = self.active_connections.lock().unwrap();
        info!("Added active connection: {}", connection_id);
        connections.insert(connection_id);
    }

    pub fn get_active_connections(&self) -> Vec<String> {
        let connections = self.active_connections.lock().unwrap();
        connections.iter().cloned().collect()
    }
    pub fn is_connection_active(&self, connection_id: &str) -> bool {
        let connections = self.active_connections.lock().unwrap();
        connections.contains(connection_id)
    }

    pub fn remove_active_connection(&self, connection_id: &str) {
        let mut connections = self.active_connections.lock().unwrap();
        if connections.remove(connection_id) {
            info!("Removed active connection: {}", connection_id);
        }
    }

    pub fn clear_active_connections(&self) {
        let mut connections = self.active_connections.lock().unwrap();
        connections.clear();
        info!("Cleared all active connections");
    }
}
