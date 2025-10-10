use crate::p2p::incoming::IncomingEvent;
use crate::p2p::P2PService;
use osvauld_core::models::{
    p2p::{LiveEditMessage, Message},
    ConnectionAction, ConnectionType, ResourceUpdateMsg,
};
use services::get_resource_state_vector;
use tokio::sync::mpsc;
use tracing::{debug, error, info, instrument, warn};

/// Implementation of P2PService methods for handling incoming events and event processing
impl P2PService {
    /// Starts processing incoming events in a background task
    #[instrument(skip_all, level = "debug")]
    pub fn start_processing_incoming_events(
        service: Self,
        mut receiver: mpsc::UnboundedReceiver<IncomingEvent>,
    ) {
        // Spawn a task to process events
        tokio::spawn(async move {
            info!("Started processing incoming events");

            while let Some(event) = receiver.recv().await {
                match event {
                    IncomingEvent::SyncUpdateBroadcast {
                        connection_ids,
                        resource_id,
                        client_id,
                        updates,
                        doc_type,
                    } => {
                        service
                            .handle_sync_update_broadcast(
                                connection_ids,
                                resource_id,
                                client_id,
                                updates,
                                doc_type,
                            )
                            .await;
                    }

                    IncomingEvent::AwarenessUpdateBroadcast {
                        connection_ids,
                        resource_id,
                        client_id,
                        awareness_data,
                    } => {
                        service
                            .handle_awareness_update_broadcast(
                                connection_ids,
                                resource_id,
                                client_id,
                                awareness_data,
                            )
                            .await;
                    }

                    IncomingEvent::LiveEditDocumentCheck {
                        connection_id,
                        resource_id,
                    } => {
                        service
                            .handle_live_edit_document_check(connection_id, resource_id)
                            .await;
                    }
                    IncomingEvent::LiveEditDocumentCheckResponse {
                        connection_id,
                        resource_id,
                        is_match,
                        state_vectors,
                    } => {
                        let _ = service
                            .handle_live_edit_document_check_response(
                                connection_id,
                                resource_id,
                                is_match,
                                state_vectors,
                            )
                            .await;
                    }
                    IncomingEvent::LiveEditUpdateExchange {
                        connection_id,
                        resource_id,
                        peer_updates,
                    } => {
                        let _ = service
                            .handle_live_edit_update_exchange(
                                connection_id,
                                resource_id,
                                peer_updates,
                            )
                            .await;
                    }
                    IncomingEvent::LiveEditUpdateExchangeResponse {
                        connection_id,
                        resource_id,
                        peer_updates,
                    } => {
                        let _ = service
                            .handle_live_edit_update_exchange_response(
                                connection_id,
                                resource_id,
                                peer_updates,
                            )
                            .await;
                    }
                    IncomingEvent::DocumentChanged {
                        connection_id,
                        resource_id,
                    } => {
                        service
                            .handle_document_changed(connection_id, resource_id)
                            .await;
                    }
                    IncomingEvent::StartLiveConnection { device_ids } => {
                        let _ = service.handle_start_live_edit(&device_ids).await;
                    }
                    IncomingEvent::BroadCastStateVectorRequest {
                        connection_ids,
                        resource_id,
                    } => {
                        let _ = service
                            .broadcast_state_vector_request(connection_ids, resource_id)
                            .await;
                    }
                    IncomingEvent::RequestFolderToken {
                        folder_id,
                        device_id,
                        domain,
                    } => {
                        service
                            .handle_request_folder_token(folder_id, device_id, domain)
                            .await;
                    }
                }
            }

            info!("Stopped processing incoming events");
        });
    }

    pub async fn handle_start_live_edit(&self, device_ids: &[String]) -> Result<(), String> {
        info!(
            "Starting live edit connections to {} devices (fire-and-forget)",
            device_ids.len()
        );

        if device_ids.is_empty() {
            return Ok(());
        }

        // Fire and forget - spawn all connection tasks and return immediately
        for device_id in device_ids.iter() {
            let self_clone = self.clone();
            let device_id = device_id.clone();

            tokio::spawn(async move {
                debug!("Attempting live edit connection to device: {}", device_id);

                match self_clone
                    .connect_with_ticket(
                        &device_id,
                        ConnectionType::User,
                        Some(ConnectionAction::LiveEdit),
                    )
                    .await
                {
                    Ok(Some(connection)) => {
                        info!(
                            "Successfully established live edit connection to device: {} (connection: {})",
                            device_id,
                            connection.get_id()
                        );
                    }
                    Ok(None) => {
                        info!(
                            "Live edit connection to device {} is being established",
                            device_id
                        );
                    }
                    Err(e) => {
                        error!(
                            "Failed to establish live edit connection to device {}: {}",
                            device_id, e
                        );
                    }
                }
            });
        }

        info!(
            "Initiated {} live edit connection attempts",
            device_ids.len()
        );
        Ok(())
    }

    /// Handles a live edit document check event
    #[instrument(skip(self), fields(connection_id = %connection_id, resource_id = %resource_id), level = "info")]

    pub async fn handle_live_edit_document_check(
        &self,
        connection_id: String,
        resource_id: String,
    ) {
        info!(
            "Processing live-edit-document-check event for resource: {}",
            resource_id
        );

        let document_check = Message::LiveEdit(LiveEditMessage::DocumentCheck {
            resource_id: resource_id.clone(),
        });

        if let Err(e) = self
            .send_or_reconnect(&connection_id, document_check, ConnectionAction::LiveEdit)
            .await
        {
            error!("Failed to send document check: {}", e);
        }
    }
    #[instrument(skip(self), fields(connection_id = %connection_id, resource_id = %resource_id, is_match = is_match), level = "info")]
    pub async fn handle_live_edit_document_check_response(
        &self,
        connection_id: String,
        resource_id: String,
        is_match: bool,
        state_vectors: String,
    ) -> Result<(), String> {
        info!(
            "Processing document check response: resource={}, is_match={}",
            resource_id, is_match
        );

        // Get the connection from the connection manager
        let connection = self
            .get_connection_by_id(&connection_id)
            .await
            .map_err(|e| format!("Failed to get connection for state vector exchange: {}", e))?;

        if is_match {
            info!("Document match confirmed, proceeding with state vector exchange");

            // Create a state vector exchange message
            let state_vector_message = Message::LiveEdit(LiveEditMessage::StateVectorExchange {
                resource_id: resource_id.clone(),
                state_vectors,
            });

            connection
                .send_message(state_vector_message)
                .await
                .map_err(|e| format!("Failed to send state vector exchange message: {}", e))?;

            info!("State vector exchange message sent successfully");
            Ok(())
        } else {
            info!("Document mismatch, sending not same document message");

            let message = Message::LiveEdit(LiveEditMessage::NotSameDocument);
            connection
                .send_message(message)
                .await
                .map_err(|e| format!("Failed to send not same document message: {}", e))?;

            info!("Not same document message sent successfully");
            Ok(())
        }
    }
    async fn handle_live_edit_update_exchange(
        &self,
        connection_id: String,
        resource_id: String,
        peer_updates: String,
    ) -> Result<(), String> {
        info!("Received update request for resource: {}", resource_id);

        match self.get_connection_by_id(&connection_id).await {
            // Generate updates and state vector based on peer's state vector
            Ok(connection) => {
                let live_edit_message = LiveEditMessage::UpdateExchange {
                    resource_id,
                    updates: peer_updates,
                };
                let message = Message::LiveEdit(live_edit_message);
                if let Err(e) = connection.send_message(message).await {
                    error!("Failed to send state vector exchange message: {}", e);
                    return Err(format!("Failed to state vector exchange: {}", e));
                } else {
                    info!("State vector exchange message sent successfully");
                    return Ok(());
                }
            }

            Err(e) => {
                error!("Failed to get connection for state vector exchange: {}", e);
                return Err(format!("Failed to state vector exchange: {}", e));
            }
        }
    }

    async fn handle_live_edit_update_exchange_response(
        &self,
        connection_id: String,
        resource_id: String,
        peer_updates: String,
    ) -> Result<(), String> {
        info!(
            "Processing update exchange response for resource: {}",
            resource_id
        );

        // Get the connection from the connection manager
        match self.get_connection_by_id(&connection_id).await {
            Ok(connection) => {
                // Create UpdateExchangeResponse message
                let live_edit_message = LiveEditMessage::UpdateExchangeResponse {
                    resource_id,
                    updates: peer_updates,
                };

                let message = Message::LiveEdit(live_edit_message);

                // Send the message
                if let Err(e) = connection.send_message(message).await {
                    error!("Failed to send update exchange response: {}", e);
                    return Err(format!("failed to send updated exchange response {}", e));
                } else {
                    info!("Update exchange response sent successfully");
                    Ok(())
                }
            }
            Err(e) => {
                error!(
                    "Failed to get connection for update exchange response: {}",
                    e
                );
                return Err(format!("failed to send updated exchange response {}", e));
            }
        }
    }
    #[instrument(skip(self), fields(connection_id = %connection_id, resource_id = %resource_id), level = "info")]
    pub async fn handle_document_changed(&self, connection_id: String, resource_id: String) {
        info!(
            "Handling document changed event for resource {} from connection {}",
            resource_id, connection_id
        );

        // Get the connection
        match self.get_connection_by_id(&connection_id).await {
            Ok(connection) => {
                let message = Message::LiveEdit(LiveEditMessage::DocumentChange { resource_id });

                match connection.send_message(message).await {
                    Ok(_) => {
                        debug!("Successfully sent update to connection: {}", connection_id);
                    }
                    Err(e) => {
                        let error_msg = format!("Failed to send to {}: {}", connection_id, e);
                        error!("{}", error_msg);
                    }
                }
                // Start a fresh disconnection check with a new timer
                // if let Err(e) = connection.check_for_possible_disconnection().await {
                //     error!("Error checking for possible disconnection: {}", e);
                // }
            }
            Err(e) => {
                error!("Failed to get connection {}: {}", connection_id, e);
            }
        }
    }
    #[instrument(skip(self, connection_ids, message), fields(connections_count = connection_ids.len(), message_type = %message_type), level = "info")]
    async fn broadcast_live_edit_message(
        &self,
        connection_ids: Vec<String>,
        message: Message,
        message_type: &str,
        resource_id: &str,
        client_id: u32,
    ) {
        let mut failed_connections = Vec::new();

        // Get connections from state
        let connections = {
            let state_guard = self.state.lock().await;
            let state = state_guard
                .as_ref()
                .expect("P2P service not initialized when broadcasting");
            state.connections.clone()
        };

        // Try sending to all connections
        for connection_id in connection_ids {
            if let Ok(conn) = connections.get_peer_connection(&connection_id).await {
                if conn.connection.close_reason().is_none() {
                    // Connection looks healthy, try to send
                    if let Err(_) = conn.send_message(message.clone()).await {
                        failed_connections.push(connection_id);
                    }
                } else {
                    // Connection is closed
                    failed_connections.push(connection_id);
                }
            } else {
                // No connection exists
                failed_connections.push(connection_id);
            }
        }

        // Spawn reconnection attempts for failed connections (non-blocking)
        if !failed_connections.is_empty() {
            let self_clone = self.clone();
            tokio::spawn(async move {
                for connection_id in failed_connections {
                    info!("Attempting to reconnect to {}", connection_id);

                    // Fire and forget reconnection
                    let _ = self_clone
                        .connect_with_ticket(
                            &connection_id,
                            ConnectionType::User,
                            Some(ConnectionAction::LiveEdit),
                        )
                        .await;
                }
            });
        }
    }

    async fn broadcast_state_vector_request(
        &self,
        connection_ids: Vec<String>,
        resource_id: String,
    ) -> Result<(), String> {
        info!(
            "Broadcasting state vector request to {} connections",
            connection_ids.len()
        );

        // Early return if no connections
        if connection_ids.is_empty() {
            info!("No connections to broadcast to");
            return Ok(());
        }

        // Get required data
        let connections = self.get_connections_by_ids(&connection_ids).await;
        let current_user = self
            .get_current_user()
            .await
            .map_err(|e| format!("Failed to fetch current user: {}", e))?;

        let state_vectors = get_resource_state_vector(
            &resource_id,
            &current_user.id,
            self.repo_ctx.clone(),
            &self.crypto_utils,
        )
        .await
        .map_err(|e| format!("Failed to get state vectors: {}", e))?;

        let ucan_token = self
            .repo_ctx
            .share_repo
            .get_ucan_token_by_resource(&resource_id, &current_user.id)
            .await
            .map_err(|e| format!("Failed to fetch UCAN token: {}", e))?;

        // Create message once
        let message = Message::MergeUpdate(ResourceUpdateMsg::StateVectorRequest {
            resource_id: resource_id.clone(),
            state_vectors,
            ucan_token,
        });

        // Send to all connections and collect results
        let mut errors = Vec::new();
        for connection in connections {
            let connection_id = connection.get_id();
            info!(
                "Sending state vector request to connection: {}",
                connection_id
            );

            if let Err(e) = connection.send_message(message.clone()).await {
                let error_msg = format!("Failed to send to connection {}: {}", connection_id, e);
                error!("{}", error_msg);
                errors.push(error_msg);
            }
        }

        // Return error if any sends failed
        if !errors.is_empty() {
            return Err(format!(
                "Failed to send to {} connections: {}",
                errors.len(),
                errors.join("; ")
            ));
        }

        info!(
            "Successfully sent state vector requests to all {} connections",
            connection_ids.len()
        );
        Ok(())
    }

    // Simplified sync update handler using the generic function
    #[instrument(skip(self, updates, connection_ids), fields(connections_count = connection_ids.len()), level = "info")]
    pub async fn handle_sync_update_broadcast(
        &self,
        connection_ids: Vec<String>,
        resource_id: String,
        client_id: u32,
        updates: Vec<u8>,
        doc_type: String,
    ) {
        let message = Message::LiveEdit(LiveEditMessage::DocumentUpdate {
            resource_id: resource_id.clone(),
            client_id,
            updates,
            doc_type,
        });

        self.broadcast_live_edit_message(
            connection_ids,
            message,
            "sync update",
            &resource_id,
            client_id,
        )
        .await;
    }

    // Simplified awareness update handler using the generic function
    #[instrument(skip(self, awareness_data, connection_ids), fields(connections_count = connection_ids.len()), level = "info")]
    pub async fn handle_awareness_update_broadcast(
        &self,
        connection_ids: Vec<String>,
        resource_id: String,
        client_id: u32,
        awareness_data: Vec<u8>,
    ) {
        let message = Message::LiveEdit(LiveEditMessage::AwarenessUpdate {
            resource_id: resource_id.clone(),
            client_id,
            awareness_data,
        });

        self.broadcast_live_edit_message(
            connection_ids,
            message,
            "awareness update",
            &resource_id,
            client_id,
        )
        .await;
    }

    /// Handles a folder token request by sending a message to the sovereign node
    #[instrument(skip(self), fields(folder_id = %folder_id, device_id = %device_id), level = "info")]
    pub async fn handle_request_folder_token(
        &self,
        folder_id: String,
        device_id: String,
        domain: String,
    ) {
        info!(
            "Handling folder token request for folder {} to device {}",
            folder_id, device_id
        );

        // Create the FolderTokenRequest message
        let message = Message::FolderTokenRequest(osvauld_core::models::p2p::FolderTokenRequest {
            folder_id: folder_id.clone(),
            domain,
        });

        // Send the message using send_or_reconnect
        match self
            .send_or_reconnect(&device_id, message, ConnectionAction::UserSync)
            .await
        {
            Ok(_) => {
                info!(
                    "Successfully sent folder token request for folder {}",
                    folder_id
                );
            }
            Err(e) => {
                error!(
                    "Failed to send folder token request for folder {}: {}",
                    folder_id, e
                );
            }
        }
    }
}
