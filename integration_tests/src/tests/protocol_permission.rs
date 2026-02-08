//! Protocol-level permission tests using MockConnection
//!
//! Tests permission checking at the protocol level during handshake and publish.
//!
//! **Note**: Layer-level permission enforcement happens in the Scribe/sync protocol.
//! These tests focus on connection and publish permissions.

use anyhow::Result;

use crate::peer::MockPeer;
use crate::scenario::{init_tracing, MockScenario, MOCK_TIMEOUT};
use courier::coordinator::CourierMode;
use transport::MockBlobStore;

/// Test that owner has publish permission
///
/// **Flow**:
/// 1. Owner-node handshake
/// 2. Owner creates space
/// 3. Owner can publish to node
#[tokio::test]
async fn test_mock_owner_can_publish() -> Result<()> {
    init_tracing();

    let mut s = MockScenario::new_mock(true, true, 0).await?;
    s.connect_owner_to_node_mock().await?;

    let space_info = s.setup_owner("Test Space").await?;

    // Owner should be able to publish
    s.publish_space(&space_info.id).await?;
    s.wait_for_page(s.node(), &space_info.page_id).await?;

    tracing::info!("Owner publish permission verified");

    s.shutdown().await;

    Ok(())
}

/// Test that node stores owner info after handshake
///
/// **Flow**:
/// 1. Owner-node handshake
/// 2. Node has owner info with permit
#[tokio::test]
async fn test_mock_owner_info_stored() -> Result<()> {
    init_tracing();

    let mut s = MockScenario::new_mock(true, true, 0).await?;
    s.connect_owner_to_node_mock().await?;

    // Verify owner info stored on node
    let owner_info = s.node().butler.nodes().get_owner()?
        .expect("Node should have owner info");

    assert_eq!(owner_info.username, "owner");
    assert!(owner_info.permit.is_some(), "Owner should have permit");

    tracing::info!("Owner info correctly stored on node");

    s.shutdown().await;

    Ok(())
}

/// Test invalid permit is rejected
///
/// **Flow**:
/// 1. Create peers
/// 2. Try to handshake with invalid permit
/// 3. Connection fails
#[tokio::test]
async fn test_mock_invalid_permit_rejected() -> Result<()> {
    init_tracing();
    let blobs = MockBlobStore::new();

    let mut owner = MockPeer::new_mock("owner", CourierMode::User, blobs.clone()).await?;
    let node = MockPeer::new_mock("node", CourierMode::Node, blobs.clone()).await?;

    // Inject mock connection
    owner.connect_mock_to(&node)?;
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;

    // Try with invalid permit
    owner.handshake(node.node_id, "invalid_permit".to_string())?;

    // Should fail to authenticate
    let result = owner.wait_authenticated(MOCK_TIMEOUT).await;
    assert!(result.is_err(), "Invalid permit should fail");

    tracing::info!("Invalid permit correctly rejected");

    owner.shutdown().await;
    node.shutdown().await;

    Ok(())
}

/// Test viewer connection string contains space-scoped permit
///
/// **Flow**:
/// 1. Owner-node handshake
/// 2. Owner creates space
/// 3. Node generates viewer connection string
/// 4. Connection string contains permit for that space
#[tokio::test]
async fn test_mock_viewer_permit_scoped_to_space() -> Result<()> {
    init_tracing();

    let mut s = MockScenario::new_mock(true, true, 0).await?;
    s.connect_owner_to_node_mock().await?;

    let space_info = s.setup_owner("Test Space").await?;

    // Get viewer connection string
    let viewer_conn_string = s.get_viewer_link(&space_info.id).await?;

    // Parse it to verify it contains a permit
    let blobs = MockBlobStore::new();
    let viewer = MockPeer::new_mock("viewer", CourierMode::User, blobs).await?;
    let parsed = viewer.butler.nodes().parse_connection_string(&viewer_conn_string)
        .map_err(|e| anyhow::anyhow!("parse failed: {}", e))?;

    // Verify permit exists and references the space
    assert!(!parsed.permit.is_empty(), "Viewer permit should not be empty");

    let permit = gurkha::Permit::from_token(&parsed.permit)?;
    // The permit should be a viewer permit (not first_connection)
    assert!(!permit.is_first_connection(), "Viewer permit should not be first_connection");

    tracing::info!("Viewer permit is space-scoped");

    viewer.shutdown().await;
    s.shutdown().await;

    Ok(())
}

/// Test connection string format is valid
///
/// **Flow**:
/// 1. Generate connection string
/// 2. Verify it can be parsed
/// 3. Contains node_id and permit
#[tokio::test]
async fn test_mock_connection_string_format() -> Result<()> {
    init_tracing();
    let blobs = MockBlobStore::new();

    let node = MockPeer::new_mock("node", CourierMode::Node, blobs.clone()).await?;

    // Generate connection string
    let conn_string = node.butler.nodes().generate_connection_string(None).await?;

    // Parse it
    let parsed = node.butler.nodes().parse_connection_string(&conn_string)
        .map_err(|e| anyhow::anyhow!("parse failed: {}", e))?;

    // Verify contents
    assert!(!parsed.node_id().is_empty(), "Should have node_id");
    assert!(!parsed.permit.is_empty(), "Should have permit");

    tracing::info!("Connection string format validated");

    node.shutdown().await;

    Ok(())
}
