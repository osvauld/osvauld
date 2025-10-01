use crate::EventManager;
use crate::current_note_state::CurrentNoteState;
use log::{error, info, warn};
use network::p2p::incoming::P2PSender;
use rand;
use tokio::time::{Duration, interval};
/// Live edit negotiation and connection management handlers
impl EventManager {
    /// Handle live edit connected event
    pub(crate) async fn handle_live_edit_connected(&self, connection_id: String) {
        info!("Live edit connection established with: {}", connection_id);

        let current_note_state = self.current_note_state.clone();
        let resource_id = match current_note_state.get_current_note().await {
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
            .add_inactive_connection(&connection_id)
            .await;
    }

    /// Handle document check
    /// For chat: Check if we have this resource in our ChatState buffers
    pub(crate) async fn handle_document_check(&self, resource_id: String, connection_id: String) {
        info!("Received document check for chat resource: {}", resource_id);

        // For chat: Try to get state vectors from ChatState
        let state_vectors = match self.chat_state.get_state_vectors(&resource_id).await {
            Ok(vector) => {
                info!("Found chat resource in buffers: {}", resource_id);
                vector
            }
            Err(e) => {
                info!("Chat resource not in buffers (will load on demand): {} - {}", resource_id, e);
                String::new()
            }
        };

        let is_match = !state_vectors.is_empty();

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
            "Sent document check response for chat resource: {}, is_match: {}",
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
    /// For chat: Generate updates for peer based on their state vectors
    pub(crate) async fn handle_document_update_request(
        &self,
        resource_id: String,
        connection_id: String,
        state_vectors: String,
    ) {
        info!("Received update request for chat resource: {}", resource_id);

        // Try to generate updates from ChatState
        let peer_updates = match self
            .chat_state
            .generate_updates_for_peer(&resource_id, &state_vectors)
            .await
        {
            Ok(updates) => updates,
            Err(e) => {
                error!("Failed to generate updates for peer from ChatState: {}", e);
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
            info!("Sent update exchange for chat resource: {}", resource_id);
        }
    }

    /// Handle document process update
    /// For chat: Apply peer updates and generate diff for response
    pub(crate) async fn handle_document_process_update(
        &self,
        resource_id: String,
        connection_id: String,
        remote_updates: String,
        client_id: u32,
    ) {
        info!("Processing document update for chat resource: {}", resource_id);

        // Apply updates and generate diff using ChatState
        let peer_updates = match self
            .chat_state
            .apply_updates_and_generate_diff(&resource_id, &remote_updates)
            .await
        {
            Ok(buffer) => buffer,
            Err(e) => {
                error!("Failed to apply updates and generate diff from ChatState: {}", e);
                return;
            }
        };

        // Mark connection as active for this resource
        self.current_note_state
            .add_active_connection(connection_id.clone())
            .await;

        // Apply the remote updates (emit to frontend)
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
    /// For chat: Apply peer updates received during reconciliation
    pub(crate) async fn handle_document_process_update_response(
        &self,
        resource_id: String,
        connection_id: String,
        updates: String,
        client_id: u32,
    ) {
        info!(
            "Processing update response for chat resource: {}, from connection: {}",
            resource_id, connection_id
        );

        // Apply peer updates to ChatState
        if let Err(e) = self.chat_state.apply_peer_updates(&resource_id, &updates).await {
            error!("Failed to apply peer updates to ChatState: {}", e);
        }

        // Add this connection to active sessions
        self.current_note_state
            .add_active_connection(connection_id.clone())
            .await;
        info!(
            "Added connection {} to active sessions for chat resource {}",
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
        if let Some(current_resource_id) = self.current_note_state.get_current_note().await {
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
            .remove_active_connection(&connection_id)
            .await;
        self.current_note_state
            .add_inactive_connection(&connection_id)
            .await;

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

    pub async fn start_reconciliation_timer(&self) {
        let current_note_state = self.current_note_state.clone();
        let p2p_sender = self.p2p_sender.clone();

        tokio::spawn(async move {
            // Random interval between 30-60 seconds to avoid thundering herd
            let base_interval = 30;
            let jitter = rand::random::<u64>() % 30; // 0-29 seconds
            let interval_secs = base_interval + jitter;

            let mut interval_timer = interval(Duration::from_secs(interval_secs));

            info!(
                "Started reconciliation timer with {} second intervals",
                interval_secs
            );

            loop {
                interval_timer.tick().await;

                // Only reconcile if we have an active note
                if let Some(resource_id) = current_note_state.get_current_note().await {
                    let active_connections = current_note_state.get_active_connections().await;

                    if !active_connections.is_empty() {
                        info!(
                            "Starting reconciliation for {} active connections on note: {}",
                            active_connections.len(),
                            resource_id
                        );

                        Self::perform_reconciliation(
                            &current_note_state,
                            &p2p_sender,
                            &resource_id,
                            active_connections,
                        )
                        .await;
                    }
                }
            }
        });
    }

    /// Perform reconciliation for active connections
    async fn perform_reconciliation(
        current_note_state: &CurrentNoteState,
        p2p_sender: &P2PSender,
        resource_id: &str,
        active_connections: Vec<String>,
    ) {
        // Get current state vectors for our local state
        let local_state_vectors = match current_note_state.get_state_vectors().await {
            Ok(vectors) => vectors,
            Err(e) => {
                error!(
                    "Failed to get local state vectors for reconciliation: {}",
                    e
                );
                return;
            }
        };

        info!(
            "Sending reconciliation requests to {} connections",
            active_connections.len()
        );

        for connection_id in active_connections {
            info!("Sending reconciliation to connection: {}", connection_id);

            // Send document check response with is_match=true and current state vectors
            // This will trigger the peer to start state vector exchange
            if let Err(e) = p2p_sender.send_live_edit_document_check_response(
                connection_id.clone(),
                resource_id.to_string(),
                true, // Always true for reconciliation
                local_state_vectors.clone(),
            ) {
                error!(
                    "Failed to send reconciliation request to connection {}: {}",
                    connection_id, e
                );
            } else {
                info!(
                    "Sent reconciliation request to connection: {}",
                    connection_id
                );
            }
        }
    }
}
