//! Handshake protocol tests
//!
//! - INV-H1: Owner first connection succeeds with valid permit
//! - INV-H2: Owner reconnection uses upgraded permit (not first_connection)
//! - INV-H3: Invalid permit is rejected

use anyhow::Result;

use crate::fixtures::init_tracing;
use crate::scenario::Scenario;

/// Owner first connection — handshake completes, OwnerInfo stored on node
#[tokio::test]
async fn test_owner_first_connection() -> Result<()> {
    init_tracing();

    let mut s = Scenario::builder()
        .owner()
        .node()
        .connected()
        .with_tracer()
        .build()
        .await?;

    // Verify OwnerInfo stored on node
    let owner_info = s
        .node()
        .butler
        .nodes()
        .get_owner()?
        .expect("Node should have owner info");
    assert_eq!(owner_info.username, "owner");

    // Verify protocol message sequence
    s.tracer()
        .assert_contains_sequence(&["Hello", "Welcome", "PermitGrant", "Ack"]);

    s.shutdown().await;
    Ok(())
}

/// Owner reconnection — permit is upgraded (not first_connection)
#[tokio::test]
async fn test_owner_reconnection_permit() -> Result<()> {
    init_tracing();

    let mut s = Scenario::builder()
        .owner()
        .node()
        .connected()
        .build()
        .await?;

    // Get permit from DB (simulates app restart)
    let node_id_str = s.node().node_id.to_string();
    let updated = s
        .owner()
        .butler
        .nodes()
        .get(&node_id_str)
        .map_err(|e| anyhow::anyhow!("get_sovereign_node: {}", e))?
        .expect("Should have sovereign node");
    let reconnect_permit = updated.permit.expect("Should have permit for reconnect");

    // Verify permit is upgraded
    let parsed = gurkha::Permit::from_token(&reconnect_permit)?;
    assert!(
        !parsed.is_first_connection(),
        "Reconnection permit should not be first_connection"
    );

    s.shutdown().await;
    Ok(())
}

/// Invalid permit is rejected — connection fails
#[tokio::test]
async fn test_invalid_permit_rejected() -> Result<()> {
    init_tracing();

    let blobs = transport::MockBlobStore::new();
    let mut owner = crate::peer::Peer::new(
        "owner",
        courier::coordinator::CourierMode::User,
        blobs.clone(),
        None,
    )
    .await?;
    let node = crate::peer::Peer::new("node", courier::coordinator::CourierMode::Node, blobs, None)
        .await?;

    owner.connect_to(&node)?;
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;

    owner.handshake(node.node_id, "invalid_garbage_permit".to_string())?;

    let result = owner
        .wait_authenticated(crate::fixtures::MOCK_TIMEOUT)
        .await;
    assert!(result.is_err(), "Invalid permit should fail authentication");

    owner.shutdown().await;
    node.shutdown().await;

    Ok(())
}
