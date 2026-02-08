//! Pluggable transport traits for P2P communication
//!
//! This module defines the core abstractions that enable:
//! - **Production flexibility**: Swap Iroh <-> Quinn based on deployment
//! - **Testability**: Mock transport for unit tests, Sim for DST
//! - **Protocol isolation**: PeerActor/sync code is transport-agnostic
//! - **Zero-cost abstraction**: Compile-time dispatch via generics
//!
//! # Architecture
//!
//! ```text
//! Protocol Layer (PeerActor<C>, Coordinator<C>)
//!              Generic over C: Connection
//!                      |
//!                      v
//!              Connection Trait
//!    send_bytes(), read_datagram(), accept_bi(), close()
//!                      |
//!          implemented by
//!    +--------+--------+--------+--------+
//!    v        v        v        v        v
//! IrohConn QuinnConn MockConn SimConn  ...
//! ```

use std::fmt::Debug;
use std::future::Future;
use std::pin::Pin;

use anyhow::Result;
use bytes::Bytes;
use tokio::io::{AsyncRead, AsyncWrite};

use crate::NodeId;

/// Bidirectional stream for reliable ordered communication
///
/// Returned by `Connection::accept_bi()` and `Connection::open_bi()`.
/// Provides send and receive halves for bidirectional streaming.
pub trait BiStream: Send + 'static {
    /// Send stream half
    type SendStream: AsyncWrite + Unpin + Send;
    /// Receive stream half
    type RecvStream: AsyncRead + Unpin + Send;

    /// Split into send and receive halves
    fn split(self) -> (Self::SendStream, Self::RecvStream);
}

/// Abstract connection for P2P communication
///
/// Implementations:
/// - `IrohConnection`: NAT traversal via iroh (production)
/// - `QuinnConnection`: Direct QUIC for static IPs (production, future)
/// - `MockConnection`: In-memory channels (unit tests)
/// - `SimConnection`: Deterministic simulation (DST, future)
///
/// # Usage
///
/// ```ignore
/// async fn send_hello<C: Connection>(conn: &C) -> Result<()> {
///     let hello = Message::Hello { ... };
///     conn.send_bytes(&hello.to_bytes()?).await
/// }
/// ```
pub trait Connection: Clone + Send + Sync + Debug + 'static {
    /// Bidirectional stream type for this connection
    type BiStream: BiStream;

    /// Send bytes on a reliable ordered stream (ephemeral: open, send, close)
    ///
    /// **Use for**: Protocol messages (Hello, SyncOffer, etc.)
    /// **Properties**: Reliable, ordered delivery
    /// **Note**: Length prefix is added automatically
    fn send_bytes(&self, data: &[u8]) -> impl Future<Output = Result<()>> + Send;

    /// Send unreliable datagram (fire-and-forget)
    ///
    /// **Use for**: Cursor sync, typing indicators, presence
    /// **Properties**: Unreliable, unordered, ~1200 byte limit
    /// **Note**: Sync, not async - matches QUIC datagram semantics
    fn send_datagram(&self, data: &[u8]) -> Result<()>;

    /// Receive next datagram (blocks until one arrives)
    ///
    /// **Use for**: Receiving cursor updates, typing indicators
    /// **Blocks**: Until a datagram arrives or connection closes
    fn read_datagram(&self) -> impl Future<Output = Result<Bytes>> + Send;

    /// Accept incoming bidirectional stream
    ///
    /// **Use for**: Reading ephemeral protocol messages
    /// **Consumer spawns**: Read loop that calls this repeatedly
    fn accept_bi(&self) -> impl Future<Output = Result<Self::BiStream>> + Send;

    /// Open outgoing bidirectional stream
    ///
    /// **Use for**: Sending reliable messages that need a response
    fn open_bi(&self) -> impl Future<Output = Result<Self::BiStream>> + Send;

    /// Get peer's node identifier
    fn node_id(&self) -> NodeId;

    /// Close the connection gracefully
    fn close(&self);

    /// Get max datagram size for this connection
    ///
    /// **Returns**: Maximum payload size, typically ~1200 bytes
    fn max_datagram_size(&self) -> Option<usize>;
}

/// Factory for creating and accepting connections
///
/// Implementations:
/// - `IrohTransport`: Production transport with NAT traversal
/// - `MockTransport`: Test transport with in-memory connections
pub trait Transport: Send + Sync + 'static {
    /// Connection type produced by this transport
    type Connection: Connection;

    /// Connect to a peer by NodeId
    fn connect(
        &self,
        node_id: NodeId,
    ) -> Pin<Box<dyn Future<Output = Result<Self::Connection>> + Send + '_>>;

    /// Get our local node ID
    fn local_node_id(&self) -> NodeId;

    /// Check if connected to a peer
    fn is_connected(
        &self,
        node_id: &NodeId,
    ) -> Pin<Box<dyn Future<Output = bool> + Send + '_>>;
}

/// Event emitted by transport layer
///
/// Transport-agnostic version of lifecycle events.
#[derive(Debug, Clone)]
pub enum ConnectionEvent<C: Connection> {
    /// New peer connection established
    Connected {
        node_id: NodeId,
        conn: C,
    },
    /// Peer disconnected
    Disconnected {
        node_id: NodeId,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    // Compile-time test that traits are object-safe where needed
    fn _assert_connection_is_clone<C: Connection>() {
        fn _needs_clone<T: Clone>() {}
        _needs_clone::<C>();
    }

    fn _assert_connection_is_send_sync<C: Connection>() {
        fn _needs_send_sync<T: Send + Sync>() {}
        _needs_send_sync::<C>();
    }
}
