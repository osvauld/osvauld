use crate::types::BaseCryptoResponse;
use crate::user_state::UserState;
use crypto_utils::CryptoUtils;
use tracing::{error, info, instrument};
use network::p2p_init;
use network::P2PService;
use persistance::database::RepositoryContext;
use services::add_known_user;
use std::sync::Arc;
use tauri::State;
use tokio::sync::RwLock;

/// Initialize P2P network after login
///
/// This handler initializes the P2P network by:
/// 1. Setting user/device context on P2PService
/// 2. Binding Iroh endpoint to the network
/// 3. Starting listener for incoming connections
///
/// Note: Does NOT auto-connect to known peers (manual connection only)
#[tauri::command]
#[instrument(skip(user_state, p2p_service))]
pub async fn start_p2p_listener(
    user_state: State<'_, UserState>,
    p2p_service: State<'_, Arc<P2PService>>,
) -> Result<BaseCryptoResponse, String> {

    // Get user and device from state
    let user = user_state.get_user().await?;
    let device = user_state.get_device().await?;

    // Call p2p_init module to initialize network
    p2p_init::initialize_p2p(&p2p_service, &user, &device)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to initialize P2P");
            format!("P2P initialization failed: {}", e)
        })?;

    info!("P2P network initialized successfully");
    Ok(BaseCryptoResponse::Success)
}

/// Add a sovereign node and establish connection
///
/// This handler adds a sovereign node to the database and immediately initiates
/// a P2P connection. The connection string contains:
/// - One-time UCAN token with role='owner' (from Kunki)
/// - Node's public keys and device information
///
/// Flow:
/// 1. Decode and parse connection string
/// 2. Save node to database with first_sync=false
/// 3. Initiate P2P connection immediately (happy path - no error handling)
/// 4. Handshake will exchange tokens with reciprocal roles
#[tauri::command]
#[instrument(skip_all)]
pub async fn handle_add_sovereign_node(
    input: String,
    crypto_utils: State<'_, Arc<RwLock<CryptoUtils>>>,
    repo_ctx: State<'_, Arc<RepositoryContext>>,
    p2p_service: State<'_, Arc<P2PService>>,
) -> Result<BaseCryptoResponse, String> {

    // 1. Parse connection string using shared service function
    let connection_data = services::parse_connection_string(&input)
        .map_err(|e| e.to_string())?;

    info!(username = %connection_data.username, "Parsed sovereign node details");

    // 2. Save node to database (first_sync=false)
    let (_user, device) = add_known_user(
        connection_data.username,
        connection_data.user_public_key,
        connection_data.device_public_key,
        connection_data.ucan_token,  // One-time token with role='owner'
        connection_data.ucan_pub_key,
        repo_ctx.inner().clone(),
        &crypto_utils,
    )
    .await
    .map_err(|e| format!("Failed to save sovereign node: {}", e))?;

    info!("Sovereign node saved to database");

    // 3. Connect immediately (happy path only)
    let node_device_id = device.id.clone();
    p2p_service
        .connect_with_ticket(&node_device_id)
        .await
        .map_err(|e| format!("Failed to connect to sovereign node: {}", e))?;

    info!("Successfully initiated connection to sovereign node");
    Ok(BaseCryptoResponse::Success)
}

/// Handle viewer connecting to a website/node
///
/// Flow:
/// 1. Parse connection string (includes folder_id for viewer connections)
/// 2. Derive user_id from public key (no DB lookup needed)
/// 3. Check if node already exists in viewer's database
/// 4. If not, add node to database
/// 5. Delegate to sync_handler to initiate P2P connection (fire-and-forget)
/// 6. Return success immediately (handshake happens in background)
#[tauri::command]
#[instrument(skip_all)]
pub async fn handle_connect_to_website(
    input: String,
    crypto_utils: State<'_, Arc<RwLock<CryptoUtils>>>,
    repo_ctx: State<'_, Arc<RepositoryContext>>,
    p2p_service: State<'_, Arc<P2PService>>,
) -> Result<BaseCryptoResponse, String> {

    // 1. Parse connection string
    let connection_data = services::parse_connection_string(&input)
        .map_err(|e| e.to_string())?;

    info!(username = %connection_data.username, "Viewer connecting to node");

    // 3. Derive user_id from public key (same as auth_service.rs:68)
    let user_id = crypto_utils::get_key_id(&connection_data.user_public_key)
        .map_err(|e| format!("Failed to derive user_id: {}", e))?;

    // Clone ucan_token before it gets moved
    let ucan_token = connection_data.ucan_token.clone();

    // 4. Check if node already exists
    let existing_user = repo_ctx
        .user_repo
        .get_user_by_id(&user_id)
        .await;

    let device_id = if existing_user.is_ok() {
        // Node exists, get first device
        info!(user_id = %user_id, "Node already exists in database, reusing connection");
        let devices = repo_ctx
            .device_repo
            .get_devices_by_user_id(&user_id)
            .await
            .map_err(|e| format!("Failed to get devices: {}", e))?;

        devices
            .first()
            .ok_or_else(|| "Node has no devices".to_string())?
            .id
            .clone()
    } else {
        // 5. Add node to viewer's database (first_sync=false)
        info!(user_id = %user_id, "Adding new node to viewer database");
        let (_, device) = add_known_user(
            connection_data.username,
            connection_data.user_public_key,
            connection_data.device_public_key,
            connection_data.ucan_token,
            connection_data.ucan_pub_key,
            repo_ctx.inner().clone(),
            &crypto_utils,
        )
        .await
        .map_err(|e| format!("Failed to add node user: {}", e))?;

        device.id
    };

    info!(device_id = %device_id, "Initiating viewer connection to node");

    // 6. Delegate to sync_handler to initiate connection (fire-and-forget)
    network::p2p::sync_handler::connect_to_website(
        device_id,
        ucan_token,
        p2p_service.inner().clone(),
    );

    info!("Viewer connection task spawned, returning success");

    // 7. Return success immediately (handshake happens in background)
    Ok(BaseCryptoResponse::Success)
}
