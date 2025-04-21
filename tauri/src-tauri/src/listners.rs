use log::{error, info};
use serde::Deserialize;

use osvauld_core::models::resource::Resource;
use osvauld_core::models::vector_clock::ResourceVectorClock;
use osvauld_services::ResourceService;
use p2p_service::p2p::{P2PEvent, incoming::P2PSender};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Listener};
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub struct CurrentNoteState {
    note_id: Arc<Mutex<Option<String>>>,
}

impl Default for CurrentNoteState {
    fn default() -> Self {
        Self {
            note_id: Arc::new(Mutex::new(None)),
        }
    }
}

impl CurrentNoteState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_current_note(&self, note_id: Option<String>) {
        let mut current = self.note_id.lock().unwrap();
        *current = note_id;
        info!("Current note set to: {:?}", current);
    }

    pub fn get_current_note(&self) -> Option<String> {
        let current = self.note_id.lock().unwrap();
        current.clone()
    }
}
/// Initializes all listeners for the application
/// This connects the Tauri event system with the P2P event system
pub struct EventManager {
    app_handle: AppHandle,
    resource_service: Arc<ResourceService>,
    p2p_receiver: mpsc::UnboundedReceiver<P2PEvent>,
    p2p_sender: P2PSender,
    current_note_state: CurrentNoteState,
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
            current_note_state: CurrentNoteState::new(),
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
        self.setup_sync_update_listener();
        self.setup_merge_complete_listener();
        self.setup_note_change_listener();
    }

    fn setup_note_change_listener(&self) {
        let current_note_state = self.current_note_state.clone();

        self.app_handle.listen("note-change", move |event| {
            let note_state = current_note_state.clone();
            let payload = event.payload().to_string();

            // Remove quotes if they exist (payload might be a JSON string)
            let note_id = payload.trim_matches('"').to_string();

            info!("Received note-change event with note_id: {}", note_id);

            // Here we can add any additional logic before setting the note

            // Set the current note ID
            note_state.set_current_note(Some(note_id));

            // Here we can add any additional logic after setting the note
            // For future implementation
        });
    }

    // Add a method to access the current note state
    pub fn get_current_note_state(&self) -> CurrentNoteState {
        self.current_note_state.clone()
    }

    /// Setup listener for sync-update events
    fn setup_sync_update_listener(&self) {
        let p2p_sender = self.p2p_sender.clone();
        self.app_handle.listen("sync-update", move |event| {
            let payload_str = event.payload().to_string();
            // Direct send without spawning a task for high-frequency events
            // if let Err(e) = p2p_sender.send_sync_update(payload_str) {
            //     error!("Failed to send sync update event: {}", e);
            // } else {
            //     debug!("Sent sync update event to P2P service"); // Using debug level for high-frequency events
            // }
        });
    }

    /// Setup listener for merge-complete events
    fn setup_merge_complete_listener(&self) {
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
                                    add_remote_vector,
                                    update_remote_vector,
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
                P2PEvent::Connected => self.handle_connected_event(),
                P2PEvent::Disconnected => self.handle_disconnected_event(),
                P2PEvent::HandshakeFailed { error } => self.handle_handshake_failed_event(error),
                P2PEvent::SyncComplete => self.handle_sync_complete_event(),
                P2PEvent::ShareComplete => self.handle_share_complete_event(),
                P2PEvent::EditingEvent { payload } => self.handle_editing_event(payload),
                P2PEvent::Error { message, source } => self.handle_error_event(message, source),
                P2PEvent::UpdateEvent {
                    vector_clock,
                    remote_resource,
                    device_id,
                    user_id,
                } => {
                    self.handle_update_event(vector_clock, remote_resource, device_id, user_id)
                        .await
                }
            }
        }

        info!("Stopped listening for P2P events");
    }

    /// Handle connected event
    fn handle_connected_event(&self) {
        if let Err(e) = self.app_handle.emit("peer-connected", true) {
            error!("Failed to emit peer-connected event: {}", e);
        }
    }

    /// Handle disconnected event
    fn handle_disconnected_event(&self) {
        info!("Peer disconnected");
        if let Err(e) = self.app_handle.emit("peer-disconnected", true) {
            error!("Failed to emit peer-disconnected event: {}", e);
        }
    }

    /// Handle handshake failed event
    fn handle_handshake_failed_event(&self, error: String) {
        info!("Handshake failed: {}", error);
        if let Err(e) = self.app_handle.emit("handshake-failed", error) {
            error!("Failed to emit handshake-failed event: {}", e);
        }
    }

    /// Handle sync complete event
    fn handle_sync_complete_event(&self) {
        info!("Sync completed");
        if let Err(e) = self.app_handle.emit("sync-complete", true) {
            error!("Failed to emit sync-complete event: {}", e);
        }
    }

    /// Handle share complete event
    fn handle_share_complete_event(&self) {
        info!("Share completed");
        if let Err(e) = self.app_handle.emit("share-complete", true) {
            error!("Failed to emit share-complete event: {}", e);
        }
    }

    /// Handle editing event
    fn handle_editing_event(&self, payload: String) {
        info!("Received editing event");
        if let Err(e) = self.app_handle.emit("sync-update-be", payload) {
            error!("Failed to emit sync-update-be event: {}", e);
        }
    }

    /// Handle error event
    fn handle_error_event(&self, message: String, source: String) {
        error!("P2P error from {}: {}", source, message);
        if let Err(e) = self.app_handle.emit("p2p-error", message) {
            error!("Failed to emit p2p-error event: {}", e);
        }
    }

    /// Handle update event
    async fn handle_update_event(
        &self,
        vector_clock: Vec<ResourceVectorClock>,
        remote_resource: Resource,
        device_id: String,
        user_id: String,
    ) {
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
                info!("Successfully processed update event, got local and remote resources");

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
