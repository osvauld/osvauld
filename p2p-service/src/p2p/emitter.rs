use log::{error, info};
use osvauld_core::models::{resource::Resource, vector_clock::ResourceVectorClock};
use tokio::sync::mpsc;

/// Enum representing various P2P events that can be emitted
#[derive(Debug, Clone)]
pub enum P2PEvent {
    /// Emitted when a connection is established
    Connected,
    /// Type of connection established (user or device)
    /// Emitted when a connection is terminated
    Disconnected,
    /// Emitted when a handshake is completed successfully
    HandshakeCompleted,
    /// Emitted when a handshake fails
    HandshakeFailed {
        /// Description of the error
        error: String,
    },
    /// Emitted when a sync operation completes
    SyncComplete,
    /// Emitted when a share operation completes
    ShareComplete,
    /// Emitted when a real-time editing event is received
    EditingEvent {
        /// Content of the editing event
        payload: String,
    },
    /// Emitted when an error occurs
    Error {
        /// Description of the error
        message: String,
        /// Source of the error
        source: String,
    },
    UpdateEvent {
        vector_clock: Vec<ResourceVectorClock>,
        remote_resource: Resource,
        device_id: String,
        user_id: String,
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
