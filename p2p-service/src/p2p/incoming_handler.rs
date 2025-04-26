use crate::p2p::incoming::IncomingEvent;
use crate::p2p::P2PService;
use osvauld_core::models::p2p::{LiveEditMessage, Message};
use tokio::sync::mpsc;
use tracing::{error, info, instrument, warn};

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
                    IncomingEvent::SyncUpdate { payload } => {
                        service.handle_sync_update(payload).await;
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
                    } => {}
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

    #[instrument(skip(self, payload), level = "info")]
    pub async fn handle_sync_update(&self, payload: String) {
        info!("Processing sync-update event");

        // Get the current state

        // let connection_id = format!("{}:{}", user_id, device_id);

        // Create a message for the update
        let message = Message::SyncEvent {
            event: "sync-update".to_string(),
            payload: payload.clone(),
        };
        // Remove the ? operator since this function returns ()
        if let Err(e) = self.send_sync_update(message).await {
            error!("Failed to send sync update: {}", e);
        }

        info!("Sync update processed and forwarded to all connections");
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

        // Documents match, proceed with state vector exchange
        info!("Document match confirmed, proceeding with state vector exchange");

        // Get the connection from the connection manager
        match self.get_connection_by_id(&connection_id).await {
            Ok(connection) => {
                if is_match {
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
        resource_id: String,
        connection_id: String,
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
                        let live_edit_message = LiveEditMessage::UpdateExhangeResponse {
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
    // In incoming_handler.rs
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
}
