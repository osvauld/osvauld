use crate::types::{BaseCryptoResponse, UserDetails};
use crate::user_state::UserState;
use base64::{Engine as _, engine::general_purpose};
use crypto_utils::CryptoUtils;
use log::{error, info};
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
pub async fn start_p2p_listener(
    user_state: State<'_, UserState>,
    p2p_service: State<'_, Arc<P2PService>>,
) -> Result<BaseCryptoResponse, String> {
    info!("Initializing P2P network");

    // Get user and device from state
    let user = user_state.get_user().await?;
    let device = user_state.get_device().await?;

    // Call p2p_init module to initialize network
    p2p_init::initialize_p2p(&p2p_service, &user, &device)
        .await
        .map_err(|e| {
            error!("Failed to initialize P2P: {}", e);
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
pub async fn handle_add_sovereign_node(
    input: String,
    crypto_utils: State<'_, Arc<RwLock<CryptoUtils>>>,
    repo_ctx: State<'_, Arc<RepositoryContext>>,
    p2p_service: State<'_, Arc<P2PService>>,
) -> Result<BaseCryptoResponse, String> {
    info!("Adding sovereign node");

    // 1. Decode the base64 connection string
    let json_bytes = general_purpose::STANDARD
        .decode(input)
        .map_err(|e| format!("Failed to decode connection string: {}", e))?;

    let json_str = String::from_utf8(json_bytes)
        .map_err(|e| format!("Invalid UTF-8 in connection string: {}", e))?;

    // 2. Deserialize to UserDetails
    let details: UserDetails = serde_json::from_str(&json_str)
        .map_err(|e| format!("Failed to parse connection string: {}", e))?;

    info!("Parsed sovereign node details for user: {}", details.username);

    // 3. Save node to database (first_sync=false)
    let (_user, device) = add_known_user(
        details.username,
        details.user_public_key,
        details.device_public_key,
        details.ucan_token,  // One-time token with role='owner'
        details.ucan_pub_key,
        repo_ctx.inner().clone(),
        &crypto_utils,
    )
    .await
    .map_err(|e| format!("Failed to save sovereign node: {}", e))?;

    info!("Sovereign node saved to database");

    // 4. Connect immediately (happy path only)
    let node_device_id = device.id.clone();
    p2p_service
        .connect_with_ticket(&node_device_id)
        .await
        .map_err(|e| format!("Failed to connect to sovereign node: {}", e))?;

    info!("Successfully initiated connection to sovereign node");
    Ok(BaseCryptoResponse::Success)
}
