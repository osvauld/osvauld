use crate::p2p::peer_connection::PeerConnection;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, error, info, instrument, trace, warn};
#[derive(Clone)]
pub struct ConnectionManager {
    pub connections: Arc<Mutex<HashMap<String, Arc<PeerConnection>>>>,
    pub connecting: Arc<Mutex<HashSet<String>>>,
}

impl ConnectionManager {
    #[instrument(level = "debug")]
    pub fn new() -> Self {
        info!("Creating new connection manager");
        Self {
            connections: Arc::new(Mutex::new(HashMap::new())),
            connecting: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    /// Get a connection by its ID (user_id:device_id)
    #[instrument(skip(self), fields(connection_id = %connection_id), level = "debug")]
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
    #[instrument(skip(self), fields(connection_id = %connection_id), level = "debug")]
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
    #[instrument(skip(self), fields(connection_id = %connection_id), level = "debug")]
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
    #[instrument(skip(self), fields(connection_id = %connection_id), level = "debug")]
    pub async fn remove_from_connecting(&self, connection_id: &str) {
        let mut connecting = self.connecting.lock().await;
        if connecting.remove(connection_id) {
            debug!("Removed from connecting state: {}", connection_id);
        } else {
            trace!("Connection wasn't in connecting state: {}", connection_id);
        }
    }

    /// Get a connection by user ID and device ID
    #[instrument(skip(self), fields(user_id = %user_id, device_id = %device_id), level = "debug")]
    pub async fn get_connection_by_user_device(
        &self,
        user_id: &str,
        device_id: &str,
    ) -> Result<Arc<PeerConnection>, String> {
        let connection_id = format!("{}:{}", user_id, device_id);
        debug!("Looking up connection by user/device: {}", connection_id);
        self.get_peer_connection(&connection_id).await
    }

    /// Get all connections for a specific user
    #[instrument(skip(self), fields(user_id = %user_id), level = "debug")]
    pub async fn get_connections_by_user(&self, user_id: &str) -> Vec<Arc<PeerConnection>> {
        debug!("Getting all connections for user: {}", user_id);
        let connections = self.connections.lock().await;

        let user_connections: Vec<Arc<PeerConnection>> = connections
            .iter()
            .filter(|(id, _)| id.starts_with(&format!("{}:", user_id)))
            .map(|(id, conn)| {
                trace!("Found connection: {}", id);
                conn.clone()
            })
            .collect();

        debug!(
            "Found {} connection(s) for user {}",
            user_connections.len(),
            user_id
        );
        user_connections
    }

    /// Insert a connection
    #[instrument(skip(self, connection), fields(connection_type = ?connection.connection_type), level = "info")]
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
    #[instrument(skip(self), fields(connection_id = %connection_id), level = "info")]
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

    /// Get all connections
    #[instrument(skip(self), level = "debug")]
    pub async fn get_all_connections(&self) -> Vec<Arc<PeerConnection>> {
        debug!("Getting all connections");
        let connections = self.connections.lock().await;
        let all_connections = connections.values().cloned().collect();
        debug!("Retrieved {} connections", connections.len());
        all_connections
    }

    /// Count connections
    #[instrument(skip(self), level = "trace")]
    pub async fn connection_count(&self) -> usize {
        trace!("Counting connections");
        let connections = self.connections.lock().await;
        let count = connections.len();
        trace!("Connection count: {}", count);
        count
    }

    /// Broadcast a message to all connections
    #[instrument(skip(self, message), fields(message_len = message.len()), level = "info")]
    pub async fn broadcast_chat_message(&self, message: String) -> Result<(), String> {
        info!("Broadcasting chat message to all connections");
        let connections = self.get_all_connections().await;
        debug!("Sending message to {} connections", connections.len());

        let mut errors = Vec::new();
        for conn in connections {
            let conn_id = conn.get_id();
            debug!("Sending to connection: {}", conn_id);

            if let Err(e) = conn.send_chat_message(message.clone()).await {
                let error_msg = format!("Failed to send to {}: {}", conn_id, e);
                error!("{}", error_msg);
                errors.push(error_msg);
            }
        }

        if errors.is_empty() {
            info!("Broadcast completed successfully");
            Ok(())
        } else {
            let err_msg = format!("Broadcast errors: {}", errors.join(", "));
            error!("{}", err_msg);
            Err(err_msg)
        }
    }
    #[instrument(skip(self), level = "debug")]
    pub async fn get_first_active_connection(&self) -> Option<Arc<PeerConnection>> {
        debug!("Attempting to get first active connection");
        let connections = self.connections.lock().await;

        if connections.is_empty() {
            debug!("No active connections found");
            return None;
        }

        // Get the first connection from the HashMap
        let first_connection = connections.values().next().cloned();

        if let Some(conn) = &first_connection {
            debug!("Found active connection: {}", conn.get_id());
        }

        first_connection
    }
}
