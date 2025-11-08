//! P2P Network Initialization Module
//!
//! Handles all P2P network initialization tasks including:
//! - Setting user/device context
//! - Binding Iroh endpoint
//! - Starting connection listener

use super::{
    connection_manager::ConnectionManager,
    constants::*,
    errors::{P2PError, P2PResult},
    p2p_service::{P2PService, P2PState},
    peer_connection::ServiceContext,
};
use iroh::{Endpoint, NodeId, RelayMode};
use osvauld_core::models::{Device, User};
use std::sync::Arc;
use tokio::time::timeout;
use tracing::{debug, error, info, info_span, instrument, warn, Instrument};

/// Sets the current user for the P2P service
#[instrument(skip_all, level = "debug")]
pub async fn set_current_user(service: &P2PService, user: User) {
    debug!("Setting current user: {}", user.username);
    let mut user_guard = service.current_user.write().await;
    *user_guard = Some(user);
    debug!("Current user set successfully");
}

/// Sets the current device for the P2P service
#[instrument(skip_all, level = "debug")]
pub async fn set_current_device(service: &P2PService, device: Device) {
    debug!("Setting current device: {}", device.id);
    let mut device_guard = service.current_device.write().await;
    *device_guard = Some(device);
    debug!("Current device set successfully");
}

/// Ensures the P2P service is initialized by binding the Iroh endpoint
#[instrument(skip(service), level = "debug")]
pub async fn ensure_initialized(service: &P2PService) -> P2PResult<()> {
    let mut state = service.state.lock().await;
    if state.is_some() {
        debug!("P2P service already initialized");
        return Ok(());
    }

    let key = service.repo_ctx.store_repo.get_node_key().await?;
    let node_public_key = service.repo_ctx.store_repo.get_device_key().await?;
    let node_id = crypto_utils::derive_node_id_from_public_key(&node_public_key)?;
    let node_id = NodeId::try_from(&node_id)
        .map_err(|e| P2PError::Configuration(format!("Invalid node ID: {}", e)))?;

    info!("Node ID: {}", node_id);

    let secret_key_bytes = {
        let crypto = service.crypto_utils.read().await;
        crypto.get_node_keypair(&key)?
    };
    let secret_key = iroh::SecretKey::from(secret_key_bytes);
    info!("Initializing P2P endpoint");

    debug!("Generated secret key for P2P endpoint");

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
        current_user: service.current_user.clone(),
        current_device: service.current_device.clone(),
    });

    *state = Some(P2PState {
        endpoint: Arc::new(endpoint),
        connections: ConnectionManager::new(),
        service_context,
    });

    info!("P2P initialization successful");
    Ok(())
}

/// Starts the P2P listener to accept incoming connections
#[instrument(skip(service), level = "info")]
pub async fn start_listening(service: &P2PService) -> P2PResult<()> {
    info!("Starting P2P listener");

    let endpoint = {
        let state_guard = service.state.lock().await;
        let state = state_guard.as_ref().ok_or(P2PError::NotInitialized)?;
        debug!(
            "Listener using endpoint with node ID: {}",
            state.endpoint.node_id()
        );
        state.endpoint.clone()
    };

    let service_clone = service.clone();

    tokio::spawn(
        async move {
            info!("Listener started for incoming connections");
            while let Some(incoming) = endpoint.accept().await {
                let connection_span = info_span!("incoming_connection",
                    remote = %incoming.remote_address());
                match incoming.accept() {
                    Ok(connecting) => {
                        info!(parent: &connection_span, "Accepting incoming connection");
                        let service_clone = service_clone.clone();

                        tokio::spawn(
                            async move {
                                debug!("Awaiting connection establishment");
                                match timeout(CONNECTION_TIMEOUT, connecting).await {
                                    Ok(Ok(conn)) => {
                                        info!("Connection established, initiating handshake");
                                        match service_clone
                                            .perform_handshake_and_create_peer(&conn, false)
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

/// Convenience method to initialize P2P network in one call
///
/// Steps performed:
/// 1. Set current user context
/// 2. Set current device context
/// 3. Initialize Iroh endpoint (bind to network)
/// 4. Start listener for incoming connections
#[instrument(skip_all, fields(user = %user.username, device = %device.id), level = "info")]
pub async fn initialize_p2p(
    service: &P2PService,
    user: &User,
    device: &Device,
) -> P2PResult<()> {
    info!("Initializing P2P network");

    set_current_user(service, user.clone()).await;
    set_current_device(service, device.clone()).await;
    ensure_initialized(service).await?;
    start_listening(service).await?;

    info!("P2P network initialized successfully");
    Ok(())
}
