//! MockConnection - In-memory transport for unit tests
//!
//! Provides a Connection implementation backed by channels for fast,
//! deterministic testing without real networking.
//!
//! # Example
//!
//! ```ignore
//! let (conn_a, conn_b) = mock_connection_pair(node_a, node_b);
//!
//! // Messages sent from A arrive at B
//! conn_a.send_bytes(b"hello").await?;
//! let stream = conn_b.accept_bi().await?;
//! // read from stream...
//! ```

use std::collections::VecDeque;
use std::io::{self, Cursor};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};

use anyhow::{anyhow, Result};
use bytes::Bytes;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::sync::{mpsc, Mutex};

use crate::traits::{BiStream, Connection};
use crate::NodeId;

/// Default max datagram size for mock connections (matches QUIC typical MTU)
const MOCK_MAX_DATAGRAM_SIZE: usize = 1200;

/// In-memory receive stream backed by a buffer
pub struct MockRecvStream {
    data: Cursor<Vec<u8>>,
}

impl MockRecvStream {
    fn new(data: Vec<u8>) -> Self {
        Self {
            data: Cursor::new(data),
        }
    }
}

impl AsyncRead for MockRecvStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let pos = self.data.position() as usize;
        let inner = self.data.get_ref();
        let remaining = &inner[pos..];

        if remaining.is_empty() {
            return Poll::Ready(Ok(()));
        }

        let to_read = remaining.len().min(buf.remaining());
        buf.put_slice(&remaining[..to_read]);
        self.data.set_position((pos + to_read) as u64);

        Poll::Ready(Ok(()))
    }
}

/// In-memory send stream (discards data, just tracks finish)
pub struct MockSendStream {
    finished: bool,
}

impl MockSendStream {
    fn new() -> Self {
        Self { finished: false }
    }

    /// Mark stream as finished (like QUIC finish)
    pub fn finish(&mut self) -> Result<()> {
        self.finished = true;
        Ok(())
    }
}

impl AsyncWrite for MockSendStream {
    fn poll_write(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        // Mock just accepts all writes
        Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

/// Bidirectional stream for mock connections
pub struct MockBiStream {
    send: MockSendStream,
    recv: MockRecvStream,
}

impl MockBiStream {
    fn new(data: Vec<u8>) -> Self {
        Self {
            send: MockSendStream::new(),
            recv: MockRecvStream::new(data),
        }
    }
}

impl BiStream for MockBiStream {
    type SendStream = MockSendStream;
    type RecvStream = MockRecvStream;

    fn split(self) -> (Self::SendStream, Self::RecvStream) {
        (self.send, self.recv)
    }
}

/// Callback type for datagram capture
pub type DatagramCallback = Arc<dyn Fn(&[u8], bool) + Send + Sync>;

/// Internal state shared between mock connection halves
struct MockConnectionInner {
    /// Channel to send bytes to peer (peer receives on their rx)
    stream_tx: mpsc::UnboundedSender<Vec<u8>>,
    /// Channel to receive bytes from peer
    stream_rx: Mutex<mpsc::UnboundedReceiver<Vec<u8>>>,
    /// Datagram channel to peer
    datagram_tx: mpsc::UnboundedSender<Vec<u8>>,
    /// Datagram channel from peer
    datagram_rx: Mutex<mpsc::UnboundedReceiver<Vec<u8>>>,
    /// Peer's node ID
    peer_id: NodeId,
    /// Connection closed flag
    closed: AtomicBool,
    /// Pending streams waiting to be accepted
    pending_streams: Mutex<VecDeque<Vec<u8>>>,
    /// Optional callback for datagram capture (for testing)
    /// Callback receives (data, is_send) where is_send=true for sent, false for received
    datagram_callback: Option<DatagramCallback>,
    /// Name for this connection endpoint (for debugging)
    name: String,
}

/// In-memory connection for unit tests
///
/// Messages delivered immediately, no network simulation.
/// Use `mock_connection_pair()` to create connected pairs.
#[derive(Clone)]
pub struct MockConnection {
    inner: Arc<MockConnectionInner>,
}

impl std::fmt::Debug for MockConnection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MockConnection")
            .field("peer_id", &self.inner.peer_id)
            .field("closed", &self.inner.closed.load(Ordering::SeqCst))
            .finish()
    }
}

impl MockConnection {
    /// Create a new mock connection with given channels
    fn new(
        stream_tx: mpsc::UnboundedSender<Vec<u8>>,
        stream_rx: mpsc::UnboundedReceiver<Vec<u8>>,
        datagram_tx: mpsc::UnboundedSender<Vec<u8>>,
        datagram_rx: mpsc::UnboundedReceiver<Vec<u8>>,
        peer_id: NodeId,
        name: String,
        datagram_callback: Option<DatagramCallback>,
    ) -> Self {
        Self {
            inner: Arc::new(MockConnectionInner {
                stream_tx,
                stream_rx: Mutex::new(stream_rx),
                datagram_tx,
                datagram_rx: Mutex::new(datagram_rx),
                peer_id,
                closed: AtomicBool::new(false),
                pending_streams: Mutex::new(VecDeque::new()),
                datagram_callback,
                name,
            }),
        }
    }

    /// Get the connection name
    pub fn name(&self) -> &str {
        &self.inner.name
    }

    /// Check if connection is closed
    pub fn is_closed(&self) -> bool {
        self.inner.closed.load(Ordering::SeqCst)
    }

    /// Inject a stream for accept_bi to return (for testing)
    pub async fn inject_stream(&self, data: Vec<u8>) {
        let mut pending = self.inner.pending_streams.lock().await;
        pending.push_back(data);
    }
}

impl Connection for MockConnection {
    type BiStream = MockBiStream;

    async fn send_bytes(&self, data: &[u8]) -> Result<()> {
        if self.inner.closed.load(Ordering::SeqCst) {
            return Err(anyhow!("connection closed"));
        }
        // Add length prefix (same as real transport's ConnectionHandle::send_bytes)
        let len = data.len() as u32;
        let mut prefixed = Vec::with_capacity(4 + data.len());
        prefixed.extend_from_slice(&len.to_be_bytes());
        prefixed.extend_from_slice(data);
        self.inner
            .stream_tx
            .send(prefixed)
            .map_err(|_| anyhow!("channel closed"))?;
        Ok(())
    }

    fn send_datagram(&self, data: &[u8]) -> Result<()> {
        if self.inner.closed.load(Ordering::SeqCst) {
            return Err(anyhow!("connection closed"));
        }
        // Capture sent datagram if callback is set
        if let Some(ref cb) = self.inner.datagram_callback {
            cb(data, true); // is_send = true
        }
        self.inner
            .datagram_tx
            .send(data.to_vec())
            .map_err(|_| anyhow!("channel closed"))?;
        Ok(())
    }

    async fn read_datagram(&self) -> Result<Bytes> {
        let mut rx = self.inner.datagram_rx.lock().await;
        let data = rx
            .recv()
            .await
            .map(Bytes::from)
            .ok_or_else(|| anyhow!("channel closed"))?;
        // Capture received datagram if callback is set
        if let Some(ref cb) = self.inner.datagram_callback {
            cb(&data, false); // is_send = false
        }
        Ok(data)
    }

    async fn accept_bi(&self) -> Result<Self::BiStream> {
        // First check for injected streams
        {
            let mut pending = self.inner.pending_streams.lock().await;
            if let Some(data) = pending.pop_front() {
                return Ok(MockBiStream::new(data));
            }
        }

        // Otherwise wait for stream data from peer
        let mut rx = self.inner.stream_rx.lock().await;
        let data = rx.recv().await.ok_or_else(|| anyhow!("channel closed"))?;
        Ok(MockBiStream::new(data))
    }

    async fn open_bi(&self) -> Result<Self::BiStream> {
        if self.inner.closed.load(Ordering::SeqCst) {
            return Err(anyhow!("connection closed"));
        }
        // For mock, open_bi just returns an empty stream
        // The actual data is sent via send_bytes
        Ok(MockBiStream::new(Vec::new()))
    }

    fn node_id(&self) -> NodeId {
        self.inner.peer_id
    }

    fn close(&self) {
        self.inner.closed.store(true, Ordering::SeqCst);
    }

    fn max_datagram_size(&self) -> Option<usize> {
        Some(MOCK_MAX_DATAGRAM_SIZE)
    }
}

/// Create a pair of connected MockConnections
///
/// Messages sent from A arrive at B, and vice versa.
/// Both connections share channels so communication is instant.
///
/// # Example
///
/// ```ignore
/// let node_a = "node_a".parse().unwrap();
/// let node_b = "node_b".parse().unwrap();
/// let (conn_a, conn_b) = mock_connection_pair(node_a, node_b);
///
/// // A sends to B
/// conn_a.send_bytes(b"hello").await?;
/// let stream = conn_b.accept_bi().await?;
/// ```
pub fn mock_connection_pair(id_a: NodeId, id_b: NodeId) -> (MockConnection, MockConnection) {
    mock_connection_pair_named(id_a, id_b, "a".to_string(), "b".to_string(), None, None)
}

/// Create a pair of connected MockConnections with names and optional datagram capture
///
/// The callback receives (data, is_send) where is_send=true for sent, false for received.
pub fn mock_connection_pair_named(
    id_a: NodeId,
    id_b: NodeId,
    name_a: String,
    name_b: String,
    callback_a: Option<DatagramCallback>,
    callback_b: Option<DatagramCallback>,
) -> (MockConnection, MockConnection) {
    // Stream channels
    let (stream_tx_a, stream_rx_a) = mpsc::unbounded_channel();
    let (stream_tx_b, stream_rx_b) = mpsc::unbounded_channel();

    // Datagram channels
    let (datagram_tx_a, datagram_rx_a) = mpsc::unbounded_channel();
    let (datagram_tx_b, datagram_rx_b) = mpsc::unbounded_channel();

    // A sends to B's rx, B sends to A's rx
    let conn_a = MockConnection::new(
        stream_tx_b,   // A sends to B
        stream_rx_a,   // A receives from B
        datagram_tx_b, // A's datagrams go to B
        datagram_rx_a, // A receives B's datagrams
        id_b,          // A sees B as peer
        name_a,
        callback_a,
    );

    let conn_b = MockConnection::new(
        stream_tx_a,   // B sends to A
        stream_rx_b,   // B receives from A
        datagram_tx_a, // B's datagrams go to A
        datagram_rx_b, // B receives A's datagrams
        id_a,          // B sees A as peer
        name_b,
        callback_b,
    );

    (conn_a, conn_b)
}

/// Create a mock NodeId from a string (for testing)
///
/// In production, NodeIds are cryptographic identifiers.
/// For tests, we use deterministic IDs derived from strings.
/// Create a NodeId from a name (for testing)
///
/// Creates a deterministic NodeId by hashing the name.
pub fn mock_node_id(name: &str) -> NodeId {
    // Create a deterministic 32-byte key from the name
    let hash = blake3::hash(name.as_bytes());
    let key_bytes: [u8; 32] = *hash.as_bytes();
    node_id_from_secret(&key_bytes)
}

/// Create a NodeId from a 32-byte secret key
///
/// **Context**: The NodeId is the public key derived from the secret.
/// This is used to create MockPeer node_ids that match Butler's device key.
pub fn node_id_from_secret(secret_bytes: &[u8; 32]) -> NodeId {
    let secret = iroh::SecretKey::from(*secret_bytes);
    secret.public().into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_connection_send_receive() {
        let id_a = mock_node_id("alice");
        let id_b = mock_node_id("bob");
        let (conn_a, conn_b) = mock_connection_pair(id_a, id_b);

        // A sends to B
        conn_a.send_bytes(b"hello from alice").await.unwrap();

        // B receives from A
        let stream = conn_b.accept_bi().await.unwrap();
        let (_, mut recv) = stream.split();

        let mut buf = vec![0u8; 100];
        use tokio::io::AsyncReadExt;
        let n = recv.read(&mut buf).await.unwrap();

        assert_eq!(&buf[..n], b"hello from alice");
    }

    #[tokio::test]
    async fn test_mock_connection_datagram() {
        let id_a = mock_node_id("alice");
        let id_b = mock_node_id("bob");
        let (conn_a, conn_b) = mock_connection_pair(id_a, id_b);

        // A sends datagram to B
        conn_a.send_datagram(b"quick message").unwrap();

        // B receives datagram
        let data = conn_b.read_datagram().await.unwrap();
        assert_eq!(&data[..], b"quick message");
    }

    #[tokio::test]
    async fn test_mock_connection_close() {
        let id_a = mock_node_id("alice");
        let id_b = mock_node_id("bob");
        let (conn_a, _conn_b) = mock_connection_pair(id_a, id_b);

        assert!(!conn_a.is_closed());

        conn_a.close();

        assert!(conn_a.is_closed());
        assert!(conn_a.send_bytes(b"should fail").await.is_err());
    }

    #[tokio::test]
    async fn test_mock_connection_node_id() {
        let id_a = mock_node_id("alice");
        let id_b = mock_node_id("bob");
        let (conn_a, conn_b) = mock_connection_pair(id_a, id_b);

        // A's peer is B
        assert_eq!(conn_a.node_id(), id_b);
        // B's peer is A
        assert_eq!(conn_b.node_id(), id_a);
    }

    #[tokio::test]
    async fn test_mock_connection_bidirectional() {
        let id_a = mock_node_id("alice");
        let id_b = mock_node_id("bob");
        let (conn_a, conn_b) = mock_connection_pair(id_a, id_b);

        // Bidirectional communication
        conn_a.send_bytes(b"a to b").await.unwrap();
        conn_b.send_bytes(b"b to a").await.unwrap();

        // Both receive
        let stream_b = conn_b.accept_bi().await.unwrap();
        let stream_a = conn_a.accept_bi().await.unwrap();

        let (_, mut recv_b) = stream_b.split();
        let (_, mut recv_a) = stream_a.split();

        use tokio::io::AsyncReadExt;
        let mut buf_b = vec![0u8; 100];
        let mut buf_a = vec![0u8; 100];

        let n_b = recv_b.read(&mut buf_b).await.unwrap();
        let n_a = recv_a.read(&mut buf_a).await.unwrap();

        assert_eq!(&buf_b[..n_b], b"a to b");
        assert_eq!(&buf_a[..n_a], b"b to a");
    }
}
