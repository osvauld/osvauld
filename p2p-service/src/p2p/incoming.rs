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
    },
    LiveEditUpdateExchange {
        connection_id: String,
        resource_id: String,
        state_vector: Vec<u8>,
        buffer: Vec<u8>,
    },
    LiveEditUpdateExchangeResponse {
        connection_id: String,
        resource_id: String,
        state_vector: Vec<u8>,
        remote_updates: Vec<u8>,
        local_buffer: Vec<u8>,
    },
    DocumentChanged {
        connection_id: String,
        resource_id: String,
    },
    CurrentBufferExchange {
        connection_id: String,
        resource_id: String,
        buffer: Vec<u8>,
    },
    SyncUpdateBroadcast {
        connection_ids: Vec<String>,
        resource_id: String,
        client_id: u32,
        updates: Vec<u8>,
    },

    AwarenessUpdateBroadcast {
        connection_ids: Vec<String>,
        resource_id: String,
        client_id: u32,
        awareness_data: Vec<u8>,
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
    ) -> Result<(), String> {
        self.send(IncomingEvent::LiveEditDocumentCheckResponse {
            connection_id,
            resource_id,
            is_match,
        })
    }

    pub fn send_live_edit_update_exchange(
        &self,
        connection_id: String,
        resource_id: String,
        state_vector: Vec<u8>,
        buffer: Vec<u8>,
    ) -> Result<(), String> {
        self.send(IncomingEvent::LiveEditUpdateExchange {
            connection_id,
            resource_id,
            state_vector,
            buffer,
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
        state_vector: Vec<u8>,
        local_buffer: Vec<u8>,
        remote_updates: Vec<u8>,
    ) -> Result<(), String> {
        self.send(IncomingEvent::LiveEditUpdateExchangeResponse {
            connection_id,
            resource_id,
            state_vector,
            local_buffer,
            remote_updates,
        })
    }
    pub fn send_current_buffer_exchange(
        &self,
        connection_id: String,
        resource_id: String,
        buffer: Vec<u8>,
    ) -> Result<(), String> {
        self.send(IncomingEvent::CurrentBufferExchange {
            connection_id,
            resource_id,
            buffer,
        })
    }
    pub fn send_sync_update_to_connections(
        &self,
        resource_id: String,
        client_id: u32,
        updates: Vec<u8>,
        connection_ids: Vec<String>,
    ) -> Result<(), String> {
        self.send(IncomingEvent::SyncUpdateBroadcast {
            resource_id,
            client_id,
            updates,
            connection_ids,
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
}
