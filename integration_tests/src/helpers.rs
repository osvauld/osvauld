//! Shared test helper functions for integration tests

use std::sync::Arc;

use ractor::ActorRef;
use tokio::sync::{RwLock, mpsc};
use transport::NodeId;
use tracing_subscriber::EnvFilter;

use butler::{Butler, RedbStore, LayerCache, signup, ScribeMessage, BroadcastPayload};
use courier::coordinator::{CoordinatorMessage, CourierMode};

use crate::TestHarness;
use crate::fixtures::{
    ACTOR_SPAWN_DELAY, HANDSHAKE_DELAY, EXTENDED_SYNC_DELAY,
    TEST_SPACE_TEMPLATE, TEST_PAGE_TEMPLATE, TEST_PAGE_LAYERS,
};

/// Initialize tracing once per test run.
/// Safe to call multiple times - will only initialize once.
pub fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env()
                .add_directive("info".parse().unwrap())
        )
        .try_init();
}

/// Setup Butler with identity for testing.
///
/// Creates a temporary database, signs up a user, and returns:
/// - Arc<Butler> - the butler instance with identity set
/// - [u8; 32] - the signing key bytes
/// - TempDir - must be kept alive for the duration of the test
///
/// # Example
/// ```ignore
/// let (butler, signing_key, _temp_dir) = setup_butler_with_identity("alice", "password").await?;
/// ```
pub async fn setup_butler_with_identity(
    name: &str,
    passphrase: &str,
) -> anyhow::Result<(Arc<Butler>, [u8; 32], tempfile::TempDir)> {
    let temp_dir = tempfile::tempdir()?;
    let db_path = temp_dir.path().join(format!("{}.redb", name));
    let store = Arc::new(RedbStore::open(&db_path)?);
    let layer_cache = Arc::new(RwLock::new(LayerCache::new(store.clone(), 100)));
    let butler = Arc::new(Butler::new(store.clone(), layer_cache));
    let signup_result = signup(&store, name, passphrase)?;
    butler.set_identity(signup_result.identity.clone()).await;
    let signing_key = butler.signing_key().await?;
    Ok((butler, signing_key, temp_dir))
}

// =============================================================================
// Connection and Handshake Helpers
// =============================================================================

/// Generate a connection string from node's Butler
///
/// This creates the base64-encoded JSON connection string that the owner
/// would scan from the node's QR code.
/// Note: node_id is derived from device_public_key when parsing
pub async fn generate_connection_string(
    node_butler: &Arc<Butler>,
) -> anyhow::Result<String> {
    node_butler.generate_connection_string(None)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to generate connection string: {}", e))
}

/// Complete the full handshake between owner and node
///
/// **Context**: After setup_connected_peers, this completes the handshake
/// **We do**: Get permit from sovereign node (set by add_sovereign_node), initiate handshake
/// **Production flow**: Permit comes from connection string, NOT generated here
///
/// Expects harness to have "owner" and "node" peers already connected,
/// and owner to have already called add_sovereign_node with the connection string.
pub async fn complete_handshake(
    harness: &TestHarness,
    owner_butler: &Arc<Butler>,
    node_butler: &Arc<Butler>,
) -> anyhow::Result<()> {
    let node_node_id = harness.peer("node").unwrap().node_id;

    // Get permit from sovereign node (was set when owner called add_sovereign_node)
    let sovereign = owner_butler.get_sovereign_node(&node_node_id.to_string())?
        .ok_or_else(|| anyhow::anyhow!(
            "Owner has no sovereign node for node_id={}. Did you call add_sovereign_node?",
            node_node_id
        ))?;

    let permit = sovereign.permit
        .ok_or_else(|| anyhow::anyhow!("Sovereign node has no permit"))?;

    let owner_coordinator = &harness.peer("owner").unwrap().coordinator;
    owner_coordinator.cast(CoordinatorMessage::InitiateHandshake {
        node_id: node_node_id,
        permit,
    })?;

    // Use deterministic wait instead of fixed sleep
    harness.wait_for_owner_authenticated(node_butler).await?;

    Ok(())
}

// =============================================================================
// High-Level Setup Helpers
// =============================================================================

/// Context returned from connected peer setup.
///
/// Holds all the pieces needed for multi-peer integration tests.
/// The temp directories must be kept alive for the duration of the test.
pub struct ConnectedPeers {
    pub harness: TestHarness,
    pub owner_butler: Arc<Butler>,
    pub node_butler: Arc<Butler>,
    pub owner_signing_key: [u8; 32],
    pub _owner_temp: tempfile::TempDir,
    pub _node_temp: tempfile::TempDir,
}

impl ConnectedPeers {
    /// Shutdown the harness
    pub async fn shutdown(&self) {
        self.harness.shutdown().await;
    }

    /// Get the node's NodeId
    pub fn node_id(&self) -> NodeId {
        self.harness.peer("node").unwrap().node_id
    }

    /// Get the owner's coordinator
    pub fn owner_coordinator(&self) -> &ActorRef<CoordinatorMessage> {
        &self.harness.peer("owner").unwrap().coordinator
    }
}

/// Setup connected owner and node peers (transport running, not yet handshaken)
///
/// Creates two peers with isolated storage, connects them via mock transport,
/// and returns the context for further testing.
pub async fn setup_connected_peers(
    owner_name: &str,
    node_name: &str,
) -> anyhow::Result<ConnectedPeers> {
    let (owner_butler, owner_signing_key, owner_temp) =
        setup_butler_with_identity(owner_name, "password").await?;
    let (node_butler, _node_signing_key, node_temp) =
        setup_butler_with_identity(node_name, "nodepass").await?;

    let mut harness = TestHarness::new();
    harness.add_peer_with_butler("owner", CourierMode::User, owner_butler.clone()).await?;
    harness.add_peer_with_butler("node", CourierMode::Node, node_butler.clone()).await?;

    let connection_string = generate_connection_string(&node_butler).await?;
    owner_butler.add_sovereign_node(&connection_string)
        .map_err(|e| anyhow::anyhow!("Failed to add sovereign node: {}", e))?;

    let _transport_handle = harness.spawn_transport();
    harness.connect_and_notify("owner", "node").await?;
    tokio::time::sleep(ACTOR_SPAWN_DELAY).await;

    Ok(ConnectedPeers {
        harness,
        owner_butler,
        node_butler,
        owner_signing_key,
        _owner_temp: owner_temp,
        _node_temp: node_temp,
    })
}

/// Setup with completed handshake
///
/// Builds on setup_connected_peers and completes the owner-node handshake
/// using production flow (permit from connection string).
pub async fn setup_with_handshake(
    owner_name: &str,
    node_name: &str,
) -> anyhow::Result<ConnectedPeers> {
    let peers = setup_connected_peers(owner_name, node_name).await?;
    complete_handshake(&peers.harness, &peers.owner_butler, &peers.node_butler).await?;
    Ok(peers)
}

/// Setup with published page (full end-to-end setup)
///
/// Creates connected peers, completes handshake, creates a space and page,
/// then publishes the space to the node.
///
/// Returns (peers, space_id, page_id)
pub async fn setup_with_published_page(
    owner_name: &str,
    node_name: &str,
) -> anyhow::Result<(ConnectedPeers, String, String)> {
    let peers = setup_with_handshake(owner_name, node_name).await?;

    let user_info = peers.owner_butler.user_info().await?;
    let space = peers.owner_butler
        .create_space("Test Space".to_string(), user_info.did.clone(), TEST_SPACE_TEMPLATE)
        .await?;

    let layer_names: Vec<String> = TEST_PAGE_LAYERS.iter().map(|s| s.to_string()).collect();
    let page = peers.owner_butler
        .create_page(&space.id, "Test Page", layer_names, TEST_PAGE_TEMPLATE)
        .await?;

    // Publish space (auto-syncs pages)
    let node_node_id = peers.harness.peer("node").unwrap().node_id;
    let owner_coordinator = &peers.harness.peer("owner").unwrap().coordinator;
    owner_coordinator.cast(CoordinatorMessage::PublishSpace {
        node_id: node_node_id,
        space_id: space.id.clone(),
    })?;

    tokio::time::sleep(EXTENDED_SYNC_DELAY).await;

    // Verify page is on node
    let node_page = peers.node_butler.get_page(&page.id)?;
    assert!(node_page.is_some(), "Node should have page after publish");

    Ok((peers, space.id, page.id))
}

// =============================================================================
// Scribe Test Helpers
// =============================================================================

/// Subscribe a peer to a Scribe actor using their stored permit
///
/// Returns a broadcast receiver that will receive updates from the Scribe.
pub async fn subscribe_peer_to_scribe(
    scribe: &ActorRef<ScribeMessage>,
    peer_butler: &Arc<Butler>,
    harness: &TestHarness,
    peer_name: &str,
    page_id: &str,
) -> anyhow::Result<mpsc::Receiver<BroadcastPayload>> {
    let peer_user_info = peer_butler.user_info().await?;
    let peer_device_id = harness.peer(peer_name).unwrap().node_id.to_string();
    let peer_page = peer_butler.get_page(page_id)?
        .ok_or_else(|| anyhow::anyhow!("Page not found for peer"))?;
    let peer_permit = peer_page.get_permit()
        .ok_or_else(|| anyhow::anyhow!("No permit for peer"))?
        .clone();

    let (broadcast_tx, broadcast_rx) = mpsc::channel::<BroadcastPayload>(32);
    scribe.cast(ScribeMessage::Subscribe {
        user_did: peer_user_info.did,
        device_id: peer_device_id,
        broadcast_tx,
        permit: peer_permit,
    })?;

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    Ok(broadcast_rx)
}

/// Create a Loro update with text content (helper for sync tests)
pub fn create_loro_text_update(peer_id: u64, container: &str, text: &str) -> Vec<u8> {
    use std::borrow::Cow;
    use loro::{LoroDoc, ExportMode, VersionVector};
    let doc = LoroDoc::new();
    doc.set_peer_id(peer_id).expect("Failed to set peer id");
    let text_container = doc.get_text(container);
    text_container.insert(0, text).expect("Failed to insert text");
    doc.export(ExportMode::Updates { from: Cow::Owned(VersionVector::new()) })
        .expect("Failed to export Loro update")
}

// =============================================================================
// Live Sync Test Helpers
// =============================================================================

/// Wait for a Scribe to contain expected text in a layer
///
/// **Context**: Tests need to verify sync happened without blocking indefinitely
/// **We do**: Poll Scribe via GetSnapshot until text appears or timeout
pub async fn wait_for_sync(
    scribe: &ActorRef<ScribeMessage>,
    layer_name: &str,
    expected_text: &str,
    timeout: std::time::Duration,
) -> anyhow::Result<()> {
    use loro::LoroDoc;
    use std::time::Instant;

    let start = Instant::now();
    while start.elapsed() < timeout {
        let (tx, rx) = tokio::sync::oneshot::channel();
        scribe.cast(ScribeMessage::GetSnapshot {
            layer_name: layer_name.to_string(),
            reply: tx,
        })?;

        if let Ok(Some(data)) = rx.await {
            let doc = LoroDoc::new();
            if doc.import(&data).is_ok() {
                let text_container = doc.get_text(layer_name);
                let content = text_container.to_string();
                if content.contains(expected_text) {
                    return Ok(());
                }
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    anyhow::bail!("Sync not received within timeout: expected '{}' in layer '{}'", expected_text, layer_name)
}

/// Context for a connected viewer
pub struct ViewerContext {
    pub butler: Arc<Butler>,
    pub node_id: NodeId,
    pub coordinator: ActorRef<CoordinatorMessage>,
    pub _temp: tempfile::TempDir,
}

/// Context for owner + node + viewer setup
pub struct MultiPartyContext {
    pub harness: TestHarness,
    pub owner_butler: Arc<Butler>,
    pub node_butler: Arc<Butler>,
    pub viewer: ViewerContext,
    pub owner_signing_key: [u8; 32],
    pub space_id: String,
    pub page_id: String,
    pub _owner_temp: tempfile::TempDir,
    pub _node_temp: tempfile::TempDir,
}

impl MultiPartyContext {
    pub async fn shutdown(&self) {
        self.harness.shutdown().await;
    }

    pub fn owner_node_id(&self) -> NodeId {
        self.harness.peer("owner").unwrap().node_id
    }

    pub fn node_node_id(&self) -> NodeId {
        self.harness.peer("node").unwrap().node_id
    }

    pub fn owner_coordinator(&self) -> &ActorRef<CoordinatorMessage> {
        &self.harness.peer("owner").unwrap().coordinator
    }

    pub fn node_coordinator(&self) -> &ActorRef<CoordinatorMessage> {
        &self.harness.peer("node").unwrap().coordinator
    }
}

/// Setup owner + node + viewer with published page
///
/// **Context**: Need multi-party scenario for live sync testing
/// **We do** (all production flows):
/// 1. Owner + node handshake via generate_connection_string → add_sovereign_node
/// 2. Owner creates space + page, publishes to node
/// 3. Viewer gets access via generate_viewer_connection_string → add_sovereign_node
/// 4. Viewer handshakes with node using InitiateHandshake
pub async fn setup_with_viewer(
    owner_name: &str,
    node_name: &str,
    viewer_name: &str,
) -> anyhow::Result<MultiPartyContext> {
    // Start fresh with all three peers
    let (owner_butler, owner_signing_key, owner_temp) =
        setup_butler_with_identity(owner_name, "password").await?;
    let (node_butler, _node_signing_key, node_temp) =
        setup_butler_with_identity(node_name, "nodepass").await?;
    let (viewer_butler, _viewer_signing_key, viewer_temp) =
        setup_butler_with_identity(viewer_name, "viewerpass").await?;

    let mut harness = TestHarness::new();
    harness.add_peer_with_butler("owner", CourierMode::User, owner_butler.clone()).await?;
    harness.add_peer_with_butler("node", CourierMode::Node, node_butler.clone()).await?;
    harness.add_peer_with_butler("viewer", CourierMode::User, viewer_butler.clone()).await?;

    let node_node_id = harness.peer("node").unwrap().node_id;
    let viewer_node_id = harness.peer("viewer").unwrap().node_id;

    // === PHASE 1: Owner-Node handshake (production flow) ===
    let connection_string = generate_connection_string(&node_butler).await?;
    owner_butler.add_sovereign_node(&connection_string)
        .map_err(|e| anyhow::anyhow!("Failed to add sovereign node: {}", e))?;

    let _transport_handle = harness.spawn_transport();

    // Connect owner ↔ node
    harness.connect_and_notify("owner", "node").await?;
    tokio::time::sleep(ACTOR_SPAWN_DELAY).await;

    // Complete owner ↔ node handshake (uses production flow via complete_handshake)
    complete_handshake(&harness, &owner_butler, &node_butler).await?;

    // === PHASE 2: Create and publish space/page ===
    let user_info = owner_butler.user_info().await?;
    let space = owner_butler
        .create_space("Test Space".to_string(), user_info.did.clone(), TEST_SPACE_TEMPLATE)
        .await?;

    let layer_names: Vec<String> = TEST_PAGE_LAYERS.iter().map(|s| s.to_string()).collect();
    let page = owner_butler
        .create_page(&space.id, "Test Page", layer_names, TEST_PAGE_TEMPLATE)
        .await?;

    // Publish space to node
    let owner_coordinator = &harness.peer("owner").unwrap().coordinator;
    owner_coordinator.cast(CoordinatorMessage::PublishSpace {
        node_id: node_node_id,
        space_id: space.id.clone(),
    })?;

    // Wait for page to arrive on node (deterministic)
    harness.wait_for_page(&node_butler, &page.id).await?;

    // === PHASE 3: Viewer connects via production flow ===
    // Production flow: Node generates viewer connection string → viewer parses and connects
    let viewer_conn_string = node_butler
        .generate_viewer_connection_string(&space.id, None)
        .await?;

    // Viewer parses connection string (does NOT store as sovereign node - that's for owners)
    let viewer_conn = viewer_butler.parse_connection_string(&viewer_conn_string)
        .map_err(|e| anyhow::anyhow!("Viewer failed to parse connection string: {}", e))?;

    // Get permit from parsed connection string
    let viewer_permit = viewer_conn.permit.clone();

    // Connect viewer ↔ node
    harness.connect_and_notify("viewer", "node").await?;
    tokio::time::sleep(ACTOR_SPAWN_DELAY).await;

    // Viewer requests space (production flow: RequestSpaceAsViewer includes handshake)
    let viewer_coordinator = harness.peer("viewer").unwrap().coordinator.clone();
    viewer_coordinator.cast(CoordinatorMessage::RequestSpaceAsViewer {
        node_id: node_node_id,
        space_id: space.id.clone(),
        viewer_permit,
    })?;

    // Wait for viewer to have the page (synced after RequestSpaceAsViewer)
    harness.wait_for_page(&viewer_butler, &page.id).await?;

    Ok(MultiPartyContext {
        harness,
        owner_butler,
        node_butler,
        viewer: ViewerContext {
            butler: viewer_butler,
            node_id: viewer_node_id,
            coordinator: viewer_coordinator,
            _temp: viewer_temp,
        },
        owner_signing_key,
        space_id: space.id,
        page_id: page.id,
        _owner_temp: owner_temp,
        _node_temp: node_temp,
    })
}
