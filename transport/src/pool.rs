//! Connection pool and handles for peer connections
//!
//! Transport owns the ConnectionPool. The protocol layer gets ConnectionHandle
//! references to send raw bytes to peers.
//!
//! Transport is a **dumb byte pipe** - it has no knowledge of message types.
//! Serialization/deserialization happens in the protocol layer (courier2).

use anyhow::Result;
use iroh::endpoint::Connection;
use iroh::NodeId;
use iroh_quinn::{RecvStream, SendStream, VarInt};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, RwLock};
use tracing::{info, trace, warn};

/// Internal connection state for a peer
pub struct PeerConnection {
    /// The iroh QUIC connection
    connection: Connection,
    /// Persistent bidirectional stream for live data (optional)
    live_stream: Mutex<Option<(SendStream, RecvStream)>>,
}

impl PeerConnection {
    pub fn new(connection: Connection) -> Self {
        Self {
            connection,
            live_stream: Mutex::new(None),
        }
    }

    /// Send raw bytes on an ephemeral stream (opens, sends, closes)
    ///
    /// Adds length prefix automatically.
    pub async fn send_bytes(&self, data: &[u8]) -> Result<()> {
        let len = data.len();

        trace!("─→ SEND ({} bytes)", len);

        let (mut send, _recv) = self.connection.open_bi().await?;

        // Write length prefix then data
        send.write_all(&(len as u32).to_be_bytes()).await?;
        send.write_all(data).await?;
        send.finish()?;

        Ok(())
    }

    /// Send raw bytes on the persistent live stream
    ///
    /// Creates the stream if it doesn't exist yet.
    /// Adds length prefix automatically.
    pub async fn send_bytes_live(&self, data: &[u8]) -> Result<()> {
        let len = data.len();

        let mut live_guard = self.live_stream.lock().await;

        // Create stream if needed
        if live_guard.is_none() {
            let (send, recv) = self.connection.open_bi().await?;
            *live_guard = Some((send, recv));
            trace!("Created persistent live stream");
        }

        trace!("─→ SEND live ({} bytes)", len);

        let (send, _recv) = live_guard.as_mut().unwrap();

        // Write length prefix then data
        send.write_all(&(len as u32).to_be_bytes()).await?;
        send.write_all(data).await?;

        Ok(())
    }

    /// Close the connection
    pub fn close(&self) {
        self.connection.close(VarInt::from_u32(0), b"closed");
    }

    /// Get the underlying connection for accepting streams
    pub fn connection(&self) -> &Connection {
        &self.connection
    }
}

/// Backend for ConnectionHandle - either real QUIC or mock channel
enum ConnectionBackend {
    /// Real iroh QUIC connection
    Real(Arc<PeerConnection>),
    /// Mock channel for testing (sends bytes through mpsc)
    Mock(MockSender),
}

/// Mock sender for testing - wraps channel sender
#[derive(Clone)]
pub struct MockSender {
    /// Channel to send bytes through
    sender: mpsc::UnboundedSender<MockSendEvent>,
    /// Our node ID
    our_node_id: NodeId,
    /// Peer's node ID
    peer_node_id: NodeId,
}

/// Event sent through mock sender
#[derive(Debug)]
pub struct MockSendEvent {
    pub from: NodeId,
    pub to: NodeId,
    pub data: Vec<u8>,
}

impl MockSender {
    pub fn new(
        sender: mpsc::UnboundedSender<MockSendEvent>,
        our_node_id: NodeId,
        peer_node_id: NodeId,
    ) -> Self {
        Self {
            sender,
            our_node_id,
            peer_node_id,
        }
    }
}

/// Handle to a peer connection
///
/// This is a lightweight reference that the protocol layer can store and use
/// to send raw bytes to peers.
/// The actual connection is owned by Transport's ConnectionPool.
///
/// Supports both real QUIC connections and mock channels for testing.
#[derive(Clone)]
pub struct ConnectionHandle {
    backend: Arc<ConnectionBackend>,
    node_id: NodeId,
}

impl std::fmt::Debug for ConnectionHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConnectionHandle")
            .field("node_id", &self.node_id)
            .finish()
    }
}

impl ConnectionHandle {
    /// Create a new connection handle for a real QUIC connection
    pub fn new(connection: Connection, node_id: NodeId) -> Self {
        Self {
            backend: Arc::new(ConnectionBackend::Real(Arc::new(PeerConnection::new(connection)))),
            node_id,
        }
    }

    /// Create a mock connection handle for testing
    pub fn from_mock_sender(sender: MockSender) -> Self {
        let node_id = sender.peer_node_id;
        Self {
            backend: Arc::new(ConnectionBackend::Mock(sender)),
            node_id,
        }
    }

    /// Send raw bytes on ephemeral stream
    ///
    /// Length prefix is added automatically.
    pub async fn send_bytes(&self, data: &[u8]) -> Result<()> {
        match self.backend.as_ref() {
            ConnectionBackend::Real(inner) => inner.send_bytes(data).await,
            ConnectionBackend::Mock(sender) => {
                sender
                    .sender
                    .send(MockSendEvent {
                        from: sender.our_node_id,
                        to: sender.peer_node_id,
                        data: data.to_vec(),
                    })
                    .map_err(|e| anyhow::anyhow!("Mock send failed: {}", e))
            }
        }
    }

    /// Send raw bytes on persistent live stream
    ///
    /// Length prefix is added automatically.
    pub async fn send_bytes_live(&self, data: &[u8]) -> Result<()> {
        match self.backend.as_ref() {
            ConnectionBackend::Real(inner) => inner.send_bytes_live(data).await,
            ConnectionBackend::Mock(sender) => {
                // Mock doesn't distinguish ephemeral vs live
                sender
                    .sender
                    .send(MockSendEvent {
                        from: sender.our_node_id,
                        to: sender.peer_node_id,
                        data: data.to_vec(),
                    })
                    .map_err(|e| anyhow::anyhow!("Mock send failed: {}", e))
            }
        }
    }

    /// Get the peer's NodeId
    pub fn node_id(&self) -> NodeId {
        self.node_id
    }

    /// Close the connection
    pub fn close(&self) {
        if let ConnectionBackend::Real(inner) = self.backend.as_ref() {
            inner.close();
        }
        // Mock connections don't need closing
    }

    /// Get reference to inner connection for accepting streams (real connections only)
    pub(crate) fn inner(&self) -> Option<&Arc<PeerConnection>> {
        match self.backend.as_ref() {
            ConnectionBackend::Real(inner) => Some(inner),
            ConnectionBackend::Mock(_) => None,
        }
    }
}

/// Pool of active peer connections
///
/// Owned by Transport. Provides lookup and management of connections.
pub struct ConnectionPool {
    connections: RwLock<HashMap<NodeId, ConnectionHandle>>,
}

impl ConnectionPool {
    pub fn new() -> Self {
        Self {
            connections: RwLock::new(HashMap::new()),
        }
    }

    /// Add a connection to the pool
    pub async fn insert(&self, handle: ConnectionHandle) {
        let node_id = handle.node_id();
        let mut conns = self.connections.write().await;

        if let Some(old) = conns.insert(node_id, handle) {
            warn!("Replaced existing connection for node {}", node_id);
            old.close();
        }

        info!("Added connection to pool: {}", node_id);
    }

    /// Remove a connection from the pool
    pub async fn remove(&self, node_id: &NodeId) -> Option<ConnectionHandle> {
        let mut conns = self.connections.write().await;
        let removed = conns.remove(node_id);

        if removed.is_some() {
            info!("Removed connection from pool: {}", node_id);
        }

        removed
    }

    /// Get a connection handle by NodeId
    pub async fn get(&self, node_id: &NodeId) -> Option<ConnectionHandle> {
        let conns = self.connections.read().await;
        conns.get(node_id).cloned()
    }

    /// Check if a connection exists
    pub async fn contains(&self, node_id: &NodeId) -> bool {
        let conns = self.connections.read().await;
        conns.contains_key(node_id)
    }

    /// Get all connected NodeIds
    pub async fn peers(&self) -> Vec<NodeId> {
        let conns = self.connections.read().await;
        conns.keys().cloned().collect()
    }

    /// Get count of connections
    pub async fn len(&self) -> usize {
        let conns = self.connections.read().await;
        conns.len()
    }

    /// Check if pool is empty
    pub async fn is_empty(&self) -> bool {
        let conns = self.connections.read().await;
        conns.is_empty()
    }
}

impl Default for ConnectionPool {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: Integration tests would require actual iroh connections
    // These are placeholder tests for the pool logic

    #[tokio::test]
    async fn test_pool_operations() {
        let pool = ConnectionPool::new();

        assert!(pool.is_empty().await);
        assert_eq!(pool.len().await, 0);

        // Can't easily test insert/get without real iroh connections
        // In integration tests, we'd use actual iroh setup
    }
}
