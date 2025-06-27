use crate::p2p::connection_manager::ConnectionManager;
use crate::p2p::constants::*;
use crate::p2p::emitter::{P2PEvent, P2PEventEmitter};
use crate::p2p::errors::P2PError;
use crate::p2p::incoming::{IncomingEvent, P2PSender};
use crate::p2p::logger;
use crate::p2p::peer_connection::{PeerConnection, ServiceContext};
use iroh::{Endpoint, RelayMode, SecretKey};
use osvauld_core::models::{User, ConnectionTicket, Device};
use osvauld_db::database::RepositoryContext;
use osvauld_services::{AuthService, SyncService, UserService};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::sync::{Mutex, mpsc};
use tokio::time::timeout;
use n0_watcher::Watcher;
use tracing::{Instrument, debug, error, info, info_span, instrument, trace, warn};
use crypto_utils::CryptoUtils;

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
        sync_service: Arc<SyncService>,
        auth_service: Arc<AuthService>,
        user_service: Arc<UserService>,
        repo_ctx: RepositoryContext,
        crypto_utils: Arc<Mutex<CryptoUtils>>
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
            repo_ctx,
            crypto_utils
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

    /// Starts listening for inceming connections
    pub async fn start_listening(&self) -> Result<(), P2PError> {

        info!("Starting P2P listener");

        let state_guard = self.state.lock().await;
        let state = state_guard.as_ref().ok_or(P2PError::NotInitialized)?;
        debug!("Listener using endpoint with node ID: {}", state.endpoint.node_id());
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

    /// Gets a connection ticket that can be used to connect to this node
    #[instrument(skip(self), level = "debug")]
pub async fn get_connection_ticket(&self) -> Result<String, P2PError> {
    debug!("Generating connection ticket");
    self.ensure_initialized().await?;
    
    let ticket = {
        let state_guard = self.state.lock().await;
        let state = state_guard.as_ref().ok_or(P2PError::NotInitialized)?;
        
        // Get addresses while holding the lock
        let direct_addresses = state.endpoint
            .direct_addresses()
            .initialized()
            .await
            .map_err(|e| {
                error!("Failed to get initialized node address: {}", e);
                P2PError::Connection(e.to_string())
            })?;
        
        let addrs = direct_addresses
            .into_iter()
            .map(|addr| addr.addr.to_string())
            .collect::<Vec<_>>();
        
        debug!("Addresses included in ticket: {:?}", addrs);
        
        ConnectionTicket {
            node_id: state.endpoint.node_id().to_string(),
            addresses: addrs,
        }
    };
    
    self.start_listening().await?;
    
    match serde_json::to_string(&ticket) {
        Ok(ticket_str) => {
            info!("Connection ticket generated successfully {}", ticket_str);
            trace!("Ticket content: {}", ticket_str);
            Ok(ticket_str)
        }
        Err(e) => {
            error!("Failed to serialize connection ticket: {}", e);
            Err(P2PError::Serialization(e.to_string()))
        }
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
}
