use crate::types::{CryptoResponse, UserDetails};
use base64::{Engine as _, engine::general_purpose};
use crypto_utils::CryptoUtils;
use log::info;
use osvauld_db::database::RepositoryContext;
use osvauld_services::{add_known_user, get_known_users};
use rendezvous_client::rendezvous_service::RendezvousService;
use std::sync::Arc;
use tauri::State;
use tokio::sync::Mutex;
#[tauri::command]
pub async fn handle_add_user(
    input: String,
    crypto_utils: State<'_, Arc<Mutex<CryptoUtils>>>,
    repo_ctx: State<'_, RepositoryContext>,
    rendezvous_service: State<'_, Arc<RendezvousService>>,
) -> Result<CryptoResponse, String> {
    // Decode the base64 string
    let json_bytes = general_purpose::STANDARD
        .decode(input)
        .map_err(|e| format!("Failed to decode input: {}", e))?;

    // Convert bytes to UTF-8 string
    let json_str = String::from_utf8(json_bytes)
        .map_err(|e| format!("Invalid UTF-8 in decoded input: {}", e))?;

    // Deserialize the JSON string to our UserDetails struct
    let details: UserDetails = serde_json::from_str(&json_str)
        .map_err(|e| format!("Failed to deserialize user details: {}", e))?;

    // Now you can use the extracted fields
    let username = details.username;
    let user_public_key = details.user_public_key;
    let device_public_key = details.device_public_key;

    let (user, device) = add_known_user(
        username,
        user_public_key,
        device_public_key,
        &repo_ctx,
        &crypto_utils,
    )
    .await
    .map_err(|e| e.to_string())?;

    let connection_id = format!("{}:{}", user.id, device.id);

    match rendezvous_service
        .mark_for_first_connection(&connection_id)
        .await
    {
        Ok(_) => info!("requested connection.."),
        Err(e) => info!("error requesting {:?}", e),
    }

    Ok(CryptoResponse::Success)
}

#[tauri::command]
pub async fn handle_get_known_users(
    repo_ctx: State<'_, RepositoryContext>,
) -> Result<CryptoResponse, String> {
    let known_users = get_known_users(&repo_ctx).await?;
    Ok(CryptoResponse::GetKnownUsers(known_users))
}
