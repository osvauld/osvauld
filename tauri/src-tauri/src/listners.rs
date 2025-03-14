use log::{error, info};
use osvauld_core::models::p2p::Message;
use p2p_service::{P2PService, p2p::P2PEvent};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Listener};
use tokio::sync::mpsc;

/// Initializes all listeners for the application
/// This connects the Tauri event system with the P2P event system
pub struct EventManager {
    app_handle: AppHandle,
    p2p_service: Arc<P2PService>,
    p2p_receiver: mpsc::UnboundedReceiver<P2PEvent>,
}

impl EventManager {
    /// Create a new EventManager that handles bidirectional events
    pub fn new(
        app_handle: AppHandle,
        p2p_service: Arc<P2PService>,
        p2p_receiver: mpsc::UnboundedReceiver<P2PEvent>,
    ) -> Self {
        Self {
            app_handle,
            p2p_service,
            p2p_receiver,
        }
    }

    /// Start listening for all events
    pub fn start_listening(mut self) {
        // Set up Tauri event listeners
        self.setup_tauri_listeners();

        // Start P2P event listener in background
        tokio::spawn(async move {
            self.listen_for_p2p_events().await;
        });
    }

    /// Set up listeners for Tauri events
    fn setup_tauri_listeners(&self) {
        // Listen for sync update events from the frontend
        let p2p_service_clone = self.p2p_service.clone();
        self.app_handle.listen("sync-update", move |event| {
            let p2p_service = p2p_service_clone.clone();
            let payload_str = event.payload().to_string();

            tokio::spawn(async move {
                info!("Received sync-update event from frontend");
                let msg = Message::SyncEvent {
                    event: "sync-update".to_string(),
                    payload: payload_str,
                };

                match serde_json::to_string(&msg) {
                    Ok(serialized) => {
                        info!("Sending sync update message to peer");
                        // if let Err(e) = p2p_service.send_message(serialized).await {
                        //     error!("Failed to send sync event: {}", e);
                        // }
                    }
                    Err(e) => {
                        error!("Failed to serialize message: {}", e);
                    }
                }
            });
        });

        // Listen for sync snapshot events from the frontend
        let p2p_service_clone = self.p2p_service.clone();
        self.app_handle.listen("sync-snapshot", move |event| {
            let p2p_service = p2p_service_clone.clone();
            let payload_str = event.payload().to_string();

            tokio::spawn(async move {
                info!("Received sync-snapshot event from frontend");
                // if let Err(e) = p2p_service.send_snapshot(payload_str).await {
                //     error!("Failed to send snapshot: {}", e);
                // }
            });
        });
    }

    /// Listen for P2P events and forward them to Tauri
    async fn listen_for_p2p_events(&mut self) {
        info!("Started listening for P2P events");

        while let Some(event) = self.p2p_receiver.recv().await {
            match event {
                P2PEvent::Connected => {
                    if let Err(e) = self.app_handle.emit("peer-connected", true) {
                        error!("Failed to emit peer-connected event: {}", e);
                    }
                }
                P2PEvent::Disconnected => {
                    info!("Peer disconnected");
                    if let Err(e) = self.app_handle.emit("peer-disconnected", true) {
                        error!("Failed to emit peer-disconnected event: {}", e);
                    }
                }
                P2PEvent::HandshakeCompleted => {
                    info!("Handshake completed successfully");
                    if let Err(e) = self.app_handle.emit("handshake-completed", true) {
                        error!("Failed to emit handshake-completed event: {}", e);
                    }
                }
                P2PEvent::HandshakeFailed { error } => {
                    info!("Handshake failed: {}", error);
                    if let Err(e) = self.app_handle.emit("handshake-failed", error) {
                        error!("Failed to emit handshake-failed event: {}", e);
                    }
                }
                P2PEvent::SyncComplete => {
                    info!("Sync completed");
                    if let Err(e) = self.app_handle.emit("sync-complete", true) {
                        error!("Failed to emit sync-complete event: {}", e);
                    }
                }
                P2PEvent::ShareComplete => {
                    info!("Share completed");
                    if let Err(e) = self.app_handle.emit("share-complete", true) {
                        error!("Failed to emit share-complete event: {}", e);
                    }
                }
                P2PEvent::EditingEvent { payload } => {
                    info!("Received editing event");
                    if let Err(e) = self.app_handle.emit("sync-update-be", payload) {
                        error!("Failed to emit sync-update-be event: {}", e);
                    }
                }
                P2PEvent::SnapshotEvent { payload } => {
                    info!("Received snapshot event");
                    if let Err(e) = self.app_handle.emit("sync-snapshot-be", payload) {
                        error!("Failed to emit sync-snapshot-be event: {}", e);
                    }
                }
                P2PEvent::Error { message, source } => {
                    error!("P2P error from {}: {}", source, message);
                    if let Err(e) = self.app_handle.emit("p2p-error", message) {
                        error!("Failed to emit p2p-error event: {}", e);
                    }
                }
            }
        }

        info!("Stopped listening for P2P events");
    }
}
