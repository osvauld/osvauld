//! Courier - Business Logic Layer for P2P
//!
//! Courier sits between Transport and the application.
//! Uses bidirectional channels:
//! - Events out: Courier -> App (CourierEvent)
//! - Commands in: App -> Courier (CourierCommand)
//!
//! # Architecture
//!
//! ```text
//! App <--events-- Courier --calls--> Butler/Gurkha
//!  |                 ^
//!  +---commands----->+
//!
//! Transport --events--> Courier
//! ```

pub mod handlers;
pub mod registry;

pub use handlers::{PairingContext, PendingHello, PermitInfo};
pub use registry::{PeerInfo, PeerRegistry, PeerType};

// Re-export Butler for initialization
pub use butler::{Butler, OwnerInfo};
pub use gurkha::Permit;

use anyhow::Result;
use base64::Engine;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};
use transport::{Message, NodeId, Transport, TransportEvent};

/// Courier mode of operation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CourierMode {
    /// Running as a user application (with UI)
    User,
    /// Running as a standalone node (no UI)
    Node,
}

// ==================== HANDSHAKE SERVICES ====================

/// Services needed for auto-processing handshake messages and publishing
///
/// When provided, Courier will automatically process Hello/Welcome/PermitGrant
/// messages instead of emitting events for the app to handle manually.
///
/// Uses Butler as the single entry point for all storage and identity operations.
pub struct HandshakeServices {
    /// Butler provides identity, storage, and all domain operations
    pub butler: Arc<Butler>,
}

impl HandshakeServices {
    pub fn new(butler: Arc<Butler>) -> Self {
        Self { butler }
    }
}

// ==================== EVENTS (Courier -> App) ====================

/// Events emitted to application layer
#[derive(Debug)]
pub enum CourierEvent {
    /// Peer authenticated and ready
    PeerAuthenticated {
        node_id: NodeId,
        peer_type: PeerType,
        username: String,
        did: String,
    },
    /// Peer disconnected
    PeerDisconnected { node_id: NodeId },
    /// Handshake: Hello received (for apps that want to handle manually)
    HelloReceived {
        node_id: NodeId,
        did: String,
        username: String,
        permit: String,
    },
    /// Handshake: Welcome received (for apps that want to handle manually)
    WelcomeReceived {
        node_id: NodeId,
        permit_for_us: String,
    },
    /// Sync request received
    SyncRequest {
        node_id: NodeId,
        request_id: String,
        resource_id: String,
        state_vector: Vec<u8>,
    },
    /// Sync push received
    SyncPush {
        node_id: NodeId,
        resource_id: String,
        updates: Vec<u8>,
    },
    /// Folder request received
    FolderRequest {
        node_id: NodeId,
        request_id: String,
        folder_id: String,
    },
    /// Error occurred
    Error { message: String },
}

// ==================== COMMANDS (App -> Courier) ====================

/// Commands sent from application to Courier
#[derive(Debug)]
pub enum CourierCommand {
    /// Connect to a peer and send Hello
    Connect {
        /// Node ID as string (parsed internally)
        node_id_str: String,
        our_did: String,
        our_username: String,
        our_public_key: Vec<u8>,
        permit: String,
        /// Callback channel for connection result
        result_tx: mpsc::Sender<Result<(), String>>,
    },
    /// Send Welcome response (used in node mode)
    SendWelcome {
        node_id: NodeId,
        node_public_key: Vec<u8>,
        signature: Vec<u8>,
        timestamp: i64,
        permit_for_peer: String,
    },
    /// Send PermitGrant (owner -> node after Welcome)
    SendPermitGrant {
        node_id: NodeId,
        permit_for_node: String,
    },
    /// Send Ack
    SendAck { node_id: NodeId },
    /// Disconnect from a peer
    Disconnect { node_id: NodeId },
    /// Send a raw message (for advanced use)
    SendMessage {
        node_id: NodeId,
        message: Message,
    },

    /// Publish a space to connected node (User mode)
    ///
    /// **Flow**: Get identity from Butler, prepare space/pages, transform to transport types, send
    PublishSpace {
        space_id: String,
        node_id: String,
        /// Callback channel for publish result
        result_tx: mpsc::Sender<Result<(), String>>,
    },
}

// ==================== COURIER HANDLE (for app) ====================

/// Handle for sending commands to Courier
#[derive(Clone)]
pub struct CourierHandle {
    cmd_tx: mpsc::Sender<CourierCommand>,
    registry: Arc<PeerRegistry>,
}

impl CourierHandle {
    /// Connect to a peer with Hello handshake
    ///
    /// `node_id` is the iroh NodeId as a string (hex format)
    pub async fn connect(
        &self,
        node_id: &str,
        our_did: &str,
        our_username: &str,
        our_public_key: &[u8],
        permit: &str,
    ) -> Result<(), String> {
        let (result_tx, mut result_rx) = mpsc::channel(1);

        self.cmd_tx
            .send(CourierCommand::Connect {
                node_id_str: node_id.to_string(),
                our_did: our_did.to_string(),
                our_username: our_username.to_string(),
                our_public_key: our_public_key.to_vec(),
                permit: permit.to_string(),
                result_tx,
            })
            .await
            .map_err(|e| format!("Failed to send connect command: {}", e))?;

        result_rx
            .recv()
            .await
            .ok_or_else(|| "No response from courier".to_string())?
    }

    /// Reconnect to a known peer using stored permit
    ///
    /// This is semantically the same as `connect()` but:
    /// - Uses the long-lived permit (our_permit from SovereignNode) instead of first_connection
    /// - Node will validate existing permit without issuing new one
    /// - Both sides already have each other's permits from previous connection
    ///
    /// Use this for reconnecting to nodes after app restart or disconnection.
    pub async fn reconnect(
        &self,
        node_id: &str,
        our_did: &str,
        our_username: &str,
        our_public_key: &[u8],
        our_stored_permit: &str,  // The permit node issued to us (SovereignNode.our_permit)
    ) -> Result<(), String> {
        // Reconnect uses the same flow as connect, just with the stored permit
        self.connect(node_id, our_did, our_username, our_public_key, our_stored_permit).await
    }

    /// Disconnect from a peer
    pub async fn disconnect(&self, node_id: &str) -> Result<(), String> {
        let parsed: NodeId = node_id
            .parse()
            .map_err(|e| format!("Invalid node_id: {}", e))?;
        self.cmd_tx
            .send(CourierCommand::Disconnect { node_id: parsed })
            .await
            .map_err(|e| format!("Failed to send disconnect command: {}", e))
    }

    /// Send Welcome response
    pub async fn send_welcome(
        &self,
        node_id: NodeId,
        node_public_key: Vec<u8>,
        signature: Vec<u8>,
        timestamp: i64,
        permit_for_peer: String,
    ) -> Result<(), String> {
        self.cmd_tx
            .send(CourierCommand::SendWelcome {
                node_id,
                node_public_key,
                signature,
                timestamp,
                permit_for_peer,
            })
            .await
            .map_err(|e| format!("Failed to send welcome command: {}", e))
    }

    /// Send PermitGrant
    pub async fn send_permit_grant(
        &self,
        node_id: NodeId,
        permit_for_node: String,
    ) -> Result<(), String> {
        self.cmd_tx
            .send(CourierCommand::SendPermitGrant {
                node_id,
                permit_for_node,
            })
            .await
            .map_err(|e| format!("Failed to send permit grant command: {}", e))
    }

    /// Send Ack
    pub async fn send_ack(&self, node_id: NodeId) -> Result<(), String> {
        self.cmd_tx
            .send(CourierCommand::SendAck { node_id })
            .await
            .map_err(|e| format!("Failed to send ack command: {}", e))
    }

    /// Send a raw message
    pub async fn send_message(&self, node_id: NodeId, message: Message) -> Result<(), String> {
        self.cmd_tx
            .send(CourierCommand::SendMessage { node_id, message })
            .await
            .map_err(|e| format!("Failed to send message command: {}", e))
    }

    // ==================== PUBLISHING ====================

    /// Publish a space and its pages to connected node
    ///
    /// **Context**: Owner manually triggers publish from UI
    /// **Flow**:
    /// 1. Get identity from Butler
    /// 2. Prepare space via Butler (issue folder permit)
    /// 3. Send PublishSpace message
    /// 4. For each page: prepare via Butler, send PublishPage
    ///
    /// # Arguments
    /// * `space_id` - Space to publish
    /// * `node_id` - Target node to publish to
    pub async fn publish_space(&self, space_id: &str, node_id: &str) -> Result<(), String> {
        let (result_tx, mut result_rx) = mpsc::channel(1);

        self.cmd_tx
            .send(CourierCommand::PublishSpace {
                space_id: space_id.to_string(),
                node_id: node_id.to_string(),
                result_tx,
            })
            .await
            .map_err(|e| format!("Failed to send publish command: {}", e))?;

        result_rx
            .recv()
            .await
            .ok_or_else(|| "No response from courier".to_string())?
    }

    // ==================== REGISTRY QUERIES ====================

    /// Check if owner is connected (for Node mode)
    pub async fn is_owner_connected(&self) -> bool {
        self.registry.get_owner().await.is_some()
    }

    /// Get owner info if connected (for Node mode)
    pub async fn get_owner(&self) -> Option<PeerInfo> {
        self.registry.get_owner().await
    }

    /// Get all connected nodes (for User mode - "my nodes")
    pub async fn get_my_nodes(&self) -> Vec<PeerInfo> {
        self.registry.get_my_nodes().await
    }

    /// Check if we have any connected nodes (for User mode)
    pub async fn has_connected_nodes(&self) -> bool {
        !self.registry.get_my_nodes().await.is_empty()
    }

    /// Get peer by NodeId
    pub async fn get_peer(&self, node_id: &NodeId) -> Option<PeerInfo> {
        self.registry.get(node_id).await
    }

    /// Check if a peer is connected
    pub async fn is_peer_connected(&self, node_id: &NodeId) -> bool {
        self.registry.contains(node_id).await
    }

    /// Get all connected peers
    pub async fn get_all_peers(&self) -> Vec<PeerInfo> {
        self.registry.all().await
    }
}

// ==================== COURIER ====================

/// Courier - Business logic layer for P2P
pub struct Courier {
    /// Mode of operation
    mode: CourierMode,
    /// Peer registry (authenticated peers)
    registry: Arc<PeerRegistry>,
    /// Pending Hello states (waiting for Welcome)
    pending_hellos: RwLock<HashMap<NodeId, PendingHello>>,
    /// Transport reference for sending
    transport: Arc<Transport>,
    /// Event sender to application layer
    event_tx: mpsc::Sender<CourierEvent>,
    /// Command receiver from application
    cmd_rx: RwLock<Option<mpsc::Receiver<CourierCommand>>>,
    /// Optional services for auto-processing handshake
    services: Option<Arc<HandshakeServices>>,
}

impl Courier {
    /// Initialize Courier with bidirectional channels (no auto-processing)
    ///
    /// Returns (CourierHandle, event_receiver, Courier)
    /// - CourierHandle: For sending commands to Courier
    /// - event_receiver: For receiving events from Courier
    /// - Courier: The courier instance (call .run() in a spawned task)
    pub fn init(
        mode: CourierMode,
        transport: Arc<Transport>,
    ) -> (CourierHandle, mpsc::Receiver<CourierEvent>, Self) {
        Self::init_with_services(mode, transport, None)
    }

    /// Initialize Courier with services for auto-processing handshake
    ///
    /// When services are provided:
    /// - Node mode: Hello messages auto-processed (store owner, send Welcome)
    /// - User mode: Welcome messages auto-processed (store node permit, send PermitGrant)
    pub fn init_with_services(
        mode: CourierMode,
        transport: Arc<Transport>,
        services: Option<Arc<HandshakeServices>>,
    ) -> (CourierHandle, mpsc::Receiver<CourierEvent>, Self) {
        let (event_tx, event_rx) = mpsc::channel(64);
        let (cmd_tx, cmd_rx) = mpsc::channel(64);

        let registry = Arc::new(PeerRegistry::new());

        let courier = Self {
            mode,
            registry: registry.clone(),
            pending_hellos: RwLock::new(HashMap::new()),
            transport,
            event_tx,
            cmd_rx: RwLock::new(Some(cmd_rx)),
            services,
        };

        let handle = CourierHandle {
            cmd_tx,
            registry,
        };

        (handle, event_rx, courier)
    }

    /// Get reference to peer registry
    pub fn registry(&self) -> &Arc<PeerRegistry> {
        &self.registry
    }

    /// Get the courier mode
    pub fn mode(&self) -> CourierMode {
        self.mode
    }

    /// Start processing transport events and commands
    ///
    /// This should be called in a spawned task.
    pub async fn run(self, mut transport_rx: mpsc::Receiver<TransportEvent>) {
        info!("Courier started in {:?} mode", self.mode);

        // Take ownership of command receiver
        let mut cmd_rx = {
            let mut guard = self.cmd_rx.write().await;
            guard.take().expect("Courier::run called twice")
        };

        loop {
            tokio::select! {
                // Handle transport events
                Some(event) = transport_rx.recv() => {
                    if let Err(e) = self.handle_transport_event(event).await {
                        error!("Error handling transport event: {}", e);
                    }
                }
                // Handle app commands
                Some(cmd) = cmd_rx.recv() => {
                    if let Err(e) = self.handle_command(cmd).await {
                        error!("Error handling command: {}", e);
                    }
                }
                else => {
                    warn!("Courier channels closed, shutting down");
                    break;
                }
            }
        }

        warn!("Courier event loop ended");
    }

    /// Handle a transport event
    async fn handle_transport_event(&self, event: TransportEvent) -> Result<()> {
        match event {
            TransportEvent::Connected { node_id, conn: _ } => {
                debug!("New connection from: {}", node_id);
                // Connection established, wait for Hello
            }

            TransportEvent::Disconnected { node_id } => {
                info!("Peer disconnected: {}", node_id);

                // Remove from registry
                if self.registry.remove(&node_id).await.is_some() {
                    self.emit_event(CourierEvent::PeerDisconnected { node_id }).await;
                }

                // Clean up pending hellos
                let mut pending = self.pending_hellos.write().await;
                pending.remove(&node_id);
            }

            TransportEvent::Message { node_id, message } => {
                self.handle_message(node_id, message).await?;
            }

            TransportEvent::LiveData {
                node_id,
                stream_id,
                data,
            } => {
                debug!(
                    "Live data from {}: stream={}, {} bytes",
                    node_id,
                    stream_id,
                    data.len()
                );
            }

            TransportEvent::Error { node_id, error } => {
                error!("Transport error (peer {:?}): {}", node_id, error);
                self.emit_event(CourierEvent::Error { message: error }).await;
            }
        }

        Ok(())
    }

    /// Handle a message from a peer
    async fn handle_message(&self, node_id: NodeId, message: Message) -> Result<()> {
        match message {
            Message::Hello {
                did,
                username,
                public_key,
                signature: _,
                timestamp: _,
                permit,
            } => {
                info!("Received Hello from {} ({})", username, node_id);

                // In Node mode with services, auto-process the Hello
                if self.mode == CourierMode::Node {
                    if let Some(ref services) = self.services {
                        match self.process_hello(node_id, &did, &username, &public_key, &permit, services).await {
                            Ok(_) => {
                                info!("Hello auto-processed for {}", username);
                                // Emit PeerAuthenticated event
                                self.emit_event(CourierEvent::PeerAuthenticated {
                                    node_id,
                                    peer_type: PeerType::Owner,
                                    username: username.clone(),
                                    did: did.clone(),
                                })
                                .await;
                                return Ok(());
                            }
                            Err(e) => {
                                error!("Failed to auto-process Hello: {}", e);
                                // Fall through to emit event for manual handling
                            }
                        }
                    }
                }

                // Emit event for app to handle manually
                self.emit_event(CourierEvent::HelloReceived {
                    node_id,
                    did,
                    username,
                    permit,
                })
                .await;
            }

            Message::Welcome {
                node_id: remote_node_id,
                node_public_key,
                signature: _,
                timestamp: _,
                permit_for_peer,
            } => {
                info!("Received Welcome from {}", node_id);

                // In User mode with services, auto-process the Welcome
                if self.mode == CourierMode::User {
                    if let Some(ref services) = self.services {
                        match self.process_welcome(node_id, &remote_node_id, &node_public_key, &permit_for_peer, services).await {
                            Ok(_) => {
                                info!("Welcome auto-processed from {}", node_id);
                                // Emit PeerAuthenticated event
                                self.emit_event(CourierEvent::PeerAuthenticated {
                                    node_id,
                                    peer_type: PeerType::MyNode,
                                    username: "node".to_string(), // Node doesn't have username
                                    did: remote_node_id.clone(),
                                })
                                .await;
                                return Ok(());
                            }
                            Err(e) => {
                                error!("Failed to auto-process Welcome: {}", e);
                                // Fall through to emit event for manual handling
                            }
                        }
                    }
                }

                // Emit event for app to handle manually
                self.emit_event(CourierEvent::WelcomeReceived {
                    node_id,
                    permit_for_us: permit_for_peer,
                })
                .await;
            }

            Message::PermitGrant { permit_for_node } => {
                info!("Received PermitGrant from {}", node_id);

                // In Node mode with services, auto-process the PermitGrant
                if self.mode == CourierMode::Node {
                    if let Some(ref services) = self.services {
                        match self.process_permit_grant(node_id, &permit_for_node, services).await {
                            Ok(_) => {
                                info!("PermitGrant auto-processed, handshake complete");
                                return Ok(());
                            }
                            Err(e) => {
                                error!("Failed to auto-process PermitGrant: {}", e);
                            }
                        }
                    }
                }
            }

            Message::Ack => {
                debug!("Received Ack from {}", node_id);
            }

            Message::Rejected { reason } => {
                warn!("Connection rejected by {}: {}", node_id, reason);
                self.transport.disconnect(&node_id).await;
            }

            Message::SyncRequest {
                id,
                resource_id,
                state_vector,
            } => {
                if !self.registry.contains(&node_id).await {
                    warn!("SyncRequest from unauthenticated peer: {}", node_id);
                    return Ok(());
                }

                self.emit_event(CourierEvent::SyncRequest {
                    node_id,
                    request_id: id,
                    resource_id,
                    state_vector,
                })
                .await;
            }

            Message::SyncResponse { id, updates: _ } => {
                debug!("Received SyncResponse {}", id);
            }

            Message::SyncPush {
                resource_id,
                updates,
            } => {
                if !self.registry.contains(&node_id).await {
                    warn!("SyncPush from unauthenticated peer: {}", node_id);
                    return Ok(());
                }

                self.emit_event(CourierEvent::SyncPush {
                    node_id,
                    resource_id,
                    updates,
                })
                .await;
            }

            Message::FolderRequest { id, folder_id } => {
                if !self.registry.contains(&node_id).await {
                    warn!("FolderRequest from unauthenticated peer: {}", node_id);
                    return Ok(());
                }

                self.emit_event(CourierEvent::FolderRequest {
                    node_id,
                    request_id: id,
                    folder_id,
                })
                .await;
            }

            Message::FolderResponse {
                id,
                folder_id,
                pages: _,
            } => {
                debug!("Received FolderResponse {} for {}", id, folder_id);
            }

            Message::LiveData { stream_id, data } => {
                debug!("Live data: stream={}, {} bytes", stream_id, data.len());
            }

            Message::Error { id, code, message } => {
                error!(
                    "Error from {}: {:?} - {} (id: {:?})",
                    node_id, code, message, id
                );
            }

            // ==================== Publishing (Node receives from Owner) ====================

            Message::PublishSpace {
                request_id,
                space,
                space_permit,
            } => {
                info!(
                    "Received PublishSpace from {}: space_id={}, name={}",
                    node_id, space.id, space.name
                );
                debug!("Space permit (preview): {}...", &space_permit[..space_permit.len().min(50)]);
                // TODO: Validate permit, store space via Butler
            }

            Message::PublishPage {
                request_id,
                page,
                page_permit,
                layers,
            } => {
                info!(
                    "Received PublishPage from {}: page_id={}, name={}, {} layers",
                    node_id, page.id, page.name, layers.len()
                );
                debug!("Page permit (preview): {}...", &page_permit[..page_permit.len().min(50)]);
                // TODO: Validate permit, re-encrypt layers, store via Butler
            }
        }

        Ok(())
    }

    /// Handle a command from the app
    async fn handle_command(&self, cmd: CourierCommand) -> Result<()> {
        match cmd {
            CourierCommand::Connect {
                node_id_str,
                our_did,
                our_username,
                our_public_key,
                permit,
                result_tx,
            } => {
                // Parse node_id from string
                let result = match node_id_str.parse::<NodeId>() {
                    Ok(node_id) => {
                        self.do_connect(node_id, &our_did, &our_username, &our_public_key, &permit)
                            .await
                    }
                    Err(e) => Err(format!("Invalid node_id '{}': {}", node_id_str, e)),
                };
                let _ = result_tx.send(result).await;
            }

            CourierCommand::SendWelcome {
                node_id,
                node_public_key,
                signature,
                timestamp,
                permit_for_peer,
            } => {
                if let Some(conn) = self.transport.get_connection(&node_id).await {
                    conn.send(&Message::Welcome {
                        node_id: node_id.to_string(),
                        node_public_key,
                        signature,
                        timestamp,
                        permit_for_peer,
                    })
                    .await?;
                    info!("Sent Welcome to {}", node_id);
                } else {
                    warn!("No connection for {} to send Welcome", node_id);
                }
            }

            CourierCommand::SendPermitGrant {
                node_id,
                permit_for_node,
            } => {
                if let Some(conn) = self.transport.get_connection(&node_id).await {
                    conn.send(&Message::PermitGrant { permit_for_node }).await?;
                    info!("Sent PermitGrant to {}", node_id);
                } else {
                    warn!("No connection for {} to send PermitGrant", node_id);
                }
            }

            CourierCommand::SendAck { node_id } => {
                if let Some(conn) = self.transport.get_connection(&node_id).await {
                    conn.send(&Message::Ack).await?;
                    debug!("Sent Ack to {}", node_id);
                } else {
                    warn!("No connection for {} to send Ack", node_id);
                }
            }

            CourierCommand::Disconnect { node_id } => {
                self.transport.disconnect(&node_id).await;
                info!("Disconnected from {}", node_id);
            }

            CourierCommand::SendMessage { node_id, message } => {
                if let Some(conn) = self.transport.get_connection(&node_id).await {
                    conn.send(&message).await?;
                    debug!("Sent message to {}", node_id);
                } else {
                    warn!("No connection for {} to send message", node_id);
                }
            }

            CourierCommand::PublishSpace {
                space_id,
                node_id,
                result_tx,
            } => {
                let result = self.do_publish_space(&space_id, &node_id).await;
                let _ = result_tx.send(result).await;
            }
        }

        Ok(())
    }

    /// Connects to peer and sends Hello message.
    ///
    /// **Context**: Called when Owner initiates connection to Node.
    /// **We send**: Hello with our identity and permit (first_connection or peer_connection)
    /// **We store**: PendingHello state to track handshake progress
    async fn do_connect(
        &self,
        node_id: NodeId,
        our_did: &str,
        our_username: &str,
        our_public_key: &[u8],
        permit: &str,
    ) -> Result<(), String> {
        info!("Connecting to peer: {}", node_id);

        // Parse permit to determine connection type
        let (is_first_connection, relationship) = match Permit::from_token(permit) {
            Ok(p) => {
                let is_first = p.is_first_connection();
                let rel = p.relationship().map(|s| s.to_string());
                (is_first, rel)
            }
            Err(e) => {
                warn!("Failed to parse permit, assuming first_connection: {}", e);
                (true, None)
            }
        };

        let conn_type = if is_first_connection { "first_connection" } else { "reconnection" };
        info!("Initiating {} to {} (relationship={:?})", conn_type, node_id, relationship);

        // Connect via transport
        let conn = self
            .transport
            .connect(node_id)
            .await
            .map_err(|e| format!("Transport connect failed: {}", e))?;

        // Send Hello with dummy signature for now (app should provide real signature)
        let timestamp = chrono::Utc::now().timestamp();
        let signature = vec![]; // TODO: App should sign

        conn.send(&Message::Hello {
            did: our_did.to_string(),
            username: our_username.to_string(),
            public_key: our_public_key.to_vec(),
            signature,
            timestamp,
            permit: permit.to_string(),
        })
        .await
        .map_err(|e| format!("Failed to send Hello: {}", e))?;

        // Store pending hello to track handshake state
        let pending = PendingHello {
            their_did: String::new(),
            their_username: String::new(),
            their_node_id: node_id,
            is_first_connection,
            timestamp,
        };

        let mut hellos = self.pending_hellos.write().await;
        hellos.insert(node_id, pending);

        info!("Sent Hello to {} ({})", node_id, conn_type);
        Ok(())
    }

    /// Emit event to application layer
    async fn emit_event(&self, event: CourierEvent) {
        if let Err(e) = self.event_tx.send(event).await {
            error!("Failed to emit app event: {}", e);
        }
    }

    /// Get authenticated peer by NodeId
    pub async fn get_peer(&self, node_id: &NodeId) -> Option<PeerInfo> {
        self.registry.get(node_id).await
    }

    /// Get the owner (if connected)
    pub async fn get_owner(&self) -> Option<PeerInfo> {
        self.registry.get_owner().await
    }

    /// Get all connected nodes
    pub async fn get_my_nodes(&self) -> Vec<PeerInfo> {
        self.registry.get_my_nodes().await
    }

    // ==================== HANDSHAKE AUTO-PROCESSING ====================

    /// Processes Hello message from peer (Node side).
    ///
    /// **Context**: Called when we (Node) receive Hello from someone claiming to be owner.
    /// **Peer sends**: Hello with identity proof and permit (first_connection or peer_connection)
    /// **We verify**: Permit is valid and has owner relationship
    /// **We store**: OwnerInfo (first connection) or update last_connected (reconnection)
    /// **We issue**: Long-lived permit_for_owner (first connection only)
    /// **We send**: Welcome with permit_for_owner
    async fn process_hello(
        &self,
        node_id: NodeId,
        did: &str,
        username: &str,
        public_key: &[u8],
        permit: &str,
        services: &HandshakeServices,
    ) -> Result<(), String> {
        info!("Processing Hello from {} ({})", username, did);
        debug!("Received permit (preview): {}...", &permit[..permit.len().min(50)]);

        let butler = &services.butler;

        // 1. Parse and verify the permit using Gurkha
        let parsed_permit = Permit::from_token(permit)
            .map_err(|e| format!("Failed to parse permit: {}", e))?;

        let is_first_connection = parsed_permit.is_first_connection();
        let relationship = parsed_permit.relationship();

        info!(
            "Permit analysis: is_first_connection={}, relationship={:?}",
            is_first_connection, relationship
        );

        // Check relationship (should be owner)
        if relationship != Some("owner") {
            return Err(format!("Expected owner relationship, got {:?}", relationship));
        }

        // Get our identity from Butler for signing Welcome
        let identity = butler.get_identity().await
            .map_err(|_| "No identity set for signing".to_string())?;

        let conn = self.transport.get_connection(&node_id).await
            .ok_or_else(|| "No connection to send Welcome".to_string())?;

        let timestamp = chrono::Utc::now().timestamp();
        let node_public_key = identity.public_device_key().to_vec();
        let signature = vec![]; // TODO: Sign with identity

        if is_first_connection {
            // ==================== FIRST CONNECTION ====================
            info!("First connection from owner {}", username);

            // Create and store owner info via Butler
            let owner_info = OwnerInfo::new(
                did.to_string(),
                username.to_string(),
                public_key.to_vec(),
                node_id.as_bytes().to_vec(), // device_public_key (iroh node id)
            );

            butler.set_owner(&owner_info)
                .map_err(|e| format!("Failed to store owner: {}", e))?;

            info!("Owner info stored for {}", username);

            // Issue a long-lived permit for the owner via Butler
            let owner_pubkey = base64::engine::general_purpose::STANDARD.encode(public_key);
            let (permit_for_owner, _) = butler.issue_peer_connection_permit(&owner_pubkey, "owner")
                .await
                .map_err(|e| format!("Failed to issue permit: {}", e))?;

            // Store the permit we issued to owner via Butler
            butler.set_owner_permit_for_owner(permit_for_owner.clone())
                .map_err(|e| format!("Failed to store permit: {}", e))?;

            // Send Welcome with new permit
            conn.send(&Message::Welcome {
                node_id: node_id.to_string(),
                node_public_key,
                signature,
                timestamp,
                permit_for_peer: permit_for_owner,
            })
            .await
            .map_err(|e| format!("Failed to send Welcome: {}", e))?;

            info!("Welcome sent to {} (first connection)", username);
        } else {
            // ==================== RECONNECTION ====================
            info!("Reconnection from owner {}", username);

            // Verify we have this owner stored via Butler
            let existing_owner = butler.get_owner()
                .map_err(|e| format!("Failed to check owner: {}", e))?
                .ok_or_else(|| "No owner stored, but got reconnection permit".to_string())?;

            // Verify DID matches
            if existing_owner.owner_did != did {
                return Err(format!("DID mismatch: expected {}, got {}", existing_owner.owner_did, did));
            }

            // Update last connected timestamp via Butler
            butler.update_owner_last_connected()
                .map_err(|e| format!("Failed to update last connected: {}", e))?;

            // Get the permit we already issued to them (re-use it)
            let permit_for_owner = existing_owner.permit_for_owner
                .ok_or_else(|| "No stored permit for owner".to_string())?;

            // Send Welcome with existing permit (re-confirmation)
            conn.send(&Message::Welcome {
                node_id: node_id.to_string(),
                node_public_key,
                signature,
                timestamp,
                permit_for_peer: permit_for_owner,
            })
            .await
            .map_err(|e| format!("Failed to send Welcome: {}", e))?;

            info!("Welcome sent to {} (reconnection)", username);
        }

        Ok(())
    }

    /// Processes Welcome message from node (Owner/User side).
    ///
    /// **Context**: Called when we (Owner) receive Welcome from Node after sending Hello.
    /// **Peer sends**: Welcome with node's identity and permit_for_us (long-lived permit)
    /// **We verify**: Permit is valid
    /// **We store**: our_permit in SovereignNode record (for future reconnections)
    /// **We issue**: Long-lived permit_for_node (first connection only)
    /// **We send**: PermitGrant with permit_for_node
    async fn process_welcome(
        &self,
        node_id: NodeId,
        _remote_node_id: &str,
        node_public_key: &[u8],
        permit_for_us: &str,
        services: &HandshakeServices,
    ) -> Result<(), String> {
        info!("Processing Welcome from {}", node_id);
        debug!("Received permit_for_us (preview): {}...", &permit_for_us[..permit_for_us.len().min(50)]);

        let butler = &services.butler;

        // 1. Parse and verify the permit
        let parsed_permit = Permit::from_token(permit_for_us)
            .map_err(|e| format!("Failed to parse permit: {}", e))?;

        debug!(
            "permit_for_us analysis: is_first_connection={}, relationship={:?}",
            parsed_permit.is_first_connection(),
            parsed_permit.relationship()
        );

        // 2. Check if this is first connection or reconnection (from our pending state)
        let is_first_connection = {
            let hellos = self.pending_hellos.read().await;
            let is_first = hellos.get(&node_id).map(|h| h.is_first_connection).unwrap_or(true);
            debug!("Connection type from pending_hellos: is_first_connection={}", is_first);
            is_first
        };

        let conn_type = if is_first_connection { "first_connection" } else { "reconnection" };
        info!("Welcome received ({})", conn_type);

        // 3. Store the permit we received (in sovereign_node record) via Butler
        //    This permit is what we'll use for future reconnections
        let node_id_str = node_id.to_string();
        let stored = butler.set_sovereign_node_permit(&node_id_str, permit_for_us.to_string())
            .map_err(|e| format!("Failed to store permit: {}", e))?;

        if stored {
            info!("Stored our_permit from node {} (for future reconnections)", node_id);
        } else {
            warn!("Failed to store our_permit - SovereignNode {} may not exist!", node_id_str);
        }

        let conn = self.transport.get_connection(&node_id).await
            .ok_or_else(|| "No connection to send PermitGrant".to_string())?;

        if is_first_connection {
            // ==================== FIRST CONNECTION ====================
            info!("First connection Welcome from {}", node_id);

            // Issue a long-lived permit for the node via Butler
            let node_pubkey = base64::engine::general_purpose::STANDARD.encode(node_public_key);
            let (permit_for_node, _) = butler.issue_peer_connection_permit(&node_pubkey, "node")
                .await
                .map_err(|e| format!("Failed to issue permit: {}", e))?;

            // Store the permit we issued TO the node via Butler
            butler.set_sovereign_node_permit_for_them(&node_id_str, permit_for_node.clone())
                .map_err(|e| format!("Failed to store permit_for_them: {}", e))?;

            // Send PermitGrant with new permit
            conn.send(&Message::PermitGrant {
                permit_for_node,
            })
            .await
            .map_err(|e| format!("Failed to send PermitGrant: {}", e))?;

            info!("PermitGrant sent to {} (first connection)", node_id);
        } else {
            // ==================== RECONNECTION ====================
            info!("Reconnection Welcome from {}", node_id);

            // Get the node record with stored permit via Butler
            let sovereign_node = butler.get_sovereign_node(&node_id_str)
                .map_err(|e| format!("Failed to get sovereign node: {}", e))?
                .ok_or_else(|| "No sovereign node found for reconnection".to_string())?;

            // Use stored permit if available, otherwise re-issue
            let permit_for_node = if let Some(stored_permit) = sovereign_node.permit_for_them {
                info!("Using stored permit for node");
                stored_permit
            } else {
                // Re-issue permit if not stored (backwards compatibility)
                info!("No stored permit, re-issuing");
                let node_pubkey = base64::engine::general_purpose::STANDARD.encode(node_public_key);
                let (permit, _) = butler.issue_peer_connection_permit(&node_pubkey, "node")
                    .await
                    .map_err(|e| format!("Failed to issue permit: {}", e))?;

                // Store it for future use via Butler
                butler.set_sovereign_node_permit_for_them(&node_id_str, permit.clone())
                    .map_err(|e| format!("Failed to store permit_for_them: {}", e))?;
                permit
            };

            // Send PermitGrant (re-confirmation)
            conn.send(&Message::PermitGrant {
                permit_for_node,
            })
            .await
            .map_err(|e| format!("Failed to send PermitGrant: {}", e))?;

            info!("PermitGrant sent to {} (reconnection)", node_id);
        }

        // Clean up pending hello
        let mut hellos = self.pending_hellos.write().await;
        hellos.remove(&node_id);

        Ok(())
    }

    /// Processes PermitGrant message from owner (Node side).
    ///
    /// **Context**: Called when we (Node) receive PermitGrant from Owner after sending Welcome.
    /// **Peer sends**: PermitGrant with permit_for_node (long-lived permit for us)
    /// **We verify**: Permit is valid
    /// **We store**: permit_from_owner (first connection only)
    /// **We send**: Ack to confirm handshake completion
    async fn process_permit_grant(
        &self,
        node_id: NodeId,
        permit_for_node: &str,
        services: &HandshakeServices,
    ) -> Result<(), String> {
        info!("Processing PermitGrant from {}", node_id);
        debug!("Received permit_for_node (preview): {}...", &permit_for_node[..permit_for_node.len().min(50)]);

        let butler = &services.butler;

        // 1. Parse and verify the permit
        let parsed_permit = Permit::from_token(permit_for_node)
            .map_err(|e| format!("Failed to parse permit: {}", e))?;

        debug!(
            "permit_for_node analysis: is_first_connection={}, relationship={:?}",
            parsed_permit.is_first_connection(),
            parsed_permit.relationship()
        );

        // 2. Check if this is a reconnection (owner already has permit stored) via Butler
        let (is_reconnection, owner_did) = if let Ok(Some(owner)) = butler.get_owner() {
            let has_permit = owner.permit_from_owner.is_some();
            debug!("Owner exists: did={}, has_permit_from_owner={}", owner.owner_did, has_permit);
            (has_permit, Some(owner.owner_did))
        } else {
            debug!("No owner stored yet");
            (false, None)
        };

        let conn_type = if is_reconnection { "reconnection" } else { "first_connection" };

        // 3. Only store if first connection (avoid redundant writes on reconnection) via Butler
        if !is_reconnection {
            butler.set_owner_permit_from_owner(permit_for_node.to_string())
                .map_err(|e| format!("Failed to store permit: {}", e))?;
            info!("Stored permit_from_owner (first connection)");
        } else {
            debug!("Reconnection with {} - permit already stored, skipping storage", owner_did.unwrap_or_default());
        }

        // 4. Send Ack to confirm handshake complete
        if let Some(conn) = self.transport.get_connection(&node_id).await {
            conn.send(&Message::Ack)
                .await
                .map_err(|e| format!("Failed to send Ack: {}", e))?;
            debug!("Sent Ack to {}", node_id);
        }

        info!("Handshake complete with owner ({})", conn_type);
        Ok(())
    }

    // ==================== PUBLISHING ====================

    /// Orchestrates publishing a space and its pages to connected node.
    ///
    /// **Context**: Owner manually triggers publish from UI
    /// **Flow**:
    /// 1. Get identity from Butler
    /// 2. Get target node's public key from Butler
    /// 3. Call Butler to prepare space (issue space permit)
    /// 4. Transform SpaceMeta → PublishedSpace, send PublishSpace message
    /// 5. For each page: prepare via Butler, transform, send PublishPage
    ///
    /// # Arguments
    /// * `space_id` - Space to publish
    /// * `node_id_str` - Target node ID (iroh node ID as string)
    async fn do_publish_space(&self, space_id: &str, node_id_str: &str) -> Result<(), String> {
        info!("Publishing space {} to node {}", space_id, node_id_str);

        // 1. Ensure we're in User mode and have services
        if self.mode != CourierMode::User {
            return Err("Publishing only available in User mode".to_string());
        }

        let services = self.services.as_ref()
            .ok_or_else(|| "No services configured for publishing".to_string())?;

        let butler = &services.butler;

        // 2. Parse node_id and get node's public key for permit issuance
        let node_id: NodeId = node_id_str.parse()
            .map_err(|e| format!("Invalid node ID: {}", e))?;

        let sovereign_node = butler
            .get_sovereign_node(node_id_str)
            .map_err(|e| format!("Failed to get sovereign node: {}", e))?
            .ok_or_else(|| format!("Sovereign node {} not found", node_id_str))?;

        // user_public_key is already base64 encoded
        let node_pubkey_b64 = sovereign_node.user_public_key;

        // 3. Prepare space via Butler (issues folder permit, uses stored identity)
        let (space_meta, space_permit) = butler
            .prepare_space_for_publish(space_id, &node_pubkey_b64)
            .await
            .map_err(|e| format!("Failed to prepare space: {}", e))?;

        info!("Space prepared, permit issued");

        // 4. Transform SpaceMeta → PublishedSpace
        let published_space = transport::PublishedSpace {
            id: space_meta.id.clone(),
            name: space_meta.name,
            parent_space_id: space_meta.parent_space_id,
            owner_did: space_meta.owner_did,
            description: space_meta.description,
            created_at: space_meta.created_at,
            updated_at: space_meta.updated_at,
        };

        // 5. Send PublishSpace message
        let conn = self.transport.get_connection(&node_id).await
            .ok_or_else(|| "No connection to node".to_string())?;

        conn.send(&Message::PublishSpace {
            request_id: space_id.to_string(),
            space: published_space,
            space_permit,
        })
        .await
        .map_err(|e| format!("Failed to send PublishSpace: {}", e))?;

        info!("PublishSpace sent, now publishing pages");

        // 6. Get all pages in space via Butler
        let pages = butler.list_pages(space_id)
            .map_err(|e| format!("Failed to list pages: {}", e))?;

        info!("Found {} pages to publish", pages.len());

        // 7. For each page: prepare, transform, send
        for page in pages {
            info!("Publishing page: {} ({})", page.name, page.id);

            // Prepare page via Butler (issues resource permit, decrypts layers, uses stored identity)
            let (page_meta, page_permit, layers) = butler
                .prepare_page_for_publish(&page.id, &node_pubkey_b64)
                .await
                .map_err(|e| format!("Failed to prepare page {}: {}", page.id, e))?;

            // Transform PageMeta → PublishedPageMeta
            let published_page = transport::PublishedPageMeta {
                id: page_meta.id,
                space_id: page_meta.space_id,
                name: page_meta.name,
                page_type: format!("{:?}", page_meta.page_type),
                owner_did: page_meta.owner_did,
                is_private: page_meta.is_private,
                created_at: page_meta.created_at,
                updated_at: page_meta.updated_at,
            };

            // Send PublishPage message
            conn.send(&Message::PublishPage {
                request_id: page.id.clone(),
                page: published_page,
                page_permit,
                layers,
            })
            .await
            .map_err(|e| format!("Failed to send PublishPage: {}", e))?;

            info!("PublishPage sent for {}", page.name);
        }

        info!("Space {} published successfully", space_id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_courier_mode() {
        assert_eq!(CourierMode::User, CourierMode::User);
        assert_ne!(CourierMode::User, CourierMode::Node);
    }
}
