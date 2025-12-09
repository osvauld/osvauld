//! Transport events emitted to the protocol layer
//!
//! Transport emits these events via a channel. The protocol layer (courier2)
//! receives and processes them.
//!
//! Transport is a **dumb byte pipe** - it has no knowledge of message types
//! or serialization. It only handles raw bytes.

use crate::pool::ConnectionHandle;
use iroh::NodeId;

/// Events emitted by Transport layer
///
/// These are sent through a tokio mpsc channel to the protocol layer.
/// Transport only deals with raw bytes - deserialization happens upstream.
#[derive(Debug)]
pub enum TransportEvent {
    /// New peer connected
    ///
    /// Includes ConnectionHandle so protocol layer can send responses.
    Connected {
        /// Peer's iroh NodeId
        node_id: NodeId,
        /// Handle for sending bytes to this peer
        conn: ConnectionHandle,
    },

    /// Peer disconnected
    Disconnected { node_id: NodeId },

    /// Raw bytes received from peer
    ///
    /// Protocol layer is responsible for deserializing these bytes.
    Bytes {
        /// Source peer
        node_id: NodeId,
        /// Raw message bytes (length-prefix already stripped)
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
