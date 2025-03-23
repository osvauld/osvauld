use log::{debug, error, info};
use serde::Deserialize;

use osvauld_core::models::{p2p::Message, vector_clock::ResourceVectorClock};
use osvauld_services::ResourceService;
use p2p_service::{
    P2PService,
    p2p::{P2PEvent, incoming::P2PSender},
};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Listener};
use tokio::sync::mpsc;

/// Initializes all listeners for the application
/// This connects the Tauri event system with the P2P event system
pub struct EventManager {
    app_handle: AppHandle,
    resource_service: Arc<ResourceService>,
    p2p_receiver: mpsc::UnboundedReceiver<P2PEvent>,
    p2p_sender: P2PSender,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MergeCompletePayload {
    merged_document: serde_json::Value,
    device_id: String,
    user_id: String,
    vector_clock: Vec<ResourceVectorClock>,
    resource_id: String,
}

impl EventManager {
    /// Create a new EventManager that handles bidirectional events
    pub fn new(
        app_handle: AppHandle,
        p2p_receiver: mpsc::UnboundedReceiver<P2PEvent>,
        resource_service: Arc<ResourceService>,
        p2p_sender: P2PSender,
    ) -> Self {
        Self {
            app_handle,
            p2p_receiver,
            resource_service,
            p2p_sender,
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

    /// Set up listeners for Tauri events
    fn setup_tauri_listeners(&self) {
        // Listen for sync update events from the frontend
        let p2p_sender = self.p2p_sender.clone();
        self.app_handle.listen("sync-update", move |event| {
            let payload_str = event.payload().to_string();
            // Direct send without spawning a task for high-frequency events
            if let Err(e) = p2p_sender.send_sync_update(payload_str) {
                error!("Failed to send sync update event: {}", e);
            } else {
                debug!("Sent sync update event to P2P service"); // Using debug level for high-frequency events
            }
        });

        // Listen for merge-complete events
        let p2p_sender = self.p2p_sender.clone();
        let resource_service_clone = self.resource_service.clone();
        self.app_handle.listen("merge-complete", move |event| {
            let resource_service = resource_service_clone.clone();
            let payload_str = event.payload().to_string();
            let p2p_sender = p2p_sender.clone();

            // Spawn a task for merge-complete since it involves resource service operations
            tokio::spawn(async move {
                info!(
                    "Received merge-complete event with payload: {}",
                    payload_str
                );

                match serde_json::from_str::<MergeCompletePayload>(&payload_str) {
                    Ok(payload) => {
                        info!(
                            "Successfully parsed merge-complete payload for resource: {}",
                            payload.resource_id
                        );

                        let merged_doc_str = serde_json::to_string(&payload.merged_document)
                            .unwrap_or_else(|_| "{}".to_string());
                        match resource_service
                            .update_merged_payload(
                                &merged_doc_str,
                                &payload.vector_clock,
                                &payload.resource_id,
                            )
                            .await
                        {
                            Ok((encrypted_data, (add_remote_vector, update_remote_vector))) => {
                                // Send merge complete event to P2P service using P2PSender
                                if let Err(e) = p2p_sender.send_merge_complete(
                                    encrypted_data,
                                    payload.vector_clock,
                                    payload.resource_id,
                                    payload.user_id,
                                    payload.device_id,
                                ) {
                                    error!("Failed to send merge complete event: {}", e);
                                } else {
                                    info!("Successfully sent merge complete event to P2P service");
                                }
                            }
                            Err(e) => {
                                error!("Failed to update merged payload: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to parse merge-complete payload: {:?}", e);
                    }
                }
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
                P2PEvent::Error { message, source } => {
                    error!("P2P error from {}: {}", source, message);
                    if let Err(e) = self.app_handle.emit("p2p-error", message) {
                        error!("Failed to emit p2p-error event: {}", e);
                    }
                }

                P2PEvent::UpdateEvent {
                    vector_clock,
                    remote_resource,
                    device_id,
                    user_id,
                } => {
                    info!(
                        "Received update event from device {} for user {}: {:?}",
                        device_id, user_id, vector_clock
                    );

                    // Handle the result from get_update_payload explicitly
                    match self
                        .resource_service
                        .get_update_payload(remote_resource)
                        .await
                    {
                        Ok((local_resource, remote_resource)) => {
                            // Now we have both decrypted resources, we can compare them
                            // or send them to the frontend for conflict resolution
                            info!(
                                "Successfully processed update event, got local and remote resources"
                            );

                            // Create a payload with both resources for the frontend to handle
                            let payload = serde_json::json!({
                                "local_resource": local_resource,
                                "remote_resource": remote_resource,
                                "device_id": device_id,
                                "user_id": user_id,
                                "vector_clock": vector_clock,
                            });

                            // Emit an event to the frontend with the resources
                            if let Err(e) = self.app_handle.emit("merge-update", payload) {
                                error!("Failed to emit resource-update event: {}", e);
                            }
                        }
                        Err(e) => {
                            // Log the error and possibly notify the frontend
                            error!("Failed to process update event: {}", e);

                            // Notify the frontend about the failure
                            let error_payload = serde_json::json!({
                                "error": e.to_string(),
                                "device_id": device_id,
                                "user_id": user_id
                            });

                            if let Err(emit_err) = self
                                .app_handle
                                .emit("resource-update-failed", error_payload)
                            {
                                error!("Failed to emit resource-update-failed event: {}", emit_err);
                            }
                        }
                    }
                }
            }
        }

        info!("Stopped listening for P2P events");
    }
}
