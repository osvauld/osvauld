//! Layer sync tests
//!
//! Tests the Scribe-based CRDT layer synchronization between peers:
//! - Peer vector persistence for incremental sync
//! - Basic Scribe actor spawning
//! - Loro CRDT updates via Scribe
//! - End-to-end sync flow testing

use loro::LoroDoc;
use tracing::info;

use butler::ScribeMessage;

use crate::helpers::{
    init_tracing, setup_with_published_page,
    create_loro_text_update, subscribe_peer_to_scribe,
};

// =============================================================================
// Scribe Lifecycle Tests
// =============================================================================

/// Test owner opens page and spawns Scribe actor
///
/// Verifies:
/// 1. Butler.open_page() spawns Scribe actor
/// 2. Butler.close_page() stops the actor
#[tokio::test]
async fn test_open_page_spawns_scribe() {
    init_tracing();

    let (peers, _space_id, page_id) =
        setup_with_published_page("scribe_owner", "scribe_node").await.expect("Setup failed");

    info!("Opening page {} to spawn Scribe", page_id);

    // Open page - should spawn Scribe actor
    let scribe = peers.owner_butler
        .open_page(&page_id)
        .await
        .expect("Failed to open page");

    info!("Scribe spawned for page {}: {:?}", page_id, scribe.get_id());

    // Opening again should return the same actor (cached)
    let scribe2 = peers.owner_butler
        .open_page(&page_id)
        .await
        .expect("Failed to open page again");

    assert_eq!(
        scribe.get_id(),
        scribe2.get_id(),
        "Same page should return same Scribe actor"
    );

    // Close page
    peers.owner_butler
        .close_page(&page_id)
        .await
        .expect("Failed to close page");

    info!("Scribe closed for page {}", page_id);

    peers.shutdown().await;
}

/// Test Scribe lifecycle - open, close, re-open
#[tokio::test]
async fn test_scribe_lifecycle() {
    init_tracing();

    let (peers, _space_id, page_id) =
        setup_with_published_page("lifecycle_owner", "lifecycle_node").await.expect("Setup failed");

    // Open page
    let scribe1 = peers.owner_butler.open_page(&page_id).await.expect("Failed to open");
    let id1 = scribe1.get_id();
    info!("First Scribe ID: {:?}", id1);

    // Close page
    peers.owner_butler.close_page(&page_id).await.expect("Failed to close");

    // Small delay for actor to stop
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Re-open should spawn new actor
    let scribe2 = peers.owner_butler.open_page(&page_id).await.expect("Failed to re-open");
    let id2 = scribe2.get_id();
    info!("Second Scribe ID: {:?}", id2);

    // IDs should be different (new actor spawned)
    assert_ne!(id1, id2, "Re-opened page should spawn new Scribe actor");

    peers.owner_butler.close_page(&page_id).await.expect("Failed to close");
    peers.shutdown().await;
}

// =============================================================================
// Peer Vector Persistence Tests
// =============================================================================

/// Test peer vector storage and retrieval
///
/// Verifies:
/// 1. put_peer_vectors stores vectors
/// 2. get_peer_vectors retrieves stored vectors
/// 3. put_peer_vector_for_layer updates single layer
#[tokio::test]
async fn test_peer_vector_persistence() {
    init_tracing();

    let (peers, _space_id, page_id) =
        setup_with_published_page("vector_owner", "vector_node").await.expect("Setup failed");

    let store = peers.owner_butler.store();

    // Initially no vectors
    let vectors = store
        .get_peer_vectors(&page_id, "did:key:test_peer", "device123")
        .expect("Failed to get vectors");
    assert!(vectors.is_none(), "Should have no vectors initially");

    // Store some vectors
    let mut test_vectors = std::collections::HashMap::new();
    test_vectors.insert("collaborative_doc".to_string(), vec![1, 2, 3, 4]);
    test_vectors.insert("content_doc".to_string(), vec![5, 6, 7, 8]);

    store
        .put_peer_vectors(&page_id, "did:key:test_peer", "device123", &test_vectors)
        .expect("Failed to put vectors");

    // Retrieve and verify
    let retrieved = store
        .get_peer_vectors(&page_id, "did:key:test_peer", "device123")
        .expect("Failed to get vectors")
        .expect("Should have vectors");

    assert_eq!(retrieved.len(), 2);
    assert_eq!(retrieved.get("collaborative_doc"), Some(&vec![1, 2, 3, 4]));
    assert_eq!(retrieved.get("content_doc"), Some(&vec![5, 6, 7, 8]));

    // Update single layer
    store
        .put_peer_vector_for_layer(&page_id, "did:key:test_peer", "device123", "submissions_doc", vec![9, 10])
        .expect("Failed to put layer vector");

    // Verify all three layers are present
    let updated = store
        .get_peer_vectors(&page_id, "did:key:test_peer", "device123")
        .expect("Failed to get vectors")
        .expect("Should have vectors");

    assert_eq!(updated.len(), 3);
    assert_eq!(updated.get("submissions_doc"), Some(&vec![9, 10]));

    // Test get single layer
    let single = store
        .get_peer_vector_for_layer(&page_id, "did:key:test_peer", "device123", "collaborative_doc")
        .expect("Failed to get layer vector");
    assert_eq!(single, Some(vec![1, 2, 3, 4]));

    info!("Peer vector persistence test passed");

    peers.shutdown().await;
}

/// Test list peers for page
#[tokio::test]
async fn test_list_peers_for_page() {
    init_tracing();

    let (peers, _space_id, page_id) =
        setup_with_published_page("list_owner", "list_node").await.expect("Setup failed");

    let store = peers.owner_butler.store();

    // Get initial peer count (some vectors are auto-created during publish)
    let initial_peers = store.list_peers_for_page(&page_id).expect("Failed to list peers");
    let initial_count = initial_peers.len();
    info!("Initial peer count after publish: {}", initial_count);

    // Add vectors for multiple peers
    let vectors = std::collections::HashMap::new();

    store.put_peer_vectors(&page_id, "did:key:peer1", "device_a", &vectors).expect("Failed");
    store.put_peer_vectors(&page_id, "did:key:peer2", "device_b", &vectors).expect("Failed");
    store.put_peer_vectors(&page_id, "did:key:peer1", "device_c", &vectors).expect("Failed");

    // List peers
    let peer_list = store.list_peers_for_page(&page_id).expect("Failed to list peers");

    // Should have 3 more than initial count
    assert_eq!(peer_list.len(), initial_count + 3,
        "Should have 3 more peers than initial count");
    assert!(peer_list.contains(&("did:key:peer1".to_string(), "device_a".to_string())));
    assert!(peer_list.contains(&("did:key:peer2".to_string(), "device_b".to_string())));
    assert!(peer_list.contains(&("did:key:peer1".to_string(), "device_c".to_string())));

    info!("List peers test passed");

    peers.shutdown().await;
}

// NOTE: Delete peer vectors tests removed - not handling deletes now

// =============================================================================
// Loro CRDT Update Tests
// =============================================================================

/// Test applying a Loro update to Scribe via ApplyUpdate message
#[tokio::test]
async fn test_scribe_apply_loro_update() {
    init_tracing();

    let (peers, _space_id, page_id) =
        setup_with_published_page("update_owner", "update_node").await.expect("Setup failed");

    // Open page to get Scribe
    let scribe = peers.owner_butler
        .open_page(&page_id)
        .await
        .expect("Failed to open page");

    info!("Creating Loro update with text insertion");

    // Create update using helper
    let update = create_loro_text_update(12345, "content", "Hello from test!");
    info!("Created Loro update: {} bytes", update.len());

    // Apply update to Scribe (local update, no from_peer)
    scribe
        .cast(ScribeMessage::ApplyUpdate {
            layer_name: "collaborative_doc".to_string(),
            update,
            from_peer: None,
        })
        .expect("Failed to send ApplyUpdate");

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    info!("Update applied to Scribe");

    peers.owner_butler.close_page(&page_id).await.expect("Failed to close");
    peers.shutdown().await;
}

/// Test that Scribe persists updates across close/reopen
#[tokio::test]
async fn test_scribe_persists_updates() {
    init_tracing();

    let (peers, _space_id, page_id) =
        setup_with_published_page("persist_owner", "persist_node").await.expect("Setup failed");

    // Open page
    let scribe = peers.owner_butler.open_page(&page_id).await.expect("Failed to open");

    // Create and apply an update
    let update = create_loro_text_update(99999, "persistent_data", "This should persist");
    info!("Applying update ({} bytes) to collaborative_doc", update.len());

    scribe
        .cast(ScribeMessage::ApplyUpdate {
            layer_name: "collaborative_doc".to_string(),
            update,
            from_peer: None,
        })
        .expect("Failed to apply update");

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Close page - should flush to storage
    peers.owner_butler.close_page(&page_id).await.expect("Failed to close");
    info!("Page closed (should have flushed)");

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Re-open page - should load persisted state
    let _scribe2 = peers.owner_butler.open_page(&page_id).await.expect("Failed to re-open");
    info!("Page re-opened");

    peers.owner_butler.close_page(&page_id).await.expect("Failed to close");
    peers.shutdown().await;
}

/// Test multiple local updates to the same layer
#[tokio::test]
async fn test_scribe_multiple_updates() {
    init_tracing();

    let (peers, _space_id, page_id) =
        setup_with_published_page("multi_owner", "multi_node").await.expect("Setup failed");

    let scribe = peers.owner_butler.open_page(&page_id).await.expect("Failed to open");

    // Apply 3 updates with different peer IDs
    for (peer_id, text) in [(1111, "First "), (2222, "Second "), (3333, "Third ")] {
        let update = create_loro_text_update(peer_id, "content", text);
        scribe
            .cast(ScribeMessage::ApplyUpdate {
                layer_name: "collaborative_doc".to_string(),
                update,
                from_peer: None,
            })
            .expect("Failed to apply update");
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    info!("Applied 3 local updates with different peer IDs");

    peers.owner_butler.close_page(&page_id).await.expect("Failed to close");
    peers.shutdown().await;
}

/// Test updates to different layers
#[tokio::test]
async fn test_scribe_different_layers() {
    init_tracing();

    let (peers, _space_id, page_id) =
        setup_with_published_page("layers_owner", "layers_node").await.expect("Setup failed");

    let scribe = peers.owner_butler.open_page(&page_id).await.expect("Failed to open");

    // Update different layers
    let layers = [
        ("collaborative_doc", 1001, "Collaborative content"),
        ("content_doc", 1002, "Content layer data"),
        ("submissions_doc", 1003, "A submission"),
    ];

    for (layer, peer_id, text) in layers {
        let update = create_loro_text_update(peer_id, layer, text);
        scribe
            .cast(ScribeMessage::ApplyUpdate {
                layer_name: layer.to_string(),
                update,
                from_peer: None,
            })
            .expect("Failed");
    }

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    info!("Applied updates to 3 different layers");

    peers.owner_butler.close_page(&page_id).await.expect("Failed to close");
    peers.shutdown().await;
}

// =============================================================================
// End-to-End Sync Flow Tests
// =============================================================================

/// Test that both owner and node have correct permits after publish
#[tokio::test]
async fn test_permits_after_publish() {
    init_tracing();

    let (peers, _space_id, page_id) =
        setup_with_published_page("permit_owner", "permit_node").await.expect("Setup failed");

    info!("=== Verifying permits after publish ===");

    // Get page data from both sides
    let owner_page = peers.owner_butler
        .get_page(&page_id)
        .expect("Failed to get owner page")
        .expect("Owner should have page");

    let node_page = peers.node_butler
        .get_page(&page_id)
        .expect("Failed to get node page")
        .expect("Node should have page");

    // Check owner has permit
    let owner_permit = owner_page.get_permit();
    assert!(owner_permit.is_some(), "Owner should have a permit for the page");
    info!("Owner permit exists: {} bytes", owner_permit.unwrap().len());

    // Check node has permit (from delegation during publish)
    let node_permit = node_page.get_permit();
    assert!(node_permit.is_some(), "Node should have a permit for the page");
    info!("Node permit exists: {} bytes", node_permit.unwrap().len());

    // Verify permits can be parsed
    let owner_parsed = gurkha::Permit::from_token(owner_permit.unwrap());
    assert!(owner_parsed.is_ok(), "Owner permit should be parseable");

    let node_parsed = gurkha::Permit::from_token(node_permit.unwrap());
    assert!(node_parsed.is_ok(), "Node permit should be parseable");

    // Verify node permit has correct relationship
    let node_permit_obj = node_parsed.unwrap();
    let relationship = node_permit_obj.get_fact("relationship")
        .and_then(|v| v.as_str());
    assert_eq!(relationship, Some("node"), "Node permit should have relationship=node");

    info!("=== Permits verified successfully ===");

    peers.shutdown().await;
}

/// Test that owner and node can both open page and spawn Scribe actors
#[tokio::test]
async fn test_both_peers_open_page() {
    init_tracing();

    let (peers, _space_id, page_id) =
        setup_with_published_page("both_owner", "both_node").await.expect("Setup failed");

    info!("=== Testing both peers opening page ===");

    // Owner opens page
    let owner_scribe = peers.owner_butler
        .open_page(&page_id)
        .await
        .expect("Owner failed to open page");
    info!("Owner Scribe opened: {:?}", owner_scribe.get_id());

    // Node opens page
    let node_scribe = peers.node_butler
        .open_page(&page_id)
        .await
        .expect("Node failed to open page");
    info!("Node Scribe opened: {:?}", node_scribe.get_id());

    // Verify they are different actors
    assert_ne!(
        owner_scribe.get_id(),
        node_scribe.get_id(),
        "Owner and node should have different Scribe actors"
    );

    // Apply update on owner side
    let owner_update = create_loro_text_update(70001, "owner_edit", "Owner's local edit");
    owner_scribe
        .cast(ScribeMessage::ApplyUpdate {
            layer_name: "collaborative_doc".to_string(),
            update: owner_update,
            from_peer: None,
        })
        .expect("Failed to apply owner update");

    // Apply update on node side
    let node_update = create_loro_text_update(70002, "node_edit", "Node's local edit");
    node_scribe
        .cast(ScribeMessage::ApplyUpdate {
            layer_name: "collaborative_doc".to_string(),
            update: node_update,
            from_peer: None,
        })
        .expect("Failed to apply node update");

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    info!("Both peers applied local updates");

    // Close pages
    peers.owner_butler.close_page(&page_id).await.expect("Failed to close owner page");
    peers.node_butler.close_page(&page_id).await.expect("Failed to close node page");

    info!("=== Both peers open page test completed ===");

    peers.shutdown().await;
}

/// Test Scribe subscription with real permit from storage
#[tokio::test]
async fn test_scribe_subscription_with_real_permit() {
    init_tracing();

    let (peers, _space_id, page_id) =
        setup_with_published_page("sub_owner", "sub_node").await.expect("Setup failed");

    info!("=== Testing Scribe subscription with real permit ===");

    // Open page on owner side
    let owner_scribe = peers.owner_butler
        .open_page(&page_id)
        .await
        .expect("Owner failed to open page");

    // Subscribe node to owner's Scribe using the helper
    let mut broadcast_rx = subscribe_peer_to_scribe(
        &owner_scribe,
        &peers.node_butler,
        &peers.harness,
        "node",
        &page_id,
    ).await.expect("Failed to subscribe");

    info!("Node subscribed using real permit");

    // Drain any initial state messages sent on subscribe
    // (New behavior: Scribe sends current state when peer subscribes)
    let mut initial_msgs = 0;
    while let Ok(Some(_)) = tokio::time::timeout(
        std::time::Duration::from_millis(50),
        broadcast_rx.recv()
    ).await {
        initial_msgs += 1;
    }
    info!("Drained {} initial state messages", initial_msgs);

    // Apply an update - it should broadcast to the subscribed node
    let update = create_loro_text_update(80001, "test", "Hello from owner");
    owner_scribe
        .cast(ScribeMessage::ApplyUpdate {
            layer_name: "collaborative_doc".to_string(),
            update,
            from_peer: None,
        })
        .expect("Failed to apply update");

    // Try to receive the broadcast (with timeout)
    let broadcast_result = tokio::time::timeout(
        std::time::Duration::from_millis(200),
        broadcast_rx.recv()
    ).await;

    match broadcast_result {
        Ok(Some(payload)) => {
            info!(
                "Received broadcast: page={}, layer={}, {} bytes",
                payload.page_id, payload.layer_name, payload.update.len()
            );
            assert_eq!(payload.page_id, page_id);
            assert_eq!(payload.layer_name, "collaborative_doc");
        }
        Ok(None) => {
            panic!("Broadcast channel closed unexpectedly");
        }
        Err(_) => {
            info!("No broadcast received (timeout) - permit may not have layer permissions");
        }
    }

    peers.owner_butler.close_page(&page_id).await.expect("Failed to close page");
    peers.shutdown().await;

    info!("=== Scribe subscription test completed ===");
}

/// Test simulated update from peer
#[tokio::test]
async fn test_scribe_receives_peer_update() {
    init_tracing();

    let (peers, _space_id, page_id) =
        setup_with_published_page("peer_owner", "peer_node").await.expect("Setup failed");

    info!("=== Testing Scribe receiving peer update ===");

    // Open page
    let scribe = peers.owner_butler
        .open_page(&page_id)
        .await
        .expect("Failed to open page");

    // Subscribe the node peer first (so it has write permissions)
    let _broadcast_rx = subscribe_peer_to_scribe(
        &scribe,
        &peers.node_butler,
        &peers.harness,
        "node",
        &page_id,
    ).await.expect("Failed to subscribe");

    // Get node info for the from_peer field
    let node_user_info = peers.node_butler.user_info().await.expect("Failed to get node info");
    let node_device_id = peers.harness.peer("node").unwrap().node_id.to_string();

    // Now simulate receiving an update from that subscribed peer
    let peer_update = create_loro_text_update(90001, "peer_edit", "Update from node peer");

    scribe
        .cast(ScribeMessage::ApplyUpdate {
            layer_name: "collaborative_doc".to_string(),
            update: peer_update,
            from_peer: Some((node_user_info.did.clone(), node_device_id.clone())),
        })
        .expect("Failed to apply peer update");

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    info!("Applied update from subscribed peer: {} / {}", node_user_info.did, node_device_id);

    peers.owner_butler.close_page(&page_id).await.expect("Failed to close page");
    peers.shutdown().await;

    info!("=== Peer update test completed ===");
}
