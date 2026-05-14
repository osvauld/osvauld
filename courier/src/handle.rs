//! This module provides a high-level API for P2P operations.
//! App communicates with Coordinator for lifecycle, and directly with PeerActor for operations.
//!
//! This module uses concrete `IrohConnection` type for production.
//! For testing with mock connections, use the generic Coordinator directly.

use std::sync::Arc;

use ractor::{Actor, ActorRef};
use serde::Serialize;
use tokio::sync::{broadcast, mpsc, oneshot};
use tracing::{error, info, warn};
use transport::{IrohConnection, NodeId, Transport, TransportEvent};

use crate::coordinator::{Coordinator, CoordinatorMessage, CourierMode};
use crate::peer_actor::PeerMessage;
use butler::Butler;

/// Production coordinator message type (uses IrohConnection)
pub type IrohCoordinatorMessage = CoordinatorMessage<IrohConnection>;

/// Production coordinator type (uses IrohConnection)
pub type IrohCoordinator = Coordinator<IrohConnection>;

/// Event emitted by Courier to the application
#[derive(Debug, Clone, Serialize)]
pub enum CourierEvent {
    /// Peer authenticated successfully
    PeerAuthenticated {
        node_id: String,
        did: String,
        username: String,
    },
    /// Peer disconnected
    PeerDisconnected { node_id: String },
    /// Connection/authentication failed
    ConnectionFailed { node_id: String, error: String },
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
    SyncConsentComplete { node_id: String, space_id: String },
    /// Shareable link received from node
    ShareableLinkReceived {
        node_id: String,
        space_id: String,
        permit: String,
    },
    /// Connection requested - app should connect via transport
    ///
    /// **Context**: ConnectAndAuth was called, transport needs to connect
    ConnectRequested { node_id: String, permit: String },
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
    /// Page permit was updated (stored on receiver side)
    ///
    /// **Context**: PermitUpdate message received and stored successfully.
    /// Emitted by both viewers (receiving from node) and nodes (receiving from coordinator distribution).
    PermitUpdated { page_id: String, version: u64 },
}

/// Handle for interacting with Courier
///
/// Provides async methods for P2P operations.
/// Uses `IrohConnection` (production) type internally.
#[derive(Clone)]
pub struct CourierHandle {
    coordinator: ActorRef<IrohCoordinatorMessage>,
}

impl CourierHandle {
    /// Create a new CourierHandle
    pub fn new(coordinator: ActorRef<IrohCoordinatorMessage>) -> Self {
        Self { coordinator }
    }

    /// Connect to a node (fire-and-forget)
    ///
    /// **Flow**:
    /// 1. Coordinator stores permit and marks as pending
    /// 2. CourierRunner connects via transport
    /// 3. on_connected spawns PeerActor with permit (auto-initiates handshake)
    /// 4. App receives PeerAuthenticated or ConnectionFailed event via event channel
    ///
    /// **Events**:
    /// - `CourierEvent::PeerAuthenticated` on success
    /// - `CourierEvent::ConnectionFailed` on failure
    pub fn connect(&self, node_id: &str, permit: &str) -> Result<(), String> {
        let node_id: NodeId = node_id
            .parse()
            .map_err(|e| format!("Invalid node_id: {}", e))?;

        self.coordinator
            .cast(IrohCoordinatorMessage::Connect {
                node_id,
                permit: permit.to_string(),
            })
            .map_err(|e| format!("Failed to send Connect: {:?}", e))
    }

    /// Get PeerActor ref for direct communication
    async fn get_peer_actor(&self, node_id: NodeId) -> Result<ActorRef<PeerMessage>, String> {
        let (tx, rx) = oneshot::channel();

        self.coordinator
            .cast(IrohCoordinatorMessage::GetPeerActor {
                node_id,
                response: tx,
            })
            .map_err(|e| format!("Failed to send GetPeerActor: {:?}", e))?;

        rx.await
            .map_err(|_| "GetPeerActor channel closed".to_string())?
            .ok_or_else(|| format!("No PeerActor for node {}", node_id))
    }

    /// Publish a space to a node (sends directly to PeerActor)
    pub async fn publish_space(&self, space_id: &str, node_id: &str) -> Result<(), String> {
        let node_id: NodeId = node_id
            .parse()
            .map_err(|e| format!("Invalid node_id: {}", e))?;

        let peer_actor = self.get_peer_actor(node_id).await?;
        peer_actor
            .cast(PeerMessage::PublishSpace {
                space_id: space_id.to_string(),
            })
            .map_err(|e| format!("Failed to send PublishSpace: {:?}", e))?;

        Ok(())
    }

    /// Get shareable link for a space (sends request to PeerActor, awaits response)
    ///
    /// **Flow**: Sends GetShareableLinkRequest to node, waits for GetShareableLinkResponse
    pub async fn get_shareable_link(
        &self,
        space_id: &str,
        node_id: &str,
    ) -> Result<String, String> {
        let node_id: NodeId = node_id
            .parse()
            .map_err(|e| format!("Invalid node_id: {}", e))?;

        let peer_actor = self.get_peer_actor(node_id).await?;

        // Create callback channel
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();

        peer_actor
            .cast(PeerMessage::GetShareableLink {
                space_id: space_id.to_string(),
                response_tx,
            })
            .map_err(|e| format!("Failed to send GetShareableLink: {:?}", e))?;

        // Wait for the response
        response_rx
            .await
            .map_err(|_| "Response channel closed".to_string())?
    }

    /// Request space as viewer (sends directly to PeerActor)
    pub async fn request_space_as_viewer(
        &self,
        space_id: &str,
        node_id: &str,
        viewer_permit: &str,
    ) -> Result<(), String> {
        let node_id: NodeId = node_id
            .parse()
            .map_err(|e| format!("Invalid node_id: {}", e))?;

        let peer_actor = self.get_peer_actor(node_id).await?;
        peer_actor
            .cast(PeerMessage::RequestSpace {
                space_id: space_id.to_string(),
                viewer_permit: viewer_permit.to_string(),
            })
            .map_err(|e| format!("Failed to send RequestSpace: {:?}", e))?;

        Ok(())
    }

    /// Get a reference to the coordinator (for tests)
    pub fn coordinator(&self) -> &ActorRef<IrohCoordinatorMessage> {
        &self.coordinator
    }

    /// Ensure sync with a user (forward SyncEvent::EnsureSync to Coordinator)
    ///
    /// **Context**: Scribe emitted EnsureSync - forward to Coordinator
    /// **Coordinator will**: Resolve user_did → device, connect if needed, PeerActor subscribes
    pub fn ensure_sync(&self, user_did: &str) -> Result<(), String> {
        self.coordinator
            .cast(IrohCoordinatorMessage::EnsureSync {
                user_did: user_did.to_string(),
            })
            .map_err(|e| format!("Failed to send EnsureSync: {:?}", e))
    }

    /// Subscribe to dynamic layers on a creator peer
    ///
    /// **Context**: Scribe detected new entries in creator's __sync_meta, needs to send
    /// LayerSubscribe to the creator to get authority.
    pub fn subscribe_layers(
        &self,
        page_id: &str,
        creator_did: &str,
        layers: Vec<String>,
    ) -> Result<(), String> {
        self.coordinator
            .cast(IrohCoordinatorMessage::SubscribeLayers {
                page_id: page_id.to_string(),
                creator_did: creator_did.to_string(),
                layers,
            })
            .map_err(|e| format!("Failed to send SubscribeLayers: {:?}", e))
    }

    /// Broadcast datagram to all authenticated peers
    ///
    /// **Context**: Send ephemeral data (cursor, typing) to all connected users
    /// **Coordinator will**: Send datagram to each authenticated peer's connection
    pub fn broadcast_datagram(&self, data: Vec<u8>) -> Result<(), String> {
        self.coordinator
            .cast(IrohCoordinatorMessage::BroadcastDatagram { data })
            .map_err(|e| format!("Failed to broadcast datagram: {:?}", e))
    }

    /// Notify that a page was opened - refreshes subscriptions
    ///
    /// **Context**: App opened a page, ensures sync subscriptions are established
    /// **Coordinator will**: Send RefreshSubscriptions to all authenticated PeerActors
    /// **Use case**: Fixes race condition where page opens after connection established
    pub fn page_opened(&self, page_id: &str) -> Result<(), String> {
        self.coordinator
            .cast(IrohCoordinatorMessage::PageOpened {
                page_id: page_id.to_string(),
            })
            .map_err(|e| format!("Failed to notify page opened: {:?}", e))
    }

    /// Distribute updated page permits to peers (forward to Coordinator)
    ///
    /// **Context**: Page permits reissued with new app layers — forward to Coordinator
    /// **Coordinator will**: Store permits in butler, send PermitUpdate to connected peers
    pub fn distribute_page_permit_updates(
        &self,
        page_id: &str,
        permits: Vec<(String, String)>,
    ) -> Result<(), String> {
        self.coordinator
            .cast(IrohCoordinatorMessage::DistributePagePermitUpdates {
                page_id: page_id.to_string(),
                permits,
            })
            .map_err(|e| format!("Failed to send DistributePagePermitUpdates: {:?}", e))
    }

    /// Check if a node is authenticated (handshake complete)
    ///
    /// **Context**: Script/app wants to wait for auth before publishing
    /// **Returns**: true if handshake complete with this node
    pub async fn is_node_authenticated(&self, node_id: &str) -> Result<bool, String> {
        let node_id: NodeId = node_id
            .parse()
            .map_err(|e| format!("Invalid node_id: {}", e))?;

        let (tx, rx) = oneshot::channel();
        self.coordinator
            .cast(IrohCoordinatorMessage::IsNodeAuthenticated {
                node_id,
                response: tx,
            })
            .map_err(|e| format!("Failed to send IsNodeAuthenticated: {:?}", e))?;

        rx.await.map_err(|_| "Channel closed".to_string())
    }

    /// Go offline - simulate network loss for E2E tests
    ///
    /// **Context**: Test wants to simulate losing network connectivity
    /// **Effect**: Stops all PeerActors, closes connections, rejects new connections
    /// **Note**: Does NOT emit PeerDisconnected events (test isolation)
    pub async fn go_offline(&self) -> Result<(), String> {
        let (tx, rx) = oneshot::channel();
        self.coordinator
            .cast(IrohCoordinatorMessage::GoOffline { response: tx })
            .map_err(|e| format!("Failed to send GoOffline: {:?}", e))?;

        rx.await
            .map_err(|_| "GoOffline channel closed".to_string())?
    }

    /// Go online - restore network connectivity for E2E tests
    ///
    /// **Context**: Test wants to restore network after simulated loss
    /// **Effect**: Clears offline flag, reconnects to all known nodes with stored permits
    pub async fn go_online(&self) -> Result<(), String> {
        let (tx, rx) = oneshot::channel();
        self.coordinator
            .cast(IrohCoordinatorMessage::GoOnline { response: tx })
            .map_err(|e| format!("Failed to send GoOnline: {:?}", e))?;

        rx.await
            .map_err(|_| "GoOnline channel closed".to_string())?
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
/// Provides a simpler initialization API that matches for Courier consumers.
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
        Self::init_with_services_and_capture(mode, transport, services, None)
    }

    /// Initialize Courier with services and optional capture broadcast
    ///
    /// Returns a handle, event receiver, and the runner that must be spawned.
    pub fn init_with_services_and_capture(
        mode: CourierMode,
        transport: Arc<Transport>,
        services: Option<Arc<HandshakeServices>>,
        capture_tx: Option<broadcast::Sender<String>>,
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
            capture_tx,
        };

        // Spawn coordinator and store in runner so run() uses the same actor
        let coordinator = runner.spawn_coordinator_sync(connect_tx);
        runner.coordinator = Some(coordinator.clone());

        let handle = CourierHandle::new(coordinator);

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
    coordinator: Option<ActorRef<IrohCoordinatorMessage>>,
    /// Capture broadcast sender — passed through to coordinator for event capture
    capture_tx: Option<broadcast::Sender<String>>,
}

impl CourierRunner {
    /// Convert TransportEvent to IrohCoordinatorMessage
    ///
    /// Maps the concrete TransportEvent (with ConnectionHandle) to the generic
    /// CoordinatorMessage<IrohConnection> by wrapping the handle.
    fn from_transport_event(event: TransportEvent) -> Option<IrohCoordinatorMessage> {
        match event {
            TransportEvent::Connected { node_id, conn } => {
                Some(IrohCoordinatorMessage::Connected {
                    node_id,
                    conn: conn.into(), // ConnectionHandle -> IrohConnection
                })
            }
            TransportEvent::Disconnected { node_id } => {
                Some(IrohCoordinatorMessage::Disconnected { node_id })
            }
        }
    }

    /// Spawn coordinator synchronously and return its ActorRef
    ///
    /// Uses block_in_place to spawn the actor synchronously so we can
    /// return the handle immediately from init_with_services.
    fn spawn_coordinator_sync(
        &self,
        connect_tx: mpsc::Sender<crate::coordinator::ConnectRequest>,
    ) -> ActorRef<IrohCoordinatorMessage> {
        let node_id = self.transport.node_id();
        let butler = self.butler.clone();
        let transport = self.transport.clone();
        let mode = self.mode;
        let event_tx = self.event_tx.clone();
        let capture_tx = self.capture_tx.clone();

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let coordinator = IrohCoordinator::new();
                let blob_store = crate::peer_actor::BlobStore::Real(transport);
                let (actor_ref, _) = Actor::spawn(
                    Some("coordinator".to_string()),
                    coordinator,
                    (
                        node_id,
                        mode,
                        butler.clone(),
                        blob_store,
                        Some(connect_tx),
                        Some(event_tx),
                        None,
                        capture_tx,
                    ),
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
                    if let Some(msg) = Self::from_transport_event(event) {
                        if let Err(e) = coordinator.cast(msg) {
                            error!("Failed to send message to coordinator: {:?}", e);
                        }
                    }
                }

                // Handle connection requests from Coordinator (e.g., EnsureSync)
                // Note: Coordinator already stored permit in pending_permits before sending this request
                Some(req) = self.connect_rx.recv() => {
                    info!(node_id = %req.node_id, "Processing connection request");

                    // Connect via transport - on_connected will use stored permit
                    // Transport::connect now includes retry with exponential backoff
                    let transport = self.transport.clone();
                    let node_id = req.node_id;
                    let event_tx = self.event_tx.clone();
                    tokio::spawn(async move {
                        match transport.connect(node_id).await {
                            Ok(_handle) => {
                                info!(node_id = %node_id, "Auto-connect successful for sync");
                                // TransportEvent::Connected will be emitted by transport,
                                // which triggers PeerActor spawn and handshake
                            }
                            Err(e) => {
                                warn!(node_id = %node_id, error = %e, "Auto-connect failed for sync (after retries)");
                                // Notify app that connection failed
                                let _ = event_tx.send(CourierEvent::ConnectionFailed {
                                    node_id: node_id.to_string(),
                                    error: e.to_string(),
                                }).await;
                            }
                        }
                    });
                }

                // Both channels closed - exit
                else => break,
            }
        }

        // Shutdown coordinator
        let _ = coordinator.cast(IrohCoordinatorMessage::Shutdown);
        info!("Courier runner stopped");
    }
}
