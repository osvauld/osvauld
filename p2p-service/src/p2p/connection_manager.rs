use crate::p2p::errors::P2PError;
use crate::p2p::peer_connection::PeerConnection;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, error, info, info_span, instrument, trace, warn, Instrument};

pub struct ConnectionManager {
    pub connections: Arc<Mutex<HashMap<String, Arc<PeerConnection>>>>,
}

impl ConnectionManager {
    #[instrument(level = "debug")]
    pub fn new() -> Self {
        info!("Creating new connection manager");
        Self {
            connections: Arc::new(Mutex::new(HashMap::new())),
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
}
