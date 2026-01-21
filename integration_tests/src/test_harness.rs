//! TestHarness - Orchestrates multi-peer protocol testing

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use tracing::info;
use transport::MockBlobStore;

use butler::Butler;
use courier::coordinator::{CoordinatorMessage, CourierMode};

use crate::fixtures::{HANDSHAKE_DELAY, EXTENDED_SYNC_DELAY};

use crate::mock_transport::MockTransport;
use crate::test_peer::TestPeer;

/// Test harness for multi-peer protocol testing
///
/// Manages MockTransport, MockBlobStore, and TestPeers, provides helpers for common operations.
pub struct TestHarness {
    /// The mock transport routing messages
    pub transport: Arc<MockTransport>,
    /// Shared blob store for asset transfers (all peers share this)
    pub blob_store: Arc<MockBlobStore>,
    /// Named peers for easy access
    pub peers: HashMap<String, TestPeer>,
}

impl TestHarness {
    /// Create a new empty test harness
    pub fn new() -> Self {
        Self {
            transport: MockTransport::new(),
            blob_store: MockBlobStore::new(),
            peers: HashMap::new(),
        }
    }

    /// Add a peer with isolated storage
    pub async fn add_peer(&mut self, name: &str, mode: CourierMode) -> Result<&TestPeer> {
        let peer = TestPeer::new(name, mode, &self.transport, self.blob_store.clone()).await?;
        self.peers.insert(name.to_string(), peer);
        Ok(self.peers.get(name).unwrap())
    }

    /// Add a peer with provided Butler (for custom identity setup)
    pub async fn add_peer_with_butler(
        &mut self,
        name: &str,
        mode: CourierMode,
        butler: Arc<Butler>,
    ) -> Result<&TestPeer> {
        let peer = TestPeer::with_butler(name, mode, butler, &self.transport, self.blob_store.clone()).await?;
        self.peers.insert(name.to_string(), peer);
        Ok(self.peers.get(name).unwrap())
    }

    /// Get a peer by name
    pub fn peer(&self, name: &str) -> Option<&TestPeer> {
        self.peers.get(name)
    }

    /// Get a peer by name (mutable)
    pub fn peer_mut(&mut self, name: &str) -> Option<&mut TestPeer> {
        self.peers.get_mut(name)
    }

    /// Connect two peers (creates bidirectional connection in transport only)
    pub async fn connect(&self, from_name: &str, to_name: &str) -> Result<()> {
        let from_peer = self
            .peers
            .get(from_name)
            .ok_or_else(|| anyhow::anyhow!("Peer '{}' not found", from_name))?;
        let to_peer = self
            .peers
            .get(to_name)
            .ok_or_else(|| anyhow::anyhow!("Peer '{}' not found", to_name))?;

        // Create connection from -> to
        self.transport
            .connect(from_peer.node_id, to_peer.node_id)
            .await;

        info!(
            "TestHarness: connected '{}' ({}) ↔ '{}' ({})",
            from_name, from_peer.node_id, to_name, to_peer.node_id
        );

        Ok(())
    }

    /// Connect two peers and notify their Coordinators (spawns PeerActors)
    ///
    /// This simulates what happens when a real transport connection is established.
    pub async fn connect_and_notify(&self, from_name: &str, to_name: &str) -> Result<()> {
        let from_peer = self
            .peers
            .get(from_name)
            .ok_or_else(|| anyhow::anyhow!("Peer '{}' not found", from_name))?;
        let to_peer = self
            .peers
            .get(to_name)
            .ok_or_else(|| anyhow::anyhow!("Peer '{}' not found", to_name))?;

        // Create connection in transport (this creates bidirectional connections)
        let conn_to_node = self
            .transport
            .connect(from_peer.node_id, to_peer.node_id)
            .await;

        // Get the reverse connection
        let conn_to_owner = self
            .transport
            .get_connection(to_peer.node_id, from_peer.node_id)
            .await
            .expect("Reverse connection should exist");

        // Notify from_peer's Coordinator about connection to to_peer
        from_peer
            .coordinator
            .cast(CoordinatorMessage::Connected {
                node_id: to_peer.node_id,
                conn: conn_to_node,
            })?;

        // Notify to_peer's Coordinator about connection from from_peer
        to_peer
            .coordinator
            .cast(CoordinatorMessage::Connected {
                node_id: from_peer.node_id,
                conn: conn_to_owner,
            })?;

        info!(
            "TestHarness: connected and notified '{}' ({}) ↔ '{}' ({})",
            from_name, from_peer.node_id, to_name, to_peer.node_id
        );

        Ok(())
    }

    /// Run the mock transport event loop
    ///
    /// Call this in a background task to enable message routing.
    pub async fn run_transport(&self) {
        self.transport.run().await;
    }

    /// Spawn transport in background and return handle
    pub fn spawn_transport(&self) -> tokio::task::JoinHandle<()> {
        let transport = self.transport.clone();
        tokio::spawn(async move {
            transport.run().await;
        })
    }

    /// Shutdown all peers
    pub async fn shutdown(&self) {
        for peer in self.peers.values() {
            peer.shutdown().await;
        }
    }

    // =========================================================================
    // Deterministic Wait Helpers (TDD Infrastructure)
    // =========================================================================

    /// Wait for a condition with timeout - replaces fragile sleep()
    ///
    /// **Context**: TDD requires deterministic tests, not timing-based ones
    /// **We do**: Poll condition every 10ms until true or timeout
    /// **We return**: Ok(()) if condition met, Err if timeout
    pub async fn wait_until<F>(&self, mut condition: F, timeout: Duration) -> Result<()>
    where
        F: FnMut() -> bool,
    {
        let start = Instant::now();
        while !condition() {
            if start.elapsed() > timeout {
                anyhow::bail!("Timeout waiting for condition after {:?}", timeout);
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        Ok(())
    }

    /// Wait for owner to be authenticated on node
    ///
    /// **Context**: After InitiateHandshake, wait for handshake completion
    /// **We check**: node_butler.get_owner() returns Some
    pub async fn wait_for_owner_authenticated(&self, node_butler: &Arc<Butler>) -> Result<()> {
        let butler = node_butler.clone();
        self.wait_until(
            || butler.get_owner().ok().flatten().is_some(),
            HANDSHAKE_DELAY,
        ).await
    }

    /// Wait for space to exist on a butler
    ///
    /// **Context**: After PublishSpace, wait for space to arrive on node
    /// **We check**: butler.get_space(space_id) returns Some
    pub async fn wait_for_space(&self, butler: &Arc<Butler>, space_id: &str) -> Result<()> {
        let butler = butler.clone();
        let space_id = space_id.to_string();
        self.wait_until(
            || butler.get_space(&space_id).ok().flatten().is_some(),
            EXTENDED_SYNC_DELAY,
        ).await
    }

    /// Wait for page to exist on a butler
    ///
    /// **Context**: After PublishSpace (which syncs pages), wait for page arrival
    /// **We check**: butler.get_page(page_id) returns Some
    pub async fn wait_for_page(&self, butler: &Arc<Butler>, page_id: &str) -> Result<()> {
        let butler = butler.clone();
        let page_id = page_id.to_string();
        self.wait_until(
            || butler.get_page(&page_id).ok().flatten().is_some(),
            EXTENDED_SYNC_DELAY,
        ).await
    }

    /// Wait for sovereign node to have a permit (post-handshake)
    ///
    /// **Context**: After viewer handshake, their sovereign node record is updated
    /// **We check**: butler.get_sovereign_node(node_id).permit is Some
    pub async fn wait_for_sovereign_permit(
        &self,
        butler: &Arc<Butler>,
        node_id: &str,
    ) -> Result<()> {
        let butler = butler.clone();
        let node_id = node_id.to_string();
        self.wait_until(
            || {
                butler
                    .get_sovereign_node(&node_id)
                    .ok()
                    .flatten()
                    .and_then(|sn| sn.permit)
                    .is_some()
            },
            HANDSHAKE_DELAY,
        ).await
    }
}

impl Default for TestHarness {
    fn default() -> Self {
        Self::new()
    }
}
