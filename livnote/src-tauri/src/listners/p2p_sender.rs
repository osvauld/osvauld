use super::EventManager;
use log::{error, info};

/// This file contains all methods that send messages to the P2P network
/// These methods are primarily called from tauri_events.rs when handling frontend events
impl EventManager {
    /// Send sync updates to active P2P connections
    pub(super) fn send_sync_update(
        &self,
        resource_id: String,
        client_id: u32,
        updates: Vec<u8>,
        connections: Vec<String>,
        doc_type: String,
    ) -> Result<(), String> {
        self.p2p_sender.send_sync_update_to_connections(
            resource_id,
            client_id,
            updates,
            connections,
            doc_type,
        )
    }

    /// Send awareness updates to active P2P connections
    pub(super) fn send_awareness_update(
        &self,
        resource_id: String,
        client_id: u32,
        updates: Vec<u8>,
        connections: Vec<String>,
    ) -> Result<(), String> {
        self.p2p_sender.send_awareness_update_to_connections(
            resource_id,
            client_id,
            updates,
            connections,
        )
    }

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

    /// Send document changed notification
    pub(super) fn send_document_changed(
        &self,
        connection_id: String,
        resource_id: String,
    ) -> Result<(), String> {
        info!(
            "Sending document changed notification for resource: {} to connection: {}",
            resource_id, connection_id
        );
        self.p2p_sender
            .send_document_changed(connection_id, resource_id)
    }

    /// Send state vector request to inactive connections
    pub(super) fn send_state_vector_request(
        &self,
        connections: Vec<String>,
        resource_id: String,
    ) -> Result<(), String> {
        info!(
            "Sending state vector request for resource: {} to {} connections",
            resource_id,
            connections.len()
        );
        self.p2p_sender
            .send_state_vector_request(connections, resource_id)
    }

    /// Send live edit requests to shared devices
    pub(super) fn send_live_edit_requests(
        &self,
        shared_devices: Vec<String>,
    ) -> Result<(), String> {
        info!(
            "Sending live edit requests to {} shared devices",
            shared_devices.len()
        );
        self.p2p_sender.send_live_edit_requests(shared_devices)
    }
}
