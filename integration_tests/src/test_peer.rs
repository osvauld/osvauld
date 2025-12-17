//! TestPeer - A simulated peer with its own identity, storage, and actors

use std::sync::Arc;

use anyhow::Result;
use tokio::sync::RwLock;
use tracing::{info, warn};
use transport::NodeId;

use butler::{Butler, LayerCache, RedbStore, SyncEvent};
use courier::coordinator::{Coordinator, CoordinatorMessage, CourierMode};
use courier::Message;
use ractor::{Actor, ActorRef};

use crate::mock_transport::MockTransport;

/// A test peer with its own identity, Coordinator, and Butler
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
    /// Keep temp dir alive (storage is deleted when this drops)
    _temp_dir: Option<tempfile::TempDir>,
}

impl TestPeer {
    /// Create a new test peer with isolated storage
    ///
    /// Each peer gets its own RedbStore in a temporary directory.
    pub async fn new(
        name: &str,
        mode: CourierMode,
        mock_transport: &Arc<MockTransport>,
    ) -> Result<Self> {
        // Create temp directory for this peer's storage
        let temp_dir = tempfile::tempdir()?;
        let db_path = temp_dir.path().join(format!("{}.redb", name));

        // Create isolated storage
        let store = Arc::new(RedbStore::open(&db_path)?);
        let layer_cache = Arc::new(RwLock::new(LayerCache::new(store.clone(), 100)));
        let butler = Arc::new(Butler::new(store, layer_cache));

        // Generate deterministic NodeId from name
        let node_id = Self::node_id_from_name(name);

        // Spawn Coordinator actor with unique name (include node_id to avoid collisions)
        let coordinator_actor = Coordinator::new();
        let (coordinator, _handle) = Actor::spawn(
            Some(format!("coordinator-{}-{}", name, node_id)),
            coordinator_actor,
            (node_id, mode, butler.clone(), None),
        )
        .await?;

        // Register with MockTransport
        mock_transport
            .register_peer(node_id, coordinator.clone())
            .await;

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
            _temp_dir: Some(temp_dir),
        })
    }

    /// Create a test peer with provided Butler (for custom storage setup)
    ///
    /// NOTE: When a butler has an identity set, we derive node_id from the
    /// identity's device key to match what generate_connection_string() produces.
    /// This ensures the node_id used in the test matches the node_id that
    /// add_sovereign_node() will derive from the connection string.
    ///
    /// Also wires up sync events to mirror production flow:
    /// Scribe → SyncEvent → Coordinator (EnsureSync)
    pub async fn with_butler(
        name: &str,
        mode: CourierMode,
        butler: Arc<Butler>,
        mock_transport: &Arc<MockTransport>,
    ) -> Result<Self> {
        // Try to derive node_id from butler's device key if identity is set
        // This ensures consistency with generate_connection_string()
        let node_id = match butler.get_identity().await {
            Ok(identity) => {
                // Use the device key as node_id (same as connection string derivation)
                let device_key = identity.public_device_key();
                NodeId::from_bytes(&device_key).expect("Valid ed25519 device key")
            }
            Err(_) => {
                // No identity yet, fall back to random node_id
                Self::node_id_from_name(name)
            }
        };

        let coordinator_actor = Coordinator::new();
        let (coordinator, _handle) = Actor::spawn(
            Some(format!("coordinator-{}-{}", name, node_id)),
            coordinator_actor,
            (node_id, mode, butler.clone(), None),
        )
        .await?;

        // Wire sync events: Butler → Coordinator (mirrors production flow)
        // This allows Scribe actors to trigger sync when updates happen
        let (sync_event_tx, mut sync_event_rx) = tokio::sync::mpsc::channel::<SyncEvent>(64);
        butler.set_sync_event_tx(sync_event_tx).await;

        // Spawn sync event bridge - forwards SyncEvent to Coordinator
        let coordinator_for_sync = coordinator.clone();
        let name_for_sync = name.to_string();
        tokio::spawn(async move {
            while let Some(event) = sync_event_rx.recv().await {
                match event {
                    SyncEvent::EnsureSync { user_did } => {
                        if let Err(e) = coordinator_for_sync.cast(CoordinatorMessage::EnsureSync {
                            user_did: user_did.clone(),
                        }) {
                            warn!(peer = %name_for_sync, error = ?e, user_did = %user_did, "Failed to forward EnsureSync");
                        }
                    }
                }
            }
        });

        mock_transport
            .register_peer(node_id, coordinator.clone())
            .await;

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
            _temp_dir: None,
        })
    }

    /// Generate a unique NodeId
    ///
    /// Creates a real ed25519 keypair and extracts the public key.
    fn node_id_from_name(_name: &str) -> NodeId {
        use ed25519_dalek::SigningKey;
        use rand::rngs::OsRng;

        // Generate a random ed25519 signing key
        let signing_key = SigningKey::generate(&mut OsRng);
        let public_key = signing_key.verifying_key();

        // NodeId is just the 32-byte public key
        NodeId::from_bytes(public_key.as_bytes()).expect("Valid ed25519 public key")
    }

    /// Send a message to another peer via MockTransport
    pub async fn send_to(
        &self,
        to: &TestPeer,
        msg: Message,
        mock_transport: &Arc<MockTransport>,
    ) -> Result<()> {
        let handle = mock_transport
            .get_connection(self.node_id, to.node_id)
            .await
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "No connection from {} ({}) to {} ({})",
                    self.name,
                    self.node_id,
                    to.name,
                    to.node_id
                )
            })?;

        // Serialize the message and send as bytes
        let bytes = msg.to_bytes()?;
        handle.send_bytes(&bytes).await
    }

    /// Shutdown the peer's coordinator
    pub async fn shutdown(&self) {
        let _ = self.coordinator.cast(CoordinatorMessage::Shutdown);
    }
}
