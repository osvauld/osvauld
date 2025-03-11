use crate::types::AddKnownUser;
use crate::types::CryptoResponse;
use base64::decode;
use osvauld_services::UserService;
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn add_known_user(
    input: AddKnownUser,
    user_service: State<'_, Arc<UserService>>,
) -> Result<CryptoResponse, String> {
    let public_key_bytes = decode(input.public_key).map_err(|e| e.to_string())?;
    let public_key = String::from_utf8(public_key_bytes).map_err(|e| e.to_string())?;
    let user = user_service
        .add_known_user(input.nickname, public_key, false)
        .await?;
    Ok(CryptoResponse::CreatedKnownUser(user))
}

#[tauri::command]
pub async fn get_known_users(
    user_service: State<'_, Arc<UserService>>,
) -> Result<CryptoResponse, String> {
    let known_users = user_service.get_known_users().await?;
    Ok(CryptoResponse::GetKnownUsers(known_users))
}
