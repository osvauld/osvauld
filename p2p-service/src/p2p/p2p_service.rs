use crate::p2p::connection_manager::ConnectionManager;
use crate::p2p::constants::*;
use crate::p2p::emitter::{P2PEvent, P2PEventEmitter};
use crate::p2p::errors::P2PError;
use crate::p2p::incoming::{IncomingEvent, P2PSender};
use crate::p2p::logger;
use crate::p2p::peer_connection::{PeerConnection, ServiceContext};
use crypto_utils::CryptoUtils;
use iroh::{Endpoint, NodeId, RelayMode};
use n0_watcher::Watcher;
use osvauld_core::models::{ConnectionAction,  ConnectionType, Device, User};
use osvauld_db::database::RepositoryContext;
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
}
