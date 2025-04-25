use tokio::sync::mpsc;

/// Events that can be received and processed by the P2P service
#[derive(Debug)]
pub enum IncomingEvent {
    /// Sent when a sync update occurs
    SyncUpdate { payload: String },
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

    /// Sends a sync update event
    pub fn send_sync_update(&self, payload: String) -> Result<(), String> {
        self.send(IncomingEvent::SyncUpdate { payload })
    }
}
