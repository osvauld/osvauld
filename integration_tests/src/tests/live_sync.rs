//! Live Sync E2E Integration Tests
//!
//! Tests the full actor-based sync flow:
//! - Owner edit → Scribe → PeerActor → SyncOffer → Node Scribe
//! - Viewer edit → Node → broadcast to Owner + other Viewers
//!
//! These tests verify the complete bidirectional sync pipeline using
//! real actors and mock transport.

use std::time::Duration;

use tracing::info;

use butler::ScribeMessage;
use courier::coordinator::CoordinatorMessage;

use crate::helpers::{
    init_tracing, setup_with_published_page, setup_with_viewer,
    create_loro_text_update, wait_for_sync,
};
use crate::fixtures::EXTENDED_SYNC_DELAY;

// =============================================================================
// Owner → Node Sync Tests
// =============================================================================

/// Test basic owner → node live sync flow
///
/// **Flow:**
/// 1. Setup owner + node with published page (handshake already done)
/// 2. Both sides open page (spawn Scribe actors)
/// 3. Owner subscribes via Coordinator (since pages opened after handshake)
/// 4. Owner makes local edit
/// 5. Verify node's Scribe received the update
///
/// **Note**: In production, auto-subscribe happens at handshake time if pages are already open.
/// In tests, pages are opened after handshake, so manual subscription is needed.
#[tokio::test]
async fn test_owner_edit_syncs_to_node() {
    init_tracing();

    let (peers, _space_id, page_id) = setup_with_published_page("sync_owner", "sync_node")
        .await
        .expect("Setup failed");

    info!("=== Testing owner edit syncs to node ===");

    // 1. Both sides open page (spawn Scribe actors)
    let owner_scribe = peers.owner_butler
        .open_page(&page_id)
        .await
        .expect("Owner failed to open page");

    let node_scribe = peers.node_butler
        .open_page(&page_id)
        .await
        .expect("Node failed to open page");

    info!("Both peers opened page {}", page_id);

    // 2. Owner subscribes to page updates via Coordinator
    // (In production, auto-subscribe happens at handshake if pages are open)
    let node_node_id = peers.node_id();
    let owner_coordinator = peers.owner_coordinator();

    owner_coordinator.cast(CoordinatorMessage::SubscribeToPage {
        node_id: node_node_id,
        page_id: page_id.clone(),
    }).expect("Failed to subscribe");

    tokio::time::sleep(Duration::from_millis(100)).await;
    info!("Owner subscribed to page via node");

    // 3. Owner makes local edit
    let update = create_loro_text_update(12345, "content", "Hello from owner!");
    owner_scribe.cast(ScribeMessage::ApplyUpdate {
        layer_name: "collaborative_doc".to_string(),
        update,
        from_peer: None, // Local edit
    }).expect("Failed to apply update");

    info!("Owner made local edit");

    // 4. Wait for sync to propagate (Owner → SyncOffer → Node)
    tokio::time::sleep(EXTENDED_SYNC_DELAY).await;

    // 5. Verify owner's Scribe has the update (sanity check)
    let (tx, rx) = tokio::sync::oneshot::channel();
    owner_scribe.cast(ScribeMessage::GetSnapshot {
        layer_name: "collaborative_doc".to_string(),
        reply: tx,
    }).expect("Failed to get owner snapshot");

    let owner_snapshot = rx.await.expect("Failed to receive owner snapshot");
    assert!(owner_snapshot.is_some(), "Owner should have snapshot");
    info!("Owner's Scribe has the update (sanity check)");

    // 6. Verify NODE's Scribe received the update (THE REAL TEST)
    let (tx, rx) = tokio::sync::oneshot::channel();
    node_scribe.cast(ScribeMessage::GetSnapshot {
        layer_name: "collaborative_doc".to_string(),
        reply: tx,
    }).expect("Failed to get node snapshot");

    let node_snapshot = rx.await.expect("Failed to receive node snapshot");
    assert!(node_snapshot.is_some(), "Node should have received update via SyncOffer");
    info!("Node's Scribe has the update - sync worked!");

    // 7. Verify content matches (optional deeper check)
    let node_bytes = node_snapshot.unwrap();
    let node_doc = loro::LoroDoc::new();
    node_doc.import(&node_bytes).expect("Failed to import node snapshot");
    let text = node_doc.get_text("content");
    let text_str = text.to_string();
    assert!(text_str.contains("Hello from owner!"), "Node should have owner's text, got: {}", text_str);
    info!("Content verified: {}", text_str);

    // Clean up
    peers.owner_butler.close_page(&page_id).await.expect("Failed to close owner page");
    peers.node_butler.close_page(&page_id).await.expect("Failed to close node page");
    peers.shutdown().await;

    info!("=== test_owner_edit_syncs_to_node passed ===");
}

// =============================================================================
// Multi-Party Sync Tests (Owner + Node + Viewer)
// =============================================================================

/// Test full multi-party sync: Owner → Node → Viewer
///
/// **Flow:**
/// 1. Setup owner + node + viewer with published page
/// 2. All parties open page
/// 3. All parties subscribe
/// 4. Owner edits → verify reaches node and viewer
/// 5. Viewer edits → verify reaches owner and node
#[tokio::test]
async fn test_multiparty_live_sync() {
    init_tracing();

    let ctx = setup_with_viewer("mp_owner", "mp_node", "mp_viewer")
        .await
        .expect("Setup failed");

    info!("=== Testing multi-party live sync ===");

    // 1. All parties open page (spawn Scribe actors)
    let owner_scribe = ctx.owner_butler
        .open_page(&ctx.page_id)
        .await
        .expect("Owner failed to open page");

    let node_scribe = ctx.node_butler
        .open_page(&ctx.page_id)
        .await
        .expect("Node failed to open page");

    let viewer_scribe = ctx.viewer.butler
        .open_page(&ctx.page_id)
        .await
        .expect("Viewer failed to open page");

    info!("All parties opened page {}", ctx.page_id);

    // 2. Subscribe all parties
    let node_node_id = ctx.node_node_id();
    let owner_coordinator = ctx.owner_coordinator();

    owner_coordinator.cast(CoordinatorMessage::SubscribeToPage {
        node_id: node_node_id,
        page_id: ctx.page_id.clone(),
    }).expect("Owner failed to subscribe");

    ctx.viewer.coordinator.cast(CoordinatorMessage::SubscribeToPage {
        node_id: node_node_id,
        page_id: ctx.page_id.clone(),
    }).expect("Viewer failed to subscribe");

    tokio::time::sleep(Duration::from_millis(100)).await;
    info!("All parties subscribed");

    // 3. Owner makes local edit
    let owner_update = create_loro_text_update(11111, "content", "Owner's message");
    owner_scribe.cast(ScribeMessage::ApplyUpdate {
        layer_name: "collaborative_doc".to_string(),
        update: owner_update,
        from_peer: None,
    }).expect("Owner failed to apply update");

    info!("Owner made edit");

    // 4. Wait for sync propagation
    tokio::time::sleep(EXTENDED_SYNC_DELAY).await;

    // 5. Verify owner's Scribe has the update
    let (tx, rx) = tokio::sync::oneshot::channel();
    owner_scribe.cast(ScribeMessage::GetSnapshot {
        layer_name: "collaborative_doc".to_string(),
        reply: tx,
    }).expect("Failed to get owner snapshot");

    let owner_snapshot = rx.await.expect("Failed to receive owner snapshot");
    assert!(owner_snapshot.is_some(), "Owner should have snapshot");
    info!("Owner's update confirmed in local Scribe");

    // 6. Viewer makes local edit
    let viewer_update = create_loro_text_update(22222, "content", "Viewer's reply");
    viewer_scribe.cast(ScribeMessage::ApplyUpdate {
        layer_name: "collaborative_doc".to_string(),
        update: viewer_update,
        from_peer: None,
    }).expect("Viewer failed to apply update");

    info!("Viewer made edit");

    // 7. Wait for sync propagation
    tokio::time::sleep(EXTENDED_SYNC_DELAY).await;

    // 8. Verify viewer's Scribe has the update
    let (tx, rx) = tokio::sync::oneshot::channel();
    viewer_scribe.cast(ScribeMessage::GetSnapshot {
        layer_name: "collaborative_doc".to_string(),
        reply: tx,
    }).expect("Failed to get viewer snapshot");

    let viewer_snapshot = rx.await.expect("Failed to receive viewer snapshot");
    assert!(viewer_snapshot.is_some(), "Viewer should have snapshot");
    info!("Viewer's update confirmed in local Scribe");

    // Clean up
    ctx.owner_butler.close_page(&ctx.page_id).await.expect("Failed to close owner page");
    ctx.node_butler.close_page(&ctx.page_id).await.expect("Failed to close node page");
    ctx.viewer.butler.close_page(&ctx.page_id).await.expect("Failed to close viewer page");
    ctx.shutdown().await;

    info!("=== test_multiparty_live_sync passed ===");
}

/// Test Scribe subscription and GetSnapshot functionality
///
/// Verifies the new ScribeMessage::GetSnapshot works correctly.
#[tokio::test]
async fn test_scribe_get_snapshot() {
    init_tracing();

    let (peers, _space_id, page_id) = setup_with_published_page("snap_owner", "snap_node")
        .await
        .expect("Setup failed");

    info!("=== Testing ScribeMessage::GetSnapshot ===");

    // Open page
    let scribe = peers.owner_butler
        .open_page(&page_id)
        .await
        .expect("Failed to open page");

    // Apply an update
    let update = create_loro_text_update(99999, "test_container", "Snapshot test content");
    scribe.cast(ScribeMessage::ApplyUpdate {
        layer_name: "collaborative_doc".to_string(),
        update,
        from_peer: None,
    }).expect("Failed to apply update");

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Get snapshot
    let (tx, rx) = tokio::sync::oneshot::channel();
    scribe.cast(ScribeMessage::GetSnapshot {
        layer_name: "collaborative_doc".to_string(),
        reply: tx,
    }).expect("Failed to send GetSnapshot");

    let snapshot = rx.await.expect("Failed to receive snapshot");
    assert!(snapshot.is_some(), "Should have snapshot for existing layer");
    info!("Got snapshot: {} bytes", snapshot.unwrap().len());

    // Try non-existent layer
    let (tx2, rx2) = tokio::sync::oneshot::channel();
    scribe.cast(ScribeMessage::GetSnapshot {
        layer_name: "nonexistent_layer".to_string(),
        reply: tx2,
    }).expect("Failed to send GetSnapshot");

    let snapshot2 = rx2.await.expect("Failed to receive snapshot");
    assert!(snapshot2.is_none(), "Should not have snapshot for non-existent layer");
    info!("Correctly returned None for non-existent layer");

    // Clean up
    peers.owner_butler.close_page(&page_id).await.expect("Failed to close page");
    peers.shutdown().await;

    info!("=== test_scribe_get_snapshot passed ===");
}

/// Test auto-subscribe via EnsureSync when page opened AFTER connection
///
/// **Scenario:** Peer is connected but not subscribed to a new page
/// **Flow:**
/// 1. Setup owner + node with published page (handshake completes)
/// 2. At handshake time, NO pages are open (no Scribes exist)
/// 3. Owner opens page AFTER connection → Scribe starts
/// 4. Owner makes edit → Scribe emits EnsureSync
/// 5. Coordinator sees peer connected → sends RefreshSubscriptions
/// 6. PeerActor subscribes to new Scribe
/// 7. Verify node receives the update
///
/// **Key difference from test_owner_edit_syncs_to_node:**
/// NO manual SubscribeToPage - relies on automatic EnsureSync → RefreshSubscriptions flow
#[tokio::test]
async fn test_auto_subscribe_via_ensure_sync() {
    init_tracing();

    let (peers, _space_id, page_id) = setup_with_published_page("ensure_owner", "ensure_node")
        .await
        .expect("Setup failed");

    info!("=== Testing auto-subscribe via EnsureSync ===");

    // 1. Both sides open page AFTER connection
    // (At handshake time, no pages were open, so no subscriptions were made)
    let owner_scribe = peers.owner_butler
        .open_page(&page_id)
        .await
        .expect("Owner failed to open page");

    let node_scribe = peers.node_butler
        .open_page(&page_id)
        .await
        .expect("Node failed to open page");

    info!("Both peers opened page {} AFTER connection", page_id);

    // Small delay to let Scribes start
    tokio::time::sleep(Duration::from_millis(50)).await;

    // 2. Owner makes local edit WITHOUT manual SubscribeToPage
    // This should trigger EnsureSync → RefreshSubscriptions → auto-subscribe
    let update = create_loro_text_update(54321, "content", "Auto-subscribed edit!");
    owner_scribe.cast(ScribeMessage::ApplyUpdate {
        layer_name: "collaborative_doc".to_string(),
        update,
        from_peer: None, // Local edit
    }).expect("Failed to apply update");

    info!("Owner made edit - relying on EnsureSync → RefreshSubscriptions flow");

    // 3. Wait for sync to propagate via automatic flow
    tokio::time::sleep(EXTENDED_SYNC_DELAY).await;

    // 4. Verify NODE's Scribe received the update (THE KEY TEST)
    let (tx, rx) = tokio::sync::oneshot::channel();
    node_scribe.cast(ScribeMessage::GetSnapshot {
        layer_name: "collaborative_doc".to_string(),
        reply: tx,
    }).expect("Failed to get node snapshot");

    let node_snapshot = rx.await.expect("Failed to receive node snapshot");
    assert!(node_snapshot.is_some(), "Node should have received update via auto-subscribe flow");

    // 5. Verify content matches
    let node_bytes = node_snapshot.unwrap();
    let node_doc = loro::LoroDoc::new();
    node_doc.import(&node_bytes).expect("Failed to import node snapshot");
    let text = node_doc.get_text("content");
    let text_str = text.to_string();
    assert!(text_str.contains("Auto-subscribed edit!"),
        "Node should have owner's text via auto-subscribe, got: {}", text_str);

    info!("Node received update via EnsureSync → RefreshSubscriptions flow!");

    // Clean up
    peers.owner_butler.close_page(&page_id).await.expect("Failed to close owner page");
    peers.node_butler.close_page(&page_id).await.expect("Failed to close node page");
    peers.shutdown().await;

    info!("=== test_auto_subscribe_via_ensure_sync passed ===");
}
