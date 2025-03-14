use crate::p2p::connection_manager::ConnectionManager;
use crate::p2p::constants::*;
use crate::p2p::emitter::{P2PEvent, P2PEventEmitter};
use crate::p2p::peer_connection::{PeerConnection, ServiceContext};
use iroh::{Endpoint, RelayMode, SecretKey};
use log::{error, info};
use osvauld_core::models::p2p::{ConnectionTicket, ConnectionType, Message, SyncPayload};
use osvauld_core::models::user::User;
use osvauld_services::{AuthService, ShareService, SyncService, UserService};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tokio::time::timeout;

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
    pub share_service: Arc<ShareService>,
    pub event_emitter: P2PEventEmitter,
}

impl P2PService {
    pub fn new(
        sync_service: Arc<SyncService>,
        auth_service: Arc<AuthService>,
        user_service: Arc<UserService>,
        share_service: Arc<ShareService>,
    ) -> (Self, mpsc::UnboundedReceiver<P2PEvent>) {
        let (emitter, receiver) = P2PEventEmitter::new();
        let service = Self {
            state: Arc::new(Mutex::new(None)),
            sync_service,
            auth_service,
            user_service,
            share_service,
            event_emitter: emitter,
        };
        (service, receiver)
    }

    pub async fn ensure_initialized(&self) -> Result<(), String> {
        let mut state = self.state.lock().await;
        if state.is_some() {
            return Ok(());
        }

        info!("Initializing P2P endpoint");
        let secret_key = SecretKey::generate(rand::rngs::OsRng);
        let endpoint = Endpoint::builder()
            .secret_key(secret_key)
            .discovery_n0()
            .relay_mode(RelayMode::Default)
            .alpns(vec![ALPN_PROTOCOL.to_vec()])
            .bind()
            .await
            .map_err(|e| format!("Failed to bind endpoint: {}", e))?;
        let service_context = Arc::new(ServiceContext {
            auth_service: self.auth_service.clone(),
            user_service: self.user_service.clone(),
            sync_service: self.sync_service.clone(),
            share_service: self.share_service.clone(),
        });
        *state = Some(P2PState {
            endpoint: Arc::new(endpoint),
            connections: ConnectionManager::new(),
            service_context,
        });

        info!("P2P initialization successful");
        Ok(())
    }

    pub async fn start_listening(&self) -> Result<(), String> {
        // Ensure P2P is initialized
        self.ensure_initialized().await?;
        let state_guard = self.state.lock().await;
        let state = state_guard.as_ref().unwrap();
        let endpoint = state.endpoint.clone();
        let self_clone = self.clone(); // Clone self for use in the spawned task

        tokio::spawn(async move {
            info!("Starting listener for incoming connections");
            while let Some(incoming) = endpoint.accept().await {
                match incoming.accept() {
                    Ok(connecting) => {
                        info!("Accepting incoming connection");
                        let self_clone = self_clone.clone();

                        tokio::spawn(async move {
                            match timeout(CONNECTION_TIMEOUT, connecting).await {
                                Ok(Ok(conn)) => {
                                    info!("Connection established, initiating handshake");
                                    // Perform handshake as the receiver (non-initiator)
                                    let _ = self_clone
                                        .perform_handshake_and_create_peer(&conn, false, None)
                                        .await;
                                }
                                Ok(Err(e)) => error!("Connection failed: {}", e),
                                Err(e) => error!("Connection timeout: {}", e),
                            }
                        });
                    }
                    Err(e) => error!("Failed to accept connection: {}", e),
                }
            }
        });

        Ok(())
    }

    pub async fn get_connection_ticket(&self) -> Result<String, String> {
        self.ensure_initialized().await?;
        let state_guard = self.state.lock().await;
        let state = state_guard.as_ref().ok_or("P2P not initialized")?;
        let node_addr = state
            .endpoint
            .node_addr()
            .await
            .map_err(|e| e.to_string())?;
        let addrs = node_addr
            .direct_addresses
            .into_iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>();
        let ticket = ConnectionTicket {
            node_id: state.endpoint.node_id().to_string(),
            addresses: addrs,
        };

        serde_json::to_string(&ticket).map_err(|e| e.to_string())
    }

    pub async fn start_user_sync(&self, user_id: &str) -> Result<(), String> {
        info!("Starting user sync with user: {}", user_id);

        // Find all connections for this user
        let state_guard = self.state.lock().await;
        let state = state_guard.as_ref().ok_or("P2P not initialized")?;
        let connections = state.connections.get_connections_by_user(user_id).await;

        if connections.is_empty() {
            return Err(format!("No connections found for user: {}", user_id));
        }

        // For now, just use the first connection
        let connection = &connections[0];

        // Start the sync process
        connection.start_user_sync().await?;

        info!("User sync initiated successfully");
        Ok(())
    }

    // Method to establish first connection with a user
    pub async fn initiate_first_user_connection(
        &self,
        user: &User,
        ticket: &str,
    ) -> Result<Arc<PeerConnection>, String> {
        info!("Initiating first connection with user: {}", user.id);

        // Connect using the ticket
        let peer_connection = self
            .connect_with_ticket(ticket, ConnectionType::User)
            .await?;

        // Send the FirstUserConnection message
        peer_connection
            .send_message(Message::FirstUserConnection(user.clone()))
            .await?;

        info!("First user connection established successfully");
        Ok(peer_connection)
    }

    // Method to add a device
    pub async fn add_device(
        &self,
        records: SyncPayload,
        ticket: String,
    ) -> Result<Arc<PeerConnection>, String> {
        info!("Adding device using ticket");

        // First establish connection with the target device
        let peer_connection = self
            .connect_with_ticket(&ticket, ConnectionType::Device)
            .await?;

        // Once connected, send the AddDevice message
        info!("Connection established, sending AddDevice message");
        peer_connection
            .send_message(Message::AddDevice(records))
            .await?;

        info!("Device addition initiated successfully");
        Ok(peer_connection)
    }

    // Method to start device sync
    pub async fn start_device_sync(&self, connection_id: &str) -> Result<(), String> {
        info!("Starting device sync with connection: {}", connection_id);
        let state_guard = self.state.lock().await;
        let state = state_guard.as_ref().ok_or("P2P not initialized")?;
        let connection = state.connections.get_peer_connection(connection_id).await?;
        // Get the connection

        // Start the sync process
        connection.start_device_sync().await?;

        info!("Device sync initiated successfully");
        Ok(())
    }

    // Overload for starting device sync with all devices of a user
    pub async fn start_device_sync_for_user(&self, user_id: &str) -> Result<(), String> {
        info!("Starting device sync for user: {}", user_id);

        let state_guard = self.state.lock().await;
        let state = state_guard.as_ref().ok_or("P2P not initialized")?;
        // Find all connections for this user
        let connections = state.connections.get_connections_by_user(user_id).await;

        if connections.is_empty() {
            return Err(format!("No connections found for user: {}", user_id));
        }

        // Start sync on all devices
        let mut errors = Vec::new();
        for connection in connections {
            if let Err(e) = connection.start_device_sync().await {
                errors.push(format!(
                    "Failed to sync device {}: {}",
                    connection.get_id(),
                    e
                ));
            }
        }

        if errors.is_empty() {
            info!("Device sync initiated successfully for all devices");
            Ok(())
        } else {
            Err(format!("Sync errors: {}", errors.join(", ")))
        }
    }
}
