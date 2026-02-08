//! IrohConnection - Production transport using iroh for NAT traversal
//!
//! Wraps the existing ConnectionHandle to implement the Connection trait,
//! providing a zero-cost abstraction for production use.

use anyhow::Result;
use bytes::Bytes;
use iroh::endpoint::{RecvStream, SendStream};

use crate::pool::ConnectionHandle;
use crate::traits::{BiStream, Connection};
use crate::NodeId;

/// Bidirectional stream backed by iroh QUIC streams
pub struct IrohBiStream {
    send: SendStream,
    recv: RecvStream,
}

impl IrohBiStream {
    /// Create from iroh stream tuple
    pub fn new(send: SendStream, recv: RecvStream) -> Self {
        Self { send, recv }
    }
}

impl BiStream for IrohBiStream {
    type SendStream = SendStream;
    type RecvStream = RecvStream;

    fn split(self) -> (Self::SendStream, Self::RecvStream) {
        (self.send, self.recv)
    }
}

/// Connection implementation backed by iroh QUIC
///
/// This is the production connection type, providing:
/// - NAT traversal via iroh's relay network
/// - Reliable streams for protocol messages
/// - Unreliable datagrams for ephemeral data
///
/// # Example
///
/// ```ignore
/// // IrohConnection wraps ConnectionHandle transparently
/// let conn: IrohConnection = handle.into();
///
/// // Use via Connection trait
/// conn.send_bytes(&msg.to_bytes()?).await?;
/// ```
#[derive(Clone, Debug)]
pub struct IrohConnection {
    inner: ConnectionHandle,
}

impl IrohConnection {
    /// Create from an existing ConnectionHandle
    pub fn new(handle: ConnectionHandle) -> Self {
        Self { inner: handle }
    }

    /// Get the underlying ConnectionHandle
    ///
    /// Useful for legacy code that needs direct access
    pub fn inner(&self) -> &ConnectionHandle {
        &self.inner
    }

    /// Consume and return the underlying ConnectionHandle
    pub fn into_inner(self) -> ConnectionHandle {
        self.inner
    }
}

impl From<ConnectionHandle> for IrohConnection {
    fn from(handle: ConnectionHandle) -> Self {
        Self::new(handle)
    }
}

impl From<IrohConnection> for ConnectionHandle {
    fn from(conn: IrohConnection) -> Self {
        conn.inner
    }
}

impl Connection for IrohConnection {
    type BiStream = IrohBiStream;

    async fn send_bytes(&self, data: &[u8]) -> Result<()> {
        self.inner.send_bytes(data).await
    }

    fn send_datagram(&self, data: &[u8]) -> Result<()> {
        self.inner.send_datagram(data)
    }

    async fn read_datagram(&self) -> Result<Bytes> {
        let bytes = self.inner.read_datagram().await?;
        Ok(Bytes::from(bytes))
    }

    async fn accept_bi(&self) -> Result<Self::BiStream> {
        let (send, recv) = self.inner.accept_bi().await?;
        Ok(IrohBiStream::new(send, recv))
    }

    async fn open_bi(&self) -> Result<Self::BiStream> {
        let (send, recv) = self.inner.open_bi().await?;
        Ok(IrohBiStream::new(send, recv))
    }

    fn node_id(&self) -> NodeId {
        self.inner.node_id()
    }

    fn close(&self) {
        self.inner.close()
    }

    fn max_datagram_size(&self) -> Option<usize> {
        self.inner.max_datagram_size()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_iroh_connection_is_clone() {
        fn assert_clone<T: Clone>() {}
        assert_clone::<IrohConnection>();
    }

    #[test]
    fn test_iroh_connection_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<IrohConnection>();
    }
}
