//! Connection pool and handles for peer connections
//!
//! Transport owns the ConnectionPool. The protocol layer gets ConnectionHandle
//! references to send raw bytes to peers.
//!
//! Transport exposes QUIC primitives for the consumer (PeerActor) to use:
//! - Ephemeral streams: open, send, close (for reliable protocol messages)
//! - Datagrams: fire-and-forget (for cursor sync, typing indicators)
//! - Persistent streams: long-lived bidirectional (for audio/video - future)

use anyhow::Result;
use bytes::Bytes;
use iroh::endpoint::{Connection, RecvStream, SendStream};
use iroh::EndpointId as NodeId;
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
        self.connection.close(0u32.into(), b"closed");
    }

    /// Get the underlying connection for accepting streams
    pub fn connection(&self) -> &Connection {
        &self.connection
    }

    // Datagram Primitives (unreliable, fire-and-forget)

    /// Send unreliable datagram (fire-and-forget)
    ///
    /// **Use for**: cursor sync, typing indicators, presence
    /// **Properties**: Unreliable, unordered, ~1200 byte limit
    /// **No length prefix**: datagrams are self-contained
    pub fn send_datagram(&self, data: &[u8]) -> Result<()> {
        trace!("─→ DATAGRAM ({} bytes)", data.len());
        self.connection
            .send_datagram(Bytes::copy_from_slice(data))?;
        Ok(())
    }

    /// Read next datagram (async)
    ///
    /// **Use for**: receiving cursor updates, typing indicators
    /// **Blocks**: Until a datagram arrives
    pub async fn read_datagram(&self) -> Result<Bytes> {
        Ok(self.connection.read_datagram().await?)
    }

    /// Get max datagram size for this connection
    ///
    /// **Returns**: Maximum payload size, typically ~1200 bytes
    pub fn max_datagram_size(&self) -> Option<usize> {
        self.connection.max_datagram_size()
    }

    // Stream Primitives (reliable, ordered)

    /// Accept incoming bidirectional stream
    ///
    /// **Use for**: reading ephemeral protocol messages (Hello, SyncOffer, etc.)
    /// **Consumer spawns**: read loop that calls this repeatedly
    pub async fn accept_bi(&self) -> Result<(SendStream, RecvStream)> {
        Ok(self.connection.accept_bi().await?)
    }

    /// Open bidirectional stream
    ///
    /// **Use for**: sending reliable messages that need a response
    pub async fn open_bi(&self) -> Result<(SendStream, RecvStream)> {
        Ok(self.connection.open_bi().await?)
    }
}

/// Handle to a peer connection
///
/// This is a lightweight reference that the protocol layer can store and use
/// to send raw bytes to peers.
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
    /// Create a new connection handle
    pub fn new(connection: Connection, node_id: NodeId) -> Self {
        Self {
            inner: Arc::new(PeerConnection::new(connection)),
            node_id,
        }
    }

    /// Send raw bytes on ephemeral stream
    ///
    /// Length prefix is added automatically.
    pub async fn send_bytes(&self, data: &[u8]) -> Result<()> {
        self.inner.send_bytes(data).await
    }

    /// Send raw bytes on persistent live stream
    ///
    /// Length prefix is added automatically.
    pub async fn send_bytes_live(&self, data: &[u8]) -> Result<()> {
        self.inner.send_bytes_live(data).await
    }

    /// Get the peer's NodeId
    pub fn node_id(&self) -> NodeId {
        self.node_id
    }

    /// Close the connection
    pub fn close(&self) {
        self.inner.close();
    }

    // Datagram Primitives (unreliable, fire-and-forget)

    /// Send unreliable datagram (fire-and-forget)
    ///
    /// **Use for**: cursor sync, typing indicators, presence
    /// **Properties**: Unreliable, unordered, ~1200 byte limit
    pub fn send_datagram(&self, data: &[u8]) -> Result<()> {
        self.inner.send_datagram(data)
    }

    /// Read next datagram (async)
    ///
    /// **Use for**: receiving cursor updates, typing indicators
    /// **Blocks**: Until a datagram arrives
    pub async fn read_datagram(&self) -> Result<Vec<u8>> {
        Ok(self.inner.read_datagram().await?.to_vec())
    }

    /// Get max datagram size for this connection
    pub fn max_datagram_size(&self) -> Option<usize> {
        self.inner.max_datagram_size()
    }

    // Stream Primitives (reliable, ordered)

    /// Accept incoming bidirectional stream
    ///
    /// **Use for**: reading ephemeral protocol messages (Hello, SyncOffer, etc.)
    /// **Consumer spawns**: read loop that calls this repeatedly
    pub async fn accept_bi(&self) -> Result<(SendStream, RecvStream)> {
        self.inner.accept_bi().await
    }

    /// Open bidirectional stream
    ///
    /// **Use for**: sending reliable messages that need a response
    pub async fn open_bi(&self) -> Result<(SendStream, RecvStream)> {
        self.inner.open_bi().await
    }
}

/// Pool of active peer connections
///
/// Owned by Transport. Provides lookup and management of connections.
pub struct ConnectionPool {
    connections: RwLock<HashMap<NodeId, ConnectionHandle>>,
}

impl std::fmt::Debug for ConnectionPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConnectionPool").finish_non_exhaustive()
    }
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

    #[tokio::test]
    async fn test_pool_operations() {
        let pool = ConnectionPool::new();

        assert!(pool.is_empty().await);
        assert_eq!(pool.len().await, 0);

        // Integration tests with real connections are in integration_tests crate
    }
}
