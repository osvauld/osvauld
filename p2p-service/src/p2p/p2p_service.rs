use crate::p2p::connection_manager::ConnectionManager;
use crate::p2p::constants::*;
use crate::p2p::emitter::{P2PEvent, P2PEventEmitter};
use crate::p2p::errors::P2PError;
use crate::p2p::incoming::{IncomingEvent, P2PSender};
use crate::p2p::logger;
use crate::p2p::peer_connection::{PeerConnection, ServiceContext};
use iroh::{Endpoint, RelayMode, SecretKey};
use osvauld_core::models::device::Device;
use osvauld_core::models::p2p::{ConnectionTicket, ConnectionType, Message, SyncPayload};
use osvauld_core::models::user::User;
use osvauld_services::{AuthService,  SyncService, UserService};
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
    pub sync_service: Arc<SyncService>,
    pub auth_service: Arc<AuthService>,
    pub user_service: Arc<UserService>,
    pub event_emitter: P2PEventEmitter,
    pub current_user: Arc<RwLock<Option<User>>>,
    pub current_device: Arc<RwLock<Option<Device>>>,
}

impl P2PService {
    /// Creates a new P2P service instance
    #[instrument(skip_all, level = "info")]
    pub fn new(
        sync_service: Arc<SyncService>,
        auth_service: Arc<AuthService>,
        user_service: Arc<UserService>,
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
            sync_service,
            auth_service,
            user_service,
            event_emitter: emitter,
            current_user: Arc::new(RwLock::new(None)),
            current_device: Arc::new(RwLock::new(None)),
        };
        let service_clone = service.clone();

        debug!("P2P service instance created successfully");
        (service, receiver, p2p_sender, incoming_receiver)
    }

    /// Sets the current user for the P2P service
    #[instrument(skip(self, user), fields(user_id = %user.id), level = "debug")]
    pub async fn set_current_user(&self, user: User) {
        debug!("Setting current user: {}", user.username);
        let mut user_guard = self.current_user.write().await;
        *user_guard = Some(user);
        debug!("Current user set successfully");
    }

    /// Sets the current device for the P2P service
    #[instrument(skip(self, device), fields(device_id = %device.id), level = "debug")]
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
    pub async fn get_current_user(&self) -> Option<User> {
        trace!("Getting current user");
        let user_guard = self.current_user.read().await;
        let user = user_guard.clone();
        if let Some(ref u) = user {
            trace!("Current user found: {}", u.id);
        } else {
            trace!("No current user set");
        }
        user
    }

    /// Gets the current device if one is set
    #[instrument(skip(self), level = "trace")]
    pub async fn get_current_device(&self) -> Option<Device> {
        trace!("Getting current device");
        let device_guard = self.current_device.read().await;
        let device = device_guard.clone();
        if let Some(ref d) = device {
            trace!("Current device found: {}", d.id);
        } else {
            trace!("No current device set");
        }
        device
    }

    /// Ensures the P2P service is initialized
    #[instrument(skip(self), level = "debug")]
    pub async fn ensure_initialized(&self) -> Result<(), P2PError> {
        let mut state = self.state.lock().await;
        if state.is_some() {
            trace!("P2P service already initialized");
            return Ok(());
        }

        info!("Initializing P2P endpoint");

        let secret_key = SecretKey::generate(rand::rngs::OsRng);
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
        let self_clone = self.clone();
        let get_current_user = Arc::new(move || {
            // Use block_on from futures-lite or similar mechanism
            // or a more manual approach using runtime handles
            let rt = tokio::runtime::Handle::current();
            match rt.block_on(self_clone.current_user.read()) {
                user_guard => user_guard.clone(),
            }
        });

        let self_clone = self.clone();
        let get_current_device = Arc::new(move || {
            let rt = tokio::runtime::Handle::current();
            match rt.block_on(self_clone.current_device.read()) {
                device_guard => device_guard.clone(),
            }
        });
        let service_context = Arc::new(ServiceContext {
            auth_service: self.auth_service.clone(),
            user_service: self.user_service.clone(),
            sync_service: self.sync_service.clone(),
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

    /// Starts listening for incoming connections
    pub async fn start_listening(&self) -> Result<(), P2PError> {
        // Ensure P2P is initialized
        self.ensure_initialized().await?;

        info!("Starting P2P listener");

        let state_guard = self.state.lock().await;
        let state = state_guard.as_ref().ok_or(P2PError::NotInitialized)?;
        let endpoint = state.endpoint.clone();
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
                                                .perform_handshake_and_create_peer(&conn, false, None)
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

    /// Gets a connection ticket that can be used to connect to this node
    #[instrument(skip(self), level = "debug")]
    pub async fn get_connection_ticket(&self) -> Result<String, P2PError> {
        debug!("Generating connection ticket");

        self.ensure_initialized().await?;
        let state_guard = self.state.lock().await;
        let state = state_guard.as_ref().ok_or(P2PError::NotInitialized)?;

        let node_addr = match state.endpoint.node_addr().await {
            Ok(addr) => {
                debug!(
                    "Got node address with {} direct addresses",
                    addr.direct_addresses.len()
                );
                addr
            }
            Err(e) => {
                error!("Failed to get node address: {}", e);
                return Err(P2PError::Connection(e.to_string()));
            }
        };

        let addrs = node_addr
            .direct_addresses
            .into_iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>();

        debug!("Addresses included in ticket: {:?}", addrs);

        let ticket = ConnectionTicket {
            node_id: state.endpoint.node_id().to_string(),
            addresses: addrs,
        };

        match serde_json::to_string(&ticket) {
            Ok(ticket_str) => {
                info!("Connection ticket generated successfully");
                trace!("Ticket content: {}", ticket_str);
                Ok(ticket_str)
            }
            Err(e) => {
                error!("Failed to serialize connection ticket: {}", e);
                Err(P2PError::Serialization(e.to_string()))
            }
        }
    }

    /// Starts a user synchronization with the specified user
    #[instrument(skip(self), fields(user_id = %user_id), level = "info")]
    pub async fn start_user_sync(&self, user_id: &str) -> Result<(), P2PError> {
        info!("Starting user sync with user: {}", user_id);

        // Find all connections for this user
        let state_guard = self.state.lock().await;
        let state = state_guard.as_ref().ok_or(P2PError::NotInitialized)?;
        let connections = state.connections.get_connections_by_user(user_id).await;

        if connections.is_empty() {
            let err = format!("No connections found for user: {}", user_id);
            warn!("{}", err);
            return Err(P2PError::PeerConnection(err));
        }

        debug!("Found {} connections for user", connections.len());

        // For now, just use the first connection
        let connection = &connections[0];
        debug!("Using connection: {}", connection.get_id());

        // Start the sync process
        // match connection.start_user_sync().await {
        //     Ok(_) => {
        //         info!("User sync initiated successfully");
        //         Ok(())
        //     }
        //     Err(e) => {
        //         error!("Failed to start user sync: {}", e);
        //         Err(P2PError::SyncService(e))
        //     }
        // }
        todo!()
    }

    /// Adds a device to the network
    #[instrument(skip(self, records, ticket), fields(ticket_len = ticket.len()), level = "info")]
    pub async fn add_device(
        &self,
        records: SyncPayload,
        ticket: String,
    ) -> Result<Arc<PeerConnection>, P2PError> {
        info!("Adding device using ticket");

        match self
            .connect_with_ticket(&ticket, ConnectionType::Device, None)
            .await?
        {
            Some(peer_connection) => {
                // New connection was established, send the AddDevice message
                info!("Connection established, sending AddDevice message");
                match peer_connection
                    .send_message(Message::AddDevice(records))
                    .await
                {
                    Ok(_) => {
                        info!("Device addition initiated successfully");
                        Ok(peer_connection)
                    }
                    Err(e) => {
                        error!("Failed to send AddDevice message: {}", e);
                        Err(P2PError::Message(e))
                    }
                }
            }
            None => {
                // Connection already exists or is being established
                error!("Connection already exists or is being established");
                Err(P2PError::Connection(
                    "Connection already exists or is being established".into(),
                ))
            }
        }
    }

    /// Starts device synchronization with a specific connection
    #[instrument(skip(self), fields(connection_id = %connection_id), level = "info")]
    pub async fn start_device_sync(&self, connection_id: &str) -> Result<(), P2PError> {
        info!("Starting device sync with connection: {}", connection_id);

        let state_guard = self.state.lock().await;
        let state = state_guard.as_ref().ok_or(P2PError::NotInitialized)?;

        let connection = match state.connections.get_peer_connection(connection_id).await {
            Ok(conn) => {
                debug!("Found connection: {}", conn.get_id());
                conn
            }
            Err(e) => {
                error!("Connection not found: {}", e);
                return Err(P2PError::PeerConnection(e));
            }
        };

        // Start the sync process
        match connection.start_device_sync().await {
            Ok(_) => {
                info!("Device sync initiated successfully");
                Ok(())
            }
            Err(e) => {
                error!("Failed to start device sync: {}", e);
                Err(P2PError::SyncService(e))
            }
        }
    }

    /// Starts device synchronization with all devices of a user
    #[instrument(skip(self), fields(user_id = %user_id), level = "info")]
    pub async fn start_device_sync_for_user(&self, user_id: &str) -> Result<(), P2PError> {
        info!("Starting device sync for user: {}", user_id);

        let state_guard = self.state.lock().await;
        let state = state_guard.as_ref().ok_or(P2PError::NotInitialized)?;

        // Find all connections for this user
        let connections = state.connections.get_connections_by_user(user_id).await;

        if connections.is_empty() {
            let err = format!("No connections found for user: {}", user_id);
            warn!("{}", err);
            return Err(P2PError::PeerConnection(err));
        }

        debug!("Found {} connections for user", connections.len());

        // Start sync on all devices
        let mut errors = Vec::new();
        for connection in connections {
            debug!("Starting sync for device: {}", connection.device.id);
            if let Err(e) = connection.start_device_sync().await {
                let err_msg = format!("Failed to sync device {}: {}", connection.get_id(), e);
                error!("{}", err_msg);
                errors.push(err_msg);
            }
        }

        if errors.is_empty() {
            info!("Device sync initiated successfully for all devices");
            Ok(())
        } else {
            let err = format!("Sync errors: {}", errors.join(", "));
            error!("{}", err);
            Err(P2PError::SyncService(err))
        }
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
}
