use crate::types::{ConnectToWebsiteInput, CryptoResponse};
use crate::user_state::UserState;
use crate::website_state::WebsiteState;
use base64::{engine::general_purpose, Engine as _};
use crypto_utils::CryptoUtils;
use crypto_utils::ucan_utils::{generate_flexible_resource_token, generate_public_view_token};
use log::{error, info};
use network::p2p::P2PService;
use osvauld_core::models::{ConnectionAction, ConnectionType, Device, User, UserWithDevices};
use persistance::database::RepositoryContext;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, State};
use tokio::sync::RwLock;

#[derive(Debug, Serialize, Deserialize)]
pub struct GenerateShareTokenInput {
    pub resource_id: String,
    pub expiry_seconds: Option<u64>,
    pub capabilities: Vec<String>,
    pub audience: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ShareTokenResponse {
    pub token: String,
    pub kunki_address: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateWebsiteStateInput {
    pub resource_id: String,
    pub update: Vec<u8>,
}

#[tauri::command]
pub async fn handle_generate_share_token(
    input: GenerateShareTokenInput,
    user_state: State<'_, UserState>,
    crypto_utils: State<'_, Arc<RwLock<CryptoUtils>>>,
    _repo_ctx: State<'_, Arc<RepositoryContext>>,
) -> Result<CryptoResponse, String> {
    let _user = user_state.get_user().await?;

    // Get user's signing key by deriving from PGP cert
    let crypto = crypto_utils.read().await;
    let cert = crypto
        .get_cert()
        .map_err(|e| format!("Failed to get certificate: {}", e))?;

    let (signing_key, verifying_key) = crypto_utils::ucan_utils::derive_ucan_keys_from_pgp(cert)
        .map_err(|e| format!("Failed to derive UCAN keys: {}", e))?;

    // Determine capability prefix
    let capability_prefix = format!("sthalam/{}", input.resource_id);

    // Convert string capabilities to &str
    let caps: Vec<&str> = input.capabilities.iter().map(|s| s.as_str()).collect();

    // Determine audience (default to "*" for public)
    let audience = input.audience.as_deref().unwrap_or("*");

    // Generate token based on whether it's a simple public view or custom
    let token = if caps == vec!["view/public"] && input.expiry_seconds == Some(30 * 24 * 60 * 60) {
        // Use convenience function for standard public view
        generate_public_view_token(
            &signing_key,
            &verifying_key,
            &input.resource_id,
            &capability_prefix,
        )
        .await
        .map_err(|e| e.to_string())?
    } else {
        // Use flexible token generation for custom settings
        generate_flexible_resource_token(
            &signing_key,
            &verifying_key,
            &input.resource_id,
            &capability_prefix,
            input.expiry_seconds,
            caps,
            audience,
        )
        .await
        .map_err(|e| e.to_string())?
    };

    Ok(CryptoResponse::ShareToken(ShareTokenResponse {
        token,
        kunki_address: None, // Not applicable for website tokens
    }))
}

#[tauri::command]
pub async fn handle_update_website_state(
    resource_id: String,
    update: Vec<u8>,
    website_state: State<'_, Arc<RwLock<WebsiteState>>>,
    app_handle: AppHandle,
) -> Result<CryptoResponse, String> {
    let state = website_state.read().await;

    // Apply update to the blocksuite_doc
    state
        .apply_update(&resource_id, update.clone(), "blocksuite_doc")
        .await;

    // Emit update to other potential listeners
    app_handle
        .emit(&format!("website-update-{}", resource_id), update.clone())
        .map_err(|e| e.to_string())?;

    Ok(CryptoResponse::Success)
}

#[tauri::command]
pub async fn handle_load_website_state(
    resource_id: String,
    website_state: State<'_, Arc<RwLock<WebsiteState>>>,
    user_state: State<'_, UserState>,
) -> Result<CryptoResponse, String> {
    let user = user_state.get_user().await?;
    let device = user_state.get_device().await?;

    let state = website_state.write().await;

    // Switch to this resource
    state
        .switch_resource(&resource_id, &user.id, &device.id)
        .await
        .map_err(|e| {
            error!("Failed to load website state: {}", e);
            e.to_string()
        })?;

    // Get the current state as bytes
    let state_bytes = state
        .get_state(&resource_id)
        .await
        .map_err(|e| e.to_string())?;

    Ok(CryptoResponse::WebsiteState(state_bytes))
}

#[derive(Debug, Serialize, Deserialize)]
struct ConnectionDetails {
    user_public_key: String,
    device_public_key: String,
    username: String,
    ucan_token: String,
    ucan_pub_key: String,
}

#[tauri::command]
pub async fn handle_connect_to_website(
    input: ConnectToWebsiteInput,
    p2p_service: State<'_, Arc<P2PService>>,
    _website_state: State<'_, Arc<RwLock<WebsiteState>>>,
    user_state: State<'_, UserState>,
    repo_ctx: State<'_, Arc<RepositoryContext>>,
) -> Result<CryptoResponse, String> {
    info!("Received website connection request");

    // 1. Decode the base64 connection string
    let decoded_bytes = general_purpose::STANDARD
        .decode(input.connection_string.trim())
        .map_err(|e| format!("Failed to decode connection string: {}", e))?;

    let connection_json = String::from_utf8(decoded_bytes)
        .map_err(|e| format!("Invalid UTF-8 in connection string: {}", e))?;

    // 2. Parse the JSON
    let connection_details: ConnectionDetails = serde_json::from_str(&connection_json)
        .map_err(|e| format!("Failed to parse connection details: {}", e))?;

    info!(
        "Parsed connection details for sovereign node: {}",
        connection_details.username
    );

    // 3. Create a User record for the sovereign node
    // user_id and user_public_key are the same
    let node_user_id = connection_details.user_public_key.clone();
    let current_timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    let node_user = User {
        id: node_user_id.clone(),
        username: connection_details.username.clone(),
        public_key: connection_details.user_public_key.clone(),
        created_at: current_timestamp,
        signature: String::new(), // Not applicable for website connections
        ucan_token: connection_details.ucan_token.clone(),
        ucan_pub_key: connection_details.ucan_pub_key.clone(),
        ucan_cid: String::new(), // Not applicable for website connections
        first_sync: false,
        updated_at: current_timestamp,
        owner: false,
        deleted: false,
        deleted_at: None,
    };

    // device_id and device_key are the same
    let node_device = Device {
        id: connection_details.device_public_key.clone(),
        device_key: connection_details.device_public_key.clone(),
        user_id: node_user_id.clone(),
        created_at: current_timestamp,
        updated_at: current_timestamp,
        last_synced_at: None,
    };

    let user_with_devices = UserWithDevices {
        user: node_user,
        devices: vec![node_device],
    };

    // 4. Store the user and device in the database
    repo_ctx
        .user_repo
        .add_users_with_devices_bulk(&vec![user_with_devices])
        .await
        .map_err(|e| format!("Failed to store sovereign node user: {}", e))?;

    info!("Stored sovereign node user and device in database");

    // 5. Get current user for the connection
    let _ = user_state.get_user().await?;
    let _ = user_state.get_device().await?;

    // 6. Initiate P2P connection
    let p2p_clone = p2p_service.inner().clone();
    let device_key = connection_details.device_public_key.clone();

    tokio::spawn(async move {
        info!("Initiating website connection to device: {}", device_key);

        if let Err(e) = p2p_clone
            .connect_with_ticket(
                &device_key,
                ConnectionType::Website,
                Some(ConnectionAction::WebsiteRequest),
            )
            .await
        {
            error!("Failed to establish website connection: {}", e);
        } else {
            info!("Successfully initiated website connection");
        }
    });

    Ok(CryptoResponse::Success)
}

#[tauri::command]
pub async fn handle_connect_to_remote(
    _address: String,
    _token: String,
    _website_state: State<'_, Arc<RwLock<WebsiteState>>>,
) -> Result<CryptoResponse, String> {
    // Deprecated - use handle_connect_to_website instead
    error!("handle_connect_to_remote is deprecated, use handle_connect_to_website");
    Err("Remote connection feature not yet implemented".to_string())
}
