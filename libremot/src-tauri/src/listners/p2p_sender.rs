use super::EventManager;
use log::info;

/// This file contains all methods that send messages to the P2P network
/// These methods are primarily called from tauri_events.rs when handling frontend events
impl EventManager {
    /// Send document check to initiate live editing
    pub(super) fn send_live_edit_document_check(
        &self,
        connection_id: String,
        resource_id: String,
    ) -> Result<(), String> {
        info!(
            "Sending document check for resource: {} to connection: {}",
            resource_id, connection_id
        );
        self.p2p_sender
            .send_live_edit_document_check(connection_id, resource_id)
    }

    /// Send document check response
    pub(super) fn send_live_edit_document_check_response(
        &self,
        connection_id: String,
        resource_id: String,
        is_match: bool,
        state_vectors: String,
    ) -> Result<(), String> {
        info!(
            "Sending document check response for resource: {}, is_match: {}",
            resource_id, is_match
        );
        self.p2p_sender.send_live_edit_document_check_response(
            connection_id,
            resource_id,
            is_match,
            state_vectors,
        )
    }

    /// Send update exchange for live editing
    pub(super) fn send_live_edit_update_exchange(
        &self,
        connection_id: String,
        resource_id: String,
        peer_updates: String,
    ) -> Result<(), String> {
        info!(
            "Sending update exchange for resource: {} to connection: {}",
            resource_id, connection_id
        );
        self.p2p_sender
            .send_live_edit_update_exchange(connection_id, resource_id, peer_updates)
    }

    /// Send update exchange response
    pub(super) fn send_live_edit_update_exchange_response(
        &self,
        connection_id: String,
        resource_id: String,
        peer_updates: String,
    ) -> Result<(), String> {
        info!(
            "Sending update exchange response for resource: {} to connection: {}",
            resource_id, connection_id
        );
        self.p2p_sender.send_live_edit_update_exchange_response(
            connection_id,
            resource_id,
            peer_updates,
        )
    }
}
