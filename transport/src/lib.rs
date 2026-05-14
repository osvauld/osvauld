//! Transport Layer for Osvauld P2P Network
//!
//! This crate provides a **dumb byte pipe** for P2P networking using iroh.
//! It handles connections, streams, and length-prefixed byte framing.
//!
//! **Transport has NO protocol knowledge.** It only sends/receives raw bytes.
//! Message types and serialization live in the protocol layer (courier).
//!
//! # Architecture
//!
//! - **Transport**: Main API, owns Router and connection pool
//! - **Router**: Dispatches incoming connections based on ALPN
//! - **OsvaualdProtocol**: ProtocolHandler for our "osvauld/p2p/1" ALPN
//! - **ConnectionPool**: Manages active connections
//! - **ConnectionHandle**: Lightweight reference for sending bytes
//! - **TransportEvent**: Events emitted to protocol layer via channel
//!
//! # Pluggable Transport Architecture
//!
//! The `traits` module defines abstract `Connection` and `Transport` traits that enable:
//! - **Production flexibility**: Swap Iroh <-> Quinn based on deployment
//! - **Testability**: Mock transport for unit tests, Sim for DST
//! - **Protocol isolation**: PeerActor/sync code is transport-agnostic
//! - **Zero-cost abstraction**: Compile-time dispatch via generics
//!
//! # Multi-ALPN Support
//!
//! The Router pattern allows handling multiple protocols on the same endpoint:
//! - `osvauld/p2p/1` - Our protocol messages (Hello, SyncOffer, etc.)
//! - `iroh-blobs` - Binary asset transfers (for asset sync)

pub mod events;
pub mod iroh_connection;
pub mod mock;
pub mod pool;
pub mod protocol;
pub mod traits;

pub use events::TransportEvent;
pub use pool::{ConnectionHandle, ConnectionPool};
pub use protocol::OsvaualdProtocol;

// Re-export trait types for convenience
pub use traits::{BiStream, Connection, ConnectionEvent, Transport as TransportTrait};

// Re-export connection implementations
pub use iroh_connection::{IrohBiStream, IrohConnection};
pub use mock::{
    mock_connection_pair, mock_connection_pair_named, mock_node_id, node_id_from_secret,
    DatagramCallback, MockBiStream, MockConnection,
};

// Re-export iroh types so consumers don't need direct iroh dependency
// Note: iroh 0.95 renamed NodeId to EndpointId, we re-export as NodeId for compatibility
pub use iroh::EndpointId as NodeId;
// Re-export iroh-blobs Hash type for asset transfers
pub use iroh_blobs::Hash as BlobHash;

use anyhow::{anyhow, Result};
use iroh::endpoint::Connection as IrohQuicConnection;
use iroh::protocol::Router;
use iroh::{Endpoint, EndpointAddr, RelayMode, SecretKey};
use iroh_blobs::store::mem::MemStore;
use iroh_blobs::BlobsProtocol;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
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
    /// Data directory for blob storage (optional, uses MemStore if None)
    pub data_dir: Option<PathBuf>,
}

impl TransportConfig {
    pub fn new(secret_key: [u8; 32]) -> Self {
        Self {
            secret_key,
            event_buffer_size: 256,
            data_dir: None,
        }
    }

    pub fn with_event_buffer_size(mut self, size: usize) -> Self {
        self.event_buffer_size = size;
        self
    }

    pub fn with_data_dir(mut self, dir: PathBuf) -> Self {
        self.data_dir = Some(dir);
        self
    }
}

/// Transport layer handle
///
/// Provides the API for P2P networking. Courier receives events via the
/// mpsc channel returned from `init()`.
///
/// Uses iroh's Router pattern to dispatch connections based on ALPN:
/// - "osvauld/p2p/1" → OsvaualdProtocol (our messages)
/// - "iroh-blobs/v1" → BlobsProtocol (asset transfers)
pub struct Transport {
    router: Router,
    pool: Arc<ConnectionPool>,
    event_tx: mpsc::Sender<TransportEvent>,
    /// Blob store for asset transfers (iroh-blobs)
    blob_store: MemStore,
}

impl Transport {
    /// Initialize the transport layer with Router pattern
    ///
    /// Returns the Transport and a receiver for TransportEvents.
    /// The caller (Courier) should spawn a task to process events.
    ///
    /// **Router Setup**:
    /// - Binds endpoint with our ALPN + iroh-blobs ALPN
    /// - Creates OsvaualdProtocol handler for our messages
    /// - Creates BlobsProtocol handler for asset transfers
    /// - Router automatically dispatches based on ALPN
    pub async fn init(config: TransportConfig) -> Result<(Self, mpsc::Receiver<TransportEvent>)> {
        let secret_key = SecretKey::from(config.secret_key);

        info!("Initializing transport layer");

        // Initialize blob store (MemStore for now, FsStore later if data_dir provided)
        let blob_store = MemStore::new();
        info!("Blob store initialized (MemStore)");

        // Build the endpoint with both ALPNs
        let endpoint = Endpoint::builder()
            .secret_key(secret_key)
            .relay_mode(RelayMode::Default)
            .alpns(vec![ALPN_PROTOCOL.to_vec(), iroh_blobs::ALPN.to_vec()])
            .bind()
            .await
            .map_err(|e| anyhow!("Failed to bind endpoint: {}", e))?;

        // Wait for endpoint to come online (establishes relay connection)
        endpoint.online().await;

        let node_id = endpoint.id();
        info!("Transport bound with node_id: {}", node_id);

        let (event_tx, event_rx) = mpsc::channel(config.event_buffer_size);
        let pool = Arc::new(ConnectionPool::new());

        // Create our protocol handler
        let osvauld_protocol = OsvaualdProtocol::new(pool.clone(), event_tx.clone());

        // Create blobs protocol handler for asset transfers
        let blobs_protocol = BlobsProtocol::new(&blob_store, None);

        // Build Router - dispatches incoming connections based on ALPN
        // Router::spawn() starts accepting connections automatically
        let router = Router::builder(endpoint)
            .accept(ALPN_PROTOCOL, osvauld_protocol)
            .accept(iroh_blobs::ALPN, blobs_protocol)
            .spawn();

        info!("Router started with osvauld + blobs protocols");

        let transport = Self {
            router,
            pool,
            event_tx,
            blob_store,
        };

        Ok((transport, event_rx))
    }

    /// Get this node's NodeId
    pub fn node_id(&self) -> NodeId {
        self.router.endpoint().id()
    }

    /// Get the endpoint's home relay URLs if available
    pub fn relay_urls(&self) -> Vec<String> {
        let addr = self.router.endpoint().addr();
        addr.relay_urls().map(|url| url.to_string()).collect()
    }

    /// Get reference to the underlying endpoint
    ///
    /// Useful for advanced operations like adding discovery services
    pub fn endpoint(&self) -> &Endpoint {
        self.router.endpoint()
    }

    /// Connect to a peer by NodeId
    ///
    /// Returns the ConnectionHandle for sending messages.
    /// Uses exponential backoff retry for connection failures (e.g., DNS/relay flakiness).
    pub async fn connect(&self, node_id: NodeId) -> Result<ConnectionHandle> {
        // Check if already connected
        if let Some(handle) = self.pool.get(&node_id).await {
            debug!("Reusing existing connection to: {}", node_id);
            return Ok(handle);
        }

        let endpoint_addr = EndpointAddr::from(node_id);
        info!("Connecting to: {}", node_id);

        // Retry with exponential backoff for DNS/relay flakiness
        let mut delay = Duration::from_millis(100);
        let max_delay = Duration::from_secs(5);
        let max_attempts = 5;
        let mut last_error = None;

        for attempt in 1..=max_attempts {
            match self
                .router
                .endpoint()
                .connect(endpoint_addr.clone(), ALPN_PROTOCOL)
                .await
            {
                Ok(conn) => {
                    if attempt > 1 {
                        info!("Connected to {} (attempt {})", node_id, attempt);
                    } else {
                        info!("Connected to: {}", node_id);
                    }

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

                    // Spawn disconnect watcher
                    let event_tx = self.event_tx.clone();
                    let pool = self.pool.clone();
                    tokio::spawn(connection_close_watcher(conn, node_id, event_tx, pool));

                    return Ok(handle);
                }
                Err(e) => {
                    last_error = Some(e);
                    if attempt < max_attempts {
                        warn!(
                            "Connection attempt {} to {} failed, retrying in {:?}",
                            attempt, node_id, delay
                        );
                        tokio::time::sleep(delay).await;
                        delay = std::cmp::min(delay * 2, max_delay);
                    }
                }
            }
        }

        Err(anyhow!(
            "Failed to connect to {} after {} attempts: {}",
            node_id,
            max_attempts,
            last_error.map(|e| e.to_string()).unwrap_or_default()
        ))
    }

    /// Connect to a peer with relay hint
    ///
    /// Uses exponential backoff retry for connection failures (e.g., DNS/relay flakiness).
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
        let endpoint_addr = EndpointAddr::from(node_id).with_relay_url(relay);

        info!("Connecting to {} via relay {}", node_id, relay_url);

        // Retry with exponential backoff for DNS/relay flakiness
        let mut delay = Duration::from_millis(100);
        let max_delay = Duration::from_secs(5);
        let max_attempts = 5;
        let mut last_error = None;

        for attempt in 1..=max_attempts {
            match self
                .router
                .endpoint()
                .connect(endpoint_addr.clone(), ALPN_PROTOCOL)
                .await
            {
                Ok(conn) => {
                    if attempt > 1 {
                        info!("Connected to {} via relay (attempt {})", node_id, attempt);
                    } else {
                        info!("Connected to: {}", node_id);
                    }

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

                    // Spawn disconnect watcher
                    let event_tx = self.event_tx.clone();
                    let pool = self.pool.clone();
                    tokio::spawn(connection_close_watcher(conn, node_id, event_tx, pool));

                    return Ok(handle);
                }
                Err(e) => {
                    last_error = Some(e);
                    if attempt < max_attempts {
                        warn!(
                            "Connection attempt {} to {} via relay failed, retrying in {:?}",
                            attempt, node_id, delay
                        );
                        tokio::time::sleep(delay).await;
                        delay = std::cmp::min(delay * 2, max_delay);
                    }
                }
            }
        }

        Err(anyhow!(
            "Failed to connect to {} via relay after {} attempts: {}",
            node_id,
            max_attempts,
            last_error.map(|e| e.to_string()).unwrap_or_default()
        ))
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

    // Blob Operations (iroh-blobs)

    /// Add plaintext bytes to blob store for transfer
    ///
    /// **Context**: Preparing asset for peer to download
    /// **Returns**: iroh-blobs Hash (blake3) for the blob
    pub async fn add_blob(&self, data: &[u8]) -> Result<iroh_blobs::Hash> {
        let tag = self
            .blob_store
            .add_slice(data.to_vec())
            .await
            .map_err(|e| anyhow!("Failed to add blob: {}", e))?;
        info!(hash = %tag.hash, size = data.len(), "Added blob to store");
        Ok(tag.hash)
    }

    /// Download blob from a peer
    ///
    /// **Context**: Peer has prepared a blob, we fetch it via iroh-blobs
    /// **Flow**: Connect to peer with blobs ALPN, stream download, verify hash
    pub async fn download_blob(
        &self,
        hash: iroh_blobs::Hash,
        from_node: NodeId,
    ) -> Result<Vec<u8>> {
        info!(hash = %hash, from = %from_node, "Downloading blob from peer");

        // Create downloader and fetch from peer
        let downloader = self.blob_store.downloader(self.router.endpoint());
        downloader
            .download(hash, Some(from_node))
            .await
            .map_err(|e| anyhow!("Blob download failed: {}", e))?;

        // Read from local store after download completes
        let bytes = self.get_blob(hash).await?;
        info!(hash = %hash, size = bytes.len(), "Blob downloaded successfully");
        Ok(bytes)
    }

    /// Get blob bytes from local store
    ///
    /// **Context**: Blob already exists locally (uploaded or downloaded)
    pub async fn get_blob(&self, hash: iroh_blobs::Hash) -> Result<Vec<u8>> {
        let bytes = self
            .blob_store
            .blobs()
            .get_bytes(hash)
            .await
            .map_err(|e| anyhow!("Failed to read blob: {}", e))?;
        Ok(bytes.to_vec())
    }

    /// Check if a blob exists in local store
    pub async fn has_blob(&self, hash: iroh_blobs::Hash) -> bool {
        self.blob_store.blobs().has(hash).await.unwrap_or(false)
    }

    /// Remove blob from local store (cleanup after transfer)
    ///
    /// **Context**: Asset transfer complete, remove temporary blob
    /// **Note**: MemStore auto-cleans via reference counting, this is a no-op for now
    pub async fn remove_blob(&self, _hash: iroh_blobs::Hash) -> Result<()> {
        // MemStore uses reference counting - blobs are cleaned when tags are dropped
        // For FsStore we'd need explicit deletion via the store API
        // For now this is a no-op since MemStore handles cleanup automatically
        Ok(())
    }

    /// Gracefully shutdown the transport
    ///
    /// Closes all connections and stops the router.
    pub async fn shutdown(self) -> Result<()> {
        info!("Shutting down transport");
        self.router
            .shutdown()
            .await
            .map_err(|e| anyhow!("Shutdown failed: {}", e))?;
        info!("Transport shutdown complete");
        Ok(())
    }
}

// Connection Lifecycle Helper

/// Watch for connection close and emit Disconnected event
///
/// **Context**: Monitors connection lifecycle for outgoing connections
/// **We do**: Wait for close, then cleanup pool and emit event
async fn connection_close_watcher(
    conn: IrohQuicConnection,
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

// Mock Blob Store (for testing without real iroh-blobs)

/// Mock blob store for testing asset transfer without real iroh-blobs infrastructure
///
/// **Context**: Integration tests use MockTransport which doesn't have real networking.
/// This provides a shared in-memory blob store that simulates the P2P blob network.
///
/// **Usage**:
/// - All test peers share the same MockBlobStore instance
/// - When peer A adds a blob, peer B can download it (simulates P2P transfer)
#[derive(Debug, Default)]
pub struct MockBlobStore {
    blobs: std::sync::RwLock<std::collections::HashMap<[u8; 32], Vec<u8>>>,
}

impl MockBlobStore {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            blobs: std::sync::RwLock::new(std::collections::HashMap::new()),
        })
    }

    /// Add a blob to the store (simulates Transport::add_blob)
    ///
    /// Returns the blake3 hash as a 32-byte array.
    pub fn add_blob(&self, data: &[u8]) -> [u8; 32] {
        let hash = blake3::hash(data);
        let hash_bytes: [u8; 32] = *hash.as_bytes();

        let mut blobs = self.blobs.write().expect("blob store lock poisoned");
        blobs.insert(hash_bytes, data.to_vec());

        debug!(hash = %hash, size = data.len(), "MockBlobStore: added blob");
        hash_bytes
    }

    /// Download a blob from the store (simulates Transport::download_blob)
    ///
    /// In real iroh, this fetches from a specific peer.
    /// In mock, we just return from the shared store.
    pub fn download_blob(&self, hash: &[u8; 32]) -> Option<Vec<u8>> {
        let blobs = self.blobs.read().expect("blob store lock poisoned");
        let result = blobs.get(hash).cloned();

        if result.is_some() {
            debug!(hash = %hex::encode(hash), "MockBlobStore: blob downloaded");
        }

        result
    }

    /// Check if a blob exists
    pub fn has_blob(&self, hash: &[u8; 32]) -> bool {
        let blobs = self.blobs.read().expect("blob store lock poisoned");
        blobs.contains_key(hash)
    }
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
