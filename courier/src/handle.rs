//! Handle module - Provides high-level API for tauri_handlers
//!
//! This module provides compatibility types that wrap the actor-based internals
//! and expose a simpler async API for the Tauri handlers.

use std::sync::Arc;
use std::time::Duration;

use ractor::{Actor, ActorRef};
use tokio::sync::{mpsc, oneshot};
use tracing::{error, info, warn};
use transport::{NodeId, Transport, TransportEvent};

use crate::coordinator::{Coordinator, CoordinatorMessage, CourierMode};
use butler::Butler;

/// Event emitted by Courier to the application
#[derive(Debug, Clone)]
pub enum CourierEvent {
    /// Peer authenticated successfully
    PeerAuthenticated {
        node_id: String,
        did: String,
        username: String,
    },
    /// Peer disconnected
    PeerDisconnected { node_id: String },
    /// Space published successfully
    SpacePublished { node_id: String, space_id: String },
    /// Publish failed
    PublishFailed {
        node_id: String,
        space_id: String,
        error: String,
    },
    /// Viewer sync completed
    ViewerSyncComplete {
        node_id: String,
        space_id: String,
        pages_synced: usize,
    },
    /// Sync consent handshake completed (viewer received ack from node)
    SyncConsentComplete {
        node_id: String,
        space_id: String,
    },
    /// Shareable link received from node
    ShareableLinkReceived {
        node_id: String,
        space_id: String,
        permit: String,
    },
    /// Connection requested - app should connect via transport
    ///
    /// **Context**: ConnectAndAuth was called, transport needs to connect
    ConnectRequested {
        node_id: String,
        permit: String,
    },
    /// Viewer received a space from node (emitted before pages stream)
    ViewerSpaceReceived {
        node_id: String,
        space: butler::Space,
        page_count: usize,
    },
    /// Viewer received a page from node
    PageReceived {
        node_id: String,
        page: butler::Page,
        is_last: bool,
    },
}

/// Handle for interacting with Courier
///
/// Provides async methods for P2P operations.
#[derive(Clone)]
pub struct CourierHandle {
    coordinator: ActorRef<CoordinatorMessage>,
    transport: Arc<Transport>,
}

impl CourierHandle {
    /// Create a new CourierHandle
    pub fn new(coordinator: ActorRef<CoordinatorMessage>, transport: Arc<Transport>) -> Self {
        Self {
            coordinator,
            transport,
        }
    }

    /// Connect to a node and initiate handshake
    ///
    /// **Flow**:
    /// 1. Mark outbound connection (so Coordinator knows to send Hello first)
    /// 2. Connect via transport
    /// 3. Coordinator's on_connected will lookup permit from SovereignNode and initiate handshake
    pub async fn connect(&self, node_id: &str) -> Result<(), String> {
        // Parse node_id to NodeId
        let node_id: NodeId = node_id
            .parse()
            .map_err(|e| format!("Invalid node_id: {}", e))?;

        // Mark as outbound BEFORE connecting so Coordinator knows to send Hello
        self.coordinator
            .cast(CoordinatorMessage::OutboundConnection { node_id })
            .map_err(|e| format!("Failed to send OutboundConnection: {:?}", e))?;

        // Connect via transport (triggers TransportEvent::Connected)
        // Coordinator's on_connected will auto-initiate handshake for outbound connections
        self.transport
            .connect(node_id)
            .await
            .map_err(|e| format!("Failed to connect: {}", e))?;

        Ok(())
    }

    /// Reconnect to a node (same as connect - permit is looked up from storage)
    pub async fn reconnect(&self, node_id: &str) -> Result<(), String> {
        self.connect(node_id).await
    }

    /// Publish a space to a node
    pub async fn publish_space(&self, space_id: &str, node_id: &str) -> Result<(), String> {
        let node_id: NodeId = node_id
            .parse()
            .map_err(|e| format!("Invalid node_id: {}", e))?;

        self.coordinator
            .cast(CoordinatorMessage::PublishSpace {
                node_id,
                space_id: space_id.to_string(),
            })
            .map_err(|e| format!("Failed to send PublishSpace: {:?}", e))?;

        Ok(())
    }

    /// Get shareable link for a space
    ///
    /// **Context**: Request node to generate viewer permit with aud:* (wildcard audience)
    /// **Returns**: Connection string containing node_id and viewer permit
    pub async fn get_shareable_link(&self, space_id: &str, node_id: &str) -> Result<String, String> {
        let node_id: NodeId = node_id
            .parse()
            .map_err(|e| format!("Invalid node_id: {}", e))?;

        let (tx, rx) = oneshot::channel();

        self.coordinator
            .cast(CoordinatorMessage::GetShareableLink {
                node_id,
                space_id: space_id.to_string(),
                response: Some(tx),
            })
            .map_err(|e| format!("Failed to send GetShareableLink: {:?}", e))?;

        // Wait for response with timeout
        match tokio::time::timeout(Duration::from_secs(10), rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err("Response channel closed".to_string()),
            Err(_) => Err("Timeout waiting for shareable link".to_string()),
        }
    }

    /// Request space as viewer
    pub async fn request_space_as_viewer(
        &self,
        space_id: &str,
        node_id: &str,
        viewer_permit: &str,
    ) -> Result<(), String> {
        let node_id: NodeId = node_id
            .parse()
            .map_err(|e| format!("Invalid node_id: {}", e))?;

        self.coordinator
            .cast(CoordinatorMessage::RequestSpace {
                node_id,
                space_id: space_id.to_string(),
                viewer_permit: viewer_permit.to_string(),
            })
            .map_err(|e| format!("Failed to send RequestSpace: {:?}", e))?;

        Ok(())
    }

    /// Connect and wait for authentication to complete
    ///
    /// **Context**: Caller wants to connect to a node and wait for handshake to complete
    /// **Returns**: Ok(NodeId) when auth completes, Err(String) on failure/timeout
    ///
    /// This replaces the old EnsureSubscription pattern - caller waits for auth,
    /// then performs post-auth actions (request space, subscribe to pages, etc.)
    pub async fn connect_and_wait_for_auth(
        &self,
        node_id: &str,
        permit: &str,
    ) -> Result<NodeId, String> {
        let node_id: NodeId = node_id
            .parse()
            .map_err(|e| format!("Invalid node_id: {}", e))?;

        let (tx, rx) = oneshot::channel();

        // 1. Register auth waiter with coordinator
        self.coordinator
            .cast(CoordinatorMessage::ConnectAndAuth {
                node_id,
                permit: permit.to_string(),
                response: tx,
            })
            .map_err(|e| format!("Failed to send ConnectAndAuth: {:?}", e))?;

        // 2. Mark as outbound connection so coordinator knows to initiate handshake
        self.coordinator
            .cast(CoordinatorMessage::OutboundConnection { node_id })
            .map_err(|e| format!("Failed to send OutboundConnection: {:?}", e))?;

        // 3. Actually connect via transport (triggers TransportEvent::Connected)
        self.transport
            .connect(node_id)
            .await
            .map_err(|e| format!("Failed to connect: {}", e))?;

        // 4. Wait for auth to complete
        rx.await.map_err(|_| "Auth channel closed".to_string())?
    }

    /// Get a reference to the coordinator (for tests)
    pub fn coordinator(&self) -> &ActorRef<CoordinatorMessage> {
        &self.coordinator
    }

    /// Ensure sync with a user (forward SyncEvent::EnsureSync to Coordinator)
    ///
    /// **Context**: Scribe emitted EnsureSync - forward to Coordinator
    /// **Coordinator will**: Resolve user_did → device, connect if needed, PeerActor subscribes
    pub fn ensure_sync(&self, user_did: &str) -> Result<(), String> {
        self.coordinator
            .cast(CoordinatorMessage::EnsureSync {
                user_did: user_did.to_string(),
            })
            .map_err(|e| format!("Failed to send EnsureSync: {:?}", e))
    }

    /// Broadcast datagram to all authenticated peers
    ///
    /// **Context**: Send ephemeral data (cursor, typing) to all connected users
    /// **Coordinator will**: Send datagram to each authenticated peer's connection
    pub fn broadcast_datagram(&self, data: Vec<u8>) -> Result<(), String> {
        self.coordinator
            .cast(CoordinatorMessage::BroadcastDatagram { data })
            .map_err(|e| format!("Failed to broadcast datagram: {:?}", e))
    }
}

/// Handshake services - wraps Butler for handshake operations
///
/// In the actor architecture, Butler is passed directly to the Coordinator.
/// This type exists for API compatibility.
pub struct HandshakeServices {
    pub butler: Arc<Butler>,
}

impl HandshakeServices {
    pub fn new(butler: Arc<Butler>) -> Self {
        Self { butler }
    }
}

/// Courier initialization helper
///
/// Provides a simpler initialization API that matches what tauri_handlers expects.
pub struct Courier;

impl Courier {
    /// Initialize Courier with services
    ///
    /// Returns a handle, event receiver, and the runner that must be spawned.
    pub fn init_with_services(
        mode: CourierMode,
        transport: Arc<Transport>,
        services: Option<Arc<HandshakeServices>>,
    ) -> (CourierHandle, mpsc::Receiver<CourierEvent>, CourierRunner) {
        let butler = services
            .map(|s| s.butler.clone())
            .expect("HandshakeServices required");

        let (event_tx, event_rx) = mpsc::channel(100);
        let (connect_tx, connect_rx) = mpsc::channel(16);

        let mut runner = CourierRunner {
            mode,
            transport: transport.clone(),
            butler,
            event_tx,
            connect_rx,
            coordinator: None,
        };

        // Spawn coordinator and store in runner so run() uses the same actor
        let coordinator = runner.spawn_coordinator_sync(connect_tx);
        runner.coordinator = Some(coordinator.clone());

        let handle = CourierHandle::new(coordinator, transport);

        (handle, event_rx, runner)
    }
}

/// Runner that must be spawned to process events
pub struct CourierRunner {
    mode: CourierMode,
    transport: Arc<Transport>,
    butler: Arc<Butler>,
    event_tx: mpsc::Sender<CourierEvent>,
    connect_rx: mpsc::Receiver<crate::coordinator::ConnectRequest>,
    coordinator: Option<ActorRef<CoordinatorMessage>>,
}

impl CourierRunner {
    /// Spawn coordinator synchronously and return its ActorRef
    ///
    /// Uses block_in_place to spawn the actor synchronously so we can
    /// return the handle immediately from init_with_services.
    fn spawn_coordinator_sync(
        &self,
        connect_tx: mpsc::Sender<crate::coordinator::ConnectRequest>,
    ) -> ActorRef<CoordinatorMessage> {
        let node_id = self.transport.node_id();
        let butler = self.butler.clone();
        let transport = self.transport.clone();
        let mode = self.mode;
        let event_tx = self.event_tx.clone();

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let coordinator = Coordinator::new();
                let blob_store = crate::peer_actor::BlobStore::Real(transport);
                let (actor_ref, _) = Actor::spawn(
                    Some("coordinator".to_string()),
                    coordinator,
                    (node_id, mode, butler.clone(), blob_store, Some(connect_tx), Some(event_tx)),
                )
                .await
                .expect("Failed to spawn Coordinator");

                actor_ref
            })
        })
    }

    /// Run the courier event loop
    ///
    /// Processes transport events and connection requests, forwarding to Coordinator.
    /// The Coordinator must have been spawned during init_with_services.
    pub async fn run(mut self, mut transport_rx: mpsc::Receiver<TransportEvent>) {
        // Use the coordinator that was spawned during init
        let coordinator = self
            .coordinator
            .take()
            .expect("Coordinator must be spawned during init_with_services");

        info!("Courier runner started");

        loop {
            tokio::select! {
                // Handle transport events
                Some(event) = transport_rx.recv() => {
                    if let Some(msg) = Coordinator::from_transport_event(event) {
                        if let Err(e) = coordinator.cast(msg) {
                            error!("Failed to send message to coordinator: {:?}", e);
                        }
                    }
                }

                // Handle connection requests from Coordinator
                Some(req) = self.connect_rx.recv() => {
                    info!(node_id = %req.node_id, "Processing connection request");

                    // Mark as outbound connection so Coordinator knows to initiate handshake
                    if let Err(e) = coordinator.cast(CoordinatorMessage::OutboundConnection {
                        node_id: req.node_id,
                    }) {
                        error!("Failed to mark outbound connection: {:?}", e);
                        continue;
                    }

                    // Connect via transport (async, spawned to not block event loop)
                    let transport = self.transport.clone();
                    let node_id = req.node_id;
                    tokio::spawn(async move {
                        match transport.connect(node_id).await {
                            Ok(_handle) => {
                                info!(node_id = %node_id, "Auto-connect successful for sync");
                            }
                            Err(e) => {
                                warn!(node_id = %node_id, error = %e, "Auto-connect failed for sync");
                            }
                        }
                    });
                }

                // Both channels closed - exit
                else => break,
            }
        }

        // Shutdown coordinator
        let _ = coordinator.cast(CoordinatorMessage::Shutdown);
        info!("Courier runner stopped");
    }
}
