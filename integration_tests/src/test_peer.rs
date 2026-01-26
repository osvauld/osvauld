//! TestPeer - A simulated peer with its own identity, storage, and real transport

use std::sync::Arc;

use anyhow::Result;
use tokio::sync::{mpsc, RwLock};
use tracing::info;
use transport::{Transport, TransportConfig, NodeId, MockBlobStore};

use butler::{Butler, LayerCache, RedbStore, SyncEvent};
use courier::coordinator::{Coordinator, CoordinatorMessage, CourierMode};
use courier::peer_actor::BlobStore;
use courier::CourierEvent;
use ractor::{Actor, ActorRef};

/// A test peer with its own identity, Transport, Coordinator, and Butler
pub struct TestPeer {
    /// Human-readable name for logging
    pub name: String,
    /// Unique node ID for this peer
    pub node_id: NodeId,
    /// Mode (User or Node)
    pub mode: CourierMode,
    /// The Coordinator actor
    pub coordinator: ActorRef<CoordinatorMessage>,
    /// Butler for storage operations
    pub butler: Arc<Butler>,
    /// Real transport (owns connection to iroh)
    pub transport: Transport,
    /// Event receiver for CourierEvents (for test assertions)
    pub event_rx: mpsc::Receiver<CourierEvent>,
    /// Keep temp dir alive (storage is deleted when this drops)
    _temp_dir: Option<tempfile::TempDir>,
}

impl TestPeer {
    /// Create a new test peer with isolated storage and real transport
    ///
    /// Each peer gets its own RedbStore in a temporary directory
    /// and its own real iroh Transport for loopback connections.
    pub async fn new(
        name: &str,
        mode: CourierMode,
        mock_blob_store: Arc<MockBlobStore>,
    ) -> Result<Self> {
        // Create temp directory for this peer's storage
        let temp_dir = tempfile::tempdir()?;
        let db_path = temp_dir.path().join(format!("{}.redb", name));

        // Create isolated storage
        let store = Arc::new(RedbStore::open(&db_path)?);
        let layer_cache = Arc::new(RwLock::new(LayerCache::new(store.clone(), 100)));
        let assets_path = temp_dir.path().join("assets");
        let asset_store = Arc::new(butler::AssetStore::new(&assets_path).expect("Failed to create asset store"));
        let butler = Arc::new(Butler::new(store, layer_cache, asset_store));

        // Generate random secret key for this peer's transport
        let secret_key: [u8; 32] = rand::random();
        let config = TransportConfig::new(secret_key);

        // Initialize real transport
        let (transport, mut event_rx) = Transport::init(config).await?;
        let node_id = transport.node_id();

        // Create BlobStore from mock blob store for asset transfer
        let blob_store = BlobStore::Mock(mock_blob_store);

        // Spawn Coordinator actor
        let coordinator_actor = Coordinator::new();
        let (coordinator, _handle) = Actor::spawn(
            Some(format!("coordinator-{}-{}", name, node_id)),
            coordinator_actor,
            (node_id, mode, butler.clone(), blob_store, None, None),
        )
        .await?;

        // Spawn task to forward transport events to coordinator
        let coordinator_for_events = coordinator.clone();
        tokio::spawn(async move {
            while let Some(event) = event_rx.recv().await {
                if let Some(msg) = Coordinator::from_transport_event(event) {
                    let _ = coordinator_for_events.cast(msg);
                }
            }
        });

        info!(
            "TestPeer '{}' created: node_id={}, mode={:?}",
            name, node_id, mode
        );

        Ok(Self {
            name: name.to_string(),
            node_id,
            mode,
            coordinator,
            butler,
            transport,
            _temp_dir: Some(temp_dir),
        })
    }

    /// Create a test peer with provided Butler (for custom storage setup)
    pub async fn with_butler(
        name: &str,
        mode: CourierMode,
        butler: Arc<Butler>,
        mock_blob_store: Arc<MockBlobStore>,
    ) -> Result<Self> {
        // Generate random secret key for this peer's transport
        let secret_key: [u8; 32] = rand::random();
        let config = TransportConfig::new(secret_key);

        // Initialize real transport
        let (transport, mut event_rx) = Transport::init(config).await?;

        // Try to derive node_id from butler's device key if identity is set
        let node_id = match butler.get_identity().await {
            Ok(identity) => {
                let device_key = identity.public_device_key();
                NodeId::from_bytes(&device_key).expect("Valid ed25519 device key")
            }
            Err(_) => transport.node_id(),
        };

        // Create BlobStore from mock blob store for asset transfer
        let blob_store = BlobStore::Mock(mock_blob_store);

        // Create connect channel for sync-initiated connections
        let (connect_tx, connect_rx) = tokio::sync::mpsc::channel::<courier::ConnectRequest>(16);

        let coordinator_actor = Coordinator::new();
        let (coordinator, _handle) = Actor::spawn(
            Some(format!("coordinator-{}-{}", name, node_id)),
            coordinator_actor,
            (node_id, mode, butler.clone(), blob_store, Some(connect_tx), None),
        )
        .await?;

        // Forward transport events to coordinator
        let coordinator_for_events = coordinator.clone();
        tokio::spawn(async move {
            while let Some(event) = event_rx.recv().await {
                if let Some(msg) = Coordinator::from_transport_event(event) {
                    let _ = coordinator_for_events.cast(msg);
                }
            }
        });

        // Handle connect requests by connecting via real transport
        let transport_for_connect = transport.node_id();
        let name_for_connect = name.to_string();
        tokio::spawn(async move {
            let mut connect_rx = connect_rx;
            while let Some(req) = connect_rx.recv().await {
                info!(
                    peer = %name_for_connect,
                    target_node_id = %req.node_id,
                    "Test: Connect request (would connect via real transport)"
                );
                // In real test scenarios, we'd call transport.connect() here
                // For now, connections are established via TestHarness.connect()
            }
        });

        // Wire sync events: Butler → Coordinator
        let (sync_event_tx, mut sync_event_rx) = tokio::sync::mpsc::channel::<SyncEvent>(64);
        butler.set_sync_event_tx(sync_event_tx).await;

        let coordinator_for_sync = coordinator.clone();
        let name_for_sync = name.to_string();
        tokio::spawn(async move {
            while let Some(event) = sync_event_rx.recv().await {
                match event {
                    SyncEvent::EnsureSync { user_did } => {
                        if let Err(e) = coordinator_for_sync.cast(CoordinatorMessage::EnsureSync {
                            user_did: user_did.clone(),
                        }) {
                            tracing::warn!(peer = %name_for_sync, error = ?e, user_did = %user_did, "Failed to forward EnsureSync");
                        }
                    }
                }
            }
        });

        info!(
            "TestPeer '{}' created with custom Butler: node_id={}, mode={:?}",
            name, node_id, mode
        );

        Ok(Self {
            name: name.to_string(),
            node_id,
            mode,
            coordinator,
            butler,
            transport,
            _temp_dir: None,
        })
    }

    /// Connect to another peer via real transport
    pub async fn connect_to(&self, other: &TestPeer) -> Result<()> {
        self.transport.connect(other.node_id).await?;
        Ok(())
    }

    /// Shutdown the peer's coordinator
    pub async fn shutdown(&self) {
        let _ = self.coordinator.cast(CoordinatorMessage::Shutdown);
    }
}
