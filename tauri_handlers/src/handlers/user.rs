//! User handlers - Simplified for new architecture

use crate::types::{BaseCryptoResponse, UserDetails, KnownUserResponse, SovereignNodeResponse};
use butler::Butler;
use base64::{Engine as _, engine::general_purpose};
use std::sync::Arc;
use sys_locale::get_locale;
use tauri::State;
use tracing::instrument;

#[tauri::command]
#[instrument(skip(input, butler))]
pub async fn handle_add_user(
    input: String,
    butler: State<'_, Arc<Butler>>,
) -> Result<BaseCryptoResponse, String> {
    // Decode the base64 string
    let json_bytes = general_purpose::STANDARD
        .decode(&input)
        .map_err(|e| format!("Failed to decode input: {}", e))?;

    // Convert bytes to UTF-8 string
    let json_str = String::from_utf8(json_bytes)
        .map_err(|e| format!("Invalid UTF-8 in decoded input: {}", e))?;

    // Deserialize the JSON string to our UserDetails struct
    let _details: UserDetails = serde_json::from_str(&json_str)
        .map_err(|e| format!("Failed to deserialize user details: {}", e))?;

    // Store as sovereign node via Butler
    butler
        .add_sovereign_node(&input)
        .map_err(|e| format!("Failed to add user: {}", e))?;

    Ok(BaseCryptoResponse::Success)
}

#[tauri::command]
#[instrument(skip(butler))]
pub async fn handle_get_known_users(
    butler: State<'_, Arc<Butler>>,
) -> Result<BaseCryptoResponse, String> {
    let nodes = butler
        .list_sovereign_nodes()
        .map_err(|e| e.to_string())?;

    let known_users: Vec<KnownUserResponse> = nodes
        .into_iter()
        .map(|node| KnownUserResponse {
            user_id: node.node_id.clone(),
            username: node.name,
            public_key: node.did,
        })
        .collect();

    Ok(BaseCryptoResponse::GetKnownUsers(known_users))
}

#[tauri::command]
pub fn get_system_locale() -> String {
    get_locale().unwrap_or_else(|| String::from("en-US"))
}

/// Get sovereign nodes (kunki nodes we've paired with)
///
/// **Context**: Frontend needs to display node info for publishing
/// **Returns**: List of sovereign nodes with connection status
#[tauri::command]
#[instrument(skip(butler))]
pub async fn handle_get_sovereign_nodes(
    butler: State<'_, Arc<Butler>>,
) -> Result<BaseCryptoResponse, String> {
    let nodes = butler
        .list_sovereign_nodes()
        .map_err(|e| e.to_string())?;

    let sovereign_nodes: Vec<SovereignNodeResponse> = nodes
        .into_iter()
        .map(|node| SovereignNodeResponse {
            node_id: node.node_id,
            username: node.name,
            user_public_key: node.did,
            device_public_key: node.device_public_key,
            is_connected: node.is_connected,
            last_connected_at: node.last_connected_at,
        })
        .collect();

    Ok(BaseCryptoResponse::GetSovereignNodes(sovereign_nodes))
}
