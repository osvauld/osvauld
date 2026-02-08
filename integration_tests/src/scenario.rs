//! Scenario - Multi-peer orchestration
//!
//! Generic over `C: Connection` to support both real network tests and mock protocol tests.
//!
//! ## Usage
//!
//! ```ignore
//! // Real network test (slower, requires relay)
//! let mut s = IrohScenario::new(true, true, 0).await?;
//! s.wait_ready().await;
//! s.connect_owner_to_node().await?;
//!
//! // Mock protocol test (fast, in-memory)
//! let mut s = MockScenario::new_mock(true, true, 0).await?;
//! s.connect_owner_to_node_mock().await?;
//! ```

use std::marker::PhantomData;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use tracing::info;
use transport::{Connection, MockBlobStore, MockConnection, NodeId};

use courier::coordinator::{CoordinatorMessage, CourierMode};
use courier::peer_actor::PeerMessage;

use crate::peer::{IrohPeer, MockPeer, Peer};
use crate::tracing::MessageTracer;

// Type Aliases

/// Scenario using real Iroh transport (production, slower, requires relay)
pub type IrohScenario = Scenario<transport::IrohConnection>;

/// Scenario using mock transport (tests, fast, in-memory)
pub type MockScenario = Scenario<MockConnection>;

// Timing constants

/// Delay for relay registration (iroh discovery)
pub const RELAY_READY_DELAY: Duration = Duration::from_secs(2);

/// Delay for handshake completion
pub const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);

/// Faster timeout for mock tests
pub const MOCK_TIMEOUT: Duration = Duration::from_millis(500);

/// Delay for page sync
pub const PAGE_SYNC_TIMEOUT: Duration = Duration::from_secs(10);

// Templates

/// Standard test template for creating spaces.
pub const SPACE_TEMPLATE: &str = r#"{
  "owner_template": {
    "operations": {
      "own": "allow",
      "get_share_link": "allow",
      "add_pages": "allow",
      "share_space": "allow"
    },
    "peer_capabilities": {
      "relay": false,
      "share": true,
      "accept_publish": true
    },
    "issue_on": {
      "node": {
        "token_type": "space_share",
        "peer_capabilities": { "relay": true, "share": true, "accept_publish": true },
        "operations": { "get_share_link": "allow", "add_pages": "allow", "share_space": "allow" },
        "auth_capabilities": { "can_connect": true, "persist_share": true, "can_delegate": false, "sync_enabled": true },
        "relationship": "node",
        "issue_on": {
          "viewer": {
            "token_type": "space_viewer",
            "peer_capabilities": { "relay": false, "share": false, "accept_publish": false },
            "operations": { "request_pages": "allow", "get_share_link": "allow" },
            "auth_capabilities": { "can_connect": true, "persist_share": false, "can_delegate": false, "sync_enabled": false },
            "relationship": "viewer"
          }
        }
      },
      "viewer": {
        "token_type": "space_viewer",
        "peer_capabilities": { "relay": false, "share": false, "accept_publish": false },
        "operations": { "request_pages": "allow", "get_share_link": "allow" },
        "auth_capabilities": { "can_connect": true, "persist_share": false, "can_delegate": false, "sync_enabled": false },
        "relationship": "viewer"
      }
    }
  }
}"#;

/// Standard test template for creating pages.
pub const PAGE_TEMPLATE: &str = r#"{
  "owner_template": {
    "operations": { "own": "allow", "share_page": "allow" },
    "peer_capabilities": { "relay": false, "share": true, "accept_publish": true },
    "presence": { "visible": true },
    "ephemeral_funcs": ["typing"],
    "layers": {
      "template_doc": { "sync": true, "write": true, "type": "crdt" },
      "content_doc": { "sync": true, "write": true, "type": "crdt" },
      "user_content_doc": { "sync": true, "write": true, "type": "crdt" },
      "collaborative_doc": { "sync": true, "write": true, "type": "crdt" },
      "submissions_doc": { "sync": true, "write": true, "type": "crdt" },
      "static_assets": { "sync": true, "write": true, "type": "asset" }
    },
    "layer_patterns": {
      "{page_id}/assets": { "create": true, "sync": true }
    },
    "sync": { "local_only": ["user_content_doc"] },
    "issue_on": {
      "node": {
        "token_type": "page_share",
        "peer_capabilities": { "relay": true, "share": true, "accept_publish": true },
        "presence": { "visible": true },
        "ephemeral_funcs": ["typing"],
        "operations": { "share_page": "allow" },
        "layers": {
          "template_doc": { "sync": true, "write": true, "type": "crdt" },
          "content_doc": { "sync": true, "write": true, "type": "crdt" },
          "user_content_doc": { "sync": true, "write": true, "type": "crdt" },
          "collaborative_doc": { "sync": true, "write": true, "type": "crdt" },
          "submissions_doc": { "sync": true, "write": true, "type": "crdt" },
          "static_assets": { "sync": true, "write": true, "type": "asset" }
        },
        "layer_patterns": {
          "{page_id}/assets": { "create": true, "sync": true }
        },
        "sync": { "local_only": ["user_content_doc"] },
        "auth_capabilities": { "can_connect": true, "persist_share": true, "can_delegate": false, "sync_enabled": true },
        "relationship": "node",
        "issue_on": {
          "viewer": {
            "token_type": "page_viewer",
            "peer_capabilities": { "relay": false, "share": false, "accept_publish": false },
            "presence": { "visible": true },
            "ephemeral_funcs": ["typing"],
            "operations": {},
            "layers": {
              "template_doc": { "sync": true, "write": false, "type": "crdt" },
              "content_doc": { "sync": true, "write": false, "type": "crdt" },
              "collaborative_doc": { "sync": true, "write": true, "type": "crdt" },
              "submissions_doc": { "sync": true, "write": true, "type": "crdt" },
              "static_assets": { "sync": true, "write": false, "type": "asset" }
            },
            "layer_patterns": {
              "{page_id}/assets": { "create": false, "sync": true }
            },
            "sync": { "local_only": ["user_content_doc"], "no_incoming_updates": ["submissions_doc"], "send_full_snapshot": ["submissions_doc"] },
            "auth_capabilities": { "can_connect": true, "persist_share": false, "can_delegate": false, "sync_enabled": false },
            "relationship": "viewer"
          }
        }
      },
      "viewer": {
        "token_type": "page_viewer",
        "peer_capabilities": { "relay": false, "share": false, "accept_publish": false },
        "presence": { "visible": true },
        "ephemeral_funcs": ["typing"],
        "operations": {},
        "layers": {
          "template_doc": { "sync": true, "write": false, "type": "crdt" },
          "content_doc": { "sync": true, "write": false, "type": "crdt" },
          "collaborative_doc": { "sync": true, "write": true, "type": "crdt" },
          "submissions_doc": { "sync": true, "write": true, "type": "crdt" },
          "static_assets": { "sync": true, "write": false, "type": "asset" }
        },
        "layer_patterns": {
          "{page_id}/assets": { "create": false, "sync": true }
        },
        "sync": { "local_only": ["user_content_doc"], "no_incoming_updates": ["submissions_doc"], "send_full_snapshot": ["submissions_doc"] },
        "auth_capabilities": { "can_connect": true, "persist_share": false, "can_delegate": false, "sync_enabled": false },
        "relationship": "viewer"
      }
    }
  }
}"#;

/// Standard page layers for testing.
pub const PAGE_LAYERS: &[&str] = &[
    "template_doc",
    "content_doc",
    "user_content_doc",
    "collaborative_doc",
    "submissions_doc",
    "static_assets",
];

/// Chat page layers (for Group Chat testing).
pub const CHAT_PAGE_LAYERS: &[&str] = &[
    "messages",
    "reactions",
];

/// Chat page template - matches osvauld-demos permit_template.json
///
/// **Note**: Layer names are relative (e.g. "messages"), but permits use
/// `{page_id}/messages` patterns. The page creation adds page_id prefix.
pub const CHAT_PAGE_TEMPLATE: &str = r#"{
  "owner_template": {
    "operations": { "own": "allow", "read": "allow", "write": "allow", "share": "allow", "share_page": "allow" },
    "peer_capabilities": { "relay": false, "share": true, "accept_publish": true },
    "layers": {
      "{page_id}/messages": { "type": "list", "sync": true, "write": true },
      "{page_id}/reactions": { "type": "map", "sync": true, "write": true },
      "{page_id}/presence": { "type": "map", "sync": true, "write": true }
    },
    "presence": { "visible": true },
    "ephemeral_funcs": ["typing"],
    "issue_on": {
      "node": {
        "token_type": "page_share",
        "peer_capabilities": { "relay": true, "share": true, "accept_publish": true },
        "operations": { "share_page": "allow", "add_pages": "allow" },
        "layers": {
          "{page_id}/messages": { "type": "list", "sync": true, "write": true },
          "{page_id}/reactions": { "type": "map", "sync": true, "write": true },
          "{page_id}/presence": { "type": "map", "sync": true, "write": true }
        },
        "auth_capabilities": { "can_connect": true, "persist_share": true, "can_delegate": true, "sync_enabled": true },
        "relationship": "node",
        "presence": { "visible": true },
        "ephemeral_funcs": ["typing"],
        "issue_on": {
          "viewer": {
            "token_type": "page_viewer",
            "peer_capabilities": { "relay": false, "share": false, "accept_publish": false },
            "operations": { "read": "allow", "write": "allow" },
            "layers": {
              "{page_id}/messages": { "type": "list", "sync": true, "write": true },
              "{page_id}/reactions": { "type": "map", "sync": true, "write": true },
              "{page_id}/presence": { "type": "map", "sync": true, "write": true }
            },
            "auth_capabilities": { "can_connect": true, "sync_enabled": true },
            "presence": { "visible": true },
            "ephemeral_funcs": ["typing"],
            "relationship": "collaborator"
          }
        }
      },
      "collaborator": {
        "token_type": "page_viewer",
        "peer_capabilities": { "relay": false, "share": false, "accept_publish": false },
        "operations": { "read": "allow", "write": "allow" },
        "layers": {
          "{page_id}/messages": { "type": "list", "sync": true, "write": true },
          "{page_id}/reactions": { "type": "map", "sync": true, "write": true },
          "{page_id}/presence": { "type": "map", "sync": true, "write": true }
        },
        "auth_capabilities": { "can_connect": true, "sync_enabled": true },
        "presence": { "visible": true },
        "ephemeral_funcs": ["typing"],
        "relationship": "collaborator"
      }
    }
  },
  "consent_template": {
    "token_type": "sync_page_consent",
    "peer_capabilities": { "relay": false, "share": false, "accept_publish": false },
    "operations": { "receive_layer_updates": "allow", "send_layer_updates": "allow" },
    "layers": {
      "{page_id}/messages": { "sync": true, "write": true },
      "{page_id}/reactions": { "sync": true, "write": true },
      "{page_id}/presence": { "sync": true, "write": true }
    },
    "auth_capabilities": { "accept_sync": true, "can_send": true },
    "presence": { "visible": true },
    "ephemeral_funcs": ["typing"],
    "relationship": "sync_consent"
  }
}"#;

// SpaceInfo

/// Information about a created space
pub struct SpaceInfo {
    pub id: String,
    pub page_id: String,
}

// Scenario<C: Connection>

/// Multi-peer test scenario
///
/// Generic over `C: Connection` to support:
/// - `Scenario<IrohConnection>` - Real network tests
/// - `Scenario<MockConnection>` - Fast protocol tests
pub struct Scenario<C: Connection> {
    /// Owner peer (if created)
    pub owner: Option<Peer<C>>,
    /// Node peer (if created)
    pub node: Option<Peer<C>>,
    /// Viewer peers
    pub viewers: Vec<Peer<C>>,
    /// Shared blob store for all peers
    pub blobs: Arc<MockBlobStore>,
    /// Optional message tracer
    pub tracer: Option<MessageTracer>,
    /// PhantomData for generic
    _phantom: PhantomData<C>,
}

// IrohScenario (real network)

impl Scenario<transport::IrohConnection> {
    /// Create a new scenario with real Iroh transport
    ///
    /// **Args**:
    /// - `owner`: Whether to create an owner peer
    /// - `node`: Whether to create a node peer
    /// - `viewer_count`: Number of viewer peers to create
    pub async fn new(owner: bool, node: bool, viewer_count: usize) -> Result<Self> {
        let blobs = MockBlobStore::new();

        let owner_peer = if owner {
            Some(IrohPeer::new("owner", CourierMode::User, blobs.clone()).await?)
        } else {
            None
        };

        let node_peer = if node {
            Some(IrohPeer::new("node", CourierMode::Node, blobs.clone()).await?)
        } else {
            None
        };

        let mut viewers = Vec::with_capacity(viewer_count);
        for i in 0..viewer_count {
            let name = format!("viewer{}", i);
            viewers.push(IrohPeer::new(&name, CourierMode::User, blobs.clone()).await?);
        }

        info!(
            "IrohScenario created: owner={}, node={}, viewers={}",
            owner, node, viewer_count
        );

        Ok(Self {
            owner: owner_peer,
            node: node_peer,
            viewers,
            blobs,
            tracer: None,
            _phantom: PhantomData,
        })
    }

    /// Create scenario with message tracing
    pub async fn with_tracer(
        owner: bool,
        node: bool,
        viewer_count: usize,
        tracer: MessageTracer,
    ) -> Result<Self> {
        let mut scenario = Self::new(owner, node, viewer_count).await?;
        scenario.tracer = Some(tracer);
        Ok(scenario)
    }

    /// Wait for all peers to register with relay
    ///
    /// This is required because iroh's relay discovery takes time.
    pub async fn wait_ready(&self) {
        tokio::time::sleep(RELAY_READY_DELAY).await;
        info!("All peers ready (relay registration complete)");
    }

    /// Connect owner to node and complete handshake
    ///
    /// **Flow**:
    /// 1. Node generates connection string
    /// 2. Owner parses and stores sovereign node
    /// 3. Owner connects transport to node
    /// 4. Owner initiates handshake
    /// 5. Both wait for authentication
    pub async fn connect_owner_to_node(&mut self) -> Result<()> {
        let node = self.node.as_ref().expect("Scenario has no node");
        let owner = self.owner.as_ref().expect("Scenario has no owner");

        // Node generates connection string
        let conn_string = node.butler.nodes().generate_connection_string(None).await?;
        info!("Node connection string generated");

        // Owner parses and stores sovereign node
        let sovereign = owner.butler.nodes().add(&conn_string)
            .map_err(|e| anyhow::anyhow!("add_sovereign_node failed: {}", e))?;
        let permit = sovereign.permit.expect("Connection string should have permit");
        info!("Owner stored sovereign node: {}", sovereign.node_id);

        // Owner connects transport to node
        owner.connect_to(node).await?;
        info!("Transport connected");

        // Small delay for PeerActor to spawn
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Owner initiates handshake
        owner.handshake(node.node_id, permit)?;
        info!("Handshake initiated");

        // Wait for both sides to authenticate
        let owner = self.owner.as_mut().expect("Scenario has no owner");
        let (_, peer_username) = owner.wait_authenticated(HANDSHAKE_TIMEOUT).await?;
        info!("Owner authenticated with: {}", peer_username);

        let node = self.node.as_mut().expect("Scenario has no node");
        let (_, peer_username) = node.wait_authenticated(HANDSHAKE_TIMEOUT).await?;
        info!("Node authenticated with: {}", peer_username);

        Ok(())
    }

    /// Setup viewer with connection string (Iroh)
    pub async fn setup_viewer(&mut self, index: usize, conn_string: &str) -> Result<()> {
        let node = self.node.as_ref().expect("Scenario has no node");
        let viewer = &self.viewers[index];

        // Viewer parses connection string
        let conn = viewer.butler.nodes().parse_connection_string(conn_string)
            .map_err(|e| anyhow::anyhow!("parse_connection_string failed: {}", e))?;
        let permit = conn.permit.clone();
        let viewer_node_id: NodeId = conn.node_id().parse()?;
        info!("Viewer parsed connection string, target: {}", viewer_node_id);

        // Viewer connects transport to node
        viewer.connect_to(node).await?;
        info!("Viewer transport connected");

        // Small delay for PeerActor to spawn
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Viewer initiates handshake
        viewer.handshake(viewer_node_id, permit)?;
        info!("Viewer handshake initiated");

        // Wait for viewer authentication
        let viewer = self.viewer_mut(index);
        let result = viewer.wait_authenticated(HANDSHAKE_TIMEOUT).await;
        match result {
            Ok((_, username)) => info!("Viewer authenticated with: {}", username),
            Err(e) => info!("Viewer auth result: {}", e),
        }

        Ok(())
    }
}

// MockScenario (in-memory, fast)

impl Scenario<MockConnection> {
    /// Create a new scenario with mock transport
    ///
    /// **Args**:
    /// - `owner`: Whether to create an owner peer
    /// - `node`: Whether to create a node peer
    /// - `viewer_count`: Number of viewer peers to create
    pub async fn new_mock(owner: bool, node: bool, viewer_count: usize) -> Result<Self> {
        let blobs = MockBlobStore::new();

        let owner_peer = if owner {
            Some(MockPeer::new_mock("owner", CourierMode::User, blobs.clone()).await?)
        } else {
            None
        };

        let node_peer = if node {
            Some(MockPeer::new_mock("node", CourierMode::Node, blobs.clone()).await?)
        } else {
            None
        };

        let mut viewers = Vec::with_capacity(viewer_count);
        for i in 0..viewer_count {
            let name = format!("viewer{}", i);
            viewers.push(MockPeer::new_mock(&name, CourierMode::User, blobs.clone()).await?);
        }

        info!(
            "MockScenario created: owner={}, node={}, viewers={}",
            owner, node, viewer_count
        );

        Ok(Self {
            owner: owner_peer,
            node: node_peer,
            viewers,
            blobs,
            tracer: None,
            _phantom: PhantomData,
        })
    }

    /// Create mock scenario with message tracing
    pub async fn with_tracer_mock(
        owner: bool,
        node: bool,
        viewer_count: usize,
        tracer: MessageTracer,
    ) -> Result<Self> {
        let mut scenario = Self::new_mock(owner, node, viewer_count).await?;
        scenario.tracer = Some(tracer);
        Ok(scenario)
    }

    /// Connect owner to node using mock connection (fast, no relay needed)
    ///
    /// **Flow**:
    /// 1. Node generates connection string
    /// 2. Owner parses and stores sovereign node
    /// 3. Inject mock connection between owner and node
    /// 4. Owner initiates handshake
    /// 5. Both wait for authentication
    pub async fn connect_owner_to_node_mock(&mut self) -> Result<()> {
        let node = self.node.as_ref().expect("Scenario has no node");
        let owner = self.owner.as_ref().expect("Scenario has no owner");

        // Node generates connection string (needed for permit)
        let conn_string = node.butler.nodes().generate_connection_string(None).await?;
        info!("Node connection string generated");

        // Owner parses and stores sovereign node
        let sovereign = owner.butler.nodes().add(&conn_string)
            .map_err(|e| anyhow::anyhow!("add_sovereign_node failed: {}", e))?;
        let permit = sovereign.permit.expect("Connection string should have permit");
        info!("Owner stored sovereign node: {}", sovereign.node_id);

        // Inject mock connection (bidirectional) - use tracer if available
        if let Some(ref tracer) = self.tracer {
            owner.connect_mock_to_with_tracer(node, tracer)?;
            info!("Mock connection established (with tracer)");
        } else {
            owner.connect_mock_to(node)?;
            info!("Mock connection established");
        }

        // Small delay for PeerActor to spawn
        tokio::time::sleep(Duration::from_millis(10)).await;

        // Owner initiates handshake
        owner.handshake(node.node_id, permit)?;
        info!("Handshake initiated");

        // Wait for both sides to authenticate (use faster timeout for mocks)
        let owner = self.owner.as_mut().expect("Scenario has no owner");
        let (_, peer_username) = owner.wait_authenticated(MOCK_TIMEOUT).await?;
        info!("Owner authenticated with: {}", peer_username);

        let node = self.node.as_mut().expect("Scenario has no node");
        let (_, peer_username) = node.wait_authenticated(MOCK_TIMEOUT).await?;
        info!("Node authenticated with: {}", peer_username);

        Ok(())
    }

    /// Setup viewer with mock connection
    pub async fn setup_viewer_mock(&mut self, index: usize, conn_string: &str) -> Result<()> {
        let node = self.node.as_ref().expect("Scenario has no node");
        let viewer = &self.viewers[index];

        // Viewer parses connection string
        let conn = viewer.butler.nodes().parse_connection_string(conn_string)
            .map_err(|e| anyhow::anyhow!("parse_connection_string failed: {}", e))?;
        let permit = conn.permit.clone();
        info!("Viewer parsed connection string");

        // Inject mock connection - use tracer if available
        if let Some(ref tracer) = self.tracer {
            viewer.connect_mock_to_with_tracer(node, tracer)?;
            info!("Viewer mock connection established (with tracer)");
        } else {
            viewer.connect_mock_to(node)?;
            info!("Viewer mock connection established");
        }

        // Small delay for PeerActor to spawn
        tokio::time::sleep(Duration::from_millis(10)).await;

        // Viewer initiates handshake
        viewer.handshake(node.node_id, permit)?;
        info!("Viewer handshake initiated");

        // Wait for viewer authentication
        let viewer = self.viewer_mut(index);
        let result = viewer.wait_authenticated(MOCK_TIMEOUT).await;
        match result {
            Ok((_, username)) => info!("Viewer authenticated with: {}", username),
            Err(e) => info!("Viewer auth result: {}", e),
        }

        Ok(())
    }

    /// Full viewer flow: connect, handshake, request space, wait for page
    ///
    /// **Flow**:
    /// 1. Viewer parses connection string (gets space_id + permit)
    /// 2. Mock connect + handshake with node
    /// 3. Send RequestSpace to viewer's PeerActor for the node
    /// 4. Wait for viewer to have the page
    pub async fn add_viewer_mock(&mut self, index: usize, conn_string: &str, page_id: &str) -> Result<()> {
        use butler::ConnectionStringExt;
        use courier::coordinator::CoordinatorMessage;
        use courier::peer_actor::PeerMessage;

        let node = self.node.as_ref().expect("Scenario has no node");
        let viewer = &self.viewers[index];

        // Parse connection string to get space_id and permit
        let conn = viewer.butler.nodes().parse_connection_string(conn_string)
            .map_err(|e| anyhow::anyhow!("parse_connection_string failed: {}", e))?;
        let permit = conn.permit.clone();
        let space_id = conn.space_id()
            .map_err(|e| anyhow::anyhow!("space_id from permit failed: {}", e))?;
        info!("Viewer parsed connection string: space_id={}", space_id);

        // Mock connect + handshake - use tracer if available
        if let Some(ref tracer) = self.tracer {
            viewer.connect_mock_to_with_tracer(node, tracer)?;
        } else {
            viewer.connect_mock_to(node)?;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
        viewer.handshake(node.node_id, permit.clone())?;

        let viewer = self.viewer_mut(index);
        viewer.wait_authenticated(MOCK_TIMEOUT).await?;
        info!("Viewer authenticated");

        // Send RequestSpace from viewer's PeerActor
        let viewer = &self.viewers[index];
        let (tx, rx) = tokio::sync::oneshot::channel();
        viewer.coordinator.cast(CoordinatorMessage::GetPeerActor {
            node_id: self.node.as_ref().unwrap().node_id,
            response: tx,
        }).map_err(|e| anyhow::anyhow!("GetPeerActor failed: {:?}", e))?;

        let peer_actor = rx.await
            .map_err(|_| anyhow::anyhow!("GetPeerActor channel closed"))?
            .ok_or_else(|| anyhow::anyhow!("No PeerActor for node on viewer"))?;

        peer_actor.cast(PeerMessage::RequestSpace {
            space_id: space_id.clone(),
            viewer_permit: permit,
        }).map_err(|e| anyhow::anyhow!("RequestSpace failed: {:?}", e))?;
        info!("Viewer sent RequestSpace for space {}", space_id);

        // Wait for page to arrive on viewer
        self.wait_for_page(&self.viewers[index], page_id).await?;
        info!("Viewer has page {}", page_id);

        Ok(())
    }
}

// Common Methods (for all Connection types)

impl<C: Connection> Scenario<C> {
    /// Get owner (panics if not created)
    pub fn owner(&self) -> &Peer<C> {
        self.owner.as_ref().expect("Scenario has no owner")
    }

    /// Get mutable owner (panics if not created)
    pub fn owner_mut(&mut self) -> &mut Peer<C> {
        self.owner.as_mut().expect("Scenario has no owner")
    }

    /// Get node (panics if not created)
    pub fn node(&self) -> &Peer<C> {
        self.node.as_ref().expect("Scenario has no node")
    }

    /// Get mutable node (panics if not created)
    pub fn node_mut(&mut self) -> &mut Peer<C> {
        self.node.as_mut().expect("Scenario has no node")
    }

    /// Get viewer by index
    pub fn viewer(&self, index: usize) -> &Peer<C> {
        &self.viewers[index]
    }

    /// Get mutable viewer by index
    pub fn viewer_mut(&mut self, index: usize) -> &mut Peer<C> {
        &mut self.viewers[index]
    }

    /// Get message tracer
    pub fn tracer(&self) -> Option<&MessageTracer> {
        self.tracer.as_ref()
    }

    /// Setup owner: create space and page
    ///
    /// **Returns**: SpaceInfo with space_id and page_id
    pub async fn setup_owner(&self, space_name: &str) -> Result<SpaceInfo> {
        let owner = self.owner();
        let user_info = owner.butler.user_info().await?;

        // Create space
        let space = owner.butler.spaces()
            .create(space_name.to_string(), user_info.did.clone(), SPACE_TEMPLATE)
            .await?;
        info!("Space created: {}", space.id);

        // Create page
        let layer_names: Vec<String> = PAGE_LAYERS.iter().map(|s| s.to_string()).collect();
        let page = owner.butler.pages()
            .create(&space.id, "Test Page", layer_names, PAGE_TEMPLATE)
            .await?;
        info!("Page created: {}", page.id);

        Ok(SpaceInfo {
            id: space.id,
            page_id: page.id,
        })
    }

    /// Setup owner with chat template: create space and page with messages layer
    ///
    /// **Returns**: SpaceInfo with space_id and page_id
    pub async fn setup_owner_chat(&self, space_name: &str) -> Result<SpaceInfo> {
        let owner = self.owner();
        let user_info = owner.butler.user_info().await?;

        // Create space
        let space = owner.butler.spaces()
            .create(space_name.to_string(), user_info.did.clone(), SPACE_TEMPLATE)
            .await?;
        info!("Space created: {}", space.id);

        // Create page with chat layers
        let layer_names: Vec<String> = CHAT_PAGE_LAYERS.iter().map(|s| s.to_string()).collect();
        let page = owner.butler.pages()
            .create(&space.id, "Chat Page", layer_names, CHAT_PAGE_TEMPLATE)
            .await?;
        info!("Page created (chat): {}", page.id);

        Ok(SpaceInfo {
            id: space.id,
            page_id: page.id,
        })
    }

    /// Get viewer link (connection string) for a space
    pub async fn get_viewer_link(&self, space_id: &str) -> Result<String> {
        let node = self.node();
        let conn_string = node.butler.nodes().generate_viewer_connection_string(space_id, None).await?;
        Ok(conn_string)
    }

    /// Publish space from owner to node
    ///
    /// **Flow**: Gets PeerActor reference, sends PublishSpace message
    pub async fn publish_space(&self, space_id: &str) -> Result<()> {
        let owner = self.owner();
        let node = self.node();

        // Get PeerActor reference for the node
        let (tx, rx) = tokio::sync::oneshot::channel();
        owner.coordinator.cast(CoordinatorMessage::GetPeerActor {
            node_id: node.node_id,
            response: tx,
        }).map_err(|e| anyhow::anyhow!("Failed to send GetPeerActor: {:?}", e))?;

        let peer_actor = rx.await
            .map_err(|_| anyhow::anyhow!("GetPeerActor channel closed"))?
            .ok_or_else(|| anyhow::anyhow!("No PeerActor for node {}", node.node_id))?;

        // Send PublishSpace to the PeerActor
        peer_actor.cast(PeerMessage::PublishSpace {
            space_id: space_id.to_string(),
        }).map_err(|e| anyhow::anyhow!("Failed to send PublishSpace: {:?}", e))?;

        info!("PublishSpace sent for: {}", space_id);
        Ok(())
    }

    /// Wait for page to sync to a peer
    pub async fn wait_for_page(&self, peer: &Peer<C>, page_id: &str) -> Result<()> {
        let butler = peer.butler.clone();
        let page_id = page_id.to_string();
        let deadline = tokio::time::Instant::now() + PAGE_SYNC_TIMEOUT;

        loop {
            if butler.pages().get(&page_id).ok().flatten().is_some() {
                info!("Page {} synced to {}", page_id, peer.name);
                return Ok(());
            }
            if tokio::time::Instant::now() > deadline {
                return Err(anyhow::anyhow!("Timeout waiting for page {} on {}", page_id, peer.name));
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    /// Shutdown all peers
    pub async fn shutdown(&mut self) {
        if let Some(owner) = &self.owner {
            owner.shutdown().await;
        }
        if let Some(node) = &self.node {
            node.shutdown().await;
        }
        for viewer in &self.viewers {
            viewer.shutdown().await;
        }
        info!("Scenario shutdown complete");
    }
}

impl<C: Connection> Drop for Scenario<C> {
    fn drop(&mut self) {
        // Peers have their own Drop impl that sends shutdown
    }
}

// Tracing helper

/// Initialize tracing for tests
pub fn init_tracing() {
    use tracing_subscriber::EnvFilter;
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env()
                .add_directive("info".parse().unwrap())
        )
        .try_init();
}
