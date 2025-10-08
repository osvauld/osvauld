use crate::types::CryptoResponse;
use crate::user_state::UserState;
use crate::website_state::WebsiteState;
use crypto_utils::ucan_utils::{generate_flexible_resource_token, generate_public_view_token};
use crypto_utils::CryptoUtils;
use log::error;
use persistance::database::RepositoryContext;
use serde::{Deserialize, Serialize};
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

    // TODO: Get actual kunki address from configuration
    let kunki_address = Some("localhost:8080".to_string());

    Ok(CryptoResponse::ShareToken(ShareTokenResponse {
        token,
        kunki_address,
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

#[tauri::command]
pub async fn handle_connect_to_remote(
    _address: String,
    _token: String,
    _website_state: State<'_, Arc<RwLock<WebsiteState>>>,
) -> Result<CryptoResponse, String> {
    // TODO: Implement remote connection logic
    // This will involve:
    // 1. Parsing the connection string
    // 2. Validating the UCAN token
    // 3. Establishing P2P connection to the kunki node
    // 4. Setting up sync for the remote resource

    error!("Remote connection not yet implemented");
    Err("Remote connection feature not yet implemented".to_string())
}
