use crate::p2p::peer_connection::PeerConnection;
use log::info;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct ConnectionManager {
    pub connections: Arc<Mutex<HashMap<String, Arc<PeerConnection>>>>,
}

impl ConnectionManager {
    pub fn new() -> Self {
        Self {
            connections: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    // Get a connection by its ID (user_id:device_id)
    pub async fn get_peer_connection(
        &self,
        connection_id: &str,
    ) -> Result<Arc<PeerConnection>, String> {
        let connections = self.connections.lock().await;
        connections
            .get(connection_id)
            .cloned()
            .ok_or_else(|| format!("Connection not found: {}", connection_id))
    }

    // Get a connection by user ID and device ID
    pub async fn get_connection_by_user_device(
        &self,
        user_id: &str,
        device_id: &str,
    ) -> Result<Arc<PeerConnection>, String> {
        let connection_id = format!("{}:{}", user_id, device_id);
        self.get_peer_connection(&connection_id).await
    }

    // Get all connections for a specific user
    pub async fn get_connections_by_user(&self, user_id: &str) -> Vec<Arc<PeerConnection>> {
        let connections = self.connections.lock().await;
        connections
            .iter()
            .filter(|(id, _)| id.starts_with(&format!("{}:", user_id)))
            .map(|(_, conn)| conn.clone())
            .collect()
    }

    // Insert a connection
    pub async fn insert_connection(&self, connection: Arc<PeerConnection>) -> Result<(), String> {
        // Create connection ID from the connection's user and device info
        let connection_id = connection.get_id();

        // Insert the connection into the map
        let mut connections = self.connections.lock().await;
        connections.insert(connection_id.clone(), connection);
        info!("Connection inserted with ID: {}", connection_id);

        Ok(())
    }

    // Remove a connection
    pub async fn remove_connection(&self, connection_id: &str) -> Result<(), String> {
        let mut connections = self.connections.lock().await;
        if connections.remove(connection_id).is_some() {
            info!("Connection removed: {}", connection_id);
            Ok(())
        } else {
            Err(format!("Connection not found: {}", connection_id))
        }
    }

    // Get all connections
    pub async fn get_all_connections(&self) -> Vec<Arc<PeerConnection>> {
        let connections = self.connections.lock().await;
        connections.values().cloned().collect()
    }

    // Count connections
    pub async fn connection_count(&self) -> usize {
        let connections = self.connections.lock().await;
        connections.len()
    }

    // Broadcast a message to all connections
    pub async fn broadcast_chat_message(&self, message: String) -> Result<(), String> {
        let connections = self.get_all_connections().await;

        let mut errors = Vec::new();
        for conn in connections {
            if let Err(e) = conn.send_chat_message(message.clone()).await {
                errors.push(format!("Failed to send to {}: {}", conn.get_id(), e));
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(format!("Broadcast errors: {}", errors.join(", ")))
        }
    }
}
