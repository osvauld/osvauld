use crate::application::services::UserService;
use crate::types::AddKnownUser;
use crate::types::CryptoResponse;
use std::sync::Arc;

use tauri::State;

#[tauri::command]
pub async fn add_known_user(
    input: AddKnownUser,
    user_service: State<'_, Arc<UserService>>,
) -> Result<CryptoResponse, String> {
    let user = user_service
        .add_known_user(input.nickname, input.public_key)
        .await?;
    Ok(CryptoResponse::CreatedKnownUser(user))
}
