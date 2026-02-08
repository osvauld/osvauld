//! Protocol handshake tests (Real Network - IrohConnection)
//!
//! Tests the handshake flows using real Iroh transport.
//! For fast protocol tests, see protocol_handshake.rs (MockConnection).

use std::time::Duration;

use anyhow::Result;

use crate::peer::IrohPeer;
use crate::scenario::{init_tracing, HANDSHAKE_TIMEOUT};
use courier::coordinator::CourierMode;
use transport::MockBlobStore;

/// Test owner first connection handshake (real network)
///
/// **Flow**:
/// 1. Owner and node connect via Iroh transport
/// 2. Handshake with first_connection permit
/// 3. Both sides authenticate
#[tokio::test]
async fn test_owner_first_connection() -> Result<()> {
    init_tracing();
    let blobs = MockBlobStore::new();

    let mut owner = IrohPeer::new("owner", CourierMode::User, blobs.clone()).await?;
    let mut node = IrohPeer::new("node", CourierMode::Node, blobs.clone()).await?;

    tokio::time::sleep(Duration::from_secs(2)).await;

    let conn_string = node.butler.nodes().generate_connection_string(None).await?;
    let sovereign = owner.butler.nodes().add(&conn_string)
        .map_err(|e| anyhow::anyhow!("add_sovereign_node failed: {}", e))?;
    let permit = sovereign.permit.expect("Connection string should have permit");

    owner.connect_to(&node).await?;
    tokio::time::sleep(Duration::from_millis(100)).await;
    owner.handshake(node.node_id, permit)?;

    let (_, peer_username_seen_by_owner) = owner.wait_authenticated(HANDSHAKE_TIMEOUT).await?;
    let (_, peer_username_seen_by_node) = node.wait_authenticated(HANDSHAKE_TIMEOUT).await?;

    assert_eq!(peer_username_seen_by_owner, "node");
    assert_eq!(peer_username_seen_by_node, "owner");

    let owner_info = node.butler.nodes().get_owner()?
        .expect("Node should have owner info");
    assert_eq!(owner_info.username, "owner");
    assert!(owner_info.permit.is_some());

    owner.shutdown().await;
    node.shutdown().await;

    Ok(())
}

/// Test owner reconnection permit upgrade (real network)
#[tokio::test]
async fn test_owner_reconnection() -> Result<()> {
    init_tracing();
    let blobs = MockBlobStore::new();

    let mut owner = IrohPeer::new("owner", CourierMode::User, blobs.clone()).await?;
    let mut node = IrohPeer::new("node", CourierMode::Node, blobs.clone()).await?;

    tokio::time::sleep(Duration::from_secs(2)).await;

    let conn_string = node.butler.nodes().generate_connection_string(None).await?;
    let sovereign = owner.butler.nodes().add(&conn_string)
        .map_err(|e| anyhow::anyhow!("add_sovereign_node: {}", e))?;
    let first_permit = sovereign.permit.clone().expect("Should have permit");

    owner.connect_to(&node).await?;
    tokio::time::sleep(Duration::from_millis(100)).await;
    owner.handshake(node.node_id, first_permit)?;

    owner.wait_authenticated(HANDSHAKE_TIMEOUT).await?;
    node.wait_authenticated(HANDSHAKE_TIMEOUT).await?;

    let updated_sovereign = owner.butler.nodes().get(&node.node_id.to_string())
        .map_err(|e| anyhow::anyhow!("get_sovereign_node: {}", e))?
        .expect("Should have sovereign node");
    let reconnect_permit = updated_sovereign.permit.expect("Should have permit for reconnect");

    let parsed = gurkha::Permit::from_token(&reconnect_permit)?;
    assert!(!parsed.is_first_connection(), "Reconnection permit should not be first_connection");

    owner.shutdown().await;
    node.shutdown().await;

    Ok(())
}
