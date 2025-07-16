use crate::p2p::{
    connection_manager::ConnectionManager,
    constants::*,
    emitter::{P2PEvent, P2PEventEmitter},
    errors::P2PError,
    incoming::{IncomingEvent, P2PSender},
    logger,
    peer_connection::{PeerConnection, ServiceContext},
};

use iroh::endpoint::Connection;
use crypto_utils::CryptoUtils;
use iroh::{Endpoint, NodeId, RelayMode, NodeAddr};
use osvauld_core::models::{ConnectionAction, ConnectionType, Device, Message, User};
use persistance::database::RepositoryContext;
use services::generate_challenge;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::sync::{Mutex, mpsc};
use tokio::time::timeout;
use tracing::{Instrument, debug, error, info, info_span, instrument, trace, warn};
pub struct P2PState {
    pub endpoint: Arc<Endpoint>,
    // HashMap of connections with user:device as the key
    pub connections: ConnectionManager,
    pub service_context: Arc<ServiceContext>,
}

#[derive(Clone)]
pub struct P2PService {
    pub state: Arc<Mutex<Option<P2PState>>>,
    pub crypto_utils: Arc<Mutex<CryptoUtils>>,
    pub repo_ctx: RepositoryContext,
    pub event_emitter: P2PEventEmitter,
    pub current_user: Arc<RwLock<Option<User>>>,
    pub current_device: Arc<RwLock<Option<Device>>>,
}

impl P2PService {
    /// Creates a new P2P service instance
    #[instrument(skip_all, level = "info")]
    pub fn new(
        repo_ctx: RepositoryContext,
        crypto_utils: Arc<Mutex<CryptoUtils>>,
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
    pub async fn get_current_user(&self) -> Result<User, String> {
        trace!("Getting current user");
        let user_guard = self.current_user.read().await;
        let user = user_guard.clone();
        if let Some(ref u) = user {
            Ok(u.clone())
        } else {
            trace!("No current user set");
            return Err("no user found".to_string());
        }
    }

    /// Gets the current device if one is set
    #[instrument(skip(self), level = "trace")]
    pub async fn get_current_device(&self) -> Result<Device, String> {
        trace!("Getting current device");
        let device_guard = self.current_device.read().await;
        let device = device_guard.clone();
        if let Some(ref d) = device {
            trace!("Current device found: {}", d.id);
            Ok(d.clone())
        } else {
            trace!("No current device set");
            return Err("no current device found".to_string());
        }
    }

    /// Ensures the P2P service is initialized
    #[instrument(skip(self), level = "debug")]
    pub async fn ensure_initialized(&self) -> Result<(), P2PError> {
        let mut state = self.state.lock().await;
        if state.is_some() {
            trace!("P2P service already initialized");
            return Ok(());
        }
        let key = self
            .repo_ctx
            .store_repo
            .get_node_key()
            .await
            .map_err(|e| P2PError::Initialization(e.to_string()))?;
        let node_public_key = self
            .repo_ctx
            .store_repo
            .get_device_key()
            .await
            .map_err(|e| P2PError::Initialization(e.to_string()))?;
        let node_id = crypto_utils::derive_node_id_from_public_key(&node_public_key)?;
        let node_id =
            NodeId::try_from(&node_id).map_err(|e| P2PError::Initialization(e.to_string()))?;
        info!("node_id {}", node_id);

        let secret_key_bytes = {
            let crypto = self.crypto_utils.lock().await;
            crypto
                .get_node_keypair(&key)
                .map_err(|e| P2PError::Initialization(e.to_string()))?
        };
        let secret_key = iroh::SecretKey::from(secret_key_bytes);
        info!("Initializing P2P endpoint");

        debug!("Generated new secret key for P2P endpoint");

        let endpoint = match Endpoint::builder()
            .secret_key(secret_key)
            .discovery_n0()
            .relay_mode(RelayMode::Default)
            .alpns(vec![ALPN_PROTOCOL.to_vec()])
            .bind()
            .await
        {
            Ok(endpoint) => {
                info!(
                    "Successfully bound P2P endpoint with node ID: {}",
                    endpoint.node_id()
                );
                endpoint
            }
            Err(e) => {
                error!("Failed to bind P2P endpoint: {}", e);
                return Err(P2PError::Initialization(format!(
                    "Failed to bind endpoint: {}",
                    e
                )));
            }
        };

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

    pub async fn start_p2p_service(&self, device: &Device, user: &User) -> Result<(), String> {
        self.set_current_user(user.clone()).await;
        self.set_current_device(device.clone()).await;
        self.ensure_initialized().await?;
        self.start_listening().await?;
        self.request_connections().await?;
        Ok(())

}
pub async fn request_connections(&self) -> Result<(), String> {
    let current_device = self.get_current_device().await?;
    let all_devices = self.repo_ctx.device_repo.get_all_devices_except(&[current_device.id]).await.map_err(|e| e.to_string())?;
    let all_users = self.repo_ctx.user_repo.get_known_users().await.map_err(|e| e.to_string())?;
    let (user_devices, other_devices): (Vec<_>, Vec<_>) = all_devices
        .into_iter()
        .partition(|device| device.user_id == current_device.user_id );
    
    let first_users: Vec<User> = all_users
        .into_iter()
        .filter(|user| user.first_sync)
        .collect();
    
    let first_user_ids: HashSet<_> = first_users.iter().map(|user| user.id.clone()).collect();
    let (first_user_connection_devices, other_devices): (Vec<_>, Vec<_>) = other_devices
        .into_iter()
        .partition(|device| first_user_ids.contains(&device.user_id));
    
    
    // Spawn all connection tasks concurrently
    
    // Connect to user devices (DeviceSync)
    for device in user_devices {
        let self_clone = self.clone();
        tokio::spawn(async move {
            match self_clone.connect_with_ticket(&device.id, ConnectionType::Device,  Some(ConnectionAction::DeviceSync)).await {
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
            match self_clone.connect_with_ticket(&device.id, ConnectionType::User, Some(ConnectionAction::UserFirstConnection)).await {
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
    for device in other_devices{
        let self_clone = self.clone();
         tokio::spawn(async move {
            match self_clone.connect_with_ticket(&device.id, ConnectionType::User,  Some(ConnectionAction::UserSync)).await {
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

    /// Starts listening for inceming connections
    pub async fn start_listening(&self) -> Result<(), P2PError> {
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
        let self_clone = self.clone(); // Clone self for use in the spawned task


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
                                            // Perform handshake as the receiver (non-initiator)
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
    ) -> Result<Arc<PeerConnection>, String> {
        // Acquire the state lock
        let state_guard = self.state.lock().await;

        // Check if service is initialized
        let state = match state_guard.as_ref() {
            Some(s) => s,
            None => return Err("P2P service not initialized".to_string()),
        };

        // Get the connection from the connection manager
        state.connections.get_peer_connection(connection_id).await
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

        // Acquire the state lock
        let state_guard = self.state.lock().await;

        // Check if service is initialized
        let state = match state_guard.as_ref() {
            Some(s) => s,
            None => {
                error!("P2P service not initialized");
                return Vec::new();
            }
        };

        // Delegate to ConnectionManager
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
    ) -> Result<Arc<PeerConnection>, P2PError> {
        debug!("Creating peer connection for handshake");

        // Get local device and user
        let local_device = self.get_current_device().await?;
        let local_user = self.get_current_user().await?;
        let peer_node_id = conn.remote_node_id().map_err(|e| e.to_string())?;

        // Get the state for service context
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
                }else {
                    error!("State not available during cleanup for connection: {}", connection_id);
                }
            });
        });
        // Create PeerConnection with local user/device (will be updated during handshake)
        let is_live_edit= state
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
            is_live_edit
        );

        let peer_connection_arc = Arc::new(peer_connection);

        // Insert the connection into the connection manager immediately
        debug!("Inserting connection into connection manager");
        if let Err(e) = state
            .connections
            .insert_connection(peer_connection_arc.clone())
            .await
        {
            error!("Failed to insert connection: {}", e);
            return Err(P2PError::PeerConnection(e));
        }

        // If initiator, start the handshake process
        if is_initiator {
            let conn_type = connection_type.ok_or_else(|| {
                P2PError::Configuration("Connection type must be specified for initiator".into())
            })?;
            let action = action.ok_or_else(|| {
                P2PError::Configuration("Connection action must be specified for initiator".into())
            })?;
            debug!("Initiating handshake as {:?}", conn_type);

            if let Err(e) = peer_connection_arc
                .initiate_handshake(conn_type, action, local_user, local_device)
                .await
            {
                error!("Failed to initiate handshake: {}", e);
                // Connection is already inserted, handshake will happen via messages
            }
        }

        debug!("Created peer connection: {}", peer_connection_arc.get_id());

        info!(
            "Handshake and peer creation successful: {}",
            peer_connection_arc.get_id()
        );
        Ok(peer_connection_arc)
    }

    #[instrument(skip(self,  conn_type), fields( conn_type = ?conn_type ), level = "info")]
    pub async fn connect_with_ticket(
        &self,
        device_id: &str,
        conn_type: ConnectionType,
        action: Option<ConnectionAction>,
    ) -> Result<Option<Arc<PeerConnection>>, P2PError> {
        info!("Starting connection process with ticket");

        let node_id_bytes = crypto_utils::derive_node_id_from_public_key(&device_id)?;
        let node_id = NodeId::try_from(&node_id_bytes).map_err(|e| e.to_string())?;
        // Ensure P2P service is initialized
        self.ensure_initialized().await?;

        let connection_id = node_id.to_string();
        // Get the state for access to connections
        let (endpoint, connections) = {
            let state_guard = self.state.lock().await;
            let state = state_guard.as_ref().ok_or(P2PError::NotInitialized)?;
            (state.endpoint.clone(), state.connections.clone())
        };

        // If a connection ID was provided, check if the connection is already active
        // Check if connection is already established or in connecting state
        if connections.is_connection_active(&connection_id).await {
            // Try to get an established connection
            if let Ok(existing_connection) = connections.get_peer_connection(&connection_id).await {
                info!("Using existing established connection: {}", &connection_id);
                if let Some(action) = action {
                    if action == ConnectionAction::LiveEdit {
                        self.event_emitter
                            .emit(P2PEvent::LiveEditConnected { connection_id });
                    } else {
                        if existing_connection.is_initiator {
                            let _ = existing_connection.execute_connection_action().await;
                        } else {
                            let _ = existing_connection
                                .send_message(Message::RetryRequest)
                                .await;
                        }
                    }
                }
                return Ok(Some(existing_connection));
            } else {
                // If we're here, the connection is in connecting state but not yet established
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
        if let Err(e) = connections.mark_as_connecting(&connection_id).await {
            // This shouldn't happen given the previous check, but handle it just in case
            warn!("Failed to mark connection as connecting: {}", e);
            return Err(P2PError::Connection(format!(
                "Failed to mark connection as connecting: {}",
                e
            )));
        }

        // Helper function to clean up connecting state on error
        let cleanup_connecting = |connection_id: Option<&str>, error: P2PError| -> P2PError {
            if let Some(id) = connection_id {
                // We need to spawn a task because we can't use .await in a closure
                let connection_id = id.to_string();
                let state_connections = connections.clone();
                tokio::spawn(async move {
                    state_connections
                        .remove_from_connecting(&connection_id)
                        .await;
                });
            }
            error
        };

        // Create node address from parsed components
        let node_addr = NodeAddr::new(node_id.clone());

        debug!("Created NodeAddr: {:?}", node_addr);
        debug!("Our endpoint ID: {}", endpoint.node_id());
        debug!("ALPN Protocol: {}", String::from_utf8_lossy(ALPN_PROTOCOL));

        // Attempt to establish connection
        info!("Connecting to remote endpoint...");
        let connection_span = info_span!("endpoint_connect", 
        remote_node_id = %node_addr.node_id,
        addresses = ?node_addr.direct_addresses.len());
        debug!("Attempting connection to node_id: {}", node_id);
        debug!("NodeAddr created: {:?}", node_addr);
        debug!("Using ALPN: {}", String::from_utf8_lossy(ALPN_PROTOCOL));

        let connect_result = endpoint
            .connect(node_addr.clone(), ALPN_PROTOCOL)
            .instrument(connection_span)
            .await;

        // Handle connection result
        let conn = match connect_result {
            Ok(conn) => {
                info!("Connection established successfully");
                conn
            }
            Err(e) => {
                error!("Connection failed: {}", e);
                debug!("Node addr used: {:?}", node_addr);

                let error = P2PError::Connection(format!("Connection failed: {}", e));
                return Err(cleanup_connecting(Some(&connection_id), error));
            }
        };

        // Proceed with handshake
        info!("Starting handshake process");
        let handshake_span =
            info_span!("handshake", initiator = true, connection_type = ?conn_type);

        // Perform handshake
        let handshake_result = self
            .perform_handshake_and_create_peer(&conn, true, Some(conn_type), action)
            .instrument(handshake_span)
            .await;

        // Clean up connecting state if handshake fails
        // If handshake succeeds, the connection will be in the established connections map
        // and handle_peer_and_create_connection will call insert_connection which removes from connecting
        match handshake_result {
            Ok(peer_connection) => {
                // Return the new peer connection
                Ok(Some(peer_connection))
            }
            Err(e) => {
                // Clean up connecting state
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
}
