//! Scenario — builder pattern for multi-peer test orchestration
//!
//! MockConnection only, no generics.

use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use tracing::info;
use transport::MockBlobStore;

use courier::coordinator::CoordinatorMessage;
use courier::peer_actor::PeerMessage;

use crate::fixtures::{self, SpaceInfo, MOCK_TIMEOUT, PAGE_SYNC_TIMEOUT};
use crate::peer::Peer;
use crate::tracer::Tracer;

/// Builder for constructing test scenarios
pub struct ScenarioBuilder {
    has_owner: bool,
    has_node: bool,
    viewer_count: usize,
    app: Option<String>,
    space_name: Option<String>,
    publish: bool,
    connect: bool,
    trace: bool,
}

impl ScenarioBuilder {
    pub fn new() -> Self {
        Self {
            has_owner: false,
            has_node: false,
            viewer_count: 0,
            app: None,
            space_name: None,
            publish: false,
            connect: false,
            trace: false,
        }
    }

    /// Add an owner peer
    pub fn owner(mut self) -> Self {
        self.has_owner = true;
        self
    }

    /// Add a node peer
    pub fn node(mut self) -> Self {
        self.has_node = true;
        self
    }

    /// Add viewer peers
    pub fn viewers(mut self, n: usize) -> Self {
        self.viewer_count = n;
        self
    }

    /// Set sample app name (e.g. "osvauld-demos")
    pub fn app(mut self, name: &str) -> Self {
        self.app = Some(name.to_string());
        self
    }

    /// Override space name (defaults to "Test Space")
    pub fn space_name(mut self, name: &str) -> Self {
        self.space_name = Some(name.to_string());
        self
    }

    /// Connect owner to node (implies owner + node)
    pub fn connected(mut self) -> Self {
        self.has_owner = true;
        self.has_node = true;
        self.connect = true;
        self
    }

    /// Full publish flow (implies connected + app)
    pub fn published(mut self) -> Self {
        self.has_owner = true;
        self.has_node = true;
        self.connect = true;
        self.publish = true;
        if self.app.is_none() {
            self.app = Some("osvauld-demos".to_string());
        }
        self
    }

    /// Enable protocol message tracing
    pub fn with_tracer(mut self) -> Self {
        self.trace = true;
        self
    }

    /// Build the scenario
    pub async fn build(self) -> Result<Scenario> {
        let blobs = MockBlobStore::new();

        // Create tracer if requested
        let (tracer, message_tx) = if self.trace {
            let (t, tx) = Tracer::new();
            (Some(t), Some(tx))
        } else {
            (None, None)
        };

        // Create peers
        let owner = if self.has_owner {
            Some(Peer::new("owner", courier::coordinator::CourierMode::User, blobs.clone(), message_tx.clone()).await?)
        } else {
            None
        };

        let node = if self.has_node {
            Some(Peer::new("node", courier::coordinator::CourierMode::Node, blobs.clone(), message_tx.clone()).await?)
        } else {
            None
        };

        let mut viewers = Vec::with_capacity(self.viewer_count);
        for i in 0..self.viewer_count {
            let name = format!("viewer{}", i);
            viewers.push(Peer::new(&name, courier::coordinator::CourierMode::User, blobs.clone(), message_tx.clone()).await?);
        }

        let mut scenario = Scenario {
            owner,
            node,
            viewers,
            blobs,
            tracer,
            space_info: None,
        };

        // Connect if requested
        if self.connect {
            scenario.connect_owner_to_node().await?;
        }

        // Publish if requested
        if self.publish {
            let app_name = self.app.as_deref().unwrap_or("osvauld-demos");
            let space_name = self.space_name.as_deref().unwrap_or("Test Space");
            let info = scenario.import_app(space_name, app_name).await?;
            scenario.publish_space(&info.space_id).await?;
            // Wait for page to sync to node
            let page_id = info.page_id.clone();
            scenario.wait_for_page_on_node(&page_id).await?;
            scenario.space_info = Some(info);
        }

        Ok(scenario)
    }
}

/// Multi-peer test scenario
pub struct Scenario {
    pub owner: Option<Peer>,
    pub node: Option<Peer>,
    pub viewers: Vec<Peer>,
    pub blobs: Arc<MockBlobStore>,
    tracer: Option<Tracer>,
    space_info: Option<SpaceInfo>,
}

impl Scenario {
    /// Start building a scenario
    pub fn builder() -> ScenarioBuilder {
        ScenarioBuilder::new()
    }

    // -- Accessors --

    pub fn owner(&self) -> &Peer {
        self.owner.as_ref().expect("Scenario has no owner")
    }

    pub fn owner_mut(&mut self) -> &mut Peer {
        self.owner.as_mut().expect("Scenario has no owner")
    }

    pub fn node(&self) -> &Peer {
        self.node.as_ref().expect("Scenario has no node")
    }

    pub fn node_mut(&mut self) -> &mut Peer {
        self.node.as_mut().expect("Scenario has no node")
    }

    pub fn viewer(&self, i: usize) -> &Peer {
        &self.viewers[i]
    }

    pub fn viewer_mut(&mut self, i: usize) -> &mut Peer {
        &mut self.viewers[i]
    }

    pub fn tracer(&self) -> &Tracer {
        self.tracer.as_ref().expect("Scenario has no tracer (use .with_tracer())")
    }

    pub fn space(&self) -> &SpaceInfo {
        self.space_info.as_ref().expect("Scenario has no space (use .published() or import_app())")
    }

    // -- Operations --

    /// Connect owner to node via mock connection + handshake
    pub async fn connect_owner_to_node(&mut self) -> Result<()> {
        let node = self.node.as_ref().expect("Scenario has no node");
        let owner = self.owner.as_ref().expect("Scenario has no owner");

        // Node generates connection string (needed for permit)
        let conn_string = node.butler.nodes().generate_connection_string(None).await?;

        // Owner parses and stores sovereign node
        let sovereign = owner.butler.nodes().add(&conn_string)
            .map_err(|e| anyhow::anyhow!("add_sovereign_node failed: {}", e))?;
        let permit = sovereign.permit.expect("Connection string should have permit");

        // Inject mock connection
        owner.connect_to(node)?;
        tokio::time::sleep(Duration::from_millis(10)).await;

        // Owner initiates handshake
        owner.handshake(node.node_id, permit)?;

        // Wait for both sides
        let owner = self.owner.as_mut().unwrap();
        owner.wait_authenticated(MOCK_TIMEOUT).await?;
        let node = self.node.as_mut().unwrap();
        node.wait_authenticated(MOCK_TIMEOUT).await?;

        info!("Owner <-> Node connected and authenticated");
        Ok(())
    }

    /// Import app from sample_apps directory, creating space + page.
    pub async fn import_app(&self, space_name: &str, app_name: &str) -> Result<SpaceInfo> {
        let owner = self.owner();
        let user_info = owner.butler.user_info().await?;

        // Resolve app directory for page import
        let app_path = fixtures::app_dir(app_name);

        // Create space
        let space = owner
            .butler
            .spaces()
            .create(space_name.to_string(), user_info.did.clone())
            .await?;
        info!("Space created: {}", space.id);

        // Import page from app directory (app.osv + app subdirs with manifest.json)
        let page = owner.butler.apps().import_page(&space.id, &app_path).await?;
        info!("Page imported: {} from {:?}", page.id, app_path);

        Ok(SpaceInfo {
            space_id: space.id,
            page_id: page.id,
        })
    }

    /// Publish a space from owner to node
    pub async fn publish_space(&self, space_id: &str) -> Result<()> {
        let owner = self.owner();
        let node = self.node();

        let (tx, rx) = tokio::sync::oneshot::channel();
        owner
            .coordinator
            .cast(CoordinatorMessage::GetPeerActor {
                node_id: node.node_id,
                response: tx,
            })
            .map_err(|e| anyhow::anyhow!("GetPeerActor failed: {:?}", e))?;

        let peer_actor = rx
            .await
            .map_err(|_| anyhow::anyhow!("GetPeerActor channel closed"))?
            .ok_or_else(|| anyhow::anyhow!("No PeerActor for node"))?;

        peer_actor
            .cast(PeerMessage::PublishSpace {
                space_id: space_id.to_string(),
            })
            .map_err(|e| anyhow::anyhow!("PublishSpace failed: {:?}", e))?;

        info!("PublishSpace sent: {}", space_id);
        Ok(())
    }

    /// Get viewer connection string for a space
    pub async fn get_viewer_link(&self, space_id: &str) -> Result<String> {
        let node = self.node();
        let conn = node
            .butler
            .nodes()
            .generate_viewer_connection_string(space_id, None)
            .await?;
        Ok(conn)
    }

    /// Full viewer flow: connect, handshake, request space, wait for page
    pub async fn add_viewer(&mut self, index: usize, conn_string: &str, page_id: &str) -> Result<()> {
        use butler::ConnectionStringExt;

        let node = self.node.as_ref().expect("Scenario has no node");
        let viewer = &self.viewers[index];

        // Parse connection string
        let conn = viewer
            .butler
            .nodes()
            .parse_connection_string(conn_string)
            .map_err(|e| anyhow::anyhow!("parse_connection_string failed: {}", e))?;
        let permit = conn.permit.clone();
        let space_id = conn
            .space_id()
            .map_err(|e| anyhow::anyhow!("space_id from permit failed: {}", e))?;

        // Connect + handshake
        viewer.connect_to(node)?;
        tokio::time::sleep(Duration::from_millis(10)).await;
        viewer.handshake(node.node_id, permit.clone())?;

        let viewer = self.viewer_mut(index);
        viewer.wait_authenticated(MOCK_TIMEOUT).await?;

        // Request space
        let viewer = &self.viewers[index];
        let (tx, rx) = tokio::sync::oneshot::channel();
        viewer
            .coordinator
            .cast(CoordinatorMessage::GetPeerActor {
                node_id: self.node.as_ref().unwrap().node_id,
                response: tx,
            })
            .map_err(|e| anyhow::anyhow!("GetPeerActor failed: {:?}", e))?;

        let peer_actor = rx
            .await
            .map_err(|_| anyhow::anyhow!("GetPeerActor channel closed"))?
            .ok_or_else(|| anyhow::anyhow!("No PeerActor for node on viewer"))?;

        peer_actor
            .cast(PeerMessage::RequestSpace {
                space_id,
                viewer_permit: permit,
            })
            .map_err(|e| anyhow::anyhow!("RequestSpace failed: {:?}", e))?;

        // Wait for page
        self.wait_for_page(&self.viewers[index], page_id).await?;
        info!("Viewer {} has page {}", index, page_id);

        Ok(())
    }

    /// Setup viewer (connect + handshake only, no space request)
    pub async fn setup_viewer(&mut self, index: usize, conn_string: &str) -> Result<()> {
        let node = self.node.as_ref().expect("Scenario has no node");
        let viewer = &self.viewers[index];

        let conn = viewer
            .butler
            .nodes()
            .parse_connection_string(conn_string)
            .map_err(|e| anyhow::anyhow!("parse_connection_string failed: {}", e))?;
        let permit = conn.permit.clone();

        viewer.connect_to(node)?;
        tokio::time::sleep(Duration::from_millis(10)).await;
        viewer.handshake(node.node_id, permit)?;

        let viewer = self.viewer_mut(index);
        viewer.wait_authenticated(MOCK_TIMEOUT).await?;

        Ok(())
    }

    /// Wait for page to sync to a specific peer
    pub async fn wait_for_page(&self, peer: &Peer, page_id: &str) -> Result<()> {
        let butler = peer.butler.clone();
        let page_id = page_id.to_string();
        let deadline = tokio::time::Instant::now() + PAGE_SYNC_TIMEOUT;

        loop {
            if butler.pages().get(&page_id).ok().flatten().is_some() {
                return Ok(());
            }
            if tokio::time::Instant::now() > deadline {
                return Err(anyhow::anyhow!(
                    "Timeout waiting for page {} on {}",
                    page_id,
                    peer.name
                ));
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    /// Wait for page to sync to the node
    pub async fn wait_for_page_on_node(&self, page_id: &str) -> Result<()> {
        self.wait_for_page(self.node(), page_id).await
    }

    /// Disconnect owner from node (simulates network drop)
    ///
    /// **Context**: Both Coordinators receive Disconnected, PeerActors are cleaned up,
    /// Scribe subscriptions are removed. Butler state persists for reconnection.
    pub async fn disconnect_owner_from_node(&mut self) -> Result<()> {
        let owner = self.owner.as_ref().expect("Scenario has no owner");
        let node = self.node.as_ref().expect("Scenario has no node");
        owner.disconnect_from(node)?;
        // Allow cleanup to propagate
        tokio::time::sleep(Duration::from_millis(100)).await;
        Ok(())
    }

    /// Reconnect owner to node after disconnect
    ///
    /// **Context**: Creates fresh MockConnection pair, re-injects into both Coordinators,
    /// initiates handshake with stored reconnection permit. Butler state (permits, pages,
    /// layers, owner info) persisted across the disconnect.
    pub async fn reconnect_owner_to_node(&mut self) -> Result<()> {
        let node = self.node.as_ref().expect("Scenario has no node");
        let owner = self.owner.as_ref().expect("Scenario has no owner");

        // Get reconnection permit from Butler (stored during first connection)
        let node_id_str = node.node_id.to_string();
        let sovereign = owner.butler.nodes().get(&node_id_str)?
            .ok_or_else(|| anyhow::anyhow!("No sovereign node record for reconnection"))?;
        let reconnect_permit = sovereign.permit
            .ok_or_else(|| anyhow::anyhow!("No reconnection permit stored"))?;

        // Create fresh mock connection
        owner.connect_to(node)?;
        tokio::time::sleep(Duration::from_millis(10)).await;

        // Handshake with reconnection permit
        owner.handshake(node.node_id, reconnect_permit)?;

        // Wait for both sides to authenticate
        let owner = self.owner.as_mut().unwrap();
        owner.wait_authenticated(MOCK_TIMEOUT).await?;
        let node = self.node.as_mut().unwrap();
        node.wait_authenticated(MOCK_TIMEOUT).await?;

        info!("Owner <-> Node reconnected and authenticated");
        Ok(())
    }

    /// Disconnect a viewer from node
    pub async fn disconnect_viewer_from_node(&mut self, index: usize) -> Result<()> {
        let node = self.node.as_ref().expect("Scenario has no node");
        let viewer = &self.viewers[index];
        viewer.disconnect_from(node)?;
        tokio::time::sleep(Duration::from_millis(100)).await;
        Ok(())
    }

    /// Reconnect a viewer to node after disconnect
    ///
    /// **Context**: Viewer needs a fresh connection string (node issues new permit each time).
    /// After reconnect, viewer requests space again to re-establish subscriptions.
    pub async fn reconnect_viewer_to_node(&mut self, index: usize, page_id: &str) -> Result<()> {
        use butler::ConnectionStringExt;

        let space_id = self.space_info.as_ref()
            .map(|s| s.space_id.clone())
            .ok_or_else(|| anyhow::anyhow!("No space_info for viewer reconnection"))?;

        // Get fresh viewer link
        let conn_string = self.get_viewer_link(&space_id).await?;

        let node = self.node.as_ref().expect("Scenario has no node");
        let viewer = &self.viewers[index];

        let conn = viewer.butler.nodes()
            .parse_connection_string(&conn_string)
            .map_err(|e| anyhow::anyhow!("parse_connection_string failed: {}", e))?;
        let permit = conn.permit.clone();
        let space_id = conn.space_id()
            .map_err(|e| anyhow::anyhow!("space_id from permit failed: {}", e))?;

        // Fresh connection + handshake
        viewer.connect_to(node)?;
        tokio::time::sleep(Duration::from_millis(10)).await;
        viewer.handshake(node.node_id, permit.clone())?;

        let viewer = self.viewer_mut(index);
        viewer.wait_authenticated(MOCK_TIMEOUT).await?;

        // Request space to re-establish subscriptions
        let viewer = &self.viewers[index];
        let (tx, rx) = tokio::sync::oneshot::channel();
        viewer.coordinator
            .cast(CoordinatorMessage::GetPeerActor {
                node_id: self.node.as_ref().unwrap().node_id,
                response: tx,
            })
            .map_err(|e| anyhow::anyhow!("GetPeerActor failed: {:?}", e))?;

        let peer_actor = rx.await
            .map_err(|_| anyhow::anyhow!("GetPeerActor channel closed"))?
            .ok_or_else(|| anyhow::anyhow!("No PeerActor for node on viewer"))?;

        peer_actor
            .cast(PeerMessage::RequestSpace { space_id, viewer_permit: permit })
            .map_err(|e| anyhow::anyhow!("RequestSpace failed: {:?}", e))?;

        // Wait for page data
        self.wait_for_page(&self.viewers[index], page_id).await?;
        info!("Viewer {} reconnected and has page {}", index, page_id);

        Ok(())
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
    }
}
