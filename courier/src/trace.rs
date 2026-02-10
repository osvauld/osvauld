//! Protocol message tracing for integration tests
//!
//! Provides a lightweight hook for capturing protocol messages
//! as they flow through PeerActor. Production code passes `None`
//! for the trace channel; test code passes `Some(tx)`.

use std::time::Instant;
use transport::NodeId;

/// Direction of a traced protocol message
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraceDirection {
    Sent,
    Received,
}

/// A single traced protocol message
#[derive(Debug, Clone)]
pub struct MessageTrace {
    /// Whether this message was sent or received
    pub direction: TraceDirection,
    /// The protocol message type name (e.g. "Hello", "Welcome")
    pub msg_name: &'static str,
    /// Our node ID
    pub node_id: NodeId,
    /// The peer's node ID
    pub peer_node_id: NodeId,
    /// When this trace was recorded
    pub timestamp: Instant,
}
