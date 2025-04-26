use crate::current_note_state::CurrentNoteState;
use crate::user_state::UserState;
use log::{error, info, warn};
use osvauld_services::{ResourceService, UserService};
use p2p_service::p2p::{P2PEvent, incoming::P2PSender};
use rendezvous_client::rendezvous_service::RendezvousService;
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
    rendezvous_service: Arc<RendezvousService>,
}

impl EventManager {
    /// Create a new EventManager that handles bidirectional events
    pub fn new(
        app_handle: AppHandle,
        p2p_receiver: mpsc::UnboundedReceiver<P2PEvent>,
        resource_service: Arc<ResourceService>,
        p2p_sender: P2PSender,
        user_service: Arc<UserService>,
        rendezvous_service: Arc<RendezvousService>,
    ) -> Self {
        Self {
            app_handle,
            p2p_receiver,
            resource_service,
            p2p_sender,
            current_note_state: CurrentNoteState::new(),
            user_service,
            rendezvous_service,
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

        let rendezvous_service = self.rendezvous_service.clone();
        let app_handle = self.app_handle.clone();
        self.app_handle.listen("note-change", move |event| {
            let note_state = current_note_state.clone();
            let payload = event.payload().to_string();
            let user_service = user_service.clone();
            let app_handle_clone = app_handle.clone();
            let note_id = payload.trim_matches('"').to_string();
            let rendezvous_service = rendezvous_service.clone();

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
                        if let Err(e) = rendezvous_service
                            .initialize_live_editing(shared_users.clone())
                            .await
                        {
                            error!("Failed to initialize live editing: {}", e);
                        } else {
                            info!(
                                "Successfully initialized live editing for note: {}",
                                note_id
                            );
                        }
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
                P2PEvent::UpdatesEvent {
                    resource_id,
                    updates,
                } => self.handle_update_event(resource_id, updates).await,
                P2PEvent::LiveEditConnected { connection_id } => {
                    self.handle_live_edit_connected(connection_id).await
                }
                P2PEvent::DocumentCheck {
                    resource_id,
                    connection_id,
                } => self.handle_document_check(resource_id, connection_id).await,
                P2PEvent::UpdateRequest {
                    resource_id,
                    connection_id,
                    state_vector,
                } => {
                    self.handle_document_update_request(resource_id, connection_id, state_vector)
                        .await
                }
                P2PEvent::ProcessUpdate {
                    resource_id,
                    connection_id,
                    state_vector,
                    updates,
                    buffer,
                } => {
                    self.handle_document_process_update(
                        resource_id,
                        connection_id,
                        state_vector,
                        buffer,
                        updates,
                    )
                    .await
                }
                P2PEvent::ProcessUpdateResponse {
                    resource_id,
                    connection_id,
                    updates,
                } => {
                    self.handle_document_process_update_response(
                        resource_id,
                        connection_id,
                        updates,
                    )
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

    async fn handle_live_edit_connected(&self, connection_id: String) {
        info!("Live edit connection established with: {}", connection_id);

        // Get current note ID
        let current_note_state = self.current_note_state.clone();
        if let Some(resource_id) = current_note_state.get_current_note() {
            info!("Initiating live editing for resource: {}", resource_id);

            // Send document check to initiate live editing
            if let Err(e) = self
                .p2p_sender
                .send_live_edit_document_check(connection_id.clone(), resource_id.clone())
            {
                error!("Failed to send live edit document check: {}", e);
            } else {
                info!("Sent document check for resource: {}", resource_id);
            }

            // Notify frontend that live editing has been initialized
            if let Err(e) = self.app_handle.emit(
                "live-edit-initialized",
                serde_json::json!({
                    "connection_id": connection_id,
                    "resource_id": resource_id,
                }),
            ) {
                error!("Failed to emit live-edit-initialized event: {}", e);
            }
        } else {
            warn!("Cannot initiate live editing - no current note selected");
        }
    }

    async fn handle_document_check(&self, resource_id: String, connection_id: String) {
        info!("Received document check for resource: {}", resource_id);

        // Check if we're currently editing this resource
        let current_note_state = self.current_note_state.clone();
        let is_match = if let Some(current_resource_id) = current_note_state.get_current_note() {
            current_resource_id == resource_id
        } else {
            false
        };

        // Send the response
        if let Err(e) = self.p2p_sender.send_live_edit_document_check_response(
            connection_id.clone(),
            resource_id.clone(),
            is_match,
        ) {
            error!("Failed to send document check response: {}", e);
        } else {
            info!(
                "Sent document check response for resource: {}, is_match: {}",
                resource_id, is_match
            );
        }

        // Notify frontend of document check
        if let Err(e) = self.app_handle.emit(
            "document-check",
            serde_json::json!({
                "connection_id": connection_id,
                "resource_id": resource_id,
                "is_match": is_match
            }),
        ) {
            error!("Failed to emit document-check event: {}", e);
        }
    }

    async fn handle_document_update_request(
        &self,
        resource_id: String,
        connection_id: String,
        state_vector: Vec<u8>,
    ) {
        info!("Received update request for resource: {}", resource_id);

        // Verify this is the document we're currently editing
        let current_note_state = self.current_note_state.clone();
        if let Some(current_resource_id) = current_note_state.get_current_note() {
            if current_resource_id != resource_id {
                error!(
                    "Resource ID mismatch in update request: expected {}, got {}",
                    current_resource_id, resource_id
                );
                return;
            }

            // Combine current and previous buffers
            let mut combined_buffer = Vec::new();

            // First add current buffer if available
            if let Some(current_buffer) = current_note_state.get_current_yjs_state() {
                info!(
                    "Adding current buffer ({} bytes) to update exchange",
                    current_buffer.len()
                );
                combined_buffer.extend_from_slice(&current_buffer);
            }

            // Then add previous buffer if available
            if let Some(previous_buffer) = current_note_state.get_previous_yjs_state() {
                info!(
                    "Adding previous buffer ({} bytes) to update exchange",
                    previous_buffer.len()
                );
                combined_buffer.extend_from_slice(&previous_buffer);
            }

            // Send the update exchange
            if let Err(e) = self.p2p_sender.send_live_edit_update_exchange(
                connection_id.clone(),
                resource_id.clone(),
                state_vector.clone(),
                combined_buffer,
            ) {
                error!("Failed to send update exchange: {}", e);
            } else {
                info!("Sent update exchange for resource: {}", resource_id);
            }
        } else {
            error!("No current document set, cannot handle update request");
        }
    }

    async fn handle_document_process_update(
        &self,
        resource_id: String,
        connection_id: String,
        state_vector: Vec<u8>,
        buffer: Vec<u8>,
        updates: Vec<u8>,
    ) {
        info!("Processing document update for resource: {}", resource_id);

        // Get current note state
        let current_note_state = self.current_note_state.clone();

        // Check if this is the document we're currently editing
        let is_current_document =
            if let Some(current_resource_id) = current_note_state.get_current_note() {
                current_resource_id == resource_id
            } else {
                false
            };

        // Combine remote updates (peer's buffer and updates)
        let mut remote_updates = Vec::new();
        remote_updates.extend_from_slice(&updates);
        if !buffer.is_empty() {
            remote_updates.extend_from_slice(&buffer);
        }

        if !is_current_document {
            // Document mismatch - notify peer
            info!(
                "Document mismatch: current is {:?}, update is for {}",
                current_note_state.get_current_note(),
                resource_id
            );

            // Emit document changed event
            if let Err(e) = self
                .p2p_sender
                .send_document_changed(connection_id.clone(), resource_id.clone())
            {
                error!("Failed to send document changed notification: {}", e);
            }

            // Still apply remote updates to frontend
            if !remote_updates.is_empty() {
                self.handle_update_event(resource_id.clone(), remote_updates.clone())
                    .await;
            }

            // Send empty local buffer with remote updates back as response
            if let Err(e) = self.p2p_sender.send_live_edit_update_exchange_response(
                connection_id,
                resource_id,
                state_vector,
                Vec::new(),     // Empty local buffer
                remote_updates, // Include remote updates in response
            ) {
                error!("Failed to send update exchange response: {}", e);
            }
        } else {
            // Current document matches - collect local buffer for response
            let mut local_buffer = Vec::new();

            // Add current buffer if available
            if let Some(current_buffer) = current_note_state.get_current_yjs_state() {
                info!(
                    "Including current buffer ({} bytes) in response",
                    current_buffer.len()
                );
                local_buffer.extend_from_slice(&current_buffer);
            }

            // Add previous buffer if available
            if let Some(previous_buffer) = current_note_state.get_previous_yjs_state() {
                info!(
                    "Including previous buffer ({} bytes) in response",
                    previous_buffer.len()
                );
                local_buffer.extend_from_slice(&previous_buffer);
            }

            // Apply remote updates to frontend
            if !remote_updates.is_empty() {
                self.handle_update_event(resource_id.clone(), remote_updates.clone())
                    .await;
            }

            // Send both local buffer and remote updates in response
            if let Err(e) = self.p2p_sender.send_live_edit_update_exchange_response(
                connection_id,
                resource_id,
                state_vector,
                local_buffer,   // Send our local buffer
                remote_updates, // Include remote updates in response
            ) {
                error!("Failed to send update exchange response: {}", e);
            }
        }
    }

    async fn handle_document_process_update_response(
        &self,
        resource_id: String,
        connection_id: String,
        updates: Vec<u8>,
    ) {
        info!(
            "Processing update response for resource: {}, from connection: {}",
            resource_id, connection_id
        );

        // Get current note state
        let current_note_state = self.current_note_state.clone();

        // Verify this is the document we're currently editing
        if let Some(current_resource_id) = current_note_state.get_current_note() {
            if current_resource_id != resource_id {
                warn!(
                    "Received update response for non-active document: {}, current is {}",
                    resource_id, current_resource_id
                );
                return;
            }

            // Add this connection to active sessions for this resource
            // (You'll need to add a method to track active connections in CurrentNoteState)
            current_note_state.add_active_connection(connection_id.clone());

            info!(
                "Added connection {} to active sessions for resource {}",
                connection_id, resource_id
            );

            // Apply updates to frontend if any
            if !updates.is_empty() {
                info!("Applying {} bytes of updates to frontend", updates.len());
                self.handle_update_event(resource_id.clone(), updates).await;
            }

            // Get current buffer to send as final step
            if let Some(current_buffer) = current_note_state.get_current_yjs_state() {
                // Only send if we have something
                if !current_buffer.is_empty() {
                    info!(
                        "Sending final current buffer ({} bytes) exchange",
                        current_buffer.len()
                    );

                    // Send current buffer to peer
                    if let Err(e) = self.p2p_sender.send_current_buffer_exchange(
                        connection_id,
                        resource_id,
                        current_buffer,
                    ) {
                        error!("Failed to send current buffer: {}", e);
                    }
                }
            }

            // Notify frontend that live editing is now active with this peer
            if let Err(e) = self.app_handle.emit(
                "live-edit-active",
                serde_json::json!({
                    "connection_id": connection_id,
                    "resource_id": resource_id,
                }),
            ) {
                error!("Failed to emit live-edit-active event: {}", e);
            }
        } else {
            warn!("No active document, ignoring update response");
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

    /// Handle updates event
    async fn handle_update_event(&self, resource_id: String, updates: Vec<u8>) {
        info!(
            "Received updates event for resource {}, with {} bytes",
            resource_id,
            updates.len()
        );

        // Keep the updates as a Vec<u8> but send them as an array to JSON
        // YJS expects Uint8Array and the frontend will convert it properly
        let payload = serde_json::json!({
            "resource_id": resource_id,
            "updates": updates
        });

        // Emit to the frontend for direct application to the ProseMirror document
        if let Err(e) = self.app_handle.emit("document-updates", payload) {
            error!("Failed to emit document-updates event: {}", e);
        } else {
            info!(
                "Successfully emitted document-updates event for resource: {}",
                resource_id
            );
        }
    }
}
