use crate::p2p::peer_connection::PeerConnection;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, info, instrument, trace, warn};
#[derive(Clone)]
pub struct ConnectionManager {
    pub connections: Arc<Mutex<HashMap<String, Arc<PeerConnection>>>>,
    pub connecting: Arc<Mutex<HashSet<String>>>,
    pub pending_live_edit_requests: Arc<Mutex<HashSet<String>>>,
}

impl ConnectionManager {
    #[instrument(level = "debug")]
    pub fn new() -> Self {
        info!("Creating new connection manager");
        Self {
            connections: Arc::new(Mutex::new(HashMap::new())),
            connecting: Arc::new(Mutex::new(HashSet::new())),
            pending_live_edit_requests: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    /// Get a connection by its ID (user_id:device_id)
    #[instrument(skip(self), level = "debug")]
    pub async fn get_peer_connection(
        &self,
        connection_id: &str,
    ) -> Result<Arc<PeerConnection>, String> {
        debug!("Looking up peer connection by ID");
        let connections = self.connections.lock().await;

        match connections.get(connection_id) {
            Some(connection) => {
                debug!("Connection found: {}", connection_id);
                Ok(connection.clone())
            }
            None => {
                warn!("Connection not found: {}", connection_id);
                Err(format!("Connection not found: {}", connection_id))
            }
        }
    }

    /// Check if a connection is active (either established or in the connecting state)
    #[instrument(skip(self), level = "debug")]
    pub async fn is_connection_active(&self, connection_id: &str) -> bool {
        // First check if it's an established connection
        let connections = self.connections.lock().await;
        if connections.contains_key(connection_id) {
            debug!("Connection is already established: {}", connection_id);
            return true;
        }

        // Then check if it's in the connecting state
        let connecting = self.connecting.lock().await;
        let is_connecting = connecting.contains(connection_id);
        if is_connecting {
            debug!(
                "Connection is currently being established: {}",
                connection_id
            );
        } else {
            debug!("No active connection found for: {}", connection_id);
        }

        is_connecting
    }

    /// Mark a connection as being in the connecting state
    #[instrument(skip(self), level = "debug")]
    pub async fn mark_as_connecting(&self, connection_id: &str) -> Result<(), String> {
        // First check if it's already an established connection
        let connections = self.connections.lock().await;
        if connections.contains_key(connection_id) {
            let msg = format!("Connection already established: {}", connection_id);
            debug!("{}", msg);
            return Err(msg);
        }
        drop(connections); // Release the lock on connections

        // Then check and update the connecting state
        let mut connecting = self.connecting.lock().await;
        if connecting.contains(connection_id) {
            let msg = format!("Connection already in connecting state: {}", connection_id);
            debug!("{}", msg);
            return Err(msg);
        }

        // Mark as connecting
        connecting.insert(connection_id.to_string());
        debug!("Connection marked as connecting: {}", connection_id);
        Ok(())
    }

    /// Remove from connecting state (used in error scenarios or when done connecting)
    #[instrument(skip(self), level = "debug")]
    pub async fn remove_from_connecting(&self, connection_id: &str) {
        let mut connecting = self.connecting.lock().await;
        if connecting.remove(connection_id) {
            debug!("Removed from connecting state: {}", connection_id);
        } else {
            trace!("Connection wasn't in connecting state: {}", connection_id);
        }
    }

    /// Insert a connection
    #[instrument(skip(self, connection), level = "info")]
    pub async fn insert_connection(&self, connection: Arc<PeerConnection>) -> Result<(), String> {
        // Create connection ID from the connection's user and device info
        let connection_id = connection.get_id();
        debug!("Inserting new connection with ID: {}", connection_id);

        // First remove from connecting state if it was there
        self.remove_from_connecting(&connection_id).await;

        // Insert the connection into the map
        let mut connections = self.connections.lock().await;
        // Check if the connection already exists
        if connections.contains_key(&connection_id) {
            debug!("Connection already exists, replacing: {}", connection_id);
        }

        connections.insert(connection_id.clone(), connection);
        info!("Connection inserted with ID: {}", connection_id);

        Ok(())
    }

    /// Remove a connection
    #[instrument(skip(self), , level = "info")]
    pub async fn remove_connection(&self, connection_id: &str) -> Result<(), String> {
        debug!("Attempting to remove connection: {}", connection_id);
        let mut connections = self.connections.lock().await;

        if connections.remove(connection_id).is_some() {
            info!("Connection removed: {}", connection_id);
            Ok(())
        } else {
            warn!("Connection not found for removal: {}", connection_id);
            Err(format!("Connection not found: {}", connection_id))
        }
    }

    /// Get connections by their IDs
    #[instrument(skip(self, connection_ids), level = "debug")]
    pub async fn get_connections_by_ids(
        &self,
        connection_ids: &[String],
    ) -> Vec<Arc<PeerConnection>> {
        debug!(
            "Getting connections by IDs, count: {}",
            connection_ids.len()
        );

        let connections_lock = self.connections.lock().await;
        let mut result = Vec::with_capacity(connection_ids.len());

        for connection_id in connection_ids {
            if let Some(connection) = connections_lock.get(connection_id) {
                debug!("Found connection: {}", connection_id);
                result.push(connection.clone());
            } else {
                warn!("Connection not found: {}", connection_id);
            }
        }

        info!(
            "Retrieved {} out of {} requested connections",
            result.len(),
            connection_ids.len()
        );

        result
    }

    #[instrument(skip(self), level = "debug")]
    pub async fn add_pending_live_edit_request(&self, connection_id: &str) {
        debug!(
            "Adding pending live edit request for connection: {}",
            connection_id
        );
        let mut pending_requests = self.pending_live_edit_requests.lock().await;
        pending_requests.insert(connection_id.to_string());
        info!("Added pending live edit request for: {}", connection_id);
    }
    #[instrument(skip(self), level = "debug")]
    pub async fn get_and_clear_pending_live_edit_requests(&self, connection_id: &str) -> bool {
        debug!(
            "Checking for pending live edit request for: {}",
            connection_id
        );
        let mut pending_requests = self.pending_live_edit_requests.lock().await;
        let was_pending = pending_requests.remove(connection_id);
        if was_pending {
            info!(
                "Found and removed pending live edit request for: {}",
                connection_id
            );
        }
        was_pending
    }
}
