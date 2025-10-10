use log::info;
use tokio::sync::mpsc;

/// Events that can be received and processed by the P2P service
#[derive(Debug)]
pub enum IncomingEvent {
    /// Sent when a sync update occurs
    LiveEditDocumentCheck {
        connection_id: String,
        resource_id: String,
    },
    LiveEditDocumentCheckResponse {
        connection_id: String,
        resource_id: String,
        is_match: bool,
        state_vectors: String,
    },
    LiveEditUpdateExchange {
        connection_id: String,
        resource_id: String,
        peer_updates: String,
    },
    LiveEditUpdateExchangeResponse {
        connection_id: String,
        resource_id: String,
        peer_updates: String,
    },
    DocumentChanged {
        connection_id: String,
        resource_id: String,
    },
    SyncUpdateBroadcast {
        connection_ids: Vec<String>,
        resource_id: String,
        client_id: u32,
        updates: Vec<u8>,
        doc_type: String,
    },

    AwarenessUpdateBroadcast {
        connection_ids: Vec<String>,
        resource_id: String,
        client_id: u32,
        awareness_data: Vec<u8>,
    },
    BroadCastStateVectorRequest {
        connection_ids: Vec<String>,
        resource_id: String,
    },
    StartLiveConnection {
        device_ids: Vec<String>,
    },
    RequestFolderToken {
        folder_id: String,
        device_id: String,
        domain: String,
    },
}

/// Sender for incoming events to be processed by the P2P service
#[derive(Clone)]
pub struct P2PSender {
    pub(crate) sender: mpsc::UnboundedSender<IncomingEvent>,
}

impl P2PSender {
    /// Creates a new P2PSender with a channel
    pub fn new() -> (Self, mpsc::UnboundedReceiver<IncomingEvent>) {
        let (sender, receiver) = mpsc::unbounded_channel();
        (Self { sender }, receiver)
    }

    /// Sends an event to be processed by the P2P service
    pub fn send(&self, event: IncomingEvent) -> Result<(), String> {
        self.sender
            .send(event)
            .map_err(|e| format!("Failed to send event: {}", e))
    }

    pub fn send_live_edit_requests(&self, device_ids: Vec<String>) -> Result<(), String> {
        self.send(IncomingEvent::StartLiveConnection { device_ids })
    }

    pub fn send_live_edit_document_check(
        &self,
        connection_id: String,
        resource_id: String,
    ) -> Result<(), String> {
        self.send(IncomingEvent::LiveEditDocumentCheck {
            connection_id,
            resource_id,
        })
    }

    pub fn send_live_edit_document_check_response(
        &self,
        connection_id: String,
        resource_id: String,
        is_match: bool,
        state_vectors: String,
    ) -> Result<(), String> {
        self.send(IncomingEvent::LiveEditDocumentCheckResponse {
            connection_id,
            resource_id,
            is_match,
            state_vectors,
        })
    }

    pub fn send_live_edit_update_exchange(
        &self,
        connection_id: String,
        resource_id: String,
        peer_updates: String,
    ) -> Result<(), String> {
        self.send(IncomingEvent::LiveEditUpdateExchange {
            connection_id,
            resource_id,
            peer_updates,
        })
    }
    pub fn send_document_changed(
        &self,
        connection_id: String,
        resource_id: String,
    ) -> Result<(), String> {
        self.send(IncomingEvent::DocumentChanged {
            connection_id,
            resource_id,
        })
    }

    pub fn send_live_edit_update_exchange_response(
        &self,
        connection_id: String,
        resource_id: String,
        peer_updates: String,
    ) -> Result<(), String> {
        self.send(IncomingEvent::LiveEditUpdateExchangeResponse {
            connection_id,
            resource_id,
            peer_updates,
        })
    }

    pub fn send_state_vector_request(
        &self,
        connection_ids: Vec<String>,
        resource_id: String,
    ) -> Result<(), String> {
        info!("sending state vector request");
        self.send(IncomingEvent::BroadCastStateVectorRequest {
            connection_ids,
            resource_id,
        })
    }
    pub fn send_sync_update_to_connections(
        &self,
        resource_id: String,
        client_id: u32,
        updates: Vec<u8>,
        connection_ids: Vec<String>,
        doc_type: String,
    ) -> Result<(), String> {
        self.send(IncomingEvent::SyncUpdateBroadcast {
            resource_id,
            client_id,
            updates,
            connection_ids,
            doc_type,
        })
    }

    pub fn send_awareness_update_to_connections(
        &self,
        resource_id: String,
        client_id: u32,
        awareness_data: Vec<u8>,
        connection_ids: Vec<String>,
    ) -> Result<(), String> {
        self.send(IncomingEvent::AwarenessUpdateBroadcast {
            resource_id,
            client_id,
            awareness_data,
            connection_ids,
        })
    }

    pub fn send_request_folder_token(
        &self,
        folder_id: String,
        device_id: String,
        domain: String,
    ) -> Result<(), String> {
        info!("Sending folder token request for folder: {}", folder_id);
        self.send(IncomingEvent::RequestFolderToken {
            folder_id,
            device_id,
            domain,
        })
    }
}
