use crate::p2p::{
    auth,
    connection_manager::ConnectionManager,
    constants::*,
    emitter::{P2PEvent, P2PEventEmitter},
    errors::{ConnectionError, P2PError, P2PResult},
    peer_connection::{PeerConnection, ServiceContext},
};

use crypto_utils::CryptoUtils;
use iroh::endpoint::Connection;
use iroh::{Endpoint, NodeAddr, NodeId};
use osvauld_core::models::{Device, Message, User};
use persistance::database::RepositoryContext;
use services::generate_challenge;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, RwLock};
use tracing::{debug, error, info, instrument};

pub struct P2PState {
    pub endpoint: Arc<Endpoint>,
    pub connections: ConnectionManager,
    pub service_context: Arc<ServiceContext>,
}

#[derive(Clone)]
pub struct P2PService {
    pub state: Arc<Mutex<Option<P2PState>>>,
    pub crypto_utils: Arc<RwLock<CryptoUtils>>,
    pub ucan_service: Arc<RwLock<gurkha::UcanService>>,
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
        ucan_service: Arc<RwLock<gurkha::UcanService>>,
        domain: Arc<String>,
    ) -> (Self, mpsc::UnboundedReceiver<P2PEvent>) {
        let (emitter, receiver) = P2PEventEmitter::new();
        info!("Creating new P2P service instance");

        let service = Self {
            state: Arc::new(Mutex::new(None)),
            event_emitter: emitter,
            current_user: Arc::new(RwLock::new(None)),
            current_device: Arc::new(RwLock::new(None)),
            repo_ctx,
            crypto_utils,
            ucan_service,
            domain,
        };

        debug!("P2P service instance created successfully");
        (service, receiver)
    }

    /// Ensures the P2P service is initialized
    async fn ensure_initialized(&self) -> P2PResult<()> {
        let state_guard = self.state.lock().await;
        if state_guard.is_none() {
            return Err(P2PError::NotInitialized);
        }
        Ok(())
    }

    /// Get current user
    pub async fn get_current_user(&self) -> P2PResult<User> {
        let user_guard = self.current_user.read().await;
        user_guard
            .clone()
            .ok_or(P2PError::Custom("Current user not set".to_string()))
    }

    /// Get current device
    pub async fn get_current_device(&self) -> P2PResult<Device> {
        let device_guard = self.current_device.read().await;
        device_guard
            .clone()
            .ok_or(P2PError::Custom("Current device not set".to_string()))
    }

    /// Get an existing connection by ID
    pub async fn get_connection_by_id(
        &self,
        connection_id: &str,
    ) -> P2PResult<Arc<PeerConnection>> {
        self.ensure_initialized().await?;

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

    /// Connect to a peer using their device ID
    /// Simplified version - just connects, handshakes, and returns the connection
    #[instrument(skip(self), fields(device_id = %device_id), level = "info")]
    pub async fn connect_with_ticket(
        &self,
        device_id: &str,
    ) -> P2PResult<Option<Arc<PeerConnection>>> {
        info!("Starting connection process");

        // Convert device_id (public key) to NodeId
        let node_id = if let Ok(id) = NodeId::from_str(device_id) {
            id
        } else {
            let node_id_bytes = crypto_utils::derive_node_id_from_public_key(device_id)?;
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

        // Check if connection already exists
        if connections.is_connection_active(&connection_id).await {
            if let Ok(existing_connection) = connections.get_peer_connection(&connection_id).await
            {
                info!("Using existing connection: {}", &connection_id);
                return Ok(Some(existing_connection));
            } else {
                info!("Connection is being established: {}", &connection_id);
                return Ok(None);
            }
        }

        // Mark as connecting
        connections
            .mark_as_connecting(&connection_id)
            .await
            .map_err(|e| {
                P2PError::Connection(ConnectionError::AlreadyConnecting { connection_id: e })
            })?;

        // Connect to peer
        let node_addr = NodeAddr::new(node_id.clone());
        debug!("Connecting to: {:?}", node_addr);

        let connect_result = endpoint.connect(node_addr.clone(), ALPN_PROTOCOL).await;

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

        // Perform handshake and create peer connection
        info!("Starting handshake");
        let handshake_result = self.perform_handshake_and_create_peer(&conn, true, Some(device_id)).await;

        match handshake_result {
            Ok(peer_connection) => {
                info!("Handshake complete, connection ready");
                Ok(Some(peer_connection))
            }
            Err(e) => {
                connections.remove_from_connecting(&connection_id).await;
                Err(e)
            }
        }
    }

    /// Send a message to a peer, reconnecting if necessary
    #[instrument(skip(self, message), fields(device_id = %device_id), level = "info")]
    pub async fn send_or_reconnect(
        &self,
        device_id: &str,
        message: Message,
    ) -> P2PResult<()> {
        info!("Sending message to peer");

        // Convert device_id to node_id
        let node_id = if let Ok(id) = NodeId::from_str(device_id) {
            id
        } else {
            let node_id_bytes = crypto_utils::derive_node_id_from_public_key(device_id)?;
            NodeId::try_from(&node_id_bytes).map_err(|e| {
                P2PError::Connection(ConnectionError::InvalidNodeId {
                    node_id: e.to_string(),
                })
            })?
        };

        let connection_id = node_id.to_string();

        // Try to get existing connection
        if let Ok(connection) = self.get_connection_by_id(&connection_id).await {
            debug!("Using existing connection");
            return connection.send_message(message).await;
        }

        // No existing connection, reconnect
        info!("No active connection, reconnecting...");
        if let Some(connection) = self.connect_with_ticket(device_id).await? {
            connection.send_message(message).await
        } else {
            Err(P2PError::Custom("Failed to establish connection".to_string()))
        }
    }

    /// Perform handshake and create peer connection
    /// Used by both outgoing (initiator) and incoming connections
    ///
    /// # Arguments
    /// * `conn` - The established connection
    /// * `is_initiator` - True if we initiated the connection
    /// * `device_id` - Required if is_initiator=true to look up peer's token
    pub async fn perform_handshake_and_create_peer(
        &self,
        conn: &Connection,
        is_initiator: bool,
        device_id: Option<&str>,
    ) -> P2PResult<Arc<PeerConnection>> {
        let current_user = self.get_current_user().await?;
        let current_device = self.get_current_device().await?;

        let state_guard = self.state.lock().await;
        let state = state_guard.as_ref().ok_or(P2PError::NotInitialized)?;

        let node_id = conn.remote_node_id().map_err(|e| {
            P2PError::Custom(format!("Failed to get remote node id: {}", e))
        })?;

        let challenge = generate_challenge();
        let connection_id = node_id.to_string();

        // Create peer connection
        let peer_connection = PeerConnection::new(
            Arc::new(conn.clone()),
            is_initiator,
            state.service_context.clone(),
            self.event_emitter.clone(),
            None, // on_close callback
            self.crypto_utils.clone(),
            self.ucan_service.clone(),
            self.repo_ctx.clone(),
            connection_id.clone(),
            current_user.clone(),
            current_device.clone(),
            challenge,
            self.domain.to_string(),
        );

        let peer_connection = Arc::new(peer_connection);

        // Add to connection manager
        state
            .connections
            .insert_connection(peer_connection.clone())
            .await
            .map_err(|e| P2PError::InvalidState(e))?;

        // Initiate handshake if we're the initiator
        if is_initiator {
            // Get the peer's user record to retrieve their UCAN token
            // The token could be either one-time (first connection) or persistent (reconnection)
            // The receiving side will parse it and determine the flow

            let device_id = device_id.ok_or_else(|| {
                P2PError::InvalidState("device_id required for initiator handshake".to_string())
            })?;

            // Look up device to get user_id
            let devices = self.repo_ctx.device_repo.get_devices_by_ids(&[device_id.to_string()]).await
                .map_err(|e| P2PError::InvalidState(format!("Failed to get device: {}", e)))?;

            let device = devices.into_iter().next()
                .ok_or_else(|| P2PError::InvalidState(format!("Device {} not found", device_id)))?;

            // Get user to retrieve their ucan_token
            let peer_user = self.repo_ctx.user_repo.get_user_by_id(&device.user_id).await
                .map_err(|e| P2PError::InvalidState(format!("Failed to get user: {}", e)))?;

            info!("🔐 Initiating handshake with token from database");
            info!("  - Peer user: {}", peer_user.id);
            info!("  - Peer device: {}", device.id);

            auth::initiate_handshake(&peer_connection, peer_user.ucan_token, current_user, current_device)
                .await?;
        }

        Ok(peer_connection)
    }
}
