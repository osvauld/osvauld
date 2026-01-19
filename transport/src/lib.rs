//! Transport Layer for Osvauld P2P Network
//!
//! This crate provides a **dumb byte pipe** for P2P networking using iroh.
//! It handles connections, streams, and length-prefixed byte framing.
//!
//! **Transport has NO protocol knowledge.** It only sends/receives raw bytes.
//! Message types and serialization live in the protocol layer (courier2).
//!
//! # Architecture
//!
//! - **Transport**: Main API, owns endpoint and connection pool
//! - **ConnectionPool**: Manages active connections
//! - **ConnectionHandle**: Lightweight reference for sending bytes
//! - **TransportEvent**: Events emitted to protocol layer via channel

pub mod events;
pub mod pool;

pub use events::TransportEvent;
pub use pool::{ConnectionHandle, ConnectionPool, MockSender, MockSendEvent};

// Re-export iroh types so consumers don't need direct iroh dependency
pub use iroh::NodeId;

use anyhow::{anyhow, Result};
use iroh::endpoint::Connection;
use iroh::{Endpoint, NodeAddr, RelayMode, SecretKey, Watcher};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

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
        watcher.get().iter().map(|url| url.to_string()).collect()
    }

    /// Start accepting incoming connections
    ///
    /// This spawns a background task that accepts connections and emits events.
    /// Messages are read from streams and emitted as TransportEvent::Message.
    pub fn start_accepting(&self) {
        let endpoint = self.endpoint.clone();
        let pool = self.pool.clone();
        let event_tx = self.event_tx.clone();

        tokio::spawn(run_accept_loop(endpoint, pool, event_tx));
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

        // Emit connected event - consumer (SessionManager) will spawn PeerSession
        let _ = self
            .event_tx
            .send(TransportEvent::Connected {
                node_id,
                conn: handle.clone(),
            })
            .await;

        // Spawn disconnect watcher - no read loop (PeerSession owns reading)
        let event_tx = self.event_tx.clone();
        let pool = self.pool.clone();
        tokio::spawn(connection_close_watcher(conn, node_id, event_tx, pool));

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

        // Emit connected event - consumer (SessionManager) will spawn PeerSession
        let _ = self
            .event_tx
            .send(TransportEvent::Connected {
                node_id,
                conn: handle.clone(),
            })
            .await;

        // Spawn disconnect watcher - no read loop (PeerSession owns reading)
        let event_tx = self.event_tx.clone();
        let pool = self.pool.clone();
        tokio::spawn(connection_close_watcher(conn, node_id, event_tx, pool));

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

    /// Broadcast raw bytes to multiple peers
    ///
    /// Fire-and-forget: spawns tasks for each send, doesn't wait for completion.
    pub async fn broadcast_bytes(&self, node_ids: &[NodeId], data: &[u8]) {
        for node_id in node_ids {
            if let Some(handle) = self.pool.get(node_id).await {
                let handle = handle.clone();
                let data = data.to_vec();
                tokio::spawn(async move {
                    if let Err(e) = handle.send_bytes(&data).await {
                        error!("Broadcast to {} failed: {}", handle.node_id(), e);
                    }
                });
            }
        }
    }

    /// Broadcast raw bytes to all connected peers
    pub async fn broadcast_bytes_all(&self, data: &[u8]) {
        let peers = self.pool.peers().await;
        self.broadcast_bytes(&peers, data).await;
    }
}

// ============================================================================
// Connection Acceptance - Flattened Helper Functions
// ============================================================================

/// Main accept loop - runs until endpoint is closed
async fn run_accept_loop(
    endpoint: Arc<Endpoint>,
    pool: Arc<ConnectionPool>,
    event_tx: mpsc::Sender<TransportEvent>,
) {
    info!("Started accepting connections");

    while let Some(incoming) = endpoint.accept().await {
        let pool = pool.clone();
        let event_tx = event_tx.clone();
        tokio::spawn(handle_incoming_connection(incoming, pool, event_tx));
    }

    warn!("Connection acceptor stopped");
}

/// Handle a single incoming connection attempt
async fn handle_incoming_connection(
    incoming: iroh::endpoint::Incoming,
    pool: Arc<ConnectionPool>,
    event_tx: mpsc::Sender<TransportEvent>,
) {
    let connecting = match incoming.accept() {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to accept incoming: {}", e);
            return;
        }
    };

    let conn = match connecting.await {
        Ok(c) => c,
        Err(e) => {
            error!("Connection failed: {}", e);
            return;
        }
    };

    setup_accepted_connection(conn, pool, event_tx).await;
}

/// Setup a successfully accepted connection
///
/// **Context**: Called when a new QUIC connection is established
/// **We do**: Store handle in pool, emit Connected event, spawn disconnect watcher
/// **Consumer does**: Spawn PeerSession with ConnectionHandle for reading/writing
async fn setup_accepted_connection(
    conn: Connection,
    pool: Arc<ConnectionPool>,
    event_tx: mpsc::Sender<TransportEvent>,
) {
    let node_id = conn.remote_node_id().expect("should have remote node id");
    info!("Accepted connection from: {}", node_id);

    let handle = ConnectionHandle::new(conn.clone(), node_id);
    pool.insert(handle.clone()).await;

    // Emit Connected - consumer (SessionManager) will spawn PeerSession
    let _ = event_tx
        .send(TransportEvent::Connected {
            node_id,
            conn: handle,
        })
        .await;

    // Spawn disconnect watcher - just monitors connection close, no read loop
    // PeerSession owns all actual reading via the ConnectionHandle
    tokio::spawn(connection_close_watcher(conn, node_id, event_tx, pool));
}

// ============================================================================
// Connection Lifecycle - No Read Loops (PeerSession owns reading)
// ============================================================================

/// Watch for connection close and emit Disconnected event
///
/// **Context**: Transport no longer has read loops - PeerSession owns reading
/// **We do**: Just wait for the connection to close, then cleanup
/// **Note**: This is a lightweight watcher, not a read loop
async fn connection_close_watcher(
    conn: Connection,
    node_id: NodeId,
    event_tx: mpsc::Sender<TransportEvent>,
    pool: Arc<ConnectionPool>,
) {
    // Wait for connection to close (triggered by either side)
    conn.closed().await;

    // Connection ended - clean up
    pool.remove(&node_id).await;
    let _ = event_tx
        .send(TransportEvent::Disconnected { node_id })
        .await;
    info!("Disconnected from: {}", node_id);
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
