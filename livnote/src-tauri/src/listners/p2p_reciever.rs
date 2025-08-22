use super::EventManager;
use log::info;
use network::p2p::P2PEvent;

/// Main P2P event receiver implementation
impl EventManager {
    /// Main loop for listening to P2P events
    pub(super) async fn listen_for_p2p_events(&mut self) {
        info!("Started listening for P2P events");

        while let Some(event) = self.p2p_receiver.recv().await {
            match event {
                // System events
                P2PEvent::Connected => self.handle_connected_event(),
                P2PEvent::Disconnected => self.handle_disconnected_event(),
                P2PEvent::HandshakeFailed { error } => self.handle_handshake_failed_event(error),
                P2PEvent::SyncComplete => self.handle_sync_complete_event(),
                P2PEvent::ShareComplete => self.handle_share_complete_event(),
                P2PEvent::Error { message, source } => self.handle_error_event(message, source),

                // Live edit negotiation events
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
                        .await
                }

                // Real-time update events
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
                    info!("awareness data received");
                    self.handle_awareness_event(resource_id, client_id, awareness_data)
                        .await
                }
                P2PEvent::UpdatesEvent {
                    resource_id,
                    updates,
                    client_id,
                } => {
                    self.handle_update_event(resource_id, updates, client_id)
                        .await
                }
                P2PEvent::ResourceAdded { resource_id } => {
                    self.handle_resource_added(resource_id).await
                }
            }
        }

        info!("Stopped listening for P2P events");
    }
}
