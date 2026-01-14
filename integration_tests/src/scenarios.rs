//! Pre-built test scenarios for TDD workflow
//!
//! These scenarios reduce boilerplate for common test setups.
//! All scenarios use production code paths (connection strings, add_sovereign_node, etc.)
//!
//! # Usage
//!
//! ```rust,ignore
//! // Instead of 12+ lines of setup:
//! let scenario = OwnerNodeScenario::new().await?;
//!
//! // For published content:
//! let scenario = PublishedPageScenario::new().await?;
//! ```

use std::sync::Arc;

use anyhow::Result;
use transport::NodeId;

use butler::{Butler};
use courier::coordinator::{CoordinatorMessage, CourierMode};
use ractor::ActorRef;

use crate::fixtures::{ACTOR_SPAWN_DELAY, TEST_SPACE_TEMPLATE, TEST_PAGE_TEMPLATE, TEST_PAGE_LAYERS};
use crate::helpers::setup_butler_with_identity;
use crate::TestHarness;

// =============================================================================
// OwnerNodeScenario - Two connected peers with completed handshake
// =============================================================================

/// Two connected peers with completed handshake
///
/// **Context**: Most integration tests need an authenticated owner-node pair
/// **We provide**: Fully handshaken owner + node using production flows
pub struct OwnerNodeScenario {
    pub harness: TestHarness,
    pub owner_butler: Arc<Butler>,
    pub node_butler: Arc<Butler>,
    /// Keep temp directories alive for test duration
    pub _temps: (tempfile::TempDir, tempfile::TempDir),
}

impl OwnerNodeScenario {
    /// Create owner + node with default names ("alice", "my-node")
    pub async fn new() -> Result<Self> {
        Self::with_names("alice", "my-node").await
    }

    /// Create owner + node with custom names
    ///
    /// **Context**: When you need specific usernames for verification
    pub async fn with_names(owner_name: &str, node_name: &str) -> Result<Self> {
        let (owner_butler, _, owner_temp) = setup_butler_with_identity(owner_name, "password").await?;
        let (node_butler, _, node_temp) = setup_butler_with_identity(node_name, "nodepass").await?;

        let mut harness = TestHarness::new();
        harness.add_peer_with_butler("owner", CourierMode::User, owner_butler.clone()).await?;
        harness.add_peer_with_butler("node", CourierMode::Node, node_butler.clone()).await?;

        let _transport = harness.spawn_transport();

        // Production flow: connection string → add_sovereign_node → handshake
        let conn_string = node_butler.generate_connection_string(None).await?;
        let sovereign = owner_butler.add_sovereign_node(&conn_string)
            .map_err(|e| anyhow::anyhow!(e))?;
        let permit = sovereign.permit.expect("Connection string should have permit");

        harness.connect_and_notify("owner", "node").await?;
        tokio::time::sleep(ACTOR_SPAWN_DELAY).await;

        let node_id = harness.peer("node").unwrap().node_id;
        harness.peer("owner").unwrap().coordinator.cast(
            CoordinatorMessage::InitiateHandshake { node_id, permit }
        )?;

        // Deterministic wait instead of fragile sleep
        harness.wait_for_owner_authenticated(&node_butler).await?;

        Ok(Self {
            harness,
            owner_butler,
            node_butler,
            _temps: (owner_temp, node_temp),
        })
    }

    /// Get the node's NodeId
    pub fn node_id(&self) -> NodeId {
        self.harness.peer("node").unwrap().node_id
    }

    /// Get the owner's NodeId
    pub fn owner_node_id(&self) -> NodeId {
        self.harness.peer("owner").unwrap().node_id
    }

    /// Get the owner's coordinator
    pub fn owner_coordinator(&self) -> &ActorRef<CoordinatorMessage> {
        &self.harness.peer("owner").unwrap().coordinator
    }

    /// Get the node's coordinator
    pub fn node_coordinator(&self) -> &ActorRef<CoordinatorMessage> {
        &self.harness.peer("node").unwrap().coordinator
    }

    /// Shutdown the scenario
    pub async fn shutdown(&self) {
        self.harness.shutdown().await;
    }
}

// =============================================================================
// PublishedPageScenario - Owner + Node + Published Space/Page
// =============================================================================

/// Owner + Node with published space and page
///
/// **Context**: Tests that need synced content on node
/// **We provide**: OwnerNodeScenario + space + page published to node
pub struct PublishedPageScenario {
    pub base: OwnerNodeScenario,
    pub space_id: String,
    pub page_id: String,
}

impl PublishedPageScenario {
    /// Create with default space/page
    pub async fn new() -> Result<Self> {
        Self::with_names("alice", "my-node", "Test Space", "Test Page").await
    }

    /// Create with custom names
    pub async fn with_names(
        owner_name: &str,
        node_name: &str,
        space_name: &str,
        page_name: &str,
    ) -> Result<Self> {
        let base = OwnerNodeScenario::with_names(owner_name, node_name).await?;

        let user_info = base.owner_butler.user_info().await?;
        let space = base.owner_butler
            .create_space(space_name.to_string(), user_info.did.clone(), TEST_SPACE_TEMPLATE)
            .await?;

        let layers: Vec<String> = TEST_PAGE_LAYERS.iter().map(|s| s.to_string()).collect();
        let page = base.owner_butler
            .create_page(&space.id, page_name, layers, TEST_PAGE_TEMPLATE)
            .await?;

        base.owner_coordinator().cast(CoordinatorMessage::PublishSpace {
            node_id: base.node_id(),
            space_id: space.id.clone(),
        })?;

        // Wait for page to arrive on node (deterministic)
        base.harness.wait_for_page(&base.node_butler, &page.id).await?;

        Ok(Self {
            base,
            space_id: space.id,
            page_id: page.id,
        })
    }

    /// Get the harness
    pub fn harness(&self) -> &TestHarness {
        &self.base.harness
    }

    /// Get owner butler
    pub fn owner_butler(&self) -> &Arc<Butler> {
        &self.base.owner_butler
    }

    /// Get node butler
    pub fn node_butler(&self) -> &Arc<Butler> {
        &self.base.node_butler
    }

    /// Get node's NodeId
    pub fn node_id(&self) -> NodeId {
        self.base.node_id()
    }

    /// Get owner's coordinator
    pub fn owner_coordinator(&self) -> &ActorRef<CoordinatorMessage> {
        self.base.owner_coordinator()
    }

    /// Shutdown the scenario
    pub async fn shutdown(&self) {
        self.base.shutdown().await;
    }
}

// =============================================================================
// MultiPartyScenario - Owner + Node + Viewer with published content
// =============================================================================

/// Owner + Node + Viewer with published space/page
///
/// **Context**: Tests that need viewer sync (live sync, permissions)
/// **We provide**: Full multi-party setup using production flows
pub struct MultiPartyScenario {
    pub harness: TestHarness,
    pub owner_butler: Arc<Butler>,
    pub node_butler: Arc<Butler>,
    pub viewer_butler: Arc<Butler>,
    pub space_id: String,
    pub page_id: String,
    /// Keep temp directories alive
    pub _temps: (tempfile::TempDir, tempfile::TempDir, tempfile::TempDir),
}

impl MultiPartyScenario {
    /// Create with default names
    pub async fn new() -> Result<Self> {
        Self::with_names("alice", "my-node", "bob-viewer").await
    }

    /// Create with custom names
    pub async fn with_names(
        owner_name: &str,
        node_name: &str,
        viewer_name: &str,
    ) -> Result<Self> {
        // Setup all three identities
        let (owner_butler, _, owner_temp) = setup_butler_with_identity(owner_name, "password").await?;
        let (node_butler, _, node_temp) = setup_butler_with_identity(node_name, "nodepass").await?;
        let (viewer_butler, _, viewer_temp) = setup_butler_with_identity(viewer_name, "viewerpass").await?;

        let mut harness = TestHarness::new();
        harness.add_peer_with_butler("owner", CourierMode::User, owner_butler.clone()).await?;
        harness.add_peer_with_butler("node", CourierMode::Node, node_butler.clone()).await?;
        harness.add_peer_with_butler("viewer", CourierMode::User, viewer_butler.clone()).await?;

        let _transport = harness.spawn_transport();

        let node_node_id = harness.peer("node").unwrap().node_id;

        // === PHASE 1: Owner-Node handshake (production flow) ===
        let conn_string = node_butler.generate_connection_string(None).await?;
        let sovereign = owner_butler.add_sovereign_node(&conn_string)
            .map_err(|e| anyhow::anyhow!(e))?;
        let owner_permit = sovereign.permit.expect("Should have permit");

        harness.connect_and_notify("owner", "node").await?;
        tokio::time::sleep(ACTOR_SPAWN_DELAY).await;

        harness.peer("owner").unwrap().coordinator.cast(
            CoordinatorMessage::InitiateHandshake {
                node_id: node_node_id,
                permit: owner_permit,
            }
        )?;

        harness.wait_for_owner_authenticated(&node_butler).await?;

        // === PHASE 2: Create and publish space/page ===
        let user_info = owner_butler.user_info().await?;
        let space = owner_butler
            .create_space("Test Space".to_string(), user_info.did.clone(), TEST_SPACE_TEMPLATE)
            .await?;

        let layers: Vec<String> = TEST_PAGE_LAYERS.iter().map(|s| s.to_string()).collect();
        let page = owner_butler
            .create_page(&space.id, "Test Page", layers, TEST_PAGE_TEMPLATE)
            .await?;

        harness.peer("owner").unwrap().coordinator.cast(
            CoordinatorMessage::PublishSpace {
                node_id: node_node_id,
                space_id: space.id.clone(),
            }
        )?;

        harness.wait_for_page(&node_butler, &page.id).await?;

        // === PHASE 3: Viewer handshake (production flow via connection string) ===
        let viewer_conn_string = node_butler
            .generate_viewer_connection_string(&space.id, None)
            .await?;

        viewer_butler.add_sovereign_node(&viewer_conn_string)
            .map_err(|e| anyhow::anyhow!(e))?;

        harness.connect_and_notify("viewer", "node").await?;
        tokio::time::sleep(ACTOR_SPAWN_DELAY).await;

        // Get viewer's permit from sovereign node
        let viewer_sovereign = viewer_butler
            .get_sovereign_node(&node_node_id.to_string())?
            .expect("Viewer should have sovereign node");
        let viewer_permit = viewer_sovereign.permit.expect("Should have viewer permit");

        harness.peer("viewer").unwrap().coordinator.cast(
            CoordinatorMessage::InitiateHandshake {
                node_id: node_node_id,
                permit: viewer_permit,
            }
        )?;

        // Wait for viewer's sovereign node to be updated with post-handshake permit
        harness.wait_for_sovereign_permit(&viewer_butler, &node_node_id.to_string()).await?;

        Ok(Self {
            harness,
            owner_butler,
            node_butler,
            viewer_butler,
            space_id: space.id,
            page_id: page.id,
            _temps: (owner_temp, node_temp, viewer_temp),
        })
    }

    /// Get node's NodeId
    pub fn node_id(&self) -> NodeId {
        self.harness.peer("node").unwrap().node_id
    }

    /// Get owner's coordinator
    pub fn owner_coordinator(&self) -> &ActorRef<CoordinatorMessage> {
        &self.harness.peer("owner").unwrap().coordinator
    }

    /// Get viewer's coordinator
    pub fn viewer_coordinator(&self) -> &ActorRef<CoordinatorMessage> {
        &self.harness.peer("viewer").unwrap().coordinator
    }

    /// Shutdown the scenario
    pub async fn shutdown(&self) {
        self.harness.shutdown().await;
    }
}
