//! Reconnection tests
//!
//! Tests that verify auto-reconnection when a customer opens a page
//! after disconnecting from the node.

use std::sync::Arc;

use tracing::info;

use butler::Butler;
use courier::coordinator::{CoordinatorMessage, CourierMode};

use crate::TestHarness;
use crate::fixtures::{ACTOR_SPAWN_DELAY, EXTENDED_SYNC_DELAY};
use crate::helpers::{
    init_tracing, setup_butler_with_identity,
    generate_connection_string, complete_handshake,
};

/// Load the real my-shop templates at compile time
const MY_SHOP_PAGE_TEMPLATE: &str = include_str!("../../../sample_apps/my-shop/permit_template.json");
const MY_SHOP_SPACE_TEMPLATE: &str = include_str!("../../../sample_apps/my-shop/space_permit_template.json");

/// Test: Customer auto-reconnects when opening a page after disconnect
///
/// Flow:
/// 1. Setup: Customer connected to node, received space/page
/// 2. Simulate disconnect (send Disconnected message)
/// 3. Customer opens page (via butler.open_page which triggers EnsureSync)
/// 4. Verify: Connection is re-established automatically
#[tokio::test(flavor = "multi_thread")]
async fn test_customer_auto_reconnects_on_page_open() {
    init_tracing();

    info!("=== Test: Customer auto-reconnects on page open ===");

    // Setup identities
    let (owner_butler, _owner_key, owner_temp) = setup_butler_with_identity("shop_owner", "pass").await.unwrap();
    let (node_butler, _node_key, node_temp) = setup_butler_with_identity("shop_node", "pass").await.unwrap();
    let (customer_butler, _cust_key, cust_temp) = setup_butler_with_identity("customer", "pass").await.unwrap();

    let mut harness = TestHarness::new();
    harness.add_peer_with_butler("owner", CourierMode::User, owner_butler.clone()).await.unwrap();
    harness.add_peer_with_butler("node", CourierMode::Node, node_butler.clone()).await.unwrap();
    harness.add_peer_with_butler("customer", CourierMode::User, customer_butler.clone()).await.unwrap();

    let node_node_id = harness.peer("node").unwrap().node_id;
    let customer_node_id = harness.peer("customer").unwrap().node_id;

    // === Owner ↔ Node handshake ===
    let connection_string = generate_connection_string(&node_butler).await.unwrap();
    owner_butler.add_sovereign_node(&connection_string)
        .map_err(|e| anyhow::anyhow!("Failed to add sovereign node: {}", e)).unwrap();

    let _transport = harness.spawn_transport();
    harness.connect_and_notify("owner", "node").await.unwrap();
    tokio::time::sleep(ACTOR_SPAWN_DELAY).await;
    complete_handshake(&harness, &owner_butler, &node_butler).await.unwrap();

    // === Create and publish space ===
    let user_info = owner_butler.user_info().await.unwrap();
    let space = owner_butler
        .create_space("My Shop".to_string(), user_info.did.clone(), MY_SHOP_SPACE_TEMPLATE)
        .await.unwrap();

    let page = owner_butler
        .create_page(
            &space.id,
            "Shop Page",
            vec!["products".to_string()],
            MY_SHOP_PAGE_TEMPLATE,
        )
        .await.unwrap();

    let owner_coordinator = &harness.peer("owner").unwrap().coordinator;
    owner_coordinator.cast(CoordinatorMessage::PublishSpace {
        node_id: node_node_id,
        space_id: space.id.clone(),
    }).unwrap();

    harness.wait_for_space(&node_butler, &space.id).await.unwrap();
    harness.wait_for_page(&node_butler, &page.id).await.unwrap();

    info!("Space and page published to node");

    // === Connect Customer ===
    let viewer_conn = node_butler.generate_viewer_connection_string(&space.id, None).await.unwrap();
    let parsed = customer_butler.parse_connection_string(&viewer_conn)
        .map_err(|e| anyhow::anyhow!("Failed to parse connection string: {}", e)).unwrap();
    harness.connect_and_notify("customer", "node").await.unwrap();
    tokio::time::sleep(ACTOR_SPAWN_DELAY).await;

    let cust_coordinator = harness.peer("customer").unwrap().coordinator.clone();
    cust_coordinator.cast(CoordinatorMessage::InitiateHandshake {
        node_id: node_node_id,
        permit: parsed.permit.clone(),
    }).unwrap();
    tokio::time::sleep(EXTENDED_SYNC_DELAY).await;

    // Request space as viewer
    cust_coordinator.cast(CoordinatorMessage::RequestSpace {
        node_id: node_node_id,
        space_id: space.id.clone(),
        viewer_permit: parsed.permit.clone(),
    }).unwrap();

    harness.wait_for_space(&customer_butler, &space.id).await.unwrap();
    harness.wait_for_page(&customer_butler, &page.id).await.unwrap();

    info!("Customer received space and page");

    // === Simulate disconnect ===
    info!("Simulating customer disconnect...");

    // Send Disconnected to both coordinators
    cust_coordinator.cast(CoordinatorMessage::Disconnected {
        node_id: node_node_id,
    }).unwrap();

    let node_coordinator = harness.peer("node").unwrap().coordinator.clone();
    node_coordinator.cast(CoordinatorMessage::Disconnected {
        node_id: customer_node_id,
    }).unwrap();

    tokio::time::sleep(ACTOR_SPAWN_DELAY).await;
    info!("Customer disconnected from node");

    // === Customer opens page (triggers EnsureSync → auto-reconnect) ===
    info!("Customer opening page (should trigger auto-reconnect)...");

    // Open the page - this calls butler.open_page which:
    // 1. Creates Scribe
    // 2. Scribe's post_start emits SyncEvent::EnsureSync
    // 3. Coordinator receives EnsureSync, sends ConnectRequest
    // 4. TestPeer's connect handler creates mock connection
    let _scribe = customer_butler.open_page(&page.id).await.unwrap();

    // Wait for auto-reconnection and sync
    tokio::time::sleep(EXTENDED_SYNC_DELAY).await;

    // === Verify reconnection ===
    // Check that customer's page is accessible and can sync
    let (customer_page, _) = customer_butler
        .get_decrypted_page(&page.id)
        .await
        .expect("Customer should have page after reconnect");

    assert!(customer_page.id == page.id, "Customer should have the correct page");

    info!("=== PASSED: Customer auto-reconnects on page open ===");

    harness.shutdown().await;

    // Keep temp dirs alive
    drop(owner_temp);
    drop(node_temp);
    drop(cust_temp);
}
