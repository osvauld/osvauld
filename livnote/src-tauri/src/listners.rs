use std::sync::Arc;

use crate::{
    current_note_state::CurrentNoteState,
    preview_generator::generate_preview_html,
    types::{ResourcePreview, ResourceResponse},
    user_state::{self, UserState},
};
use crypto_utils::CryptoUtils;
use log::{error, info, warn};
use network::p2p::{P2PEvent, incoming::P2PSender};
use persistance::database::RepositoryContext;
use serde_json::Value;
use services::{get_resource_by_id_direct, get_shared_user_devices_for_note};
use tauri::{AppHandle, Emitter, Listener, Manager};
use tokio::sync::{Mutex, mpsc};

/// Initializes all listeners for the application
/// This connects the Tauri event system with the P2P event system
pub struct EventManager {
    app_handle: AppHandle,
    p2p_receiver: mpsc::UnboundedReceiver<P2PEvent>,
    p2p_sender: P2PSender,
    current_note_state: CurrentNoteState,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: Arc<Mutex<CryptoUtils>>,
}

#[derive(Debug, Clone)]
enum UpdateType {
    SyncUpdate,
    AwarenessUpdate,
}

impl UpdateType {
    fn event_name(&self) -> &'static str {
        match self {
            UpdateType::SyncUpdate => "sync-update",
            UpdateType::AwarenessUpdate => "awareness-update",
        }
    }

    fn description(&self) -> &'static str {
        match self {
            UpdateType::SyncUpdate => "sync update",
            UpdateType::AwarenessUpdate => "awareness update",
        }
    }
}

impl EventManager {
    /// Create a new EventManager that handles bidirectional events
    pub fn new(
        app_handle: AppHandle,
        p2p_receiver: mpsc::UnboundedReceiver<P2PEvent>,
        p2p_sender: P2PSender,
        repo_ctx: Arc<RepositoryContext>,
        crypto_utils: Arc<Mutex<CryptoUtils>>,
    ) -> Self {
        Self {
            app_handle,
            p2p_receiver,
            p2p_sender,
            current_note_state: CurrentNoteState::new(),
            repo_ctx,
            crypto_utils,
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
        self.setup_update_listener(UpdateType::SyncUpdate);
        self.setup_update_listener(UpdateType::AwarenessUpdate);
        self.setup_note_change_listener();
        self.setup_resource_update_complete_listener();
    }
    fn setup_update_listener(&self, update_type: UpdateType) {
        let p2p_sender = self.p2p_sender.clone();
        let current_note_state = self.current_note_state.clone();
        let event_name = update_type.event_name();
        let description = update_type.description();

        self.app_handle.listen(event_name, move |event| {
            let payload_str = event.payload();
            let update_type = update_type.clone();
            let p2p_sender = p2p_sender.clone();
            let current_note_state = current_note_state.clone();

            // Try to parse the payload to extract update data and resource_id
            match serde_json::from_str::<Value>(&payload_str) {
                Ok(payload) => {
                    let update_array = payload.get("update").and_then(|u| u.as_array()).cloned();
                    let resource_id = payload
                        .get("resource_id")
                        .and_then(|r| r.as_str())
                        .map(|s| s.to_string());
                    let client_id = payload.get("clientID").and_then(|c| c.as_u64());
                    let doc_type = payload
                        .get("doc_type")
                        .and_then(|c| c.as_str())
                        .unwrap_or("default")
                        .to_string();
                    if let (Some(update), Some(resource_id), Some(client_id), doc_type) =
                        (update_array, resource_id, client_id, doc_type)
                    {
                        // Check if this update is for the current note
                        if let Some(current_id) = current_note_state.get_current_note() {
                            if current_id == resource_id {
                                // Convert update array to Vec<u8>
                                let update_bytes: Vec<u8> = update
                                    .iter()
                                    .filter_map(|v| v.as_u64().map(|n| n as u8))
                                    .collect();

                                if !update_bytes.is_empty() {
                                    // Handle sync updates differently - they need buffer management
                                    if matches!(update_type, UpdateType::SyncUpdate) {
                                        let note_state = current_note_state.clone();
                                        let resource_id_clone = resource_id.to_string();

                                        let update_bytes_clone = update_bytes.clone();
                                        // Update the buffer with the new state (only for sync updates)
                                        let doc_type = doc_type.clone();
                                        tokio::spawn(async move {
                                            info!(
                                                "Applying {} bytes of {} to Yjs buffer",
                                                update_bytes_clone.clone().len(),
                                                description
                                            );
                                            note_state
                                                .merge_to_current(
                                                    update_bytes_clone.clone(),
                                                    &doc_type,
                                                )
                                                .await;
                                            info!(
                                                "Updated Yjs state buffer for note: {}",
                                                resource_id_clone
                                            );
                                        });
                                    }

                                    // Broadcast to active connections (both sync and awareness)
                                    Self::broadcast_update_to_connections(
                                        &p2p_sender,
                                        &current_note_state,
                                        &update_type,
                                        resource_id.to_string(),
                                        client_id as u32,
                                        update_bytes,
                                        description,
                                        doc_type.to_string(),
                                    );
                                } else {
                                    info!(
                                        "Empty update array received for {} on note: {}",
                                        description, resource_id
                                    );
                                }
                            } else {
                                info!(
                                    "Ignoring {} for non-active note: {}",
                                    description, resource_id
                                );
                            }
                        } else {
                            info!(
                                "Received {} but no active note set: {}",
                                description, resource_id
                            );
                        }
                    } else {
                        error!(
                            "Missing update, resource_id, or clientID in {} payload",
                            description
                        );
                    }
                }
                Err(e) => {
                    error!("Failed to parse {} payload: {}", description, e);
                }
            }
        });
    }

    // Helper function to broadcast updates to active connections
    fn broadcast_update_to_connections(
        p2p_sender: &P2PSender,
        current_note_state: &CurrentNoteState,
        update_type: &UpdateType,
        resource_id: String,
        client_id: u32,
        update_bytes: Vec<u8>,
        description: &str,
        doc_type: String,
    ) {
        info!("update_type {:?}", update_type);
        let active_connections = current_note_state.get_active_connections();
        if !active_connections.is_empty() {
            info!(
                "Broadcasting {} to {} active connections",
                description,
                active_connections.len()
            );

            let result = match update_type {
                UpdateType::SyncUpdate => p2p_sender.send_sync_update_to_connections(
                    resource_id,
                    client_id,
                    update_bytes,
                    active_connections,
                    doc_type,
                ),
                UpdateType::AwarenessUpdate => p2p_sender.send_awareness_update_to_connections(
                    resource_id,
                    client_id,
                    update_bytes,
                    active_connections,
                ),
            };

            match result {
                Ok(_) => {
                    info!(
                        "Successfully broadcasted {} to all active connections",
                        description
                    );
                }
                Err(e) => {
                    error!("Failed to broadcast {}: {}", description, e);
                }
            }
        } else {
            info!("No active connections to broadcast {} to", description);
        }
    }
    fn setup_resource_update_complete_listener(&self) {
        let current_note_state = self.current_note_state.clone();
        let p2p_sender = self.p2p_sender.clone();
        self.app_handle
        .listen("resource-update-complete", move |event| {
            let note_state = current_note_state.clone();
            let p2p_sender = p2p_sender.clone();
            let payload = event.payload();
            if let Ok(json) = serde_json::from_str::<Value>(payload) {
                if let Some(id) = json.get("id").and_then(|v| v.as_str()) {
                    if let Some(current_id) = note_state.get_current_note() {
                        if current_id == id {
                            note_state.move_current_to_previous();
                            info!("Moved current to previous buffer after resource update: {}", id);
                            let should_send = note_state.increment_and_check_counter();
                            if should_send {
                                if let Some(state_vectors) = json.get("state_vectors").and_then(|v| v.as_str()) {
                                    let resource_id = id.to_string();
                                        let state_vectors = state_vectors.to_string();
                                    tokio::spawn(async move {
                                        if let Err(e) = Self::broadcast_update_to_inactive_connections(
                                            &note_state,
                                            &p2p_sender,
                                            &resource_id,
                                            &state_vectors,
                                        ).await {
                                            error!("Failed to broadcast updates to inactive connections: {}", e);
                                        }
                                    });
                                } else {
                                    error!("No state_vectors found in payload");
                                }
                            }
                        }
                    }
                }
            }
        });
    }

    async fn broadcast_update_to_inactive_connections(
        note_state: &CurrentNoteState,
        p2p_sender: &P2PSender,
        resource_id: &str,
        state_vectors: &str,
    ) -> Result<(), String> {
        let inactive_connections = note_state.get_inactive_connections();
        if inactive_connections.is_empty() {
            info!("No inactive connections to update");
            return Ok(());
        }

        info!(
            "Broadcasting updates to {} inactive connections",
            inactive_connections.len()
        );

        p2p_sender.send_state_vector_request(
            inactive_connections.clone(),
            resource_id.to_string(),
            state_vectors.to_string(),
        );

        Ok(())
    }

    fn setup_note_change_listener(&self) {
        let current_note_state = self.current_note_state.clone();

        let app_handle = self.app_handle.clone();
        let p2p_sender = self.p2p_sender.clone();
        let repo_ctx = self.repo_ctx.clone();
        self.app_handle.listen("note-change", move |event| {
            let note_state = current_note_state.clone();
            let payload = event.payload().to_string();
            let app_handle_clone = app_handle.clone();
                let note_id = if payload == "null" || payload.trim_matches('"').is_empty() {
            None
        } else {
            Some(payload.trim_matches('"').to_string())
        };
            let p2p_sender_clone = p2p_sender.clone();

            info!("Received note-change event with note_id: {:?}", note_id);
             let previous_note_id = note_state.get_current_note();
        // Check if we're actually changing documents (not just refreshing the same one)
              if let Some(prev_id) = previous_note_id.clone() {
            // If changing from a document to null OR to a different document
            if note_id.is_none() || (note_id.is_some() && note_id.as_ref().unwrap() != &prev_id) {
                info!("Document changing from {} to {:?}", prev_id, note_id);
                // Get active connections for the previous note before clearing
                let active_connections = note_state.get_active_connections();
                if !active_connections.is_empty() {
                    info!(
                        "Found {} active connections for previous note, sending document changed notifications",
                        active_connections.len()
                    );
                    // Send document changed notification to all active connections
                    for connection_id in &active_connections {
                        if let Err(e) = p2p_sender_clone.send_document_changed(
                            connection_id.clone(),
                            prev_id.clone()
                        ) {
                            error!("Failed to notify connection {} about document change: {}", connection_id, e);
                        } else {
                            info!("Sent document change notification to connection: {}", connection_id);
                        }
                    }
                    // Clear active connections for the previous note
                    note_state.clear_active_connections();
                    info!("Cleared all active connections for previous note: {}", prev_id);
                }
            }
        }
            // Clear previous shared users state
            note_state.clear_shared_users();

            // Set current note
            note_state.set_current_note(note_id.clone());

          if let Some(note_id_str) = note_id {
            // Clone what we need for the async block
            let note_state = note_state.clone();
            let note_id = note_id_str.clone();
            let repo_ctx = repo_ctx.clone();
            let p2p_sender = p2p_sender.clone();
            let app_handle = app_handle.clone();
            // Spawn an async task to fetch shared users
            tokio::spawn(async move {
                // Use UserService to get shared users for the note
                let user_state = app_handle_clone.state::<UserState>();
                let current_user = match user_state.get_user().await {
                    Ok(user) => user,
                    Err(e) => {
                        error!("Failed to get current user: {}", e);
                        return;
                    }
                };
                let current_device = match user_state.get_device().await {
                    Ok(device) => device,
                    Err(e) => {
                        error!("Failed to get current device: {}", e);
                        return;
                    }
                };
                let current_user_id = current_user.id;
                let current_device_id = current_device.id;
                match get_shared_user_devices_for_note(&note_id, &current_user_id, &current_device_id, true, repo_ctx.clone())
                    .await
                {
                    Ok((shared_devices, shared_users)) => {
                        if let Err(e) = p2p_sender.send_live_edit_requests(shared_devices.clone()) {
                            error!("Failed to send live edit requests: {}", e);
                        }
                        let _ = app_handle.emit("shared-users-update", shared_users); 
                        // Update the note state with the shared users
                        note_state.set_shared_users(shared_devices.clone());
                    }
                    Err(e) => {
                        error!("Failed to get shared users for note {}: {}", note_id, e);
                    }
                }
            });
        } else {
            info!("Note cleared - no shared users to fetch");
        }
    });
    }

    // Add a method to access the current note state
    pub fn get_current_note_state(&self) -> CurrentNoteState {
        self.current_note_state.clone()
    }

    /// Setup listener for sync-update events
    async fn listen_for_p2p_events(&mut self) {
        info!("Started listening for P2P events");

        while let Some(event) = self.p2p_receiver.recv().await {
            match event {
                P2PEvent::Connected => self.handle_connected_event(),
                P2PEvent::Disconnected => self.handle_disconnected_event(),
                P2PEvent::HandshakeFailed { error } => self.handle_handshake_failed_event(error),
                P2PEvent::SyncComplete => self.handle_sync_complete_event(),
                P2PEvent::ShareComplete => self.handle_share_complete_event(),
                P2PEvent::EditingEvent {
                    resource_id,
                    client_id,
                    updates,
                    doc_type,
                } => {
                    self.handle_editing_event(resource_id, client_id, updates, doc_type)
                        .await
                }
                P2PEvent::AwarenessEvent {
                    resource_id,
                    client_id,
                    awareness_data,
                } => {
                    info!("awareness data recieved");
                    self.handle_awareness_event(resource_id, client_id, awareness_data)
                        .await
                }
                P2PEvent::Error { message, source } => self.handle_error_event(message, source),
                P2PEvent::UpdatesEvent {
                    resource_id,
                    updates,
                    client_id,
                } => {
                    self.handle_update_event(resource_id, updates, client_id)
                        .await
                }
                P2PEvent::LiveEditConnected { connection_id } => {
                    self.handle_live_edit_connected(connection_id).await
                }
                P2PEvent::DocumentCheck {
                    resource_id,
                    connection_id,
                } => self.handle_document_check(resource_id, connection_id).await,
                P2PEvent::DocumentMismatch { connection_id } => {
                    self.handle_document_missmatch(connection_id).await
                }
                P2PEvent::UpdateRequest {
                    resource_id,
                    connection_id,
                    state_vectors,
                    current_user_id,
                } => {
                    self.handle_document_update_request(
                        resource_id,
                        connection_id,
                        state_vectors,
                        current_user_id,
                    )
                    .await
                }
                P2PEvent::ProcessUpdate {
                    resource_id,
                    connection_id,
                    updates,
                    client_id,
                } => {
                    self.handle_document_process_update(
                        resource_id,
                        connection_id,
                        updates,
                        client_id,
                    )
                    .await
                }
                P2PEvent::ProcessUpdateResponse {
                    resource_id,
                    connection_id,
                    updates,
                    client_id,
                } => {
                    self.handle_document_process_update_response(
                        resource_id,
                        connection_id,
                        updates,
                        client_id,
                    )
                    .await
                }
                P2PEvent::CurrentBufferExchange {
                    resource_id,
                    connection_id,
                    updates,
                    client_id,
                } => {
                    self.handle_current_buffer_exchange(
                        resource_id,
                        connection_id,
                        updates,
                        client_id,
                    )
                    .await
                }
                P2PEvent::DocumentChanged {
                    resource_id,
                    connection_id,
                } => {
                    self.handle_document_changed_event(resource_id, connection_id)
                        .await;
                }
                P2PEvent::ResourceAdded { resource_id } => {
                    let user_state = self.app_handle.state::<UserState>();
                    let current_user = match user_state.get_user().await {
                        Ok(user) => user,
                        Err(e) => {
                            error!("Failed to get current user for ResourceAdded event: {}", e);
                            continue; // Skip this event and continue processing
                        }
                    };
                    let decrypted_resource = get_resource_by_id_direct(
                        &resource_id,
                        &current_user.id,
                        self.repo_ctx.clone(),
                        &self.crypto_utils,
                    )
                    .await
                    .unwrap();

                    let (preview, title) =
                        match generate_preview_html(&decrypted_resource.data, 3).await {
                            Ok((preview, title)) => (preview, title),
                            Err(e) => {
                                eprintln!(
                                    "Failed to generate preview for resource {}: {}",
                                    decrypted_resource.id, e
                                );
                                (String::new(), String::new()) // Use empty string as fallback
                            }
                        };

                    let resource_preview = ResourcePreview {
                        id: decrypted_resource.id.clone(),
                        title,
                        preview,
                        favourite: decrypted_resource.favourite,
                        last_accessed: decrypted_resource.last_accessed,
                        folder_id: decrypted_resource.folder_id.clone(),
                        last_modified: decrypted_resource.last_accessed,
                    };

                    if let Err(e) = self.app_handle.emit("resource-added", resource_preview) {
                        error!("Failed to emit live-edit-initialized event: {}", e);
                    }
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

    async fn handle_document_missmatch(&self, connection_id: String) {
        self.current_note_state
            .add_inactive_connection(&connection_id);
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
        state_vectors: String,
        current_user_id: String,
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
            match current_note_state.get_combined_updates().await {
                Ok(combined_updates) => {
                    if let Err(e) = self.p2p_sender.send_live_edit_update_exchange(
                        connection_id.clone(),
                        resource_id.clone(),
                        state_vectors.clone(),
                        combined_updates,
                        current_user_id,
                    ) {
                        error!("Failed to send update exchange: {}", e);
                    } else {
                        info!("Sent update exchange for resource: {}", resource_id);
                    }
                }
                Err(e) => {
                    error!("failed to get buffer {}", e);
                }
            }
            // Send the update exchange
        } else {
            error!("No current document set, cannot handle update request");
        }
    }

    async fn handle_document_process_update(
        &self,
        resource_id: String,
        connection_id: String,
        remote_updates: String,
        client_id: u32,
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

        if is_current_document {
            match current_note_state.get_combined_updates().await {
                Ok(local_buffer) => {
                    self.handle_update_event(
                        resource_id.clone(),
                        remote_updates.clone(),
                        client_id,
                    )
                    .await;

                    // Send both local buffer and remote updates in response
                    if let Err(e) = self.p2p_sender.send_live_edit_update_exchange_response(
                        connection_id,
                        resource_id,
                        local_buffer,
                        remote_updates,
                    ) {
                        error!("Failed to send update exchange response: {}", e);
                    }
                }
                Err(e) => {
                    error!("failed to get buffer details")
                }
            }
        }
    }

    async fn handle_document_process_update_response(
        &self,
        resource_id: String,
        connection_id: String,
        updates: String,
        client_id: u32,
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
            current_note_state.add_active_connection(connection_id.clone());

            info!(
                "Added connection {} to active sessions for resource {}",
                connection_id, resource_id
            );

            // Apply updates to frontend if any
            if !updates.is_empty() {
                info!("Applying {} bytes of updates to frontend", updates.len());
                self.handle_update_event(resource_id.clone(), updates, client_id)
                    .await;
            }
            //TODO: make this current buffer
            match current_note_state.get_combined_updates().await {
                Ok(current_buffer) => {
                    // Send current buffer to peer (now we always send, even if empty)
                    info!(
                        "Sending final current buffer ({} bytes) exchange",
                        current_buffer.len()
                    );

                    if let Err(e) = self.p2p_sender.send_current_buffer_exchange(
                        connection_id.clone(),
                        resource_id.clone(),
                        current_buffer,
                    ) {
                        error!("Failed to send current buffer: {}", e);
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
                }
                Err(e) => {
                    error!("Failed to get combined updates: {}", e);
                }
            };
        } else {
            warn!("No active document, ignoring update response");
        }
    }

    async fn handle_current_buffer_exchange(
        &self,
        resource_id: String,
        connection_id: String,
        updates: String,
        client_id: u32,
    ) {
        info!(
            "Processing current buffer exchange for resource: {}, from connection: {}",
            resource_id, connection_id
        );

        // Get current note state
        let current_note_state = self.current_note_state.clone();

        // Forward updates to the frontend
        if !updates.is_empty() {
            info!(
                "Forwarding {} bytes of buffer updates to frontend",
                updates.len()
            );
            self.handle_update_event(resource_id.clone(), updates, client_id)
                .await;
        }

        // Verify this is the document we're currently editing
        if let Some(current_resource_id) = current_note_state.get_current_note() {
            if current_resource_id != resource_id {
                warn!(
                    "Received buffer exchange for non-active document: {}, current is {}",
                    resource_id, current_resource_id
                );
                return;
            }

            // Check if this connection is already in active connections
            let is_active = current_note_state.is_connection_active(&connection_id);

            if !is_active {
                // Add this connection to active sessions for this resource
                current_note_state.add_active_connection(connection_id.clone());

                info!(
                    "Added connection {} to active sessions for resource {}",
                    connection_id, resource_id
                );

                // Get current buffer to send back

                match current_note_state.get_combined_updates().await {
                    Ok(current_buffer) => {
                        if let Err(e) = self.p2p_sender.send_current_buffer_exchange(
                            connection_id.clone(),
                            resource_id.clone(),
                            current_buffer,
                        ) {
                            error!("Failed to send current buffer response: {}", e);
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
                    }

                    Err(e) => {
                        error!("Failed to emit live-edit-active event: {}", e);
                    }
                }
                // Send current buffer to peer
            } else {
                // Connection is already active, no need to send anything back
                info!(
                    "Connection {} is already active for resource {}, no response needed",
                    connection_id, resource_id
                );
            }
        } else {
            warn!("No active document, ignoring buffer exchange");
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

    async fn handle_editing_event(
        &self,
        resource_id: String,
        client_id: u32,
        updates: Vec<u8>,
        doc_type: String,
    ) {
        info!(
            "Received editing event for resource {}, from client {}, with {} bytes",
            resource_id,
            client_id,
            updates.len()
        );

        // Check if this is for the current document
        let current_note_state = self.current_note_state.clone();
        if let Some(current_resource_id) = current_note_state.get_current_note() {
            if current_resource_id != resource_id {
                info!(
                    "Received editing event for non-active document: {}, current is {}",
                    resource_id, current_resource_id
                );
                return;
            }

            // Create payload for frontend
            let payload = serde_json::json!({
                "resource_id": resource_id,
                "updates": updates,
                "client_id": client_id.to_string(),
                "doc_type": doc_type.clone(),
            });

            // Emit to the frontend for direct application to the ProseMirror document
            if let Err(e) = self.app_handle.emit("live-updates", payload) {
                error!("Failed to emit document-updates event: {}", e);
            } else {
                info!(
                    "Successfully emitted document-updates event for resource: {}",
                    resource_id
                );
            }
        } else {
            info!("No active document, ignoring editing event");
        }
    }

    async fn handle_awareness_event(
        &self,
        resource_id: String,
        client_id: u32,
        awareness_data: Vec<u8>,
    ) {
        info!(
            "Received awareness event for resource {}, from client {}, with {} bytes",
            resource_id,
            client_id,
            awareness_data.len()
        );

        // Check if this is for the current document
        let current_note_state = self.current_note_state.clone();
        if let Some(current_resource_id) = current_note_state.get_current_note() {
            if current_resource_id != resource_id {
                info!(
                    "Received awareness event for non-active document: {}, current is {}",
                    resource_id, current_resource_id
                );
                return;
            }

            // Create payload for frontend with client_id as string for consistency with existing code
            let payload = serde_json::json!({
                "resource_id": resource_id,
                "updates": awareness_data,
                "client_id": client_id.to_string()
            });

            // Emit to the frontend for application to awareness
            if let Err(e) = self.app_handle.emit("awareness-updates", payload) {
                error!("Failed to emit awareness-updates event: {}", e);
            } else {
                info!(
                    "Successfully emitted awareness-updates event for resource: {}",
                    resource_id
                );
            }
        } else {
            info!("No active document, ignoring awareness event");
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
    async fn handle_update_event(&self, resource_id: String, updates: String, client_id: u32) {
        info!(
            "Received updates event for resource {}, with {} bytes",
            resource_id,
            updates.len()
        );

        let current_note_state = self.current_note_state.clone();
        if let Some(resource_id) = current_note_state.get_current_note() {
            let payload = serde_json::json!({
                "resource_id": resource_id,
                "updates": updates,
                "client_id": client_id
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
        } else {
            let user_state = self.app_handle.state::<UserState>();
            let current_user = match user_state.get_user().await {
                Ok(user) => user,
                Err(e) => {
                    error!("Failed to get current user: {}", e);
                    return;
                }
            };
            match get_resource_by_id_direct(
                &resource_id,
                &current_user.id,
                self.repo_ctx.clone(),
                &self.crypto_utils,
            )
            .await
            {
                Ok(decrypted_resource) => {
                    let (preview, title) =
                        match generate_preview_html(&decrypted_resource.data, 3).await {
                            Ok((preview, title)) => (preview, title),
                            Err(e) => {
                                eprintln!(
                                    "Failed to generate preview for resource {}: {}",
                                    decrypted_resource.id.clone(),
                                    e
                                );
                                (String::new(), String::new()) // Use empty string as fallback
                            }
                        };

                    let resource_preview = ResourcePreview {
                        id: decrypted_resource.id.clone(),
                        title,
                        preview,
                        favourite: decrypted_resource.favourite,
                        last_accessed: decrypted_resource.last_accessed,
                        folder_id: decrypted_resource.folder_id.clone(),
                        last_modified: decrypted_resource.last_accessed,
                    };
                    let _ = self.app_handle.emit("resource-update", resource_preview);
                }
                Err(e) => {}
            }
        }
    }

    async fn handle_document_changed_event(&self, resource_id: String, connection_id: String) {
        info!(
            "Received document changed event from connection {} for resource {}",
            connection_id, resource_id
        );

        // Get current note state
        let current_note_state = self.current_note_state.clone();
        if let Some(current_resource_id) = current_note_state.get_current_note() {
            if current_resource_id != resource_id {
                warn!(
                    "Resource ID mismatch in document changed event: expected {}, got {}. 
                This might indicate state inconsistency.",
                    current_resource_id, resource_id
                );
            }
        } else {
            warn!(
                "Received document changed event for resource {} when no current document is set",
                resource_id
            );
        }
        // Remove this connection from active connections
        current_note_state.remove_active_connection(&connection_id);
        current_note_state.add_inactive_connection(&connection_id);

        info!(
            "Removed connection {} from active connections for resource {}",
            connection_id, resource_id
        );

        // Notify frontend if needed
        if let Err(e) = self.app_handle.emit(
            "peer-document-changed",
            serde_json::json!({
                "connection_id": connection_id,
                "resource_id": resource_id,
            }),
        ) {
            error!("Failed to emit peer-document-changed event: {}", e);
        }
    }
}
