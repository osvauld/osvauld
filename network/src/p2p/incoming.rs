use log::info;
use tokio::sync::mpsc;

/// Events that can be received and processed by the P2P service
#[derive(Debug)]
pub enum IncomingEvent {
    /// Request a folder connection token
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

    /// Send a folder token request
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
