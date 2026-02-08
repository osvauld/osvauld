//! Peer - Single participant in P2P network
//!
//! Generic over `C: Connection` to support both real network tests (IrohConnection)
//! and fast protocol tests (MockConnection).
//!
//! ## Usage
//!
//! ```ignore
//! // Real network tests (requires relay, slower)
//! let owner = IrohPeer::new("owner", CourierMode::User, blobs).await?;
//!
//! // Mock protocol tests (in-memory, fast)
//! let owner = MockPeer::new_mock("owner", CourierMode::User, blobs).await?;
//! let node = MockPeer::new_mock("node", CourierMode::Node, blobs).await?;
//! owner.connect_mock_to(&node)?;
//! ```

use std::marker::PhantomData;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use tokio::sync::{mpsc, RwLock};
use tracing::{info, warn};
use transport::{Connection, MockBlobStore, MockConnection, NodeId, Transport, TransportConfig, TransportEvent, mock_connection_pair, mock_connection_pair_named, node_id_from_secret, DatagramCallback};

use crate::tracing::{MessageTracer, Direction};

use butler::{Butler, LayerCache, RedbStore, signup};
use courier::coordinator::{Coordinator, CoordinatorMessage, CourierMode};
use courier::peer_actor::BlobStore;
use courier::CourierEvent;
use ractor::{Actor, ActorRef};

// Type Aliases

/// Type alias for production coordinator message (uses IrohConnection)
pub type IrohCoordinatorMessage = CoordinatorMessage<transport::IrohConnection>;

/// Type alias for mock coordinator message (uses MockConnection)
pub type MockCoordinatorMessage = CoordinatorMessage<MockConnection>;

/// Peer using real Iroh transport (production, slower, requires relay)
pub type IrohPeer = Peer<transport::IrohConnection>;

/// Peer using mock transport (tests, fast, in-memory)
pub type MockPeer = Peer<MockConnection>;

// Peer<C: Connection>

/// Test peer with P2P capabilities
///
/// Generic over `C: Connection` to support:
/// - `Peer<IrohConnection>` - Real network with NAT traversal
/// - `Peer<MockConnection>` - In-memory channels for fast tests
pub struct Peer<C: Connection> {
    /// Human-readable name
    pub name: String,
    /// Unique node ID
    pub node_id: NodeId,
    /// Mode: User or Node
    pub mode: CourierMode,
    /// Coordinator actor reference
    pub coordinator: ActorRef<CoordinatorMessage<C>>,
    /// Butler for storage/crypto operations
    pub butler: Arc<Butler>,
    /// Event receiver for test assertions
    pub events: mpsc::Receiver<CourierEvent>,
    /// Transport (only for IrohConnection peers)
    transport: Option<Arc<Transport>>,
    /// Keep temp dir alive
    _temp_dir: tempfile::TempDir,
    /// PhantomData for generic
    _phantom: PhantomData<C>,
}

impl Peer<transport::IrohConnection> {
    /// Create a new peer with real Iroh transport
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
        let butler = Arc::new(Butler::new(store.clone(), layer_cache, asset_store, None));

        // Create identity
        let signup_result = signup(&store, name, "password")?;
        butler.set_identity(signup_result.identity).await;

        // Transport - use Butler's device key for consistent node_id
        let device_key = butler.device_key().await?;
        let (transport, mut transport_rx) = Transport::init(TransportConfig::new(device_key)).await?;
        let node_id = transport.node_id();
        let transport = Arc::new(transport);

        // Event channel (for test assertions)
        let (event_tx, event_rx) = mpsc::channel::<CourierEvent>(100);

        // Connect request channel (Coordinator -> transport)
        let (connect_tx, mut connect_rx) = mpsc::channel::<courier::ConnectRequest>(16);

        // Coordinator
        let blob = BlobStore::Mock(blob_store);
        let (coordinator, _) = Actor::spawn(
            Some(format!("coordinator-{}-{}", name, node_id)),
            Coordinator::<transport::IrohConnection>::new(),
            (node_id, mode, butler.clone(), blob, Some(connect_tx), Some(event_tx)),
        ).await?;

        // Forward transport events to coordinator
        let coord = coordinator.clone();
        tokio::spawn(async move {
            while let Some(event) = transport_rx.recv().await {
                let msg = match event {
                    TransportEvent::Connected { node_id, conn } => {
                        CoordinatorMessage::Connected { node_id, conn: conn.into() }
                    }
                    TransportEvent::Disconnected { node_id } => {
                        CoordinatorMessage::Disconnected { node_id }
                    }
                };
                let _ = coord.cast(msg);
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
            events: event_rx,
            transport: Some(transport),
            _temp_dir: temp_dir,
            _phantom: PhantomData,
        })
    }

    /// Connect to another peer (transport level)
    ///
    /// Only available for IrohConnection peers.
    pub async fn connect_to(&self, other: &Peer<transport::IrohConnection>) -> Result<()> {
        let transport = self.transport.as_ref()
            .ok_or_else(|| anyhow::anyhow!("No transport available"))?;
        transport.connect(other.node_id).await?;
        Ok(())
    }

    /// Get the transport (for advanced operations)
    pub fn transport(&self) -> Option<&Arc<Transport>> {
        self.transport.as_ref()
    }
}

impl Peer<MockConnection> {
    /// Create a new mock peer for protocol testing
    ///
    /// **Context**: Fast tests without real networking
    /// **Note**: Use `connect_mock_to` to establish mock connections
    pub async fn new_mock(name: &str, mode: CourierMode, blob_store: Arc<MockBlobStore>) -> Result<Self> {
        let temp_dir = tempfile::tempdir()?;
        let db_path = temp_dir.path().join(format!("{}.redb", name));

        // Storage
        let store = Arc::new(RedbStore::open(&db_path)?);
        let layer_cache = Arc::new(RwLock::new(LayerCache::new(store.clone(), 100)));
        let assets_path = temp_dir.path().join("assets");
        let asset_store = Arc::new(butler::AssetStore::new(&assets_path)?);
        // Wire sync_event channel so Scribe → EnsureSync → Coordinator works
        let (sync_tx, mut sync_rx) = mpsc::channel::<butler::SyncEvent>(100);
        let butler = Arc::new(Butler::new(store.clone(), layer_cache, asset_store, Some(sync_tx)));

        // Create identity
        let signup_result = signup(&store, name, "password")?;
        butler.set_identity(signup_result.identity).await;

        // Use Butler's device key for node_id (same as connection strings use)
        let device_key = butler.device_key().await?;
        let node_id = node_id_from_secret(&device_key);

        // Event channel (for test assertions)
        let (event_tx, event_rx) = mpsc::channel::<CourierEvent>(100);

        // No connect request channel needed for mock - we inject connections directly
        let (connect_tx, _connect_rx) = mpsc::channel::<courier::ConnectRequest>(16);

        // Coordinator
        let blob = BlobStore::Mock(blob_store);
        let (coordinator, _) = Actor::spawn(
            Some(format!("coordinator-mock-{}-{}", name, node_id)),
            Coordinator::<MockConnection>::new(),
            (node_id, mode, butler.clone(), blob, Some(connect_tx), Some(event_tx)),
        ).await?;

        // Forward EnsureSync from Scribe to Coordinator (mirrors production wiring)
        let coordinator_for_sync = coordinator.clone();
        tokio::spawn(async move {
            while let Some(event) = sync_rx.recv().await {
                match event {
                    butler::SyncEvent::EnsureSync { user_did } => {
                        let _ = coordinator_for_sync.cast(
                            courier::coordinator::CoordinatorMessage::EnsureSync { user_did }
                        );
                    }
                }
            }
        });

        info!("MockPeer '{}' created: node_id={}, mode={:?}", name, node_id, mode);

        Ok(Self {
            name: name.to_string(),
            node_id,
            mode,
            coordinator,
            butler,
            events: event_rx,
            transport: None,
            _temp_dir: temp_dir,
            _phantom: PhantomData,
        })
    }

    /// Connect to another mock peer using in-memory channels
    ///
    /// **Context**: Creates mock connection pair, injects Connected events to both coordinators
    /// **Note**: Both sides get Connected events, enabling bidirectional communication
    pub fn connect_mock_to(&self, other: &Peer<MockConnection>) -> Result<()> {
        // Create mock connection pair
        let (conn_to_other, conn_to_self) = mock_connection_pair(self.node_id, other.node_id);

        // Inject Connected event to self (we see other as peer)
        self.coordinator.cast(CoordinatorMessage::Connected {
            node_id: other.node_id,
            conn: conn_to_other,
        }).map_err(|e| anyhow::anyhow!("Failed to inject connection to self: {:?}", e))?;

        // Inject Connected event to other (they see us as peer)
        other.coordinator.cast(CoordinatorMessage::Connected {
            node_id: self.node_id,
            conn: conn_to_self,
        }).map_err(|e| anyhow::anyhow!("Failed to inject connection to other: {:?}", e))?;

        info!("Mock connection established: {} <-> {}", self.name, other.name);
        Ok(())
    }

    /// Connect to another mock peer with ephemeral datagram capture
    ///
    /// **Context**: Same as connect_mock_to, but captures all datagrams to the tracer
    /// **Note**: Captured datagrams can be inspected via tracer.ephemerals()
    pub fn connect_mock_to_with_tracer(
        &self,
        other: &Peer<MockConnection>,
        tracer: &MessageTracer,
    ) -> Result<()> {
        // Create capture callbacks for both sides
        let tracer_self = tracer.clone();
        let self_name = self.name.clone();
        let callback_self: DatagramCallback = Arc::new(move |data: &[u8], is_send: bool| {
            // Parse the ephemeral datagram to extract page_id
            if let Ok(datagram) = courier::EphemeralDatagram::from_bytes(data) {
                if is_send {
                    tracer_self.trace_ephemeral_sent(&self_name, &datagram.page_id, &datagram.payload);
                } else {
                    tracer_self.trace_ephemeral_received(&self_name, &datagram.page_id, &datagram.payload);
                }
            }
        });

        let tracer_other = tracer.clone();
        let other_name = other.name.clone();
        let callback_other: DatagramCallback = Arc::new(move |data: &[u8], is_send: bool| {
            if let Ok(datagram) = courier::EphemeralDatagram::from_bytes(data) {
                if is_send {
                    tracer_other.trace_ephemeral_sent(&other_name, &datagram.page_id, &datagram.payload);
                } else {
                    tracer_other.trace_ephemeral_received(&other_name, &datagram.page_id, &datagram.payload);
                }
            }
        });

        // Create mock connection pair with capture
        let (conn_to_other, conn_to_self) = mock_connection_pair_named(
            self.node_id,
            other.node_id,
            self.name.clone(),
            other.name.clone(),
            Some(callback_self),
            Some(callback_other),
        );

        // Inject Connected event to self (we see other as peer)
        self.coordinator.cast(CoordinatorMessage::Connected {
            node_id: other.node_id,
            conn: conn_to_other,
        }).map_err(|e| anyhow::anyhow!("Failed to inject connection to self: {:?}", e))?;

        // Inject Connected event to other (they see us as peer)
        other.coordinator.cast(CoordinatorMessage::Connected {
            node_id: self.node_id,
            conn: conn_to_self,
        }).map_err(|e| anyhow::anyhow!("Failed to inject connection to other: {:?}", e))?;

        info!("Mock connection established with tracer: {} <-> {}", self.name, other.name);
        Ok(())
    }
}

// Common Methods (for all Connection types)

impl<C: Connection> Peer<C> {
    /// Initiate handshake with permit
    ///
    /// **Context**: After transport connected, send Connect to start handshake
    pub fn handshake(&self, node_id: NodeId, permit: String) -> Result<()> {
        self.coordinator.cast(CoordinatorMessage::Connect { node_id, permit })
            .map_err(|e| anyhow::anyhow!("Failed to send Connect: {:?}", e))
    }

    /// Alias for handshake (backward compatibility)
    pub fn connect_with_permit(&self, node_id: NodeId, permit: String) -> Result<()> {
        self.handshake(node_id, permit)
    }

    /// Wait for PeerAuthenticated event
    ///
    /// **Returns**: (did, username) of the authenticated peer
    pub async fn wait_authenticated(&mut self, timeout: Duration) -> Result<(String, String)> {
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

    /// Alias for wait_authenticated (backward compatibility)
    pub async fn wait_for_authenticated(&mut self, timeout: Duration) -> Result<(String, String)> {
        self.wait_authenticated(timeout).await
    }

    /// Wait for any CourierEvent (for custom assertions)
    pub async fn wait_event(&mut self, timeout: Duration) -> Result<CourierEvent> {
        tokio::time::timeout(timeout, self.events.recv())
            .await
            .map_err(|_| anyhow::anyhow!("Timeout waiting for event"))?
            .ok_or_else(|| anyhow::anyhow!("Event channel closed"))
    }

    /// Wait for event matching a filter
    pub async fn wait_event_matching<F>(&mut self, timeout: Duration, filter: F) -> Result<CourierEvent>
    where
        F: Fn(&CourierEvent) -> bool,
    {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            match tokio::time::timeout_at(deadline, self.events.recv()).await {
                Ok(Some(event)) if filter(&event) => {
                    return Ok(event);
                }
                Ok(Some(_)) => continue,
                Ok(None) => return Err(anyhow::anyhow!("Event channel closed")),
                Err(_) => return Err(anyhow::anyhow!("Timeout waiting for matching event")),
            }
        }
    }

    /// Alias for wait_event (backward compatibility)
    pub async fn wait_for_event(&mut self, timeout: Duration) -> Result<CourierEvent> {
        self.wait_event(timeout).await
    }

    /// Get user info (DID, username)
    pub async fn user_info(&self) -> Result<butler::UserInfo> {
        self.butler.user_info().await
            .map_err(|e| anyhow::anyhow!("Failed to get user info: {}", e))
    }

    /// Shutdown the peer
    pub async fn shutdown(&self) {
        let _ = self.coordinator.cast(CoordinatorMessage::Shutdown);
    }
}

impl<C: Connection> Drop for Peer<C> {
    fn drop(&mut self) {
        // Send shutdown signal (non-blocking)
        let _ = self.coordinator.cast(CoordinatorMessage::Shutdown);
    }
}

// Fixture Data

/// Fixture data for export to Python tests
#[derive(Debug, Clone)]
pub struct PeerFixture {
    pub name: String,
    pub node_id: String,
    pub did: String,
    pub username: String,
    pub mode: String,
}

impl<C: Connection> Peer<C> {
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
