//! Connection pool and handles for peer connections
//!
//! Transport owns the ConnectionPool. Courier gets ConnectionHandle references
//! to send messages directly to peers.

use crate::protocol::Message;
use anyhow::Result;
use iroh::endpoint::Connection;
use iroh::NodeId;
use iroh_quinn::{RecvStream, SendStream, VarInt};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
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

    /// Send a message on an ephemeral stream (opens, sends, closes)
    pub async fn send(&self, msg: &Message) -> Result<()> {
        let serialized = bincode::serialize(msg)?;
        let len = serialized.len();

        trace!("─→ SEND ({} bytes): {:?}", len, msg);

        let (mut send, _recv) = self.connection.open_bi().await?;

        // Write length prefix then data
        send.write_all(&(len as u32).to_be_bytes()).await?;
        send.write_all(&serialized).await?;
        send.finish()?;

        Ok(())
    }

    /// Send a message on the persistent live stream
    ///
    /// Creates the stream if it doesn't exist yet.
    pub async fn send_live(&self, msg: &Message) -> Result<()> {
        let serialized = bincode::serialize(msg)?;
        let len = serialized.len();

        let mut live_guard = self.live_stream.lock().await;

        // Create stream if needed
        if live_guard.is_none() {
            let (send, recv) = self.connection.open_bi().await?;
            *live_guard = Some((send, recv));
            trace!("Created persistent live stream");
        }

        trace!("─→ SEND live ({} bytes): {:?}", len, msg);

        let (send, _recv) = live_guard.as_mut().unwrap();

        // Write length prefix then data
        send.write_all(&(len as u32).to_be_bytes()).await?;
        send.write_all(&serialized).await?;

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

/// Handle to a peer connection
///
/// This is a lightweight reference that Courier can store and use to send messages.
/// The actual connection is owned by Transport's ConnectionPool.
#[derive(Clone)]
pub struct ConnectionHandle {
    inner: Arc<PeerConnection>,
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
    pub fn new(connection: Connection, node_id: NodeId) -> Self {
        Self {
            inner: Arc::new(PeerConnection::new(connection)),
            node_id,
        }
    }

    /// Send message on ephemeral stream
    pub async fn send(&self, msg: &Message) -> Result<()> {
        self.inner.send(msg).await
    }

    /// Send message on persistent live stream
    pub async fn send_live(&self, msg: &Message) -> Result<()> {
        self.inner.send_live(msg).await
    }

    /// Get the peer's NodeId
    pub fn node_id(&self) -> NodeId {
        self.node_id
    }

    /// Close the connection
    pub fn close(&self) {
        self.inner.close()
    }

    /// Get reference to inner connection for accepting streams
    pub(crate) fn inner(&self) -> &Arc<PeerConnection> {
        &self.inner
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
