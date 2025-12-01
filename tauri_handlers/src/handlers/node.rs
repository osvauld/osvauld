//! Node handler for sovereign node registration

use crate::types::BaseCryptoResponse;
use butler::Butler;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::State;
use tracing::{info, instrument};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterNodeInput {
    pub node_did: String,
    pub device_id: String,
    pub iroh_node_id: String,
    pub node_addr: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeInfoResponse {
    pub user_did: String,
    pub node_did: String,
    pub device_id: String,
    pub iroh_node_id: String,
    pub node_addr: Option<String>,
    pub is_online: bool,
    pub last_seen_at: Option<i64>,
    pub registered_at: i64,
}

/// Register my sovereign node
#[tauri::command]
#[instrument(skip(input, butler))]
pub async fn handle_register_my_node(
    input: RegisterNodeInput,
    butler: State<'_, Arc<Butler>>,
) -> Result<BaseCryptoResponse, String> {
    let node = butler
        .register_my_node(
            input.node_did,
            input.device_id,
            input.iroh_node_id,
            input.node_addr,
        )
        .map_err(|e| format!("Failed to register node: {}", e))?;

    info!(node_did = %node.node_did, "My node registered");

    Ok(BaseCryptoResponse::NodeRegistered(NodeInfoResponse {
        user_did: node.user_did,
        node_did: node.node_did,
        device_id: node.device_id,
        iroh_node_id: node.iroh_node_id,
        node_addr: node.node_addr,
        is_online: node.is_online,
        last_seen_at: node.last_seen_at,
        registered_at: node.registered_at,
    }))
}
