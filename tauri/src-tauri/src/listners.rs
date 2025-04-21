use crate::current_note_state::CurrentNoteState;
use crate::user_state::UserState;
use log::{error, info};
use osvauld_core::models::resource::Resource;
use osvauld_core::models::vector_clock::ResourceVectorClock;
use osvauld_services::{ResourceService, UserService};
use p2p_service::p2p::{P2PEvent, incoming::P2PSender};
use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Listener, Manager};
use tokio::sync::mpsc;

/// Initializes all listeners for the application
/// This connects the Tauri event system with the P2P event system
pub struct EventManager {
    app_handle: AppHandle,
    resource_service: Arc<ResourceService>,
    p2p_receiver: mpsc::UnboundedReceiver<P2PEvent>,
    p2p_sender: P2PSender,
    current_note_state: CurrentNoteState,
    user_service: Arc<UserService>,
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
        user_service: Arc<UserService>,
    ) -> Self {
        Self {
            app_handle,
            p2p_receiver,
            resource_service,
            p2p_sender,
            current_note_state: CurrentNoteState::new(),
            user_service,
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
        self.setup_resource_update_complete_listener();
    }
    /// Setup listener for resource-update-complete events
    fn setup_resource_update_complete_listener(&self) {
        let current_note_state = self.current_note_state.clone();

        self.app_handle
            .listen("resource-update-complete", move |event| {
                let note_state = current_note_state.clone();

                let payload = event.payload();
                if let Ok(json) = serde_json::from_str::<Value>(payload) {
                    if let Some(id) = json.get("id").and_then(|v| v.as_str()) {
                        // Only move current to previous if it's for the current note
                        if let Some(current_id) = note_state.get_current_note() {
                            if current_id == id {
                                note_state.move_current_to_previous();
                                info!(
                                    "Moved current to previous buffer after resource update: {}",
                                    id
                                );
                            } else {
                                info!("Ignoring buffer update for non-active note: {}", id);
                            }
                        }
                    }
                }
            });
    }

    fn setup_note_change_listener(&self) {
        let current_note_state = self.current_note_state.clone();
        let user_service = self.user_service.clone();

        let app_handle = self.app_handle.clone();
        self.app_handle.listen("note-change", move |event| {
            let note_state = current_note_state.clone();
            let payload = event.payload().to_string();
            let user_service = user_service.clone();
            let app_handle_clone = app_handle.clone();
            // Remove quotes if they exist (payload might be a JSON string)
            let note_id = payload.trim_matches('"').to_string();

            info!("Received note-change event with note_id: {}", note_id);

            // Clear previous shared users state
            note_state.clear_shared_users();

            // Set current note
            note_state.set_current_note(Some(note_id.clone()));

            // Clone what we need for the async block
            let note_state = note_state.clone();
            let note_id = note_id.clone();

            // Spawn an async task to fetch shared users
            tokio::spawn(async move {
                // Get current user to exclude from the shared list

                let user_state = app_handle_clone.state::<UserState>();

                // Get current user to exclude from the shared list
                let current_user = match user_state.get_user().await {
                    Ok(user) => Some(user),
                    Err(e) => {
                        error!("Failed to get current user: {}", e);
                        None
                    }
                };

                // Current user ID to exclude
                let current_user_id = match current_user {
                    Some(user) => user.id.clone(),
                    None => {
                        error!("No current user found, cannot fetch shared users");
                        return;
                    }
                };
                // Use UserService to get shared users for the note
                match user_service
                    .get_shared_users_for_note(&note_id, &current_user_id)
                    .await
                {
                    Ok(shared_users) => {
                        // Update the note state with the shared users
                        note_state.set_shared_users(shared_users.clone());
                    }
                    Err(e) => {
                        error!("Failed to get shared users for note {}: {}", note_id, e);
                    }
                }
            });
        });
    }

    // Add a method to access the current note state
    pub fn get_current_note_state(&self) -> CurrentNoteState {
        self.current_note_state.clone()
    }

    /// Setup listener for sync-update events
    fn setup_sync_update_listener(&self) {
        let p2p_sender = self.p2p_sender.clone();
        let current_note_state = self.current_note_state.clone();

        self.app_handle.listen("sync-update", move |event| {
            let payload_str = event.payload();

            // Try to parse the payload to extract update data and resource_id
            match serde_json::from_str::<Value>(&payload_str) {
                Ok(payload) => {
                    if let (Some(update), Some(resource_id)) = (
                        payload.get("update").and_then(|u| u.as_array()),
                        payload.get("resource_id").and_then(|r| r.as_str()),
                    ) {
                        // Check if this update is for the current note
                        if let Some(current_id) = current_note_state.get_current_note() {
                            info!("current id is {}", current_id);
                            if current_id == resource_id {
                                // Convert update array to Vec<u8>
                                let update_bytes: Vec<u8> = update
                                    .iter()
                                    .filter_map(|v| v.as_u64().map(|n| n as u8))
                                    .collect();

                                if !update_bytes.is_empty() {
                                    // Update the buffer with the new state
                                    current_note_state.update_yjs_state(update_bytes);
                                    info!("Updated Yjs state buffer for note: {}", resource_id);
                                } else {
                                    info!("Empty update array received for note: {}", resource_id);
                                }
                            } else {
                                info!("Ignoring update for non-active note: {}", resource_id);
                            }
                        } else {
                            info!("Received update but no active note set: {}", resource_id);
                        }
                    } else {
                        error!("Missing update or resource_id in sync-update payload");
                    }
                }
                Err(e) => {
                    error!("Failed to parse sync-update payload: {}", e);
                }
            }

            // Forward the event to P2P service if needed
            // if let Err(e) = p2p_sender.send_sync_update(payload_str) {
            //     error!("Failed to send sync update event: {}", e);
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
