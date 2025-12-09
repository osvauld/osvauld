//! Event Bridge - Translates Courier events to Tauri events
//!
//! Consumes CourierEvent from the protocol layer and emits
//! corresponding Tauri events to the frontend.
//!
//! This module maintains a clean separation between the protocol layer
//! (Courier) and the application layer (Tauri). Courier emits protocol-agnostic
//! events, and this bridge translates them to Tauri-specific events.

use courier::CourierEvent;
use tauri::{AppHandle, Emitter};
use tokio::sync::mpsc;
use tracing::info;

/// Spawns a task that bridges Courier events to Tauri events
///
/// **Context**: Called during P2P initialization
/// **We do**: Consume events from event_rx and emit corresponding Tauri events
/// **Runs until**: event_rx channel is closed (Courier shutdown)
pub fn spawn_event_bridge(
    mut event_rx: mpsc::Receiver<CourierEvent>,
    app: AppHandle,
) {
    tokio::spawn(async move {
        info!("Event bridge started");
        while let Some(event) = event_rx.recv().await {
            handle_courier_event(&app, event);
        }
        info!("Event bridge stopped");
    });
}

fn handle_courier_event(app: &AppHandle, event: CourierEvent) {
    match event {
        CourierEvent::ShareableLinkReceived { node_id, space_id, permit } => {
            info!("Emitting folder-token-received for space {}", space_id);
            let _ = app.emit("folder-token-received", serde_json::json!({
                "folderId": space_id,
                "connectionString": permit,
                "nodeId": node_id,
            }));
        }

        CourierEvent::ViewerSyncComplete { node_id, space_id, pages_synced } => {
            info!("Emitting viewer-sync-complete for space {}", space_id);
            let _ = app.emit("viewer-sync-complete", serde_json::json!({
                "spaceId": space_id,
                "nodeId": node_id,
                "pagesSynced": pages_synced,
            }));
        }

        CourierEvent::SpacePublished { node_id, space_id } => {
            info!("Emitting space-published for space {}", space_id);
            let _ = app.emit("space-published", serde_json::json!({
                "spaceId": space_id,
                "nodeId": node_id,
            }));
        }

        CourierEvent::PublishFailed { node_id, space_id, error } => {
            info!("Emitting publish-failed for space {}", space_id);
            let _ = app.emit("publish-failed", serde_json::json!({
                "spaceId": space_id,
                "nodeId": node_id,
                "error": error,
            }));
        }

        CourierEvent::PeerAuthenticated { node_id, did, username } => {
            info!("Emitting peer-authenticated for node {}", node_id);
            let _ = app.emit("peer-authenticated", serde_json::json!({
                "nodeId": node_id,
                "did": did,
                "username": username,
            }));
        }

        CourierEvent::PeerDisconnected { node_id } => {
            info!("Emitting peer-disconnected for node {}", node_id);
            let _ = app.emit("peer-disconnected", serde_json::json!({
                "nodeId": node_id,
            }));
        }

        CourierEvent::ConnectRequested { node_id, permit } => {
            info!("Emitting connect-requested for node {}", node_id);
            let _ = app.emit("connect-requested", serde_json::json!({
                "nodeId": node_id,
                "permit": permit,
            }));
        }

        CourierEvent::ViewerSpaceReceived { node_id, space, page_count } => {
            info!("Emitting viewer-space-received for space {}", space.id);
            let _ = app.emit("viewer-space-received", serde_json::json!({
                "nodeId": node_id,
                "space": space,
                "pageCount": page_count,
            }));
        }

        CourierEvent::ViewerPageReceived { node_id, page, is_last } => {
            info!("Emitting viewer-page-received for page {}", page.id);
            let _ = app.emit("viewer-page-received", serde_json::json!({
                "nodeId": node_id,
                "page": page,
                "isLast": is_last,
            }));
        }

        CourierEvent::SyncConsentComplete { node_id, space_id } => {
            info!("Emitting sync-consent-complete for space {}", space_id);
            let _ = app.emit("sync-consent-complete", serde_json::json!({
                "spaceId": space_id,
                "nodeId": node_id,
            }));
        }
    }
}
