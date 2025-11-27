//! Transport Layer for Osvauld P2P Network
//!
//! This crate provides the pure P2P transport layer using iroh.
//! It handles connections, streams, and message framing.
//! Business logic lives in the Courier layer.
//!
//! # Architecture
//!
//! - **Transport**: Main API, owns endpoint and connection pool
//! - **ConnectionPool**: Manages active connections
//! - **ConnectionHandle**: Lightweight reference for sending messages
//! - **TransportEvent**: Events emitted to Courier via channel
//! - **Message**: Protocol messages for P2P communication

pub mod events;
pub mod pool;
pub mod protocol;

pub use events::TransportEvent;
pub use pool::{ConnectionHandle, ConnectionPool};
pub use protocol::{ConnectionString, ErrorCode, Message};

use anyhow::{anyhow, Result};
use iroh::endpoint::Connection;
use iroh::{Endpoint, NodeAddr, NodeId, RelayMode, SecretKey, Watcher};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, error, info, trace, warn};

/// ALPN protocol identifier for Osvauld P2P
pub const ALPN_PROTOCOL: &[u8] = b"osvauld/p2p/1";

/// Configuration for Transport initialization
#[derive(Clone)]
pub struct TransportConfig {
    /// Secret key for the iroh endpoint (32 bytes)
    pub secret_key: [u8; 32],
    /// Channel buffer size for events
    pub event_buffer_size: usize,
}

impl TransportConfig {
    pub fn new(secret_key: [u8; 32]) -> Self {
        Self {
            secret_key,
            event_buffer_size: 256,
        }
    }

    pub fn with_event_buffer_size(mut self, size: usize) -> Self {
        self.event_buffer_size = size;
        self
    }
}

/// Transport layer handle
///
/// Provides the API for P2P networking. Courier receives events via the
/// mpsc channel returned from `init()`.
pub struct Transport {
    endpoint: Arc<Endpoint>,
    pool: Arc<ConnectionPool>,
    event_tx: mpsc::Sender<TransportEvent>,
}

impl Transport {
    /// Initialize the transport layer
    ///
    /// Returns the Transport and a receiver for TransportEvents.
    /// The caller (Courier) should spawn a task to process events.
    pub async fn init(config: TransportConfig) -> Result<(Self, mpsc::Receiver<TransportEvent>)> {
        let secret_key = SecretKey::from(config.secret_key);

        info!("Initializing transport layer");

        let endpoint = Endpoint::builder()
            .secret_key(secret_key)
            .discovery_n0()
            .relay_mode(RelayMode::Default)
            .alpns(vec![ALPN_PROTOCOL.to_vec()])
            .bind()
            .await
            .map_err(|e| anyhow!("Failed to bind endpoint: {}", e))?;

        let node_id = endpoint.node_id();
        info!("Transport bound with node_id: {}", node_id);

        let (event_tx, event_rx) = mpsc::channel(config.event_buffer_size);

        let transport = Self {
            endpoint: Arc::new(endpoint),
            pool: Arc::new(ConnectionPool::new()),
            event_tx,
        };

        Ok((transport, event_rx))
    }

    /// Get this node's NodeId
    pub fn node_id(&self) -> NodeId {
        self.endpoint.node_id()
    }

    /// Get the endpoint's home relay URLs if available
    pub fn relay_urls(&self) -> Vec<String> {
        let mut watcher = self.endpoint.home_relay();
        watcher
            .get()
            .iter()
            .map(|url| url.to_string())
            .collect()
    }

    /// Start accepting incoming connections
    ///
    /// This spawns a background task that accepts connections and emits events.
    /// Messages are read from streams and emitted as TransportEvent::Message.
    pub fn start_accepting(&self) {
        let endpoint = self.endpoint.clone();
        let pool = self.pool.clone();
        let event_tx = self.event_tx.clone();

        tokio::spawn(async move {
            info!("Started accepting connections");

            while let Some(incoming) = endpoint.accept().await {
                let pool = pool.clone();
                let event_tx = event_tx.clone();

                match incoming.accept() {
                    Ok(connecting) => {
                        tokio::spawn(async move {
                            match connecting.await {
                                Ok(conn) => {
                                    let node_id = conn.remote_node_id()
                                        .expect("connection should have remote node id");
                                    info!("Accepted connection from: {}", node_id);

                                    let handle = ConnectionHandle::new(conn.clone(), node_id);
                                    pool.insert(handle.clone()).await;

                                    // Emit connected event
                                    let _ = event_tx
                                        .send(TransportEvent::Connected {
                                            node_id,
                                            conn: handle.clone(),
                                        })
                                        .await;

                                    // Start reading messages from this connection
                                    spawn_message_reader(conn, node_id, event_tx, pool).await;
                                }
                                Err(e) => {
                                    error!("Connection failed: {}", e);
                                }
                            }
                        });
                    }
                    Err(e) => {
                        error!("Failed to accept incoming: {}", e);
                    }
                }
            }

            warn!("Connection acceptor stopped");
        });
    }

    /// Connect to a peer by NodeId
    ///
    /// Returns the ConnectionHandle for sending messages.
    pub async fn connect(&self, node_id: NodeId) -> Result<ConnectionHandle> {
        // Check if already connected
        if let Some(handle) = self.pool.get(&node_id).await {
            debug!("Reusing existing connection to: {}", node_id);
            return Ok(handle);
        }

        let node_addr = NodeAddr::new(node_id);
        info!("Connecting to: {}", node_id);

        let conn = self
            .endpoint
            .connect(node_addr, ALPN_PROTOCOL)
            .await
            .map_err(|e| anyhow!("Failed to connect: {}", e))?;

        info!("Connected to: {}", node_id);

        let handle = ConnectionHandle::new(conn.clone(), node_id);
        self.pool.insert(handle.clone()).await;

        // Emit connected event
        let _ = self
            .event_tx
            .send(TransportEvent::Connected {
                node_id,
                conn: handle.clone(),
            })
            .await;

        // Start reading messages
        let event_tx = self.event_tx.clone();
        let pool = self.pool.clone();
        spawn_message_reader(conn, node_id, event_tx, pool).await;

        Ok(handle)
    }

    /// Connect to a peer with relay hint
    pub async fn connect_with_relay(
        &self,
        node_id: NodeId,
        relay_url: &str,
    ) -> Result<ConnectionHandle> {
        // Check if already connected
        if let Some(handle) = self.pool.get(&node_id).await {
            debug!("Reusing existing connection to: {}", node_id);
            return Ok(handle);
        }

        let relay = relay_url
            .parse()
            .map_err(|e| anyhow!("Invalid relay URL: {}", e))?;
        let node_addr = NodeAddr::new(node_id).with_relay_url(relay);

        info!("Connecting to {} via relay {}", node_id, relay_url);

        let conn = self
            .endpoint
            .connect(node_addr, ALPN_PROTOCOL)
            .await
            .map_err(|e| anyhow!("Failed to connect: {}", e))?;

        info!("Connected to: {}", node_id);

        let handle = ConnectionHandle::new(conn.clone(), node_id);
        self.pool.insert(handle.clone()).await;

        // Emit connected event
        let _ = self
            .event_tx
            .send(TransportEvent::Connected {
                node_id,
                conn: handle.clone(),
            })
            .await;

        // Start reading messages
        let event_tx = self.event_tx.clone();
        let pool = self.pool.clone();
        spawn_message_reader(conn, node_id, event_tx, pool).await;

        Ok(handle)
    }

    /// Get a connection handle for a peer
    pub async fn get_connection(&self, node_id: &NodeId) -> Option<ConnectionHandle> {
        self.pool.get(node_id).await
    }

    /// Disconnect from a peer
    pub async fn disconnect(&self, node_id: &NodeId) {
        if let Some(handle) = self.pool.remove(node_id).await {
            handle.close();
            let _ = self
                .event_tx
                .send(TransportEvent::Disconnected { node_id: *node_id })
                .await;
        }
    }

    /// Get all connected peer NodeIds
    pub async fn connected_peers(&self) -> Vec<NodeId> {
        self.pool.peers().await
    }

    /// Check if connected to a peer
    pub async fn is_connected(&self, node_id: &NodeId) -> bool {
        self.pool.contains(node_id).await
    }

    /// Get the connection pool (for advanced usage)
    pub fn pool(&self) -> &Arc<ConnectionPool> {
        &self.pool
    }

    /// Broadcast a message to multiple peers
    ///
    /// Fire-and-forget: spawns tasks for each send, doesn't wait for completion.
    pub async fn broadcast(&self, node_ids: &[NodeId], msg: &Message) {
        for node_id in node_ids {
            if let Some(handle) = self.pool.get(node_id).await {
                let handle = handle.clone();
                let msg = msg.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle.send(&msg).await {
                        error!("Broadcast to {} failed: {}", handle.node_id(), e);
                    }
                });
            }
        }
    }

    /// Broadcast to all connected peers
    pub async fn broadcast_all(&self, msg: &Message) {
        let peers = self.pool.peers().await;
        self.broadcast(&peers, msg).await;
    }
}

/// Spawn a task to read messages from a connection
async fn spawn_message_reader(
    conn: Connection,
    node_id: NodeId,
    event_tx: mpsc::Sender<TransportEvent>,
    pool: Arc<ConnectionPool>,
) {
    tokio::spawn(async move {
        loop {
            match conn.accept_bi().await {
                Ok((send, mut recv)) => {
                    // Drop send - we don't need to respond on the same stream
                    drop(send);

                    // Read length prefix
                    let mut len_buf = [0u8; 4];
                    if let Err(e) = recv.read_exact(&mut len_buf).await {
                        // Check if it's a connection close (stream finished)
                        let err_str = e.to_string();
                        if err_str.contains("closed") || err_str.contains("reset") || err_str.contains("finished") {
                            debug!("Stream closed by peer: {}", node_id);
                            continue; // Try accepting next stream
                        }
                        trace!("Failed to read message length: {}", e);
                        continue;
                    }

                    let len = u32::from_be_bytes(len_buf) as usize;
                    if len > 10 * 1024 * 1024 {
                        // 10MB max
                        warn!("Message too large from {}: {} bytes", node_id, len);
                        continue;
                    }

                    // Read message data
                    let mut data = vec![0u8; len];
                    if let Err(e) = recv.read_exact(&mut data).await {
                        error!("Failed to read message data: {}", e);
                        continue;
                    }

                    // Deserialize
                    match bincode::deserialize::<Message>(&data) {
                        Ok(message) => {
                            trace!("←─ RECV ({} bytes) from {}: {:?}", len, node_id, message);
                            let _ = event_tx
                                .send(TransportEvent::Message { node_id, message })
                                .await;
                        }
                        Err(e) => {
                            error!("Failed to deserialize message: {}", e);
                            let _ = event_tx
                                .send(TransportEvent::Error {
                                    node_id: Some(node_id),
                                    error: format!("Deserialization error: {}", e),
                                })
                                .await;
                        }
                    }
                }
                Err(e) => {
                    // Connection error - check if it's expected closure
                    let err_str = e.to_string();
                    if err_str.contains("closed") || err_str.contains("reset") || err_str.contains("timed out") {
                        debug!("Connection closed: {}", node_id);
                    } else {
                        error!("Failed to accept stream from {}: {}", node_id, e);
                    }
                    break;
                }
            }
        }

        // Connection ended - clean up
        pool.remove(&node_id).await;
        let _ = event_tx
            .send(TransportEvent::Disconnected { node_id })
            .await;
        info!("Disconnected from: {}", node_id);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_builder() {
        let config = TransportConfig::new([0u8; 32]).with_event_buffer_size(512);

        assert_eq!(config.event_buffer_size, 512);
    }
}
