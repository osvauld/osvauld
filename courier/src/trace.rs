//! Protocol message tracing for integration tests
//!
//! Provides a lightweight hook for capturing protocol messages
//! as they flow through PeerActor. Production code passes `None`
//! for the trace channel; test code passes `Some(tx)`.

use serde::Serialize;
use std::time::Instant;
use transport::NodeId;

/// Direction of a traced protocol message
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
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
    /// ISO 8601 timestamp for serialization
    pub ts: String,
    /// Page ID context (for sync messages)
    pub page_id: Option<String>,
    /// Layer name context (for sync messages)
    pub layer_name: Option<String>,
}

impl MessageTrace {
    /// Create a new MessageTrace with both Instant and ISO 8601 timestamps
    pub fn new(
        direction: TraceDirection,
        msg_name: &'static str,
        node_id: NodeId,
        peer_node_id: NodeId,
    ) -> Self {
        Self {
            direction,
            msg_name,
            node_id,
            peer_node_id,
            timestamp: Instant::now(),
            ts: chrono::Utc::now()
                .format("%Y-%m-%dT%H:%M:%S%.3fZ")
                .to_string(),
            page_id: None,
            layer_name: None,
        }
    }
}
