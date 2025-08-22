use super::{EventManager, UpdateType};
use crate::current_note_state::CurrentNoteState;
use crate::user_state::UserState;
use log::{error, info};
use network::p2p::{P2PEvent, incoming::P2PSender};
use persistance::database::RepositoryContext;
use serde_json::Value;
use services::get_shared_user_devices_for_note;
use std::sync::Arc;
use tauri::{Emitter, Listener, Manager};

struct UpdatePayload {
    update_bytes: Vec<u8>,
    resource_id: String,
    client_id: u32,
    doc_type: String,
}
impl EventManager {
    /// Set up listeners for Tauri events
    pub(super) fn setup_tauri_listeners(&self) {
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
            let description = description.clone();

            // Parse and validate payload
            let payload = match Self::parse_update_payload(payload_str, &description) {
                Some(p) => p,
                None => return,
            };

            // Check if update is for current note
            if !Self::is_current_note(&current_note_state, &payload.resource_id) {
                info!(
                    "Ignoring {} for non-active note: {}",
                    description, payload.resource_id
                );
                return;
            }

            // Handle sync update buffer management if needed
            if matches!(update_type, UpdateType::SyncUpdate) {
                Self::apply_sync_update_to_buffer(
                    current_note_state.clone(),
                    payload.update_bytes.clone(),
                    payload.doc_type.clone(),
                    payload.resource_id.clone(),
                    description.to_string(),
                );
            }

            // Broadcast to active connections
            Self::broadcast_update(
                &p2p_sender,
                &current_note_state,
                &update_type,
                payload,
                description,
            );
        });
    }

    /// Parse and validate the update payload from the event
    fn parse_update_payload(payload_str: &str, description: &str) -> Option<UpdatePayload> {
        let payload = match serde_json::from_str::<Value>(payload_str) {
            Ok(p) => p,
            Err(e) => {
                error!("Failed to parse {} payload: {}", description, e);
                return None;
            }
        };

        let update_array = payload.get("update")?.as_array()?;
        let resource_id = payload.get("resource_id")?.as_str()?.to_string();
        let client_id = payload.get("clientID")?.as_u64()? as u32;
        let doc_type = payload
            .get("doc_type")
            .and_then(|c| c.as_str())
            .unwrap_or("default")
            .to_string();

        // Convert update array to Vec<u8>
        let update_bytes: Vec<u8> = update_array
            .iter()
            .filter_map(|v| v.as_u64().map(|n| n as u8))
            .collect();

        if update_bytes.is_empty() {
            info!(
                "Empty update array received for {} on note: {}",
                description, resource_id
            );
            return None;
        }

        Some(UpdatePayload {
            update_bytes,
            resource_id,
            client_id,
            doc_type,
        })
    }

    /// Check if the given resource_id matches the current note
    pub fn is_current_note(current_note_state: &CurrentNoteState, resource_id: &str) -> bool {
        match current_note_state.get_current_note() {
            Some(current_id) => current_id == resource_id,
            None => {
                info!("No active note set for resource: {}", resource_id);
                false
            }
        }
    }

    /// Apply sync updates to the Yjs buffer
    fn apply_sync_update_to_buffer(
        note_state: CurrentNoteState,
        update_bytes: Vec<u8>,
        doc_type: String,
        resource_id: String,
        description: String,
    ) {
        tokio::spawn(async move {
            info!(
                "Applying {} bytes of {} to Yjs buffer",
                update_bytes.len(),
                description
            );
            note_state.merge_to_current(update_bytes, &doc_type).await;
            info!("Updated Yjs state buffer for note: {}", resource_id);
        });
    }

    /// Broadcast updates to active P2P connections
    fn broadcast_update(
        p2p_sender: &P2PSender,
        current_note_state: &CurrentNoteState,
        update_type: &UpdateType,
        payload: UpdatePayload,
        description: &str,
    ) {
        let active_connections = current_note_state.get_active_connections();
        if active_connections.is_empty() {
            info!("No active connections to broadcast {} to", description);
            return;
        }

        info!(
            "Broadcasting {} to {} active connections",
            description,
            active_connections.len()
        );

        let result = match update_type {
            UpdateType::SyncUpdate => p2p_sender.send_sync_update_to_connections(
                payload.resource_id,
                payload.client_id,
                payload.update_bytes,
                active_connections,
                payload.doc_type,
            ),
            UpdateType::AwarenessUpdate => p2p_sender.send_awareness_update_to_connections(
                payload.resource_id,
                payload.client_id,
                payload.update_bytes,
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
    }

    fn setup_resource_update_complete_listener(&self) {
        let current_note_state = self.current_note_state.clone();

        self.app_handle
            .listen("resource-update-complete", move |event| {
                let note_state = current_note_state.clone();
                let payload = event.payload();

                if let Ok(json) = serde_json::from_str::<Value>(payload) {
                    if let Some(id) = json.get("id").and_then(|v| v.as_str()) {
                        if let Some(current_id) = note_state.get_current_note() {
                            if current_id == id {
                                note_state.move_current_to_previous();
                                info!(
                                    "Moved current to previous buffer after resource update: {}",
                                    id
                                );
                            }
                        }
                    }
                }
            });
    }

    fn setup_note_change_listener(&self) {
        let current_note_state = self.current_note_state.clone();
        let app_handle = self.app_handle.clone();
        let p2p_sender = self.p2p_sender.clone();
        let repo_ctx = self.repo_ctx.clone();

        self.app_handle.listen("note-change", move |event| {
            let note_state = current_note_state.clone();
            let p2p_sender = p2p_sender.clone();
            let app_handle = app_handle.clone();
            let repo_ctx = repo_ctx.clone();

            // Parse the note ID from the payload
            let new_note_id = Self::parse_note_id(event.payload());
            info!("Received note-change event with note_id: {:?}", new_note_id);

            let previous_note_id = note_state.get_current_note();

            // Handle closing/switching away from the previous note
            if let Some(prev_id) = previous_note_id {
                Self::handle_leaving_note(&note_state, &p2p_sender, &prev_id, &new_note_id);
            }

            // Handle opening a new note
            match new_note_id {
                None => {
                    // No new note, just reset state
                    note_state.reset_to_default();
                }
                Some(note_id) => {
                    // Clear previous shared users and set new note
                    note_state.clear_shared_users();
                    note_state.set_current_note(Some(note_id.clone()));

                    // Fetch and set up shared users for the new note
                    Self::setup_shared_users_for_note(
                        note_state, note_id, app_handle, repo_ctx, p2p_sender,
                    );
                }
            }
        });
    }

    /// Parse note ID from event payload
    fn parse_note_id(payload: &str) -> Option<String> {
        let payload = payload.to_string();
        info!("payload {}", payload);

        if payload == "null" || payload.trim_matches('"').is_empty() {
            None
        } else {
            Some(payload.trim_matches('"').to_string())
        }
    }

    /// Handle leaving the current note (closing or switching)
    fn handle_leaving_note(
        note_state: &crate::current_note_state::CurrentNoteState,
        p2p_sender: &network::p2p::incoming::P2PSender,
        previous_note_id: &str,
        new_note_id: &Option<String>,
    ) {
        // Notify inactive connections about state vectors
        Self::notify_inactive_connections(note_state, p2p_sender, previous_note_id);

        // Check if we're actually switching to a different note
        let is_switching = match new_note_id {
            None => true,                               // Closing the note
            Some(new_id) => new_id != previous_note_id, // Switching to different note
        };

        if !is_switching {
            return; // Same note, no need to handle connections
        }

        info!(
            "Document changing from {} to {:?}",
            previous_note_id, new_note_id
        );

        // Notify active connections about document change
        Self::notify_active_connections_document_changed(note_state, p2p_sender, previous_note_id);

        // Clear all connections
        note_state.clear_active_connections();
        note_state.clear_inactive_connections();
        info!(
            "Cleared all connections for previous note: {}",
            previous_note_id
        );
    }

    /// Notify inactive connections about state vectors
    fn notify_inactive_connections(
        note_state: &CurrentNoteState,
        p2p_sender: &P2PSender,
        note_id: &str,
    ) {
        let inactive_connections = note_state.get_inactive_connections();
        if inactive_connections.is_empty() {
            return;
        }

        info!(
            "Broadcasting state vectors for note {} to {} inactive connections",
            note_id,
            inactive_connections.len()
        );

        if let Err(e) =
            p2p_sender.send_state_vector_request(inactive_connections, note_id.to_string())
        {
            error!(
                "Failed to broadcast state vectors to inactive connections: {}",
                e
            );
        }
    }

    /// Notify active connections that the document has changed
    fn notify_active_connections_document_changed(
        note_state: &CurrentNoteState,
        p2p_sender: &P2PSender,
        previous_note_id: &str,
    ) {
        let active_connections = note_state.get_active_connections();
        if active_connections.is_empty() {
            return;
        }

        info!(
            "Found {} active connections for previous note, sending document changed notifications",
            active_connections.len()
        );

        for connection_id in &active_connections {
            if let Err(e) = p2p_sender
                .send_document_changed(connection_id.clone(), previous_note_id.to_string())
            {
                error!(
                    "Failed to notify connection {} about document change: {}",
                    connection_id, e
                );
            } else {
                info!(
                    "Sent document change notification to connection: {}",
                    connection_id
                );
            }
        }
    }

    /// Set up shared users for the newly opened note
    fn setup_shared_users_for_note(
        note_state: CurrentNoteState,
        note_id: String,
        app_handle: tauri::AppHandle,
        repo_ctx: Arc<RepositoryContext>,
        p2p_sender: P2PSender,
    ) {
        tokio::spawn(async move {
            // Get current user and device
            let user_state = app_handle.state::<UserState>();

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
            info!("note_id {}, user_id {}", &note_id, &current_user_id);

            // Fetch shared users and devices for the note
            match get_shared_user_devices_for_note(
                &note_id,
                &current_user_id,
                &current_device_id,
                true,
                repo_ctx,
            )
            .await
            {
                Ok((shared_devices, shared_users)) => {
                    // Send live edit requests to shared devices
                    if let Err(e) = p2p_sender.send_live_edit_requests(shared_devices.clone()) {
                        error!("Failed to send live edit requests: {}", e);
                    }

                    // Emit shared users update to frontend
                    let _ = app_handle.emit("shared-users-update", shared_users);

                    // Update the note state with shared devices
                    note_state.set_shared_users(shared_devices);
                }
                Err(e) => {
                    error!("Failed to get shared users for note {}: {}", note_id, e);
                }
            }
        });
    }
}
