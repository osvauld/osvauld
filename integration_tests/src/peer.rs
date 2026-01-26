//! Peer - Clean test peer abstraction
//!
//! A Peer represents a single participant in the P2P network with:
//! - Identity (DID, username)
//! - Storage (Butler with RedbStore)
//! - Transport (real iroh connection)
//! - Coordinator (manages peer actors)
//!
//! Designed for both Rust tests and as setup for Python app tests.

use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use tokio::sync::{mpsc, RwLock};
use tracing::{info, warn};
use transport::{MockBlobStore, NodeId, Transport, TransportConfig};

use butler::{Butler, LayerCache, RedbStore, signup};
use courier::coordinator::{Coordinator, CoordinatorMessage, CourierMode};
use courier::peer_actor::BlobStore;
use courier::CourierEvent;
use ractor::{Actor, ActorRef};

/// Test peer with full P2P capabilities
pub struct Peer {
    /// Human-readable name
    pub name: String,
    /// Unique node ID (from transport)
    pub node_id: NodeId,
    /// Mode: User or Node
    pub mode: CourierMode,
    /// Coordinator actor reference
    pub coordinator: ActorRef<CoordinatorMessage>,
    /// Butler for storage/crypto operations
    pub butler: Arc<Butler>,
    /// Transport for network operations
    pub transport: Arc<Transport>,
    /// Event receiver for test assertions
    pub events: mpsc::Receiver<CourierEvent>,
    /// Keep temp dir alive
    _temp_dir: tempfile::TempDir,
}

impl Peer {
    /// Create a new peer with identity
    ///
    /// **Context**: Creates isolated storage, transport, and coordinator
    /// **Relay**: Waits for relay registration (required for discovery)
    pub async fn new(name: &str, mode: CourierMode, blob_store: Arc<MockBlobStore>) -> Result<Self> {
        let temp_dir = tempfile::tempdir()?;
        let db_path = temp_dir.path().join(format!("{}.redb", name));

        // Storage
        let store = Arc::new(RedbStore::open(&db_path)?);
        let layer_cache = Arc::new(RwLock::new(LayerCache::new(store.clone(), 100)));
        let assets_path = temp_dir.path().join("assets");
        let asset_store = Arc::new(butler::AssetStore::new(&assets_path)?);
        let butler = Arc::new(Butler::new(store.clone(), layer_cache, asset_store));

        // Create identity
        let signup_result = signup(&store, name, "password")?;
        butler.set_identity(signup_result.identity).await;

        // Transport
        let secret_key: [u8; 32] = rand::random();
        let (transport, mut transport_rx) = Transport::init(TransportConfig::new(secret_key)).await?;
        let node_id = transport.node_id();
        let transport = Arc::new(transport);

        // Event channel (for test assertions)
        let (event_tx, event_rx) = mpsc::channel::<CourierEvent>(100);

        // Connect request channel (Coordinator -> transport)
        let (connect_tx, mut connect_rx) = mpsc::channel::<courier::ConnectRequest>(16);

        // Coordinator (unique name for parallel tests)
        let blob = BlobStore::Mock(blob_store);
        let (coordinator, _) = Actor::spawn(
            Some(format!("coordinator-{}-{}", name, node_id)),
            Coordinator::new(),
            (node_id, mode, butler.clone(), blob, Some(connect_tx), Some(event_tx)),
        ).await?;

        // Forward transport events to coordinator
        let coord = coordinator.clone();
        tokio::spawn(async move {
            while let Some(event) = transport_rx.recv().await {
                if let Some(msg) = Coordinator::from_transport_event(event) {
                    let _ = coord.cast(msg);
                }
            }
        });

        // Handle connect requests (transport.connect)
        let transport_for_connect = transport.clone();
        let peer_name = name.to_string();
        tokio::spawn(async move {
            while let Some(req) = connect_rx.recv().await {
                info!("Peer '{}' connecting to {}", peer_name, req.node_id);
                if let Err(e) = transport_for_connect.connect(req.node_id).await {
                    warn!("Peer '{}' failed to connect to {}: {}", peer_name, req.node_id, e);
                }
            }
        });

        info!("Peer '{}' created: node_id={}, mode={:?}", name, node_id, mode);

        Ok(Self {
            name: name.to_string(),
            node_id,
            mode,
            coordinator,
            butler,
            transport,
            events: event_rx,
            _temp_dir: temp_dir,
        })
    }

    /// Connect to another peer (transport level)
    pub async fn connect_to(&self, other: &Peer) -> Result<()> {
        self.transport.connect(other.node_id).await?;
        Ok(())
    }

    /// Initiate handshake with permit
    ///
    /// **Context**: After transport connected, send Connect to start handshake
    pub fn connect_with_permit(&self, node_id: NodeId, permit: String) -> Result<()> {
        self.coordinator.cast(CoordinatorMessage::Connect { node_id, permit })
            .map_err(|e| anyhow::anyhow!("Failed to send Connect: {:?}", e))
    }

    /// Wait for PeerAuthenticated event
    ///
    /// **Returns**: (did, username) of the authenticated peer
    pub async fn wait_for_authenticated(&mut self, timeout: Duration) -> Result<(String, String)> {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            match tokio::time::timeout_at(deadline, self.events.recv()).await {
                Ok(Some(CourierEvent::PeerAuthenticated { node_id, did, username })) => {
                    info!("Peer '{}' authenticated with: {} ({})", self.name, username, node_id);
                    return Ok((did, username));
                }
                Ok(Some(CourierEvent::ConnectionFailed { node_id, error })) => {
                    return Err(anyhow::anyhow!("Connection failed to {}: {}", node_id, error));
                }
                Ok(Some(other)) => {
                    info!("Peer '{}' ignoring event: {:?}", self.name, other);
                    continue;
                }
                Ok(None) => return Err(anyhow::anyhow!("Event channel closed")),
                Err(_) => return Err(anyhow::anyhow!("Timeout waiting for authentication")),
            }
        }
    }

    /// Wait for any CourierEvent (for custom assertions)
    pub async fn wait_for_event(&mut self, timeout: Duration) -> Result<CourierEvent> {
        tokio::time::timeout(timeout, self.events.recv())
            .await
            .map_err(|_| anyhow::anyhow!("Timeout waiting for event"))?
            .ok_or_else(|| anyhow::anyhow!("Event channel closed"))
    }

    /// Get user info (DID, username)
    pub async fn user_info(&self) -> Result<butler::UserInfo> {
        self.butler.user_info().await
    }

    /// Shutdown the peer
    pub async fn shutdown(&self) {
        let _ = self.coordinator.cast(CoordinatorMessage::Shutdown);
    }
}

/// Fixture data for export to Python tests
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PeerFixture {
    pub name: String,
    pub node_id: String,
    pub did: String,
    pub username: String,
    pub mode: String,
}

impl Peer {
    /// Export fixture data for Python tests
    pub async fn to_fixture(&self) -> Result<PeerFixture> {
        let user_info = self.butler.user_info().await?;
        Ok(PeerFixture {
            name: self.name.clone(),
            node_id: self.node_id.to_string(),
            did: user_info.did,
            username: user_info.username,
            mode: match self.mode {
                CourierMode::User => "user".to_string(),
                CourierMode::Node => "node".to_string(),
            },
        })
    }
}
