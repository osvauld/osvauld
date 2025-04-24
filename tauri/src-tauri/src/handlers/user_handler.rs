use crate::types::{CryptoResponse, UserDetails};
use crate::user_state::{self, UserState};
use base64::{Engine as _, engine::general_purpose};
use osvauld_services::{TransactionService, UserService};
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn add_known_user(
    input: String,
    user_service: State<'_, Arc<UserService>>,
    transaction_service: State<'_, Arc<TransactionService>>,
    user_state: State<'_, UserState>,
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

    let current_user = user_state.get_user().await.map_err(|e| e.to_string())?;
    let current_device = user_state.get_device().await?;
    let (new_user, new_device) = user_service
        .add_known_user(
            username,
            user_public_key,
            device_public_key,
            &current_user.id,
            &current_device.id,
        )
        .await
        .map_err(|e| e.to_string())?;
    transaction_service
        .add_new_user(&new_user, &new_device)
        .await
        .map_err(|e| e.to_string())?;

    Ok(CryptoResponse::CreatedKnownUser {
        user: new_user,
        device: new_device,
    })
}

#[tauri::command]
pub async fn get_known_users(
    user_service: State<'_, Arc<UserService>>,
) -> Result<CryptoResponse, String> {
    let known_users = user_service.get_known_users().await?;
    Ok(CryptoResponse::GetKnownUsers(known_users))
}

#[tauri::command]
pub async fn get_details_for_share(
    user_state: State<'_, UserState>,
    user_service: State<'_, Arc<UserService>>,
) -> Result<CryptoResponse, String> {
    let user = user_state.get_user().await.map_err(|e| e.to_string())?;
    let device = user_state.get_device().await?;
    let username = user_service
        .get_username(&user.id)
        .await
        .map_err(|e| e.to_string())?;

    // Combine all details into a single struct
    let details = UserDetails {
        user_public_key: user.public_key.clone(),
        device_public_key: device.device_key.clone(),
        username,
    };

    // Serialize to JSON
    let json = serde_json::to_string(&details)
        .map_err(|e| format!("Failed to serialize user details: {}", e))?;

    // Encode the JSON string to base64
    let encoded = general_purpose::STANDARD.encode(json);

    // Return just the encoded string
    Ok(CryptoResponse::UserDetailsForShare(encoded))
}
