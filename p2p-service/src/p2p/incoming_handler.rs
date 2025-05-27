use crate::p2p::incoming::IncomingEvent;
use crate::p2p::P2PService;
use osvauld_core::models::p2p::{LiveEditMessage, Message};
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
                    } => {
                        service
                            .handle_sync_update_broadcast(
                                connection_ids,
                                resource_id,
                                client_id,
                                updates,
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
                    } => {
                        service
                            .handle_live_edit_document_check_response(
                                connection_id,
                                resource_id,
                                is_match,
                            )
                            .await;
                    }
                    IncomingEvent::LiveEditUpdateExchange {
                        connection_id,
                        resource_id,
                        state_vector,
                        buffer,
                    } => {
                        service
                            .handle_live_edit_update_exchange(
                                connection_id,
                                resource_id,
                                state_vector,
                                buffer,
                            )
                            .await;
                    }
                    IncomingEvent::LiveEditUpdateExchangeResponse {
                        connection_id,
                        resource_id,
                        state_vector,
                        local_buffer,
                        remote_updates,
                    } => {
                        service
                            .handle_live_edit_update_exchange_response(
                                connection_id,
                                resource_id,
                                state_vector,
                                local_buffer,
                                remote_updates,
                            )
                            .await
                    }
                    IncomingEvent::DocumentChanged {
                        connection_id,
                        resource_id,
                    } => {
                        service
                            .handle_document_changed(connection_id, resource_id)
                            .await;
                    }
                    IncomingEvent::CurrentBufferExchange {
                        connection_id,
                        resource_id,
                        buffer,
                    } => {
                        service
                            .handle_current_buffer_exchange(connection_id, resource_id, buffer)
                            .await;
                    }
                }
            }

            info!("Stopped processing incoming events");
        });
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

        // Get the connection from the connection manager
        match self.get_connection_by_id(&connection_id).await {
            Ok(connection) => {
                //mark connection for live editing.
                connection.set_live_editing_active().await;
                // Create a LiveEdit DocumentCheck message
                let document_check = Message::LiveEdit(LiveEditMessage::DocumentCheck {
                    resource_id: resource_id.clone(),
                });

                // Send the document check message
                if let Err(e) = connection.send_message(document_check).await {
                    error!("Failed to send document check message: {}", e);
                } else {
                    info!(
                        "Document check message sent successfully for resource: {}",
                        resource_id
                    );
                }
            }
            Err(e) => {
                error!("Failed to get connection for live editing: {}", e);
            }
        }
    }

    #[instrument(skip(self), fields(connection_id = %connection_id, resource_id = %resource_id, is_match = is_match), level = "info")]
    pub async fn handle_live_edit_document_check_response(
        &self,
        connection_id: String,
        resource_id: String,
        is_match: bool,
    ) {
        info!(
            "Processing document check response: resource={}, is_match={}",
            resource_id, is_match
        );

        // Get the connection from the connection manager
        match self.get_connection_by_id(&connection_id).await {
            Ok(connection) => {
                if is_match {
                    info!("Document match confirmed, proceeding with state vector exchange");
                    match self
                        .sync_service
                        .resource_service
                        .get_resource_state_vector(&resource_id)
                        .await
                    {
                        Ok(state_vector) => {
                            // Create a state vector exchange message
                            let state_vector_message =
                                Message::LiveEdit(LiveEditMessage::StateVectorExchange {
                                    resource_id: resource_id.clone(),
                                    state_vector,
                                });

                            // Send the state vector exchange message
                            if let Err(e) = connection.send_message(state_vector_message).await {
                                error!("Failed to send state vector exchange message: {}", e);
                            } else {
                                info!("State vector exchange message sent successfully");
                            }
                        }
                        Err(e) => {
                            error!(
                                "Failed to get state vector for resource {}: {}",
                                resource_id, e
                            );
                        }
                    }
                } else {
                    let message = Message::LiveEdit(LiveEditMessage::NotSameDocument);
                    connection.set_live_editing_inactive().await;
                    if let Err(e) = connection.send_message(message).await {
                        error!("Failed to send state vector exchange message: {}", e);
                    } else {
                        info!("State vector exchange message sent successfully");
                    }
                }
            }
            Err(e) => {
                error!("Failed to get connection for state vector exchange: {}", e);
            }
        }
    }
    async fn handle_live_edit_update_exchange(
        &self,
        connection_id: String,
        resource_id: String,
        state_vector: Vec<u8>,
        buffer: Vec<u8>,
    ) {
        info!("Received update request for resource: {}", resource_id);

        match self.get_connection_by_id(&connection_id).await {
            // Generate updates and state vector based on peer's state vector
            Ok(connection) => {
                match self
                    .sync_service
                    .resource_service
                    .apply_updates_and_get_peer_updates(&resource_id, &buffer, &state_vector)
                    .await
                {
                    Ok((updates, current_state_vector)) => {
                        info!("Generated {} bytes of updates for peer", updates.len());
                        let live_edit_message = LiveEditMessage::UpdateExchange {
                            resource_id,
                            updates,
                            buffer,
                            state_vector: current_state_vector,
                        };
                        let message = Message::LiveEdit(live_edit_message);
                        if let Err(e) = connection.send_message(message).await {
                            error!("Failed to send state vector exchange message: {}", e);
                        } else {
                            info!("State vector exchange message sent successfully");
                        }
                    }
                    Err(e) => {
                        error!("Failed to generate updates for peer: {}", e);
                    }
                }
            }

            Err(e) => {
                error!("Failed to get connection for state vector exchange: {}", e);
            }
        }
    }

    async fn handle_live_edit_update_exchange_response(
        &self,
        connection_id: String,
        resource_id: String,
        state_vector: Vec<u8>,
        local_buffer: Vec<u8>,
        remote_updates: Vec<u8>,
    ) {
        info!(
            "Processing update exchange response for resource: {}",
            resource_id
        );

        // Get the connection from the connection manager
        match self.get_connection_by_id(&connection_id).await {
            Ok(connection) => {
                // Combine local buffer and remote updates for processing
                let mut combined_updates = Vec::new();

                // Add local buffer first if it's not empty
                if !local_buffer.is_empty() {
                    info!(
                        "Adding local buffer ({} bytes) to processing",
                        local_buffer.len()
                    );
                    combined_updates.extend_from_slice(&local_buffer);
                }

                // Add remote updates if they're not empty
                if !remote_updates.is_empty() {
                    info!(
                        "Adding remote updates ({} bytes) to processing",
                        remote_updates.len()
                    );
                    combined_updates.extend_from_slice(&remote_updates);
                }

                if combined_updates.is_empty() {
                    info!("No updates to process for resource: {}", resource_id);
                    return;
                }

                // Process the combined updates and get updates for peer
                match self
                    .sync_service
                    .resource_service
                    .apply_updates_and_get_peer_updates(
                        &resource_id,
                        &combined_updates,
                        &state_vector,
                    )
                    .await
                {
                    Ok((updates, current_state_vector)) => {
                        info!("Generated {} bytes of updates for peer", updates.len());

                        // Create UpdateExchangeResponse message
                        let live_edit_message = LiveEditMessage::UpdateExchangeResponse {
                            resource_id,
                            updates,
                            state_vector: current_state_vector,
                        };

                        let message = Message::LiveEdit(live_edit_message);

                        // Send the message
                        if let Err(e) = connection.send_message(message).await {
                            error!("Failed to send update exchange response: {}", e);
                        } else {
                            info!("Update exchange response sent successfully");
                        }
                    }
                    Err(e) => {
                        error!("Failed to generate updates for peer: {}", e);
                    }
                }
            }
            Err(e) => {
                error!(
                    "Failed to get connection for update exchange response: {}",
                    e
                );
            }
        }
    }
    async fn handle_current_buffer_exchange(
        &self,
        connection_id: String,
        resource_id: String,
        buffer: Vec<u8>,
    ) {
        info!(
            "Sending current buffer exchange for resource: {}",
            resource_id
        );

        match self.get_connection_by_id(&connection_id).await {
            Ok(connection) => {
                let message = Message::LiveEdit(LiveEditMessage::CurrentBufferExchange {
                    resource_id,
                    buffer,
                });

                if let Err(e) = connection.send_message(message).await {
                    error!("Failed to send current buffer exchange: {}", e);
                } else {
                    info!("Current buffer exchange sent successfully");
                }
            }
            Err(e) => {
                error!(
                    "Failed to get connection for current buffer exchange: {}",
                    e
                );
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
                // Cancel any existing disconnection timer
                connection.cancel_disconnection_timer().await;

                // Disable live editing for this connection
                connection.set_live_editing_inactive().await;
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
                if let Err(e) = connection.check_for_possible_disconnection().await {
                    error!("Error checking for possible disconnection: {}", e);
                }
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
        info!(
            "Processing {} broadcast for resource {} from client {} to {} connections",
            message_type,
            resource_id,
            client_id,
            connection_ids.len()
        );

        if connection_ids.is_empty() {
            warn!(
                "Empty connection IDs list, no {} broadcast performed",
                message_type
            );
            return;
        }

        // Get the connections from the connection manager
        let connections = self.get_connections_by_ids(&connection_ids).await;

        if connections.is_empty() {
            warn!(
                "No valid connections found for {} broadcasting",
                message_type
            );
            return;
        }

        info!(
            "Broadcasting {} to {} active connections",
            message_type,
            connections.len()
        );

        let mut success_count = 0;
        let mut errors = Vec::new();
        let connections_len = connections.len();

        // Send to each connection
        for connection in connections {
            let conn_id = connection.get_id();
            debug!("Sending {} to: {}", message_type, conn_id);

            match connection.send_message(message.clone()).await {
                Ok(_) => {
                    success_count += 1;
                    debug!(
                        "Successfully sent {} to connection: {}",
                        message_type, conn_id
                    );
                }
                Err(e) => {
                    let error_msg =
                        format!("Failed to send {} to {}: {}", message_type, conn_id, e);
                    error!("{}", error_msg);
                    errors.push(error_msg);
                }
            }
        }

        if errors.is_empty() {
            info!(
                "{} broadcast completed successfully to all {} connections",
                message_type, success_count
            );
        } else {
            error!(
                "{} broadcast partially successful: {}/{} connections succeeded, errors: {}",
                message_type,
                success_count,
                connections_len,
                errors.join(", ")
            );
        }
    }

    // Simplified sync update handler using the generic function
    #[instrument(skip(self, updates, connection_ids), fields(connections_count = connection_ids.len()), level = "info")]
    pub async fn handle_sync_update_broadcast(
        &self,
        connection_ids: Vec<String>,
        resource_id: String,
        client_id: u32,
        updates: Vec<u8>,
    ) {
        let message = Message::LiveEdit(LiveEditMessage::DocumentUpdate {
            resource_id: resource_id.clone(),
            client_id,
            updates,
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
}
