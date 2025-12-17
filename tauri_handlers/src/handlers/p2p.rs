//! P2P Handlers - Using new Transport + Courier architecture
//!
//! These handlers use:
//! - Butler for all storage and identity operations
//! - CourierHandle for sending P2P commands
//! - Transport for P2P connectivity

use crate::events::spawn_event_bridge;
use crate::types::BaseCryptoResponse;
use butler::{Butler, ConnectionType, SyncEvent};
use courier::{Courier, CourierHandle, CourierMode, HandshakeServices};
use tracing::{info, warn, instrument};
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
    app: tauri::AppHandle,
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
    let handshake_services = Arc::new(HandshakeServices::new((*butler).clone()));

    // Initialize Courier with services for auto-processing
    let (handle, event_rx, courier) = Courier::init_with_services(
        CourierMode::User,
        transport,
        Some(handshake_services),
    );

    // Wire sync events: Butler → Coordinator
    // This allows Scribe actors to trigger sync when updates happen
    let (sync_event_tx, mut sync_event_rx) = tokio::sync::mpsc::channel::<SyncEvent>(64);
    butler.set_sync_event_tx(sync_event_tx).await;

    // Spawn sync event bridge - forwards SyncEvent to Coordinator
    let handle_for_sync = handle.clone();
    tokio::spawn(async move {
        info!("Sync event bridge started");
        while let Some(event) = sync_event_rx.recv().await {
            match event {
                SyncEvent::EnsureSync { user_did } => {
                    if let Err(e) = handle_for_sync.ensure_sync(&user_did) {
                        warn!(error = %e, user_did = %user_did, "Failed to forward EnsureSync");
                    }
                }
            }
        }
        info!("Sync event bridge stopped");
    });

    // Spawn courier event loop
    tokio::spawn(async move {
        courier.run(transport_rx).await;
    });

    // Spawn event bridge - translates CourierEvent to Tauri events
    // Also handles ConnectRequested by auto-connecting (for sync flow)
    spawn_event_bridge(event_rx, app, handle.clone());

    // Store handle in state (clone for auto-reconnect)
    let handle_for_reconnect = handle.clone();
    p2p_state.set_handle(handle).await;

    info!("P2P listener started successfully");

    // Auto-reconnect to known OWNER nodes with stored permits
    // Viewer connections don't auto-reconnect
    if let Ok(nodes) = butler.list_sovereign_nodes() {
        let owner_nodes: Vec<_> = nodes.iter()
            .filter(|n| n.connection_type == ConnectionType::Owner)
            .collect();
        info!("Found {} owner nodes for potential reconnection (skipping {} viewer nodes)",
            owner_nodes.len(),
            nodes.len() - owner_nodes.len()
        );
        for node in owner_nodes {
            if let Some(permit) = &node.permit {
                info!(
                    "Auto-reconnecting to {} (node_id={})",
                    node.name,
                    node.node_id,
                );
                // Use connect_and_wait_for_auth with the stored permit
                let result = handle_for_reconnect.connect_and_wait_for_auth(&node.node_id, permit).await;

                match result {
                    Ok(_) => info!("Reconnection successful to {}", node.name),
                    Err(e) => info!("Failed to reconnect to {}: {}", node.name, e),
                }
            } else {
                info!(
                    "Skipping {} - no permit stored (first connection incomplete?)",
                    node.name
                );
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
        name = %sovereign_node.name,
        node_id = %sovereign_node.node_id,
        "Sovereign node saved"
    );

    // 2. Connect and initiate handshake with the permit
    let permit = sovereign_node.permit.clone()
        .ok_or("SovereignNode missing permit")?;

    courier_handle
        .connect_and_wait_for_auth(&sovereign_node.node_id, &permit)
        .await
        .map_err(|e| format!("Failed to connect: {}", e))?;

    info!("Connection and handshake completed with sovereign node");
    Ok(BaseCryptoResponse::Success)
}

/// Handle viewer connecting to a website/node
///
/// Flow:
/// 1. Parse connection string
/// 2. Extract space_id from permit
/// 3. Connect and wait for authentication
/// 4. Store node as Contact (type=Node) for future reconnection
/// 5. Request space as viewer (triggers space + page sync)
#[tauri::command]
#[instrument(skip_all)]
pub async fn handle_connect_to_website(
    input: String,
    butler: State<'_, Arc<Butler>>,
    p2p_state: State<'_, P2PState>,
) -> Result<BaseCryptoResponse, String> {
    info!("Viewer connecting to website");

    let courier_handle = p2p_state.get_handle().await?;

    // 1. Parse connection string
    let conn = butler
        .parse_connection_string(&input)
        .map_err(|e| format!("Failed to parse connection string: {}", e))?;

    let node_id = conn.node_id();
    let permit = conn.permit.clone();

    info!(name = %conn.name, node_id = %node_id, "Viewer connecting to node");

    // 2. Extract space_id from permit
    let parsed_permit = gurkha::Permit::from_token(&permit)
        .map_err(|e| format!("Invalid permit: {}", e))?;
    let space_id = parsed_permit.space_id()
        .ok_or("Permit missing space_id")?;

    // 3. Connect and wait for authentication
    let peer_id = courier_handle
        .connect_and_wait_for_auth(&node_id, &permit)
        .await
        .map_err(|e| format!("Connection failed: {}", e))?;

    info!("Viewer authenticated with node {}", peer_id);

    // 4. Store node as Contact (type=Node) for future reconnection
    // This allows viewer to reconnect later for updates
    // Convert base64 public key to DID format for consistent storage
    let node_did = herald::Identity::did_from_base64_pubkey(&conn.node_public_key)
        .map_err(|e| format!("Invalid node public key: {}", e))?;
    butler
        .add_node_contact(
            &node_did,                  // Node's DID (converted from base64)
            &conn.node_encryption_key,  // Node's encryption key (for ECDH)
            &conn.name,                 // Node name
            &node_id,                   // Iroh NodeId
            &permit,                    // Viewer permit
        )
        .map_err(|e| format!("Failed to store node contact: {}", e))?;

    info!("Stored node {} as contact for future reconnection", conn.name);

    // 5. Request space as viewer (triggers space + page sync)
    courier_handle
        .request_space_as_viewer(&space_id, &node_id, &permit)
        .await
        .map_err(|e| format!("Failed to request space: {}", e))?;

    info!("Space request sent for {}", space_id);
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

/// Get shareable link for a space from a specific node
///
/// **Flow**: Request node to generate a viewer permit with aud:* (wildcard audience)
/// Response comes asynchronously via folder-token-received event
#[tauri::command]
#[instrument(skip_all, fields(space_id = %space_id, node_id = %node_id))]
pub async fn handle_get_share_link(
    space_id: String,
    node_id: String,
    p2p_state: State<'_, P2PState>,
) -> Result<BaseCryptoResponse, String> {
    info!("Requesting shareable link for space {} from node {}", space_id, node_id);

    // Get CourierHandle (must be initialized)
    let courier_handle = p2p_state.get_handle().await?;

    // Request shareable link via Courier
    // Response will come via ShareableLinkReceived -> folder-token-received event
    courier_handle
        .get_shareable_link(&space_id, &node_id)
        .await
        .map_err(|e| format!("Failed to request shareable link: {}", e))?;

    info!("Share link request sent");
    Ok(BaseCryptoResponse::Success)
}
