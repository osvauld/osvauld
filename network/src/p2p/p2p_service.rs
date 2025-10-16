use crate::p2p::{
    connection_manager::ConnectionManager,
    constants::*,
    emitter::{P2PEvent, P2PEventEmitter},
    errors::{ConnectionError, P2PError, P2PResult},
    incoming::{IncomingEvent, P2PSender},
    logger,
    peer_connection::{PeerConnection, ServiceContext},
};
use std::str::FromStr;

use crypto_utils::CryptoUtils;
use iroh::endpoint::Connection;
use iroh::{Endpoint, NodeAddr, NodeId, RelayMode};
use osvauld_core::models::{
    ConnectionAction, ConnectionType, Device, Message, ShareOperation, User,
};
use persistance::database::RepositoryContext;
use services::generate_challenge;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::time::timeout;
use tracing::{debug, error, info, info_span, instrument, trace, warn, Instrument};

pub struct P2PState {
    pub endpoint: Arc<Endpoint>,
    pub connections: ConnectionManager,
    pub service_context: Arc<ServiceContext>,
}

#[derive(Clone)]
pub struct P2PService {
    pub state: Arc<Mutex<Option<P2PState>>>,
    pub crypto_utils: Arc<RwLock<CryptoUtils>>,
    pub repo_ctx: Arc<RepositoryContext>,
    pub event_emitter: P2PEventEmitter,
    pub current_user: Arc<RwLock<Option<User>>>,
    pub current_device: Arc<RwLock<Option<Device>>>,
    pub domain: Arc<String>,
}

impl P2PService {
    /// Creates a new P2P service instance
    #[instrument(skip_all, level = "info")]
    pub fn new(
        repo_ctx: Arc<RepositoryContext>,
        crypto_utils: Arc<RwLock<CryptoUtils>>,
        domain: Arc<String>,
    ) -> (
        Self,
        mpsc::UnboundedReceiver<P2PEvent>,
        P2PSender,
        mpsc::UnboundedReceiver<IncomingEvent>,
    ) {
        let _guard = match logger::init_default_tracing() {
            Ok(guard) => guard,
            Err(e) => {
                eprintln!("Failed to initialize tracing: {}", e);
                None
            }
        };

        let (emitter, receiver) = P2PEventEmitter::new();
        let (p2p_sender, incoming_receiver) = P2PSender::new();
        info!("Creating new P2P service instance");

        let service = Self {
            state: Arc::new(Mutex::new(None)),
            event_emitter: emitter,
            current_user: Arc::new(RwLock::new(None)),
            current_device: Arc::new(RwLock::new(None)),
            repo_ctx,
            crypto_utils,
            domain,
        };

        debug!("P2P service instance created successfully");
        (service, receiver, p2p_sender, incoming_receiver)
    }

    /// Sets the current user for the P2P service
    #[instrument(skip_all, level = "debug")]
    pub async fn set_current_user(&self, user: User) {
        debug!("Setting current user: {}", user.username);
        let mut user_guard = self.current_user.write().await;
        *user_guard = Some(user);
        debug!("Current user set successfully");
    }

    /// Sets the current device for the P2P service
    #[instrument(skip_all, level = "debug")]
    pub async fn set_current_device(&self, device: Device) {
        debug!("Setting current device: {}", device.id);
        let mut device_guard = self.current_device.write().await;
        *device_guard = Some(device);
        debug!("Current device set successfully");
    }

    /// Clears the current user and device session
    #[instrument(skip(self), level = "info")]
    pub async fn clear_current_session(&self) {
        info!("Clearing current session");
        let mut user_guard = self.current_user.write().await;
        *user_guard = None;
        let mut device_guard = self.current_device.write().await;
        *device_guard = None;
        debug!("Current session cleared successfully");
    }

    /// Gets the current user if one is set
    #[instrument(skip(self), level = "trace")]
    pub async fn get_current_user(&self) -> P2PResult<User> {
        trace!("Getting current user");
        let user_guard = self.current_user.read().await;
        user_guard
            .clone()
            .ok_or_else(|| P2PError::InvalidState("No current user set".to_string()))
    }

    /// Gets the current device if one is set
    #[instrument(skip(self), level = "trace")]
    pub async fn get_current_device(&self) -> P2PResult<Device> {
        trace!("Getting current device");
        let device_guard = self.current_device.read().await;
        device_guard
            .clone()
            .ok_or_else(|| P2PError::InvalidState("No current device set".to_string()))
    }

    /// Ensures the P2P service is initialized
    #[instrument(skip(self), level = "debug")]
    pub async fn ensure_initialized(&self) -> P2PResult<()> {
        let mut state = self.state.lock().await;
        if state.is_some() {
            trace!("P2P service already initialized");
            return Ok(());
        }

        let key = self.repo_ctx.store_repo.get_node_key().await?;
        let node_public_key = self.repo_ctx.store_repo.get_device_key().await?;
        let node_id = crypto_utils::derive_node_id_from_public_key(&node_public_key)?;
        let node_id = NodeId::try_from(&node_id)
            .map_err(|e| P2PError::Configuration(format!("Invalid node ID: {}", e)))?;

        info!("node_id {}", node_id);

        let secret_key_bytes = {
            let crypto = self.crypto_utils.read().await;
            crypto.get_node_keypair(&key)?
        };
        let secret_key = iroh::SecretKey::from(secret_key_bytes);
        info!("Initializing P2P endpoint");

        debug!("Generated new secret key for P2P endpoint");

        let endpoint = Endpoint::builder()
            .secret_key(secret_key)
            .discovery_n0()
            .relay_mode(RelayMode::Default)
            .alpns(vec![ALPN_PROTOCOL.to_vec()])
            .bind()
            .await
            .map_err(|e| P2PError::Configuration(format!("Failed to bind endpoint: {}", e)))?;

        info!(
            "Successfully bound P2P endpoint with node ID: {}",
            endpoint.node_id()
        );

        let service_context = Arc::new(ServiceContext {
            current_user: self.current_user.clone(),
            current_device: self.current_device.clone(),
        });

        *state = Some(P2PState {
            endpoint: Arc::new(endpoint),
            connections: ConnectionManager::new(),
            service_context,
        });

        info!("P2P initialization successful");
        Ok(())
    }

    pub async fn start_p2p_service(&self, device: &Device, user: &User) -> P2PResult<()> {
        self.set_current_user(user.clone()).await;
        self.set_current_device(device.clone()).await;
        self.ensure_initialized().await?;
        self.start_listening().await?;
        self.request_connections().await?;
        Ok(())
    }

    pub async fn request_connections(&self) -> P2PResult<()> {
        let current_device = self.get_current_device().await?;
        let all_devices = self
            .repo_ctx
            .device_repo
            .get_all_devices_except(&[current_device.id.clone()])
            .await?;
        let all_users = self.repo_ctx.user_repo.get_known_users().await?;

        let (user_devices, other_devices): (Vec<_>, Vec<_>) = all_devices
            .into_iter()
            .partition(|device| device.user_id == current_device.user_id);

        let first_users: Vec<User> = all_users
            .into_iter()
            .filter(|user| user.first_sync)
            .collect();

        let first_user_ids: HashSet<_> = first_users.iter().map(|user| user.id.clone()).collect();
        let (first_user_connection_devices, other_devices): (Vec<_>, Vec<_>) = other_devices
            .into_iter()
            .partition(|device| first_user_ids.contains(&device.user_id));

        // Connect to user devices (DeviceSync)
        for device in user_devices {
            let self_clone = self.clone();
            tokio::spawn(async move {
                match self_clone
                    .connect_with_ticket(
                        &device.id,
                        ConnectionType::Device,
                        Some(ConnectionAction::DeviceSync),
                    )
                    .await
                {
                    Ok(_) => {
                        info!("Successfully connected to user device: {}", &device.id);
                    }
                    Err(e) => {
                        error!("Failed to connect to user device {}: {}", &device.id, e);
                    }
                }
            });
        }

        // Connect to first-time users (UserFirstConnection)
        for device in first_user_connection_devices {
            let self_clone = self.clone();
            tokio::spawn(async move {
                match self_clone
                    .connect_with_ticket(
                        &device.id,
                        ConnectionType::User,
                        Some(ConnectionAction::UserSync),
                    )
                    .await
                {
                    Ok(_) => {
                        info!("Successfully connected to first-time user: {}", &device.id);
                    }
                    Err(e) => {
                        error!("Failed to connect to first-time user {}: {}", &device.id, e);
                    }
                }
            });
        }

        // Connect to other users (UserSync)
        for device in other_devices {
            let self_clone = self.clone();
            tokio::spawn(async move {
                match self_clone
                    .connect_with_ticket(
                        &device.id,
                        ConnectionType::User,
                        Some(ConnectionAction::UserSync),
                    )
                    .await
                {
                    Ok(_) => {
                        info!("Successfully connected to other user: {}", &device.id);
                    }
                    Err(e) => {
                        error!("Failed to connect to other user {}: {}", &device.id, e);
                    }
                }
            });
        }

        Ok(())
    }

    pub async fn sync_folders(&self, folder_id: &str) -> P2PResult<()> {
        let share_records = self
            .repo_ctx
            .folder_share_repo
            .get_records_by_folder_id(folder_id)
            .await?;
        let user_ids: Vec<String> = share_records
            .into_iter()
            .map(|record| record.recipient_user_id)
            .collect();
        self.connect_with_users(&user_ids).await?;
        Ok(())
    }

    async fn connect_with_users(&self, user_ids: &[String]) -> P2PResult<()> {
        let current_user = self.get_current_user().await?;
        let user_ids: Vec<String> = user_ids
            .iter()
            .filter(|user_id| **user_id != current_user.id)
            .cloned()
            .collect();
        let user_devices = self
            .repo_ctx
            .device_repo
            .get_devices_by_user_ids(&user_ids)
            .await?;

        for device in user_devices {
            let self_clone = self.clone();
            tokio::spawn(async move {
                match self_clone
                    .connect_with_ticket(
                        &device.id,
                        ConnectionType::User,
                        Some(ConnectionAction::UserSync),
                    )
                    .await
                {
                    Ok(_) => {
                        info!("Successfully connected to other user: {}", &device.id);
                    }
                    Err(e) => {
                        error!("Failed to connect to other user {}: {}", &device.id, e);
                    }
                }
            });
        }
        Ok(())
    }

    pub async fn sync_resource(&self, resource_id: &str) -> P2PResult<()> {
        let resource_share_records = self
            .repo_ctx
            .share_repo
            .find_by_resource_and_operation(resource_id, &ShareOperation::Share.to_string())
            .await?;
        let user_ids: Vec<String> = resource_share_records
            .into_iter()
            .map(|record| record.recipient_user_id)
            .collect();
        info!("found ids  {:?}", user_ids);
        self.connect_with_users(&user_ids).await?;

        Ok(())
    }

    pub async fn start_listening(&self) -> P2PResult<()> {
        info!("Starting P2P listener");

        let endpoint = {
            let state_guard = self.state.lock().await;
            let state = state_guard.as_ref().ok_or(P2PError::NotInitialized)?;
            debug!(
                "Listener using endpoint with node ID: {}",
                state.endpoint.node_id()
            );
            state.endpoint.clone()
        };

        let self_clone = self.clone();

        tokio::spawn(
            async move {
                info!("Listener started for incoming connections");
                while let Some(incoming) = endpoint.accept().await {
                    let connection_span = info_span!("incoming_connection", 
                        remote = %incoming.remote_address());
                    match incoming.accept() {
                        Ok(connecting) => {
                            info!(parent: &connection_span, "Accepting incoming connection");
                            let self_clone = self_clone.clone();

                            tokio::spawn(
                                async move {
                                    debug!("Awaiting connection establishment");
                                    match timeout(CONNECTION_TIMEOUT, connecting).await {
                                        Ok(Ok(conn)) => {
                                            info!("Connection established, initiating handshake");
                                            match self_clone
                                                .perform_handshake_and_create_peer(&conn, false, None, None)
                                                .await
                                            {
                                                Ok(peer) => {
                                                    info!("Handshake completed successfully with peer: {}", peer.get_id());
                                                }
                                                Err(e) => {
                                                    error!("Handshake failed: {}", e);
                                                }
                                            }
                                        }
                                        Ok(Err(e)) => {
                                            error!("Connection failed: {}", e);
                                        }
                                        Err(e) => {
                                            error!("Connection timeout after {} seconds: {}", 
                                                   CONNECTION_TIMEOUT.as_secs(), e);
                                        }
                                    }
                                }
                                .instrument(connection_span)
                            );
                        }
                        Err(e) => {
                            error!("Failed to accept connection: {}", e);
                        }
                    }
                }
                warn!("P2P listener stopped accepting connections");
            }
            .instrument(info_span!("p2p_listener"))
        );

        info!("P2P listener started successfully");
        Ok(())
    }

    #[instrument(skip(self), fields(connection_id = %connection_id), level = "debug")]
    pub async fn get_connection_by_id(
        &self,
        connection_id: &str,
    ) -> P2PResult<Arc<PeerConnection>> {
        let state_guard = self.state.lock().await;
        let state = state_guard.as_ref().ok_or(P2PError::NotInitialized)?;

        state
            .connections
            .get_peer_connection(connection_id)
            .await
            .map_err(|_| {
                P2PError::Connection(ConnectionError::NotFound {
                    connection_id: connection_id.to_string(),
                })
            })
    }

    /// Get connections by IDs
    #[instrument(skip(self, connection_ids), level = "debug")]
    pub async fn get_connections_by_ids(
        &self,
        connection_ids: &[String],
    ) -> Vec<Arc<PeerConnection>> {
        debug!(
            "Getting connections by IDs, count: {}",
            connection_ids.len()
        );

        let state_guard = self.state.lock().await;
        let state = match state_guard.as_ref() {
            Some(s) => s,
            None => {
                error!("P2P service not initialized");
                return Vec::new();
            }
        };

        state
            .connections
            .get_connections_by_ids(connection_ids)
            .await
    }

    pub async fn perform_handshake_and_create_peer(
        &self,
        conn: &Connection,
        is_initiator: bool,
        connection_type: Option<ConnectionType>,
        action: Option<ConnectionAction>,
    ) -> P2PResult<Arc<PeerConnection>> {
        debug!("Creating peer connection for handshake");

        let local_device = self.get_current_device().await?;
        let local_user = self.get_current_user().await?;
        let peer_node_id = conn
            .remote_node_id()
            .map_err(|e| P2PError::Connection(ConnectionError::RemoteNodeIdError(e.to_string())))?;

        let state_guard = self.state.lock().await;
        let state = state_guard.as_ref().ok_or(P2PError::NotInitialized)?;

        // Create cleanup callback
        let self_clone = self.clone();
        let cleanup_callback = Box::new(move |connection_id: String| {
            let service = self_clone.clone();
            tokio::spawn(async move {
                info!(
                    "Connection cleanup callback triggered for: {}",
                    connection_id
                );
                let state_guard = service.state.lock().await;
                if let Some(state) = state_guard.as_ref() {
                    if let Err(e) = state.connections.remove_connection(&connection_id).await {
                        error!("Failed to remove connection {}: {}", connection_id, e);
                    } else {
                        info!(
                            "Successfully removed connection from manager: {}",
                            connection_id
                        );
                    }
                } else {
                    error!(
                        "State not available during cleanup for connection: {}",
                        connection_id
                    );
                }
            });
        });

        let is_live_edit = state
            .connections
            .get_and_clear_pending_live_edit_requests(&peer_node_id.to_string())
            .await;

        if is_live_edit {
            info!(
                "Found pending live edit request for connection: {}",
                &peer_node_id.to_string()
            );
        }

        let challenge = generate_challenge();

        let peer_connection = PeerConnection::new(
            Arc::new(conn.clone()),
            is_initiator,
            state.service_context.clone(),
            self.event_emitter.clone(),
            Some(cleanup_callback),
            self.crypto_utils.clone(),
            self.repo_ctx.clone(),
            action.clone(),
            connection_type.clone(),
            peer_node_id.to_string(),
            local_user.clone(),
            local_device.clone(),
            challenge,
            is_live_edit,
            self.domain.to_string(),
        );

        let peer_connection_arc = Arc::new(peer_connection);

        debug!("Inserting connection into connection manager");
        state
            .connections
            .insert_connection(peer_connection_arc.clone())
            .await
            .map_err(|e| {
                P2PError::Connection(ConnectionError::AlreadyExists { connection_id: e })
            })?;

        // If initiator, start the handshake process
        if is_initiator {
            let conn_type = connection_type.ok_or_else(|| {
                P2PError::Configuration("Connection type must be specified for initiator".into())
            })?;
            let action = action.ok_or_else(|| {
                P2PError::Configuration("Connection action must be specified for initiator".into())
            })?;
            debug!("Initiating handshake as {:?}", conn_type);

            peer_connection_arc
                .initiate_handshake(conn_type, action, local_user, local_device)
                .await?;
        }

        debug!("Created peer connection: {}", peer_connection_arc.get_id());
        info!(
            "Handshake and peer creation successful: {}",
            peer_connection_arc.get_id()
        );
        Ok(peer_connection_arc)
    }

    #[instrument(skip(self, conn_type), level = "info")]
    pub async fn connect_with_ticket(
        &self,
        device_or_node_id: &str,
        conn_type: ConnectionType,
        action: Option<ConnectionAction>,
    ) -> P2PResult<Option<Arc<PeerConnection>>> {
        info!("Starting connection process with ticket");

        let node_id = if let Ok(id) = NodeId::from_str(device_or_node_id) {
            id
        } else {
            let node_id_bytes = crypto_utils::derive_node_id_from_public_key(device_or_node_id)?;
            NodeId::try_from(&node_id_bytes).map_err(|e| {
                P2PError::Connection(ConnectionError::InvalidNodeId {
                    node_id: e.to_string(),
                })
            })?
        };

        self.ensure_initialized().await?;

        let connection_id = node_id.to_string();
        let (endpoint, connections) = {
            let state_guard = self.state.lock().await;
            let state = state_guard.as_ref().ok_or(P2PError::NotInitialized)?;
            (state.endpoint.clone(), state.connections.clone())
        };

        // Check if connection is already established or in connecting state
        if connections.is_connection_active(&connection_id).await {
            if let Ok(existing_connection) = connections.get_peer_connection(&connection_id).await {
                info!("Using existing established connection: {}", &connection_id);
                if let Some(action) = action {
                    match action {
                        ConnectionAction::LiveEdit => {
                            self.event_emitter
                                .emit(P2PEvent::LiveEditConnected { connection_id });
                        }

                        _ => {
                            if existing_connection.is_initiator {
                                existing_connection.start_user_network_sync().await?;
                            } else {
                                existing_connection
                                    .send_message(Message::RetryRequest)
                                    .await?;
                            }
                        }
                    }
                }
                return Ok(Some(existing_connection));
            } else {
                info!(
                    "Connection is currently being established (returning None): {}",
                    &connection_id
                );
                connections
                    .add_pending_live_edit_request(&connection_id)
                    .await;
                return Ok(None);
            }
        }

        // Mark the connection as connecting
        connections
            .mark_as_connecting(&connection_id)
            .await
            .map_err(|e| {
                P2PError::Connection(ConnectionError::AlreadyConnecting { connection_id: e })
            })?;

        // Create node address from parsed components
        let node_addr = NodeAddr::new(node_id.clone());

        debug!("Created NodeAddr: {:?}", node_addr);
        debug!("Our endpoint ID: {}", endpoint.node_id());
        debug!("ALPN Protocol: {}", String::from_utf8_lossy(ALPN_PROTOCOL));

        info!("Connecting to remote endpoint...");
        let connection_span = info_span!("endpoint_connect", 
            remote_node_id = %node_addr.node_id,
            addresses = ?node_addr.direct_addresses.len());

        let connect_result = endpoint
            .connect(node_addr.clone(), ALPN_PROTOCOL)
            .instrument(connection_span)
            .await;

        let conn = match connect_result {
            Ok(conn) => {
                info!("Connection established successfully");
                conn
            }
            Err(e) => {
                error!("Connection failed: {}", e);
                connections.remove_from_connecting(&connection_id).await;
                return Err(P2PError::Connection(ConnectionError::EstablishmentFailed {
                    node_id: node_id.to_string(),
                    reason: e.to_string(),
                }));
            }
        };

        info!("Starting handshake process");
        let handshake_span =
            info_span!("handshake", initiator = true, connection_type = ?conn_type);

        let handshake_result = self
            .perform_handshake_and_create_peer(&conn, true, Some(conn_type), action)
            .instrument(handshake_span)
            .await;

        match handshake_result {
            Ok(peer_connection) => Ok(Some(peer_connection)),
            Err(e) => {
                let state_guard = self.state.lock().await;
                if let Some(state) = &*state_guard {
                    state
                        .connections
                        .remove_from_connecting(&connection_id)
                        .await;
                }
                Err(e)
            }
        }
    }

    /// Send a message, reconnecting if necessary
    /// The connection_id parameter can be either a device_id (public key) or node_id
    pub async fn send_or_reconnect(
        &self,
        device_or_node_id: &str,
        message: Message,
        action: ConnectionAction,
    ) -> P2PResult<()> {
        // Convert device_id to node_id if necessary
        // Connection manager stores connections by node_id, not device_id
        let node_id = if let Ok(id) = NodeId::from_str(device_or_node_id) {
            id
        } else {
            // It's a device public key, derive the node_id
            let node_id_bytes = crypto_utils::derive_node_id_from_public_key(device_or_node_id)?;
            NodeId::try_from(&node_id_bytes).map_err(|e| {
                P2PError::Connection(ConnectionError::InvalidNodeId {
                    node_id: e.to_string(),
                })
            })?
        };

        let connection_id = node_id.to_string();

        // Try to get and use existing connection
        match self.get_connection_by_id(&connection_id).await {
            Ok(connection) => {
                if connection.connection.close_reason().is_none() {
                    debug!("Using existing healthy connection: {}", connection_id);
                    return connection.send_message(message).await;
                } else {
                    info!("Connection {} is closed, cleaning up", connection_id);
                    let state_guard = self.state.lock().await;
                    if let Some(state) = state_guard.as_ref() {
                        let _ = state.connections.remove_connection(&connection_id).await;
                    }
                }
            }
            Err(_) => {
                // No connection exists, fall through to reconnect
                debug!("No existing connection found for {}", connection_id);
            }
        }

        info!(
            "Connection {} not healthy, attempting reconnection",
            connection_id
        );

        // Use the original device_or_node_id for connect_with_ticket
        // (it handles both device_id and node_id)
        match self
            .connect_with_ticket(device_or_node_id, ConnectionType::User, Some(action))
            .await
        {
            Ok(Some(new_conn)) => {
                info!("Reconnected to {}, sending message", connection_id);
                new_conn.send_message(message).await
            }
            Ok(None) => Err(P2PError::InvalidState("Connection in progress".to_string())),
            Err(e) => Err(e),
        }
    }
}
