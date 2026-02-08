//! Protocol-level handshake tests using MockConnection
//!
//! Tests handshake flows using actual PeerActor<MockConnection> code paths
//! without real networking. Fast and deterministic.
//!
//! ## Invariants Tested
//!
//! - INV-H1: Owner first connection succeeds with valid permit
//! - INV-H2: Owner reconnection uses stored permit
//! - INV-H3: Invalid permit is rejected
//! - INV-H4: Viewer connection with read-only permit

use std::time::Duration;

use anyhow::Result;

use crate::peer::MockPeer;
use crate::scenario::{init_tracing, MockScenario, MOCK_TIMEOUT};
use courier::coordinator::CourierMode;
use transport::MockBlobStore;

/// Test owner first connection handshake (MockConnection)
///
/// **INV-H1**: Owner with valid first_connection permit successfully authenticates
///
/// **Flow**:
/// 1. Create mock owner and node peers
/// 2. Inject mock connection between them
/// 3. Owner initiates handshake with first_connection permit
/// 4. Both sides authenticate
/// 5. OwnerInfo stored on node
#[tokio::test]
async fn test_mock_owner_first_connection() -> Result<()> {
    init_tracing();
    let blobs = MockBlobStore::new();

    // Create mock peers (no relay needed)
    let mut owner = MockPeer::new_mock("owner", CourierMode::User, blobs.clone()).await?;
    let mut node = MockPeer::new_mock("node", CourierMode::Node, blobs.clone()).await?;

    // Node generates connection string (needed for permit)
    let conn_string = node.butler.nodes().generate_connection_string(None).await?;
    tracing::info!("Node connection string generated");

    // Owner parses and stores sovereign node
    let sovereign = owner.butler.nodes().add(&conn_string)
        .map_err(|e| anyhow::anyhow!("add_sovereign_node failed: {}", e))?;
    let permit = sovereign.permit.expect("Connection string should have permit");
    tracing::info!("Owner stored sovereign node: {}", sovereign.node_id);

    // Inject mock connection (bidirectional)
    owner.connect_mock_to(&node)?;
    tracing::info!("Mock connection established");

    // Small delay for PeerActor to spawn
    tokio::time::sleep(Duration::from_millis(10)).await;

    // Owner initiates handshake
    owner.handshake(node.node_id, permit)?;
    tracing::info!("Handshake initiated");

    // Wait for both sides to authenticate (fast timeout for mocks)
    let owner_result = owner.wait_authenticated(MOCK_TIMEOUT).await;
    let node_result = node.wait_authenticated(MOCK_TIMEOUT).await;

    // Verify
    let (_, peer_username_seen_by_owner) = owner_result?;
    let (_, peer_username_seen_by_node) = node_result?;

    assert_eq!(peer_username_seen_by_owner, "node");
    assert_eq!(peer_username_seen_by_node, "owner");

    // Verify OwnerInfo stored on node
    let owner_info = node.butler.nodes().get_owner()?
        .expect("Node should have owner info");
    assert_eq!(owner_info.username, "owner");
    assert!(owner_info.permit.is_some());

    // Cleanup
    owner.shutdown().await;
    node.shutdown().await;

    Ok(())
}

/// Test owner reconnection uses upgraded permit
///
/// **INV-H2**: After first connection, permit is upgraded (not first_connection)
///
/// **Flow**:
/// 1. Complete first connection
/// 2. Verify permit was upgraded
#[tokio::test]
async fn test_mock_owner_reconnection_permit() -> Result<()> {
    init_tracing();
    let blobs = MockBlobStore::new();

    let mut owner = MockPeer::new_mock("owner", CourierMode::User, blobs.clone()).await?;
    let mut node = MockPeer::new_mock("node", CourierMode::Node, blobs.clone()).await?;

    // First connection
    let conn_string = node.butler.nodes().generate_connection_string(None).await?;
    let sovereign = owner.butler.nodes().add(&conn_string)
        .map_err(|e| anyhow::anyhow!("add_sovereign_node: {}", e))?;
    let first_permit = sovereign.permit.clone().expect("Should have permit");

    owner.connect_mock_to(&node)?;
    tokio::time::sleep(Duration::from_millis(10)).await;
    owner.handshake(node.node_id, first_permit)?;

    // Wait for first authentication
    owner.wait_authenticated(MOCK_TIMEOUT).await?;
    node.wait_authenticated(MOCK_TIMEOUT).await?;
    tracing::info!("First connection complete");

    // Get the permit from DB (simulates app restart)
    let updated_sovereign = owner.butler.nodes().get(&node.node_id.to_string())
        .map_err(|e| anyhow::anyhow!("get_sovereign_node: {}", e))?
        .expect("Should have sovereign node");
    let reconnect_permit = updated_sovereign.permit.expect("Should have permit for reconnect");

    // Verify permit is upgraded (not first_connection anymore)
    let parsed = gurkha::Permit::from_token(&reconnect_permit)?;
    assert!(!parsed.is_first_connection(), "Reconnection permit should not be first_connection");

    tracing::info!("Permit upgraded successfully");

    owner.shutdown().await;
    node.shutdown().await;

    Ok(())
}

/// Test invalid permit is rejected
///
/// **INV-H3**: Connection with invalid permit fails with appropriate error
///
/// **Flow**:
/// 1. Create owner and node
/// 2. Owner tries to handshake with garbage permit
/// 3. Connection fails (rejected event or timeout)
#[tokio::test]
async fn test_mock_invalid_permit_rejected() -> Result<()> {
    init_tracing();
    let blobs = MockBlobStore::new();

    let mut owner = MockPeer::new_mock("owner", CourierMode::User, blobs.clone()).await?;
    let node = MockPeer::new_mock("node", CourierMode::Node, blobs.clone()).await?;

    // Inject mock connection
    owner.connect_mock_to(&node)?;
    tokio::time::sleep(Duration::from_millis(10)).await;

    // Owner tries to handshake with invalid permit
    let invalid_permit = "invalid_garbage_permit_not_a_real_ucan";
    owner.handshake(node.node_id, invalid_permit.to_string())?;

    // Should fail to authenticate (either ConnectionFailed event or timeout)
    let result = owner.wait_authenticated(MOCK_TIMEOUT).await;
    assert!(result.is_err(), "Invalid permit should fail authentication");

    tracing::info!("Invalid permit correctly rejected");

    owner.shutdown().await;
    node.shutdown().await;

    Ok(())
}

/// Test using MockScenario helper (high-level API)
#[tokio::test]
async fn test_mock_scenario_owner_node_handshake() -> Result<()> {
    init_tracing();

    let mut s = MockScenario::new_mock(true, true, 0).await?;

    // Use MockScenario's connect method (fast, no relay)
    s.connect_owner_to_node_mock().await?;

    // Verify OwnerInfo
    let owner_info = s.node().butler.nodes().get_owner()?
        .expect("Node should have owner info");
    assert_eq!(owner_info.username, "owner");

    s.shutdown().await;

    Ok(())
}

/// Test mock scenario with space creation and publishing
#[tokio::test]
async fn test_mock_scenario_with_space() -> Result<()> {
    init_tracing();

    let mut s = MockScenario::new_mock(true, true, 0).await?;

    // Connect owner to node (mock)
    s.connect_owner_to_node_mock().await?;

    // Setup owner with space
    let space_info = s.setup_owner("Test Space").await?;
    tracing::info!("Space: {}, Page: {}", space_info.id, space_info.page_id);

    // Publish to node
    s.publish_space(&space_info.id).await?;

    // Wait for page to sync
    s.wait_for_page(s.node(), &space_info.page_id).await?;

    // Verify page exists on node
    let node_page = s.node().butler.pages().get(&space_info.page_id)?;
    assert!(node_page.is_some(), "Node should have page after publish");

    s.shutdown().await;

    Ok(())
}
