//! P2P Handlers - Using new Transport + Courier architecture
//!
//! These handlers use:
//! - Butler for all storage and identity operations
//! - CourierHandle for sending P2P commands
//! - Transport for P2P connectivity

use crate::types::BaseCryptoResponse;
use butler::Butler;
use courier::{Courier, CourierHandle, CourierMode, HandshakeServices};
use tracing::{info, instrument};
use std::sync::Arc;
use tauri::State;
use tokio::sync::RwLock;
use transport::{Transport, TransportConfig};

/// P2P state wrapper - holds CourierHandle after initialization
pub struct P2PState {
    pub handle: RwLock<Option<CourierHandle>>,
}

impl P2PState {
    pub fn new() -> Self {
        Self {
            handle: RwLock::new(None),
        }
    }

    pub async fn get_handle(&self) -> Result<CourierHandle, String> {
        self.handle
            .read()
            .await
            .clone()
            .ok_or_else(|| "P2P not initialized. Call start_p2p_listener first.".to_string())
    }

    pub async fn set_handle(&self, handle: CourierHandle) {
        let mut guard = self.handle.write().await;
        *guard = Some(handle);
    }
}

impl Default for P2PState {
    fn default() -> Self {
        Self::new()
    }
}

/// Start P2P listener with the user's device key
///
/// This must be called after login when the Herald identity is available.
/// Initializes Transport + Courier and stores the handle.
#[tauri::command]
#[instrument(skip_all)]
pub async fn start_p2p_listener(
    butler: State<'_, Arc<Butler>>,
    p2p_state: State<'_, P2PState>,
) -> Result<BaseCryptoResponse, String> {
    // Check if already initialized
    if p2p_state.handle.read().await.is_some() {
        info!("P2P already initialized");
        return Ok(BaseCryptoResponse::Success);
    }

    // Get device key from Butler (requires login)
    let device_key = butler.device_key().await
        .map_err(|e| format!("Failed to get device key: {}", e))?;

    info!("Initializing P2P with device key");

    // Initialize Transport
    let config = TransportConfig::new(device_key);
    let (transport, transport_rx) = Transport::init(config)
        .await
        .map_err(|e| format!("Failed to initialize transport: {}", e))?;

    let transport = Arc::new(transport);

    // Get our node ID for logging
    let node_id = transport.node_id();
    info!("Transport initialized with node ID: {}", node_id);

    // Create HandshakeServices with Butler (owns identity and all services)
    let handshake_services = Arc::new(HandshakeServices::new(butler.inner().clone()));

    // Initialize Courier with services for auto-processing
    let (handle, mut event_rx, courier) = Courier::init_with_services(
        CourierMode::User,
        transport,
        Some(handshake_services),
    );

    // Spawn courier event loop
    tokio::spawn(async move {
        courier.run(transport_rx).await;
    });

    // Spawn event consumer (events are auto-processed, but we still need to drain the channel)
    tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            info!("P2P event: {:?}", event);
        }
    });

    // Store handle in state (clone for auto-reconnect)
    let handle_for_reconnect = handle.clone();
    p2p_state.set_handle(handle).await;

    info!("P2P listener started successfully");

    // Auto-reconnect to known nodes with stored permits
    let user_info = butler.user_info().await;
    if let Ok(user) = user_info {
        if let Ok(nodes) = butler.list_sovereign_nodes() {
            info!("Found {} sovereign nodes for potential reconnection", nodes.len());
            for node in nodes {
                if let Some(our_permit) = &node.our_permit {
                    info!(
                        "Auto-reconnecting to {} (node_id={}, permit_preview={}...)",
                        node.username,
                        node.node_id,
                        &our_permit[..our_permit.len().min(30)]
                    );
                    let result = handle_for_reconnect
                        .reconnect(
                            &node.node_id,
                            &user.did,
                            &user.username,
                            &user.public_key,
                            our_permit,
                        )
                        .await;

                    match result {
                        Ok(_) => info!("Reconnection initiated to {}", node.username),
                        Err(e) => info!("Failed to reconnect to {}: {}", node.username, e),
                    }
                } else {
                    info!(
                        "Skipping {} - no our_permit stored (first connection incomplete?)",
                        node.username
                    );
                }
            }
        }
    }

    Ok(BaseCryptoResponse::Success)
}

/// Add a sovereign node and initiate handshake
///
/// Flow:
/// 1. Parse connection string using Butler
/// 2. Store sovereign node info
/// 3. Use CourierHandle to connect and send Hello
#[tauri::command]
#[instrument(skip_all)]
pub async fn handle_add_sovereign_node(
    input: String,
    butler: State<'_, Arc<Butler>>,
    p2p_state: State<'_, P2PState>,
) -> Result<BaseCryptoResponse, String> {
    info!("Adding sovereign node from connection string");

    // Get CourierHandle (must be initialized)
    let courier_handle = p2p_state.get_handle().await?;

    // 1. Parse and store sovereign node via Butler
    let sovereign_node = butler
        .add_sovereign_node(&input)
        .map_err(|e| format!("Failed to add sovereign node: {}", e))?;

    info!(
        username = %sovereign_node.username,
        node_id = %sovereign_node.node_id,
        "Sovereign node saved"
    );

    // 2. Get our identity info for the Hello message via Butler
    let user = butler.user_info().await
        .map_err(|e| format!("Failed to get user info: {}", e))?;

    // 3. Connect via Courier (sends Hello with permit)
    // Courier handles node_id parsing internally
    courier_handle
        .connect(
            &sovereign_node.node_id,
            &user.did,
            &user.username,
            &user.public_key,
            &sovereign_node.their_permit,
        )
        .await
        .map_err(|e| format!("Failed to connect: {}", e))?;

    info!("Connection initiated to sovereign node");
    Ok(BaseCryptoResponse::Success)
}

/// Handle viewer connecting to a website/node
///
/// Flow:
/// 1. Parse connection string via Butler
/// 2. Store node info
/// 3. Initiate connection
#[tauri::command]
#[instrument(skip_all)]
pub async fn handle_connect_to_website(
    input: String,
    butler: State<'_, Arc<Butler>>,
    p2p_state: State<'_, P2PState>,
) -> Result<BaseCryptoResponse, String> {
    info!("Viewer connecting to website");

    // Get CourierHandle (must be initialized)
    let courier_handle = p2p_state.get_handle().await?;

    // 1. Parse and store node via Butler (reuse sovereign node storage)
    let node = butler
        .add_sovereign_node(&input)
        .map_err(|e| format!("Failed to add node: {}", e))?;

    info!(username = %node.username, "Node saved for viewer connection");

    // 2. Get our identity via Butler
    let user = butler.user_info().await
        .map_err(|e| format!("Failed to get user info: {}", e))?;

    // 3. Connect via Courier (handles node_id parsing internally)
    courier_handle
        .connect(
            &node.node_id,
            &user.did,
            &user.username,
            &user.public_key,
            &node.their_permit,
        )
        .await
        .map_err(|e| format!("Failed to connect: {}", e))?;

    info!("Viewer connection initiated");
    Ok(BaseCryptoResponse::Success)
}

/// Publish a space to a specific sovereign node
///
/// **Flow**: Thin handler - delegates to Courier (orchestrator)
/// Courier handles: get identity from Butler, prepare space/pages, transform to transport types, send
#[tauri::command]
#[instrument(skip_all, fields(space_id = %space_id, node_id = %node_id))]
pub async fn handle_publish_space(
    space_id: String,
    node_id: String,
    p2p_state: State<'_, P2PState>,
) -> Result<BaseCryptoResponse, String> {
    info!("Publishing space to node {}", node_id);

    // Get CourierHandle (must be initialized)
    let courier_handle = p2p_state.get_handle().await?;

    // Delegate to Courier - it gets identity from Butler and orchestrates everything
    courier_handle
        .publish_space(&space_id, &node_id)
        .await
        .map_err(|e| format!("Failed to publish space: {}", e))?;

    info!("Space published successfully");
    Ok(BaseCryptoResponse::Success)
}
