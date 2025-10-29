use crate::types::{ConnectToWebsiteInput, CryptoResponse};
use crate::user_state::UserState;
use crate::website_state::WebsiteState;
use base64::{Engine as _, engine::general_purpose};
use crypto_utils::CryptoUtils;
use crypto_utils::ucan_utils::{generate_flexible_resource_token, generate_public_view_token};
use log::{error, info};
use network::p2p::P2PService;
use osvauld_core::models::{ConnectionAction, ConnectionType};
use persistance::database::RepositoryContext;
use serde::{Deserialize, Serialize};
use services;
use std::sync::Arc;
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
    crypto_utils: State<'_, Arc<RwLock<CryptoUtils>>>,
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

    // 3. Add the sovereign node user and device using the service function
    let (node_user, _node_device) = services::add_known_user(
        connection_details.username.clone(),
        connection_details.user_public_key.clone(),
        connection_details.device_public_key.clone(),
        connection_details.ucan_token.clone(),
        connection_details.ucan_pub_key.clone(),
        repo_ctx.inner().clone(),
        &crypto_utils.inner().clone(),
    )
    .await
    .map_err(|e| format!("Failed to add sovereign node user: {}", e))?;

    info!(
        "Stored sovereign node user {} and device in database",
        node_user.id
    );

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
                //TODO: depretiate connection action
                Some(ConnectionAction::UserSync),
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
pub async fn handle_sync_resource(
    resource_id: String,
    p2p_service: State<'_, Arc<P2PService>>,
) -> Result<CryptoResponse, String> {
    info!("Received sync resource request for: {}", resource_id);

    // Spawn async task to sync resource to P2P network
    let p2p_clone = p2p_service.inner().clone();
    let resource_id_clone = resource_id.clone();

    tokio::spawn(async move {
        info!("Syncing resource to P2P network: {}", resource_id_clone);

        if let Err(e) = p2p_clone.sync_resource(&resource_id_clone).await {
            error!("Failed to sync resource: {}", e);
        } else {
            info!("Successfully synced resource: {}", resource_id_clone);
        }
    });

    Ok(CryptoResponse::Success)
}

#[tauri::command]
pub async fn handle_folder_sync_viewer(
    folder_id: String,
    p2p_service: State<'_, Arc<P2PService>>,
) -> Result<CryptoResponse, String> {
    info!("Received folder sync viewer request for: {}", folder_id);

    // Spawn async task to sync folder in viewer mode
    let p2p_clone = p2p_service.inner().clone();
    let folder_id_clone = folder_id.clone();

    tokio::spawn(async move {
        info!("Syncing folder in viewer mode: {}", folder_id_clone);

        if let Err(e) = p2p_clone.folder_sync_viewer(&folder_id_clone).await {
            error!("Failed to sync folder in viewer mode: {}", e);
        } else {
            info!("Successfully synced folder in viewer mode: {}", folder_id_clone);
        }
    });

    Ok(CryptoResponse::Success)
}
