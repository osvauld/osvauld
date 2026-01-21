//! MockTransport - Routes messages between TestPeers without real network
//!
//! All bytes flow through this central router, which delivers them to
//! the appropriate peer's Coordinator.
//!
//! Note: MockBlobStore for asset transfers is provided by the transport crate
//! and shared via TestHarness. This allows the same blob store implementation
//! to be used by both real transport and tests.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{mpsc, RwLock};
use tracing::debug;
use transport::{ConnectionHandle, MockSender, MockSendEvent, NodeId};

use courier::coordinator::CoordinatorMessage;
use ractor::ActorRef;

/// Mock transport that routes messages between TestPeers
///
/// All bytes go through this central router, which delivers them to
/// the appropriate peer's Coordinator.
pub struct MockTransport {
    /// Channel for receiving transport events from mock connections
    receiver: RwLock<mpsc::UnboundedReceiver<MockSendEvent>>,
    /// Channel sender for creating MockConnectionHandles
    sender: mpsc::UnboundedSender<MockSendEvent>,
    /// Registry of peer Coordinators (public for test access)
    pub peers: RwLock<HashMap<NodeId, ActorRef<CoordinatorMessage>>>,
    /// Track mock connections for getting handles
    connections: RwLock<HashMap<(NodeId, NodeId), MockSender>>,
}

impl MockTransport {
    pub fn new() -> Arc<Self> {
        let (sender, receiver) = mpsc::unbounded_channel();
        Arc::new(Self {
            receiver: RwLock::new(receiver),
            sender,
            peers: RwLock::new(HashMap::new()),
            connections: RwLock::new(HashMap::new()),
        })
    }

    /// Register a peer's Coordinator
    pub async fn register_peer(
        &self,
        node_id: NodeId,
        coordinator: ActorRef<CoordinatorMessage>,
    ) {
        let mut peers = self.peers.write().await;
        peers.insert(node_id, coordinator);
        debug!("MockTransport: registered peer {}", node_id);
    }

    /// Create a mock connection between two peers (bidirectional)
    ///
    /// Returns a ConnectionHandle that can be used to send messages
    /// from `from` to `to`.
    pub async fn connect(&self, from: NodeId, to: NodeId) -> ConnectionHandle {
        // Create mock senders for both directions
        let sender_to_peer = MockSender::new(self.sender.clone(), from, to);
        let sender_to_from = MockSender::new(self.sender.clone(), to, from);

        // Store for later retrieval
        let mut conns = self.connections.write().await;
        conns.insert((from, to), sender_to_peer.clone());
        conns.insert((to, from), sender_to_from);

        // Return connection handle from `from` to `to`
        ConnectionHandle::from_mock_sender(sender_to_peer)
    }

    /// Get an existing connection handle
    pub async fn get_connection(&self, from: NodeId, to: NodeId) -> Option<ConnectionHandle> {
        let conns = self.connections.read().await;
        conns
            .get(&(from, to))
            .map(|sender| ConnectionHandle::from_mock_sender(sender.clone()))
    }

    /// Get the sender for creating new MockSenders
    pub fn sender(&self) -> mpsc::UnboundedSender<MockSendEvent> {
        self.sender.clone()
    }

    /// Run the transport event loop
    ///
    /// Delivers bytes from senders to receivers.
    pub async fn run(&self) {
        let mut receiver = self.receiver.write().await;

        while let Some(event) = receiver.recv().await {
            debug!(
                "MockTransport: {} → {} ({} bytes)",
                event.from,
                event.to,
                event.data.len()
            );

            let peers = self.peers.read().await;
            if let Some(coordinator) = peers.get(&event.to) {
                // Deliver bytes to the target's Coordinator
                let _ = coordinator.cast(CoordinatorMessage::TransportBytes {
                    node_id: event.from,
                    data: event.data,
                });
            } else {
                debug!("MockTransport: no peer registered for {}", event.to);
            }
        }
    }
}
