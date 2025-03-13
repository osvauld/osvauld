use iroh::endpoint::Connection;
use log::info;
use osvauld_core::models::device::Device;
use osvauld_core::models::p2p::ConnectionType;
use osvauld_core::models::user::User;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

// Define a struct for per-connection state
pub struct PeerConnection {
    pub connection: Arc<Connection>,
    pub connection_type: ConnectionType,
    pub device: Device,
    pub user: User,
    pub is_initiator: bool,
    pub task_handle: tokio::task::JoinHandle<()>,
}

pub struct ConnectionManager {
    pub connections: Arc<Mutex<HashMap<String, Arc<PeerConnection>>>>,
}

impl ConnectionManager {
    pub fn new() -> Self {
        Self {
            connections: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    // Get a connection by its ID
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
    pub async fn get_connection(&self, connection_id: &str) -> Result<Arc<Connection>, String> {
        let connections = self.connections.lock().await;
        let peer_connection = connections
            .get(connection_id)
            .cloned()
            .ok_or_else(|| format!("Connection not found: {}", connection_id))?;
        Ok(peer_connection.connection.clone())
    }

    pub async fn insert_connection(&self, connection: Arc<PeerConnection>) -> Result<(), String> {
        // Create connection ID from the connection's user and device info
        let connection_id = format!("{}:{}", connection.user.id, connection.device.id);

        // Insert the connection into the map
        let mut connections = self.connections.lock().await;
        connections.insert(connection_id.clone(), connection);
        info!("Connection inserted with ID: {}", connection_id);

        Ok(())
    }
}
