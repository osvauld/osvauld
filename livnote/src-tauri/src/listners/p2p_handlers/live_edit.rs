use crate::EventManager;
use log::{error, info, warn};

/// Live edit negotiation and connection management handlers
impl EventManager {
    /// Handle live edit connected event
    pub(crate) async fn handle_live_edit_connected(&self, connection_id: String) {
        info!("Live edit connection established with: {}", connection_id);

        let current_note_state = self.current_note_state.clone();
        let resource_id = match current_note_state.get_current_note() {
            Some(id) => id,
            None => {
                warn!("Cannot initiate live editing - no current note selected");
                return;
            }
        };

        info!("Initiating live editing for resource: {}", resource_id);

        // Send document check to initiate live editing
        if let Err(e) =
            self.send_live_edit_document_check(connection_id.clone(), resource_id.clone())
        {
            error!("Failed to send live edit document check: {}", e);
            return;
        }

        info!("Sent document check for resource: {}", resource_id);

        // Notify frontend that live editing has been initialized
        let _ = self.emit_json(
            "live-edit-initialized",
            serde_json::json!({
                "connection_id": connection_id,
                "resource_id": resource_id,
            }),
        );
    }

    /// Handle document mismatch
    pub(crate) async fn handle_document_missmatch(&self, connection_id: String) {
        self.current_note_state
            .add_inactive_connection(&connection_id);
    }

    /// Handle document check
    pub(crate) async fn handle_document_check(&self, resource_id: String, connection_id: String) {
        info!("Received document check for resource: {}", resource_id);

        // Check if we're currently editing this resource
        let is_match = EventManager::is_current_note(&self.current_note_state, &resource_id);
        let mut state_vectors = String::new();
        if is_match {
            state_vectors = match self.current_note_state.get_state_vectors().await {
                Ok(vector) => vector,
                Err(e) => {
                    error!("failed to get state_vectors{} ", e);
                    return;
                }
            }
        }

        // Send the response
        if let Err(e) = self.send_live_edit_document_check_response(
            connection_id.clone(),
            resource_id.clone(),
            is_match,
            state_vectors,
        ) {
            error!("Failed to send document check response: {}", e);
            return;
        }

        info!(
            "Sent document check response for resource: {}, is_match: {}",
            resource_id, is_match
        );

        // Notify frontend of document check
        let _ = self.emit_json(
            "document-check",
            serde_json::json!({
                "connection_id": connection_id,
                "resource_id": resource_id,
                "is_match": is_match
            }),
        );
    }

    /// Handle document update request
    pub(crate) async fn handle_document_update_request(
        &self,
        resource_id: String,
        connection_id: String,
        state_vectors: String,
    ) {
        info!("Received update request for resource: {}", resource_id);

        // Verify this is the document we're currently editing
        if !EventManager::is_current_note(&self.current_note_state, &resource_id) {
            error!(
                "Resource ID mismatch in update request: got {}",
                resource_id
            );
            return;
        }

        // Combine current and previous buffers
        let peer_updates = match self
            .current_note_state
            .generate_updates_for_peer(&state_vectors)
            .await
        {
            Ok(updates) => updates,
            Err(e) => {
                error!("Failed to get buffer: {}", e);
                return;
            }
        };

        if let Err(e) = self.send_live_edit_update_exchange(
            connection_id.clone(),
            resource_id.clone(),
            peer_updates,
        ) {
            error!("Failed to send update exchange: {}", e);
        } else {
            info!("Sent update exchange for resource: {}", resource_id);
        }
    }

    /// Handle document process update
    pub(crate) async fn handle_document_process_update(
        &self,
        resource_id: String,
        connection_id: String,
        remote_updates: String,
        client_id: u32,
    ) {
        info!("Processing document update for resource: {}", resource_id);

        if !EventManager::is_current_note(&self.current_note_state, &resource_id) {
            return;
        }

        let peer_updates = match self
            .current_note_state
            .apply_updates_and_generate_diff(&remote_updates)
            .await
        {
            Ok(buffer) => buffer,
            Err(e) => {
                error!("Failed to get buffer details: {}", e);
                return;
            }
        };

        // Apply the remote updates
        self.handle_update_event(resource_id.clone(), remote_updates.clone(), client_id)
            .await;

        // Send both local buffer and remote updates in response
        if let Err(e) =
            self.send_live_edit_update_exchange_response(connection_id, resource_id, peer_updates)
        {
            error!("Failed to send update exchange response: {}", e);
        }
    }

    /// Handle document process update response
    pub(crate) async fn handle_document_process_update_response(
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

        // Verify this is the document we're currently editing
        if !EventManager::is_current_note(&self.current_note_state, &resource_id) {
            warn!(
                "Received update response for non-active document: {}",
                resource_id
            );
            return;
        }

        // Add this connection to active sessions
        self.current_note_state
            .add_active_connection(connection_id.clone());
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

        // Notify frontend that live editing is now active
        let _ = self.emit_json(
            "live-edit-active",
            serde_json::json!({
                "connection_id": connection_id,
                "resource_id": resource_id,
            }),
        );
    }

    /// Handle document changed event
    pub(crate) async fn handle_document_changed_event(
        &self,
        resource_id: String,
        connection_id: String,
    ) {
        info!(
            "Received document changed event from connection {} for resource {}",
            connection_id, resource_id
        );

        // Verify resource ID consistency
        if let Some(current_resource_id) = self.current_note_state.get_current_note() {
            if current_resource_id != resource_id {
                warn!(
                    "Resource ID mismatch in document changed event: expected {}, got {}",
                    current_resource_id, resource_id
                );
            }
        } else {
            warn!(
                "Received document changed event for resource {} when no current document is set",
                resource_id
            );
        }

        // Move connection from active to inactive
        self.current_note_state
            .remove_active_connection(&connection_id);
        self.current_note_state
            .add_inactive_connection(&connection_id);

        info!(
            "Moved connection {} from active to inactive for resource {}",
            connection_id, resource_id
        );

        // Notify frontend
        let _ = self.emit_json(
            "peer-document-changed",
            serde_json::json!({
                "connection_id": connection_id,
                "resource_id": resource_id,
            }),
        );
    }
}
