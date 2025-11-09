//! Tauri event listener - handles events from frontend
//!
//! Listens to Tauri events emitted from the frontend and calls
//! appropriate network/service functions.

use crypto_utils::CryptoUtils;
use network::p2p::{sync_handler, P2PService};
use persistance::database::RepositoryContext;
use serde_json::Value;
use std::sync::Arc;
use tauri::{AppHandle, Listener};
use tokio::sync::RwLock;
use tracing::{error, info};

/// Listens to Tauri events from the frontend
pub async fn listen_to_tauri_events(
    app_handle: AppHandle,
    p2p_service: Arc<P2PService>,
    repo_ctx: Arc<RepositoryContext>,
    _crypto_utils: Arc<RwLock<CryptoUtils>>,
) {
    info!("Started listening to Tauri events from frontend");

    // Clone for the closure
    let p2p_service_clone = p2p_service.clone();
    let repo_ctx_clone = repo_ctx.clone();

    // Listen for "request-folder-token" event
    app_handle.listen("request-folder-token", move |event| {
        let p2p_service = p2p_service_clone.clone();
        let repo_ctx = repo_ctx_clone.clone();
        info!("Received request-folder-token event");

        // Parse payload
        let payload: Value = match serde_json::from_str(event.payload()) {
            Ok(p) => p,
            Err(e) => {
                error!("Failed to parse request-folder-token payload: {}", e);
                return;
            }
        };

        let folder_id = match payload.get("folderId").and_then(|v| v.as_str()) {
            Some(id) => id.to_string(),
            None => {
                error!("Missing folderId in request-folder-token payload");
                return;
            }
        };

        let user_id = match payload.get("deviceId").and_then(|v| v.as_str()) {
            Some(id) => id.to_string(),
            None => {
                error!("Missing deviceId (userId) in request-folder-token payload");
                return;
            }
        };

        info!(
            "Processing folder token request - folder: {}, user: {}",
            folder_id, user_id
        );

        // Call sync_handler to request folder token from the node
        tokio::spawn(async move {
            if let Err(e) = sync_handler::request_folder_token(
                folder_id.clone(),
                user_id.clone(),
                repo_ctx,
                p2p_service,
            )
            .await
            {
                error!("Failed to request folder token: {}", e);
            }
        });
    });

    // Keep the listener alive
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await;
    }
}
