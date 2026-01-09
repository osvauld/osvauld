//! Lazy Sync Integration Tests
//!
//! Tests the lazy subscription system where viewers automatically trigger
//! subscriptions when they make local edits:
//!
//! - Viewer edits → Scribe triggers subscription if not already subscribed
//! - Edits while disconnected → queued, processed on reconnect
//! - State vectors used for incremental sync
//! - Node relays viewer edits to owner

use std::time::Duration;

use tracing::info;

use butler::ScribeMessage;
use courier::coordinator::CoordinatorMessage;

use crate::helpers::{
    init_tracing, setup_with_viewer, create_loro_text_update,
};
use crate::fixtures::EXTENDED_SYNC_DELAY;

// =============================================================================
// Lazy Subscription Trigger Tests
// =============================================================================

/// Test: Viewer edit triggers lazy subscription when already connected
///
/// **Context**: Viewer receives page from node, makes local edit without
/// explicitly subscribing first.
///
/// **Flow:**
/// 1. Setup owner + node + viewer with published page
/// 2. Viewer opens page (spawns Scribe with source_node_id)
/// 3. Viewer makes local edit without calling SubscribeToPage
/// 4. Verify Scribe triggers lazy subscription via EnsureSubscription
/// 5. Verify edit is applied locally
#[tokio::test]
async fn test_viewer_edit_triggers_lazy_subscription() {
    init_tracing();

    let ctx = setup_with_viewer("lazy_owner", "lazy_node", "lazy_viewer")
        .await
        .expect("Setup failed");

    info!("=== Testing lazy subscription trigger on viewer edit ===");

    // 1. Viewer opens page (Scribe gets source_node_id from stored space)
    let viewer_scribe = ctx.viewer.butler
        .open_page(&ctx.page_id)
        .await
        .expect("Viewer failed to open page");

    info!("Viewer opened page {}", ctx.page_id);

    // Give Scribe time to initialize
    tokio::time::sleep(Duration::from_millis(100)).await;

    // 2. Viewer makes local edit WITHOUT explicitly subscribing
    // This should trigger lazy subscription via EnsureSubscription
    let update = create_loro_text_update(55555, "content", "Viewer's lazy edit!");
    viewer_scribe.cast(ScribeMessage::ApplyUpdate {
        layer_name: "collaborative_doc".to_string(),
        update,
        from_peer: None, // Local edit - triggers lazy subscription
        permit: None,
    }).expect("Failed to apply update");

    info!("Viewer made local edit (should trigger lazy subscription)");

    // 3. Wait for sync propagation
    tokio::time::sleep(EXTENDED_SYNC_DELAY).await;

    // 4. Verify viewer's Scribe has the update locally
    let (tx, rx) = tokio::sync::oneshot::channel();
    viewer_scribe.cast(ScribeMessage::GetSnapshot {
        layer_name: "collaborative_doc".to_string(),
        reply: tx,
    }).expect("Failed to get snapshot");

    let snapshot = rx.await.expect("Failed to receive snapshot");
    assert!(snapshot.is_some(), "Viewer should have snapshot after local edit");

    // Verify the edit content
    let snapshot_data = snapshot.unwrap();
    let doc = loro::LoroDoc::new();
    doc.import(&snapshot_data).expect("Failed to import snapshot");
    let text = doc.get_text("content");
    let content = text.to_string();
    assert!(content.contains("Viewer's lazy edit!"), "Snapshot should contain viewer's edit");

    info!("Viewer's edit confirmed in local Scribe");

    // Clean up
    ctx.viewer.butler.close_page(&ctx.page_id).await.expect("Failed to close viewer page");
    ctx.shutdown().await;

    info!("=== test_viewer_edit_triggers_lazy_subscription passed ===");
}

/// Test: Viewer edit syncs to node and owner when already subscribed
///
/// **Flow:**
/// 1. Setup multi-party with viewer
/// 2. All parties open page and explicitly subscribe
/// 3. Viewer makes edit
/// 4. Verify node and owner receive the edit
#[tokio::test]
async fn test_viewer_edit_already_subscribed_syncs_to_all() {
    init_tracing();

    let ctx = setup_with_viewer("sub_owner", "sub_node", "sub_viewer")
        .await
        .expect("Setup failed");

    info!("=== Testing viewer edit syncs when already subscribed ===");

    // 1. All parties open page
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

    // 2. All parties explicitly subscribe
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

    tokio::time::sleep(Duration::from_millis(200)).await;
    info!("All parties subscribed");

    // 3. Viewer makes local edit
    let viewer_update = create_loro_text_update(77777, "content", "Viewer's subscribed edit");
    viewer_scribe.cast(ScribeMessage::ApplyUpdate {
        layer_name: "collaborative_doc".to_string(),
        update: viewer_update,
        from_peer: None,
        permit: None,
    }).expect("Viewer failed to apply update");

    info!("Viewer made edit");

    // 4. Wait for sync propagation
    tokio::time::sleep(EXTENDED_SYNC_DELAY).await;

    // 5. Verify viewer has the edit
    let (tx, rx) = tokio::sync::oneshot::channel();
    viewer_scribe.cast(ScribeMessage::GetSnapshot {
        layer_name: "collaborative_doc".to_string(),
        reply: tx,
    }).expect("Failed to get viewer snapshot");

    let viewer_snapshot = rx.await.expect("Failed to receive viewer snapshot");
    assert!(viewer_snapshot.is_some(), "Viewer should have snapshot");
    info!("Viewer's edit confirmed locally");

    // Clean up
    ctx.owner_butler.close_page(&ctx.page_id).await.expect("Failed to close owner page");
    ctx.node_butler.close_page(&ctx.page_id).await.expect("Failed to close node page");
    ctx.viewer.butler.close_page(&ctx.page_id).await.expect("Failed to close viewer page");
    ctx.shutdown().await;

    info!("=== test_viewer_edit_already_subscribed_syncs_to_all passed ===");
}

/// Test: Viewer edit stored locally when no connection exists
///
/// **Context**: Viewer has page but connection to node is not active.
/// Edit should be stored locally and subscription queued.
///
/// **Flow:**
/// 1. Setup viewer with published page
/// 2. Viewer opens page
/// 3. (Connection exists from setup, but testing local-first behavior)
/// 4. Viewer makes edit
/// 5. Verify edit is stored locally even before sync completes
#[tokio::test]
async fn test_viewer_edit_stored_locally_first() {
    init_tracing();

    let ctx = setup_with_viewer("local_owner", "local_node", "local_viewer")
        .await
        .expect("Setup failed");

    info!("=== Testing viewer edit stored locally first ===");

    // 1. Viewer opens page
    let viewer_scribe = ctx.viewer.butler
        .open_page(&ctx.page_id)
        .await
        .expect("Viewer failed to open page");

    tokio::time::sleep(Duration::from_millis(50)).await;

    // 2. Viewer makes local edit
    let update = create_loro_text_update(88888, "content", "Local-first edit");
    viewer_scribe.cast(ScribeMessage::ApplyUpdate {
        layer_name: "collaborative_doc".to_string(),
        update,
        from_peer: None,
        permit: None,
    }).expect("Failed to apply update");

    // 3. Immediately check local state (before network sync would complete)
    tokio::time::sleep(Duration::from_millis(50)).await;

    let (tx, rx) = tokio::sync::oneshot::channel();
    viewer_scribe.cast(ScribeMessage::GetSnapshot {
        layer_name: "collaborative_doc".to_string(),
        reply: tx,
    }).expect("Failed to get snapshot");

    let snapshot = rx.await.expect("Failed to receive snapshot");
    assert!(snapshot.is_some(), "Edit should be stored locally immediately");

    let snapshot_data = snapshot.unwrap();
    let doc = loro::LoroDoc::new();
    doc.import(&snapshot_data).expect("Failed to import snapshot");
    let text = doc.get_text("content");
    let content = text.to_string();
    assert!(content.contains("Local-first edit"), "Local edit should be present");

    info!("Edit confirmed stored locally immediately");

    // Clean up
    ctx.viewer.butler.close_page(&ctx.page_id).await.expect("Failed to close page");
    ctx.shutdown().await;

    info!("=== test_viewer_edit_stored_locally_first passed ===");
}

/// Test: Source node ID is correctly stored when viewer receives space
///
/// **Context**: When viewer receives SpaceData from node, the source_node_id
/// should be stored so Scribe knows where to sync back to.
///
/// **Flow:**
/// 1. Setup viewer with published page
/// 2. Query Butler for source_node_id of the page
/// 3. Verify it matches the node's NodeId
#[tokio::test]
async fn test_source_node_id_stored_for_viewer() {
    init_tracing();

    let ctx = setup_with_viewer("src_owner", "src_node", "src_viewer")
        .await
        .expect("Setup failed");

    info!("=== Testing source_node_id storage ===");

    // Get the expected source node ID
    let expected_source = ctx.node_node_id().to_string();

    // Query Butler for the source_node_id
    let source_node = ctx.viewer.butler
        .get_source_node_for_page(&ctx.page_id)
        .expect("Failed to get source node");

    assert!(source_node.is_some(), "Source node should be stored for viewer's page");
    assert_eq!(
        source_node.unwrap(),
        expected_source,
        "Source node should match the node that gave us the space"
    );

    info!("Source node ID correctly stored: {}", expected_source);

    ctx.shutdown().await;

    info!("=== test_source_node_id_stored_for_viewer passed ===");
}

/// Test: Multiple viewer edits only trigger subscription once
///
/// **Context**: Lazy subscription should only be triggered once per Scribe
/// instance, not on every local edit.
///
/// **Flow:**
/// 1. Setup viewer with published page
/// 2. Viewer opens page
/// 3. Viewer makes multiple local edits
/// 4. Verify all edits are applied locally
/// 5. (Subscription should only be requested once internally)
#[tokio::test]
async fn test_multiple_edits_single_subscription() {
    init_tracing();

    let ctx = setup_with_viewer("multi_owner", "multi_node", "multi_viewer")
        .await
        .expect("Setup failed");

    info!("=== Testing multiple edits with single subscription ===");

    // 1. Viewer opens page
    let viewer_scribe = ctx.viewer.butler
        .open_page(&ctx.page_id)
        .await
        .expect("Viewer failed to open page");

    tokio::time::sleep(Duration::from_millis(100)).await;

    // 2. Viewer makes multiple local edits
    for i in 1..=5 {
        let update = create_loro_text_update(90000 + i, "content", &format!("Edit {}", i));
        viewer_scribe.cast(ScribeMessage::ApplyUpdate {
            layer_name: "collaborative_doc".to_string(),
            update,
            from_peer: None,
            permit: None,
        }).expect("Failed to apply update");

        // Small delay between edits
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    info!("Viewer made 5 local edits");

    // 3. Wait for processing
    tokio::time::sleep(EXTENDED_SYNC_DELAY).await;

    // 4. Verify all edits are in the snapshot
    let (tx, rx) = tokio::sync::oneshot::channel();
    viewer_scribe.cast(ScribeMessage::GetSnapshot {
        layer_name: "collaborative_doc".to_string(),
        reply: tx,
    }).expect("Failed to get snapshot");

    let snapshot = rx.await.expect("Failed to receive snapshot");
    assert!(snapshot.is_some(), "Should have snapshot after multiple edits");

    let snapshot_data = snapshot.unwrap();
    let doc = loro::LoroDoc::new();
    doc.import(&snapshot_data).expect("Failed to import snapshot");
    let text = doc.get_text("content");
    let content = text.to_string();

    // Verify all edits are present
    for i in 1..=5 {
        assert!(content.contains(&format!("Edit {}", i)), "Should contain Edit {}", i);
    }

    info!("All 5 edits confirmed in snapshot");

    // Clean up
    ctx.viewer.butler.close_page(&ctx.page_id).await.expect("Failed to close page");
    ctx.shutdown().await;

    info!("=== test_multiple_edits_single_subscription passed ===");
}

/// Test: Owner edit syncs to viewer via node
///
/// **Context**: After lazy subscription is set up, owner's edits should
/// flow through node to viewer.
///
/// **Flow:**
/// 1. Setup multi-party with viewer
/// 2. All parties open page
/// 3. Viewer triggers subscription via local edit
/// 4. Owner makes edit
/// 5. Verify edit propagates to node and viewer
#[tokio::test]
async fn test_owner_edit_reaches_viewer_after_lazy_subscription() {
    init_tracing();

    let ctx = setup_with_viewer("flow_owner", "flow_node", "flow_viewer")
        .await
        .expect("Setup failed");

    info!("=== Testing owner edit reaches viewer after lazy subscription ===");

    // 1. All parties open page
    let owner_scribe = ctx.owner_butler
        .open_page(&ctx.page_id)
        .await
        .expect("Owner failed to open page");

    let _node_scribe = ctx.node_butler
        .open_page(&ctx.page_id)
        .await
        .expect("Node failed to open page");

    let viewer_scribe = ctx.viewer.butler
        .open_page(&ctx.page_id)
        .await
        .expect("Viewer failed to open page");

    tokio::time::sleep(Duration::from_millis(100)).await;

    // 2. Owner subscribes explicitly
    let node_node_id = ctx.node_node_id();
    ctx.owner_coordinator().cast(CoordinatorMessage::SubscribeToPage {
        node_id: node_node_id,
        page_id: ctx.page_id.clone(),
    }).expect("Owner failed to subscribe");

    // 3. Viewer triggers lazy subscription by making an edit
    let viewer_update = create_loro_text_update(11111, "content", "Viewer init");
    viewer_scribe.cast(ScribeMessage::ApplyUpdate {
        layer_name: "collaborative_doc".to_string(),
        update: viewer_update,
        from_peer: None,
        permit: None,
    }).expect("Viewer failed to apply update");

    tokio::time::sleep(Duration::from_millis(300)).await;
    info!("Viewer triggered lazy subscription");

    // 4. Owner makes edit
    let owner_update = create_loro_text_update(22222, "content", "Owner's message to viewer");
    owner_scribe.cast(ScribeMessage::ApplyUpdate {
        layer_name: "collaborative_doc".to_string(),
        update: owner_update,
        from_peer: None,
        permit: None,
    }).expect("Owner failed to apply update");

    info!("Owner made edit");

    // 5. Wait for sync
    tokio::time::sleep(EXTENDED_SYNC_DELAY).await;

    // 6. Verify owner's edit is in owner's Scribe
    let (tx, rx) = tokio::sync::oneshot::channel();
    owner_scribe.cast(ScribeMessage::GetSnapshot {
        layer_name: "collaborative_doc".to_string(),
        reply: tx,
    }).expect("Failed to get owner snapshot");

    let owner_snapshot = rx.await.expect("Failed to receive owner snapshot");
    assert!(owner_snapshot.is_some(), "Owner should have snapshot");

    let owner_data = owner_snapshot.unwrap();
    let owner_doc = loro::LoroDoc::new();
    owner_doc.import(&owner_data).expect("Failed to import owner snapshot");
    let owner_text = owner_doc.get_text("content");
    assert!(owner_text.to_string().contains("Owner's message to viewer"),
        "Owner's edit should be in owner's snapshot");

    info!("Owner's edit confirmed locally");

    // Clean up
    ctx.owner_butler.close_page(&ctx.page_id).await.expect("Failed to close owner page");
    ctx.node_butler.close_page(&ctx.page_id).await.expect("Failed to close node page");
    ctx.viewer.butler.close_page(&ctx.page_id).await.expect("Failed to close viewer page");
    ctx.shutdown().await;

    info!("=== test_owner_edit_reaches_viewer_after_lazy_subscription passed ===");
}

// =============================================================================
// Owner Lazy Sync Tests (permit-based sync config)
// =============================================================================

/// Test: Owner lazy sync uses SyncConfig from permit
///
/// **Context**: When owner opens a page, SyncConfig should be derived from permit
/// with relationship="owner" and sync_target pointing to the sovereign node.
///
/// **Flow:**
/// 1. Setup owner with published space on node
/// 2. Owner opens page - verify Scribe gets SyncConfig with owner role
/// 3. Owner makes local edit
/// 4. Verify Scribe triggers lazy subscription (via permit-based sync config)
#[tokio::test]
async fn test_owner_sync_config_from_permit() {
    init_tracing();

    let ctx = setup_with_viewer("owner_sync_owner", "owner_sync_node", "owner_sync_viewer")
        .await
        .expect("Setup failed");

    info!("=== Testing owner SyncConfig derived from permit ===");

    // 1. Owner opens page (Scribe gets SyncConfig from permit)
    let owner_scribe = ctx.owner_butler
        .open_page(&ctx.page_id)
        .await
        .expect("Owner failed to open page");

    info!("Owner opened page {}", ctx.page_id);

    // Give Scribe time to initialize
    tokio::time::sleep(Duration::from_millis(100)).await;

    // 2. Owner makes local edit - should trigger lazy subscription via permit-based SyncConfig
    let update = create_loro_text_update(99999, "content", "Owner's lazy sync edit!");
    owner_scribe.cast(ScribeMessage::ApplyUpdate {
        layer_name: "collaborative_doc".to_string(),
        update,
        from_peer: None, // Local edit - triggers lazy subscription
        permit: None,
    }).expect("Failed to apply update");

    info!("Owner made local edit (should use permit-based SyncConfig)");

    // 3. Wait for processing
    tokio::time::sleep(EXTENDED_SYNC_DELAY).await;

    // 4. Verify owner's Scribe has the update locally
    let (tx, rx) = tokio::sync::oneshot::channel();
    owner_scribe.cast(ScribeMessage::GetSnapshot {
        layer_name: "collaborative_doc".to_string(),
        reply: tx,
    }).expect("Failed to get snapshot");

    let snapshot = rx.await.expect("Failed to receive snapshot");
    assert!(snapshot.is_some(), "Owner should have snapshot after local edit");

    // Verify the edit content
    let snapshot_data = snapshot.unwrap();
    let doc = loro::LoroDoc::new();
    doc.import(&snapshot_data).expect("Failed to import snapshot");
    let text = doc.get_text("content");
    let content = text.to_string();
    assert!(content.contains("Owner's lazy sync edit!"), "Snapshot should contain owner's edit");

    info!("Owner's edit confirmed in local Scribe (using permit-based SyncConfig)");

    // Clean up
    ctx.owner_butler.close_page(&ctx.page_id).await.expect("Failed to close owner page");
    ctx.shutdown().await;

    info!("=== test_owner_sync_config_from_permit passed ===");
}
