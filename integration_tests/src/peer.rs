//! Peer — single participant using MockConnection only
//!
//! No generics, no IrohConnection support. Fast, in-memory protocol testing.

use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use tokio::sync::{mpsc, RwLock};
use tracing::info;
use transport::{mock_connection_pair, node_id_from_secret, MockBlobStore, MockConnection, NodeId};

use butler::{signup, Butler, LayerCache, RedbStore};
use courier::coordinator::{Coordinator, CoordinatorMessage, CourierMode};
use courier::peer_actor::BlobStore;
use courier::trace::MessageTrace;
use courier::CourierEvent;
use ractor::{Actor, ActorRef};

/// Mock-only test peer
pub struct Peer {
    pub name: String,
    pub node_id: NodeId,
    pub mode: CourierMode,
    pub coordinator: ActorRef<CoordinatorMessage<MockConnection>>,
    pub butler: Arc<Butler>,
    pub events: mpsc::Receiver<CourierEvent>,
    _temp_dir: tempfile::TempDir,
}

impl Peer {
    /// Create a new mock peer
    ///
    /// Pass `message_tx` from `Tracer::new()` to enable protocol tracing,
    /// or `None` for tests that don't need it.
    pub async fn new(
        name: &str,
        mode: CourierMode,
        blobs: Arc<MockBlobStore>,
        message_tx: Option<mpsc::UnboundedSender<MessageTrace>>,
    ) -> Result<Self> {
        let temp_dir = tempfile::tempdir()?;
        let db_path = temp_dir.path().join(format!("{}.redb", name));

        // Storage
        let store = Arc::new(RedbStore::open(&db_path)?);
        let layer_cache = Arc::new(RwLock::new(LayerCache::new(store.clone(), 100)));
        let assets_path = temp_dir.path().join("assets");
        let asset_store = Arc::new(butler::AssetStore::new(&assets_path)?);

        // Wire sync_event channel so Scribe -> EnsureSync -> Coordinator works
        let (sync_tx, mut sync_rx) = mpsc::channel::<butler::SyncEvent>(100);
        let butler = Arc::new(Butler::new(
            store.clone(),
            layer_cache,
            asset_store,
            Some(sync_tx),
        ));

        // Create identity
        let signup_result = signup(&store, name, "password")?;
        butler.set_identity(signup_result.identity).await;

        // Derive node_id from device key
        let device_key = butler.device_key().await?;
        let node_id = node_id_from_secret(&device_key);

        // Event channel
        let (event_tx, event_rx) = mpsc::channel::<CourierEvent>(100);

        // Connect request channel (unused for mock — we inject connections directly)
        let (connect_tx, _connect_rx) = mpsc::channel::<courier::ConnectRequest>(16);

        // Coordinator
        let blob = BlobStore::Mock(blobs);
        let (coordinator, _) = Actor::spawn(
            Some(format!("coordinator-{}-{}", name, node_id)),
            Coordinator::<MockConnection>::new(),
            (
                node_id,
                mode,
                butler.clone(),
                blob,
                Some(connect_tx),
                Some(event_tx),
                message_tx,
                None,
            ),
        )
        .await?;

        // Forward EnsureSync from Scribe to Coordinator
        let coord = coordinator.clone();
        tokio::spawn(async move {
            while let Some(event) = sync_rx.recv().await {
                match event {
                    butler::SyncEvent::EnsureSync { user_did } => {
                        let _ = coord.cast(CoordinatorMessage::EnsureSync { user_did });
                    }
                    butler::SyncEvent::SubscribeLayers {
                        page_id,
                        creator_did,
                        layers,
                    } => {
                        let _ = coord.cast(CoordinatorMessage::SubscribeLayers {
                            page_id,
                            creator_did,
                            layers,
                        });
                    }
                }
            }
        });

        info!(
            "Peer '{}' created: node_id={}, mode={:?}",
            name, node_id, mode
        );

        Ok(Self {
            name: name.to_string(),
            node_id,
            mode,
            coordinator,
            butler,
            events: event_rx,
            _temp_dir: temp_dir,
        })
    }

    /// Connect to another mock peer using in-memory channels
    pub fn connect_to(&self, other: &Peer) -> Result<()> {
        let (conn_to_other, conn_to_self) = mock_connection_pair(self.node_id, other.node_id);

        self.coordinator
            .cast(CoordinatorMessage::Connected {
                node_id: other.node_id,
                conn: conn_to_other,
            })
            .map_err(|e| anyhow::anyhow!("Failed to inject connection: {:?}", e))?;

        other
            .coordinator
            .cast(CoordinatorMessage::Connected {
                node_id: self.node_id,
                conn: conn_to_self,
            })
            .map_err(|e| anyhow::anyhow!("Failed to inject connection: {:?}", e))?;

        info!("Mock connection: {} <-> {}", self.name, other.name);
        Ok(())
    }

    /// Initiate handshake with a permit
    pub fn handshake(&self, node_id: NodeId, permit: String) -> Result<()> {
        self.coordinator
            .cast(CoordinatorMessage::Connect { node_id, permit })
            .map_err(|e| anyhow::anyhow!("Failed to send Connect: {:?}", e))
    }

    /// Wait for PeerAuthenticated event, returns (did, username)
    pub async fn wait_authenticated(&mut self, timeout: Duration) -> Result<(String, String)> {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            match tokio::time::timeout_at(deadline, self.events.recv()).await {
                Ok(Some(CourierEvent::PeerAuthenticated { did, username, .. })) => {
                    info!("Peer '{}' authenticated with: {}", self.name, username);
                    return Ok((did, username));
                }
                Ok(Some(CourierEvent::ConnectionFailed { node_id, error })) => {
                    return Err(anyhow::anyhow!(
                        "Connection failed to {}: {}",
                        node_id,
                        error
                    ));
                }
                Ok(Some(_)) => continue,
                Ok(None) => return Err(anyhow::anyhow!("Event channel closed")),
                Err(_) => return Err(anyhow::anyhow!("Timeout waiting for authentication")),
            }
        }
    }

    /// Wait for any CourierEvent
    pub async fn wait_event(&mut self, timeout: Duration) -> Result<CourierEvent> {
        tokio::time::timeout(timeout, self.events.recv())
            .await
            .map_err(|_| anyhow::anyhow!("Timeout waiting for event"))?
            .ok_or_else(|| anyhow::anyhow!("Event channel closed"))
    }

    /// Wait for a PermitUpdated event matching a page with version >= min_version
    ///
    /// **Context**: Tests need deterministic waiting for permit distribution.
    /// Drains the event channel looking for `PermitUpdated` where
    /// `page_id == expected_page_id && version >= min_version`.
    pub async fn wait_permit_updated(
        &mut self,
        expected_page_id: &str,
        min_version: u64,
        timeout: Duration,
    ) -> Result<u64> {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            match tokio::time::timeout_at(deadline, self.events.recv()).await {
                Ok(Some(CourierEvent::PermitUpdated { page_id, version }))
                    if page_id == expected_page_id && version >= min_version =>
                {
                    return Ok(version);
                }
                Ok(Some(_)) => continue,
                Ok(None) => return Err(anyhow::anyhow!("Event channel closed")),
                Err(_) => {
                    return Err(anyhow::anyhow!(
                        "Timeout waiting for PermitUpdated (page={}, min_version={})",
                        expected_page_id,
                        min_version
                    ))
                }
            }
        }
    }

    /// Simulate disconnect from another peer
    ///
    /// **Context**: Injects `CoordinatorMessage::Disconnected` into both coordinators.
    /// This triggers PeerActor cleanup, Scribe unsubscription, and event emission.
    /// Butler state (permits, pages, layers) persists for reconnection.
    pub fn disconnect_from(&self, other: &Peer) -> Result<()> {
        self.coordinator
            .cast(CoordinatorMessage::Disconnected {
                node_id: other.node_id,
            })
            .map_err(|e| anyhow::anyhow!("Failed to inject disconnect: {:?}", e))?;

        other
            .coordinator
            .cast(CoordinatorMessage::Disconnected {
                node_id: self.node_id,
            })
            .map_err(|e| anyhow::anyhow!("Failed to inject disconnect: {:?}", e))?;

        info!("Mock disconnect: {} <-> {}", self.name, other.name);
        Ok(())
    }

    /// Shutdown the peer
    pub async fn shutdown(&self) {
        let _ = self.coordinator.cast(CoordinatorMessage::Shutdown);
    }
}

impl Drop for Peer {
    fn drop(&mut self) {
        let _ = self.coordinator.cast(CoordinatorMessage::Shutdown);
    }
}
