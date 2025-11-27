//! User handlers - Simplified for new architecture

use crate::types::{BaseCryptoResponse, UserDetails, KnownUserResponse};
use butler::NodeService;
use base64::{Engine as _, engine::general_purpose};
use std::sync::Arc;
use sys_locale::get_locale;
use tauri::State;
use tracing::instrument;

#[tauri::command]
#[instrument(skip(input, node_service))]
pub async fn handle_add_user(
    input: String,
    node_service: State<'_, Arc<NodeService>>,
) -> Result<BaseCryptoResponse, String> {
    // Decode the base64 string
    let json_bytes = general_purpose::STANDARD
        .decode(&input)
        .map_err(|e| format!("Failed to decode input: {}", e))?;

    // Convert bytes to UTF-8 string
    let json_str = String::from_utf8(json_bytes)
        .map_err(|e| format!("Invalid UTF-8 in decoded input: {}", e))?;

    // Deserialize the JSON string to our UserDetails struct
    let details: UserDetails = serde_json::from_str(&json_str)
        .map_err(|e| format!("Failed to deserialize user details: {}", e))?;

    // Store as sovereign node using butler
    node_service
        .add_sovereign_node(&input)
        .map_err(|e| format!("Failed to add user: {}", e))?;

    Ok(BaseCryptoResponse::Success)
}

#[tauri::command]
#[instrument(skip(node_service))]
pub async fn handle_get_known_users(
    node_service: State<'_, Arc<NodeService>>,
) -> Result<BaseCryptoResponse, String> {
    let nodes = node_service
        .list_sovereign_nodes()
        .map_err(|e| e.to_string())?;

    let known_users: Vec<KnownUserResponse> = nodes
        .into_iter()
        .map(|node| KnownUserResponse {
            user_id: node.node_id.clone(),
            username: node.username,
            public_key: node.user_public_key,
        })
        .collect();

    Ok(BaseCryptoResponse::GetKnownUsers(known_users))
}

#[tauri::command]
pub fn get_system_locale() -> String {
    get_locale().unwrap_or_else(|| String::from("en-US"))
}
