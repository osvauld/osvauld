//! Transport events emitted to Courier
//!
//! Transport emits these events via a channel. Courier receives and processes them.
//! ConnectionHandle is included so Courier can send responses directly.

use crate::pool::ConnectionHandle;
use crate::protocol::Message;
use iroh::NodeId;

/// Events emitted by Transport layer
///
/// These are sent through a tokio mpsc channel to the Courier layer.
/// Each event includes enough context for Courier to respond.
#[derive(Debug)]
pub enum TransportEvent {
    /// New peer connected
    ///
    /// Includes ConnectionHandle so Courier can send messages back.
    Connected {
        /// Peer's iroh NodeId
        node_id: NodeId,
        /// Handle for sending messages to this peer
        conn: ConnectionHandle,
    },

    /// Peer disconnected
    Disconnected {
        node_id: NodeId,
    },

    /// Message received from peer
    ///
    /// Courier processes this and may respond via the peer registry.
    Message {
        /// Source peer
        node_id: NodeId,
        /// The message
        message: Message,
    },

    /// Live data received on persistent stream
    LiveData {
        node_id: NodeId,
        stream_id: String,
        data: Vec<u8>,
    },

    /// Error occurred
    Error {
        /// Peer if applicable
        node_id: Option<NodeId>,
        /// Error description
        error: String,
    },
}
