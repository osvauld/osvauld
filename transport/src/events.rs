//! Transport events emitted to the protocol layer
//!
//! Transport emits lifecycle events only. Message reading is done by the
//! consumer (PeerSession) directly from the ConnectionHandle.
//!
//! This architecture enables per-peer parallelism - no central bottleneck.

use crate::pool::ConnectionHandle;
use iroh::EndpointId as NodeId;

/// Events emitted by Transport layer
///
/// Transport only emits connection lifecycle events.
/// Message reading is done by the consumer (PeerSession) which spawns
/// its own read loops from the ConnectionHandle.
///
/// **Why lifecycle only?**
/// - No central bottleneck - N peers = N parallel handlers
/// - Clear ownership - PeerSession owns its connection entirely
/// - Easy to extend - add audio = add reader loop in PeerSession
#[derive(Debug)]
pub enum TransportEvent {
    /// New peer connected
    ///
    /// Consumer gets ConnectionHandle to read from directly.
    /// Consumer (SessionManager) spawns PeerSession with this handle.
    Connected {
        /// Peer's iroh NodeId
        node_id: NodeId,
        /// Handle for reading/writing to this peer
        conn: ConnectionHandle,
    },

    /// Peer disconnected
    ///
    /// Consumer should clean up any PeerSession for this peer.
    Disconnected { node_id: NodeId },
}
