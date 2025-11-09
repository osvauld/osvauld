//! P2P event listener - handles events from network layer
//!
//! Listens to P2PEvents from the network layer and emits
//! corresponding Tauri events to the frontend.

use network::p2p::emitter::P2PEvent;
use serde_json::json;
use tauri::{AppHandle, Emitter};
use tokio::sync::mpsc;
use tracing::{error, info};

/// Listens to P2P events from the network layer
pub async fn listen_to_p2p_events(
    app_handle: AppHandle,
    mut p2p_receiver: mpsc::UnboundedReceiver<P2PEvent>,
) {
    info!("Started listening to P2P events from network");

    while let Some(event) = p2p_receiver.recv().await {
        match event {
            P2PEvent::Connected { peer_id } => {
                info!("P2P Connected event: {}", peer_id);
                if let Err(e) = app_handle.emit(
                    "peer-connected",
                    json!({
                        "peerId": peer_id,
                        "type": "connected"
                    }),
                ) {
                    error!("Failed to emit peer-connected event: {}", e);
                }
            }

            P2PEvent::NodeConnected { peer_id } => {
                info!("P2P NodeConnected event: {}", peer_id);
                if let Err(e) = app_handle.emit(
                    "peer-connected",
                    json!({
                        "peerId": peer_id,
                        "type": "node"
                    }),
                ) {
                    error!("Failed to emit peer-connected (node) event: {}", e);
                }
            }

            P2PEvent::UserConnected { peer_id } => {
                info!("P2P UserConnected event: {}", peer_id);
                if let Err(e) = app_handle.emit(
                    "peer-connected",
                    json!({
                        "peerId": peer_id,
                        "type": "user"
                    }),
                ) {
                    error!("Failed to emit peer-connected (user) event: {}", e);
                }
            }

            P2PEvent::Disconnected => {
                info!("P2P Disconnected event");
                if let Err(e) = app_handle.emit("peer-disconnected", json!({})) {
                    error!("Failed to emit peer-disconnected event: {}", e);
                }
            }

            P2PEvent::Error { message, source } => {
                error!("P2P Error event: {} (source: {})", message, source);
                if let Err(e) = app_handle.emit(
                    "p2p-error",
                    json!({
                        "message": message,
                        "source": source
                    }),
                ) {
                    error!("Failed to emit p2p-error event: {}", e);
                }
            }

            P2PEvent::FolderTokenReceived {
                folder_id,
                connection_string,
            } => {
                info!("P2P FolderTokenReceived event: folder {}", folder_id);
                if let Err(e) = app_handle.emit(
                    "folder-token-received",
                    json!({
                        "folderId": folder_id,
                        "connectionString": connection_string
                    }),
                ) {
                    error!("Failed to emit folder-token-received event: {}", e);
                }
            }
        }
    }

    info!("Stopped listening to P2P events");
}
