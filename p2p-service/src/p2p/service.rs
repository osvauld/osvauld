use crate::p2p::constants::*;
use crate::p2p::emitter::{P2PEvent, P2PEventEmitter};
use iroh::{endpoint::Connection, Endpoint, NodeAddr, RelayMode, SecretKey};
use log::{error, info};
use osvauld_core::models::device::Device;
use osvauld_core::models::p2p::{ConnectionTicket, ConnectionType};
use osvauld_core::models::user::User;
use osvauld_services::{AuthService, ShareService, SyncService, UserService};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tokio::time::timeout;

pub struct P2PState {
    pub endpoint: Arc<Endpoint>,
    pub active_connection: Arc<Mutex<Option<Arc<Connection>>>>,
    pub connection_type: Arc<Mutex<Option<ConnectionType>>>,
}

#[derive(Clone)]
pub struct P2PService {
    pub state: Arc<Mutex<Option<P2PState>>>,
    pub is_initiator: Arc<Mutex<Option<bool>>>,
    pub device: Arc<Mutex<Option<Device>>>,
    pub user: Arc<Mutex<Option<User>>>,
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
            is_initiator: Arc::new(Mutex::new(None)),
            device: Arc::new(Mutex::new(None)),
            user: Arc::new(Mutex::new(None)),
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

        *state = Some(P2PState {
            endpoint: Arc::new(endpoint),
            active_connection: Arc::new(Mutex::new(None)),
            connection_type: Arc::new(Mutex::new(None)),
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
        let active_connection = state.active_connection.clone();
        let self_clone = self.clone(); // Clone self for use in the spawned task

        tokio::spawn(async move {
            info!("Starting listener for incoming connections");
            while let Some(incoming) = endpoint.accept().await {
                match incoming.accept() {
                    Ok(connecting) => {
                        info!("Accepting incoming connection");

                        let active_connection = active_connection.clone();
                        let self_clone = self_clone.clone();

                        tokio::spawn(async move {
                            match timeout(CONNECTION_TIMEOUT, connecting).await {
                                Ok(Ok(conn)) => {
                                    info!("Connection established, initiating handshake");
                                    // Perform handshake as the receiver (non-initiator)
                                    match self_clone.perform_handshake(&conn, false).await {
                                        Ok(()) => {
                                            info!("Handshake completed successfully");

                                            let mut active_conn = active_connection.lock().await;
                                            *active_conn = Some(Arc::new(conn));
                                        }
                                        Err(e) => {
                                            error!("Handshake failed: {}", e);
                                        }
                                    }
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

    // Helper methods used across different functionality
    pub async fn get_active_connection(&self) -> Result<Arc<Connection>, String> {
        let state_guard = self.state.lock().await;
        let state = state_guard.as_ref().ok_or("P2P not initialized")?;
        let active_conn = state.active_connection.lock().await;
        active_conn
            .clone()
            .ok_or_else(|| "No active connection".to_string())
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
}
