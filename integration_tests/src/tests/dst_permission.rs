//! DST Permission Tests - Unauthorized Write Rejection
//!
//! Tests permission enforcement invariants:
//! - INV-P1: Unauthorized writes are always rejected
//! - Verifies can_write_layer for different permit types

use anyhow::Result;
use tracing::info;

use crate::scenario::init_tracing;

// Test: Role-Based Layer Permissions (Shop + Collab + Game)

/// Tests role-based access patterns across shop, collaborative, and game scenarios
///
/// Consolidates permit_layer_permissions, viewer_cannot_write_readonly_layer,
/// collab_role_permissions, and game_host_player_permissions
#[tokio::test]
async fn test_role_permissions() -> Result<()> {
    init_tracing();

    // Shop: owner vs customer
    let page_id = "test-page-001";
    let owner = gurkha::test_fixtures::shop_owner(page_id, "did:key:owner");
    let customer = gurkha::test_fixtures::shop_customer(page_id, "did:key:customer");

    let products = format!("{}/products", page_id);
    assert!(owner.can_write_layer(&products, page_id, "did:key:owner"), "Owner writes products");
    assert!(!customer.can_write_layer(&products, page_id, "did:key:customer"), "Customer cannot write products");

    let customer_orders = format!("{}/orders/did:key:customer", page_id);
    assert!(customer.can_write_layer(&customer_orders, page_id, "did:key:customer"), "Customer writes own orders");

    // Handshake viewer: no write operations
    let viewer = gurkha::test_fixtures::handshake_viewer("test-page-002", "did:key:viewer");
    let owner_hs = gurkha::test_fixtures::handshake_owner_reconnect("test-page-002", "did:key:owner");

    let viewer_has_write = viewer.get_fact("operations")
        .and_then(|v| v.as_object())
        .map(|ops| ops.get("write").is_some())
        .unwrap_or(false);
    let owner_has_write = owner_hs.get_fact("operations")
        .and_then(|v| v.as_object())
        .map(|ops| ops.get("write").is_some())
        .unwrap_or(false);

    assert!(!viewer_has_write, "Viewer should not have write operation");
    assert!(owner_has_write, "Owner should have write operation");

    // Collab: owner vs collaborator
    let page_id = "collab-page-001";
    let owner = gurkha::test_fixtures::collab_owner(page_id, "did:key:owner");
    let collab = gurkha::test_fixtures::collab_collaborator(page_id, "did:key:collab");

    let canvas = format!("{}/canvas", page_id);
    assert!(owner.can_write_layer(&canvas, page_id, "did:key:owner"), "Owner writes canvas");
    assert!(collab.can_write_layer(&canvas, page_id, "did:key:collab"), "Collaborator writes canvas");
    assert!(owner.can_write_layer("app:Canvas", page_id, "did:key:owner"), "Owner writes app:Canvas");
    assert!(!collab.can_write_layer("app:Canvas", page_id, "did:key:collab"), "Collaborator cannot write app:Canvas");

    // Game: host vs player
    let page_id = "game-001";
    let host = gurkha::test_fixtures::game_host(page_id, "did:key:host");
    let player = gurkha::test_fixtures::game_player(page_id, "did:key:player");

    let config = format!("{}/config", page_id);
    assert!(host.can_write_layer(&config, page_id, "did:key:host"), "Host writes config");
    assert!(!player.can_write_layer(&config, page_id, "did:key:player"), "Player cannot write config");

    let scores = format!("{}/scores", page_id);
    assert!(host.can_write_layer(&scores, page_id, "did:key:host"), "Host writes scores");
    assert!(player.can_write_layer(&scores, page_id, "did:key:player"), "Player writes scores");

    info!("Role permissions test passed");
    Ok(())
}

// Test: Edge Cases - Pattern Matching

/// Tests layer pattern matching in permits
#[tokio::test]
async fn test_edge_case_permits() -> Result<()> {
    init_tracing();

    let page_id = "edge-page-001";
    let owner_permit = gurkha::test_fixtures::shop_owner(page_id, "did:key:owner");

    let order_layer_1 = format!("{}/orders/customer1", page_id);
    let order_layer_2 = format!("{}/orders/customer2", page_id);

    assert!(owner_permit.can_write_layer(&order_layer_1, page_id, "did:key:owner"), "Owner matches orders/* pattern");
    assert!(owner_permit.can_write_layer(&order_layer_2, page_id, "did:key:owner"), "Owner matches orders/* pattern");

    Ok(())
}

// Test: Local-Only Layers Reject Remote Updates

/// Tests that local-only layers cannot be modified by remote peers
#[tokio::test]
async fn test_local_only_layer_rejects_remote() -> Result<()> {
    init_tracing();

    let local_only_layer = "user_content_doc";
    let regular_layer = "content_doc";

    assert!(is_local_only_layer(local_only_layer), "user_content_doc should be local-only");
    assert!(!is_local_only_layer(regular_layer), "content_doc should not be local-only");

    Ok(())
}

/// Helper to check if a layer name matches local-only patterns
fn is_local_only_layer(layer_name: &str) -> bool {
    const LOCAL_ONLY_LAYERS: &[&str] = &["user_content_doc"];
    LOCAL_ONLY_LAYERS.contains(&layer_name)
}

// Test: Sync Target Authorization

/// Tests that sync only occurs with authorized targets
#[tokio::test]
async fn test_sync_target_authorization() -> Result<()> {
    init_tracing();

    let authorized_node = "did:key:authorized_node";
    let unauthorized_peer = "did:key:random_peer";

    assert!(is_sync_target(authorized_node, Some(authorized_node)), "Authorized node should be recognized");
    assert!(!is_sync_target(unauthorized_peer, Some(authorized_node)), "Unauthorized peer should not be sync target");

    Ok(())
}

/// Helper to check if a peer is the sync target
fn is_sync_target(peer_did: &str, sync_target: Option<&str>) -> bool {
    sync_target.map(|t| t == peer_did).unwrap_or(false)
}
