use crate::types::{CryptoResponse, UserDetails};
use base64::{Engine as _, engine::general_purpose};
use crypto_utils::CryptoUtils;
use log::{error, info};
use network::P2PService;
use osvauld_core::models::{ConnectionAction, ConnectionType};
use persistance::database::RepositoryContext;
use services::{add_known_user, get_known_users};
use std::sync::Arc;
use sys_locale::get_locale;
use tauri::State;
use tokio::sync::Mutex;
#[tauri::command]
pub async fn handle_add_user(
    input: String,
    crypto_utils: State<'_, Arc<Mutex<CryptoUtils>>>,
    repo_ctx: State<'_, RepositoryContext>,
    p2p_service: State<'_, Arc<P2PService>>,
) -> Result<CryptoResponse, String> {
    // // Decode the base64 string
    // let json_bytes = general_purpose::STANDARD
    //     .decode(input)
    //     .map_err(|e| format!("Failed to decode input: {}", e))?;
    //
    // // Convert bytes to UTF-8 string
    // let json_str = String::from_utf8(json_bytes)
    //     .map_err(|e| format!("Invalid UTF-8 in decoded input: {}", e))?;

    // Deserialize the JSON string to our UserDetails struct
    let details: UserDetails = serde_json::from_str(&input)
        .map_err(|e| format!("Failed to deserialize user details: {}", e))?;

    // Now you can use the extracted fields
    let username = details.username;
    let user_public_key = details.user_public_key;
    let device_public_key = details.device_public_key;
    let one_time_token = details.ucan_token;
    let ucan_pub_key = details.ucan_pub_key;

    let (user, device) = add_known_user(
        username,
        user_public_key,
        device_public_key,
        one_time_token,
        ucan_pub_key,
        &repo_ctx,
        &crypto_utils,
    )
    .await
    .map_err(|e| e.to_string())?;
    let device = device.clone();
    let p2p_service_clone = p2p_service.inner().clone();

    tokio::spawn(async move {
        if let Err(e) = p2p_service_clone
            .connect_with_ticket(
                &device.id,
                ConnectionType::User,
                Some(ConnectionAction::UserFirstConnection),
            )
            .await
        {
            error!("Failed to start P2P service: {}", e);
        }
    });
    Ok(CryptoResponse::Success)
}

#[tauri::command]
pub async fn handle_get_known_users(
    repo_ctx: State<'_, RepositoryContext>,
) -> Result<CryptoResponse, String> {
    let known_users = get_known_users(&repo_ctx).await?;
    Ok(CryptoResponse::GetKnownUsers(known_users))
}

#[tauri::command]
pub fn get_system_locale() -> String {
    get_locale().unwrap_or_else(|| String::from("en-US"))
}
