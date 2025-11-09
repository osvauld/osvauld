use log::{error, info};
use tokio::sync::mpsc;

/// Enum representing various P2P events that can be emitted
#[derive(Debug, Clone)]
pub enum P2PEvent {
    /// Emitted when a connection is established (after handshake complete)
    Connected {
        peer_id: String,
    },
    /// Emitted when a sovereign node connection is established
    NodeConnected {
        peer_id: String,
    },
    /// Emitted when a user connection is established
    UserConnected {
        peer_id: String,
    },
    /// Emitted when a connection is terminated
    Disconnected,
    /// Emitted when an error occurs
    Error {
        message: String,
        source: String,
    },
    /// Emitted when a folder connection token is received
    FolderTokenReceived {
        folder_id: String,
        connection_string: String,
    },
}

/// Handles event emission for the P2P service
#[derive(Clone)]
pub struct P2PEventEmitter {
    sender: mpsc::UnboundedSender<P2PEvent>,
}

impl P2PEventEmitter {
    /// Creates a new P2PEventEmitter with a channel
    pub fn new() -> (Self, mpsc::UnboundedReceiver<P2PEvent>) {
        let (sender, receiver) = mpsc::unbounded_channel();
        (Self { sender }, receiver)
    }

    /// Emits an event
    pub fn emit(&self, event: P2PEvent) {
        let event_type = format!("{:?}", event);
        if let Err(e) = self.sender.send(event) {
            error!("Failed to emit {}: {}", event_type, e);
        } else {
            info!("Emitted {} event", event_type);
        }
    }
}
