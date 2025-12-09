//! Publish flow tests
//!
//! Tests the owner publishing spaces to their node:
//! - Owner creates space locally
//! - Owner publishes space to connected node
//! - Node receives and stores space with permit
//! - Owner receives ack with permit echoed back

use tracing::info;

use butler::PageType;
use courier::coordinator::{CoordinatorMessage, CourierMode};

use crate::TestHarness;
use crate::fixtures::{
    ACTOR_SPAWN_DELAY, HANDSHAKE_DELAY, MESSAGE_DELIVERY_DELAY,
    PAGE_SYNC_DELAY, EXTENDED_SYNC_DELAY, TEST_SPACE_TEMPLATE,
    TEST_PAGE_TEMPLATE, TEST_PAGE_LAYERS,
};
use crate::helpers::{
    init_tracing, setup_butler_with_identity,
    generate_connection_string, complete_handshake,
};

/// Test owner publishes a space to connected node
///
/// Flow:
/// 1. Complete handshake
/// 2. Owner creates space locally
/// 3. Owner sends PublishSpace command
/// 4. Node receives, validates permit, stores space
/// 5. Node sends PublishSpaceAck with permit echoed back
/// 6. Owner receives ack, marks space as published
#[tokio::test]
async fn test_owner_publishes_space_to_node() {
    init_tracing();

    // Setup owner with identity
    let (owner_butler, _owner_signing_key, _owner_temp) = setup_butler_with_identity("alice", "password123")
        .await
        .expect("Failed to setup owner identity");

    // Setup node with identity
    let (node_butler, _node_signing_key, _node_temp) = setup_butler_with_identity("my-node", "nodepass")
        .await
        .expect("Failed to setup node identity");

    // Create harness with custom butlers
    let mut harness = TestHarness::new();

    harness
        .add_peer_with_butler("owner", CourierMode::User, owner_butler.clone())
        .await
        .expect("Failed to create owner peer");

    harness
        .add_peer_with_butler("node", CourierMode::Node, node_butler.clone())
        .await
        .expect("Failed to create node peer");

    // Spawn transport in background
    let _transport_handle = harness.spawn_transport();

    // Production flow: generate connection string, add sovereign node
    let connection_string = generate_connection_string(&node_butler).await
        .expect("Failed to generate connection string");
    owner_butler.add_sovereign_node(&connection_string)
        .expect("Failed to add sovereign node");

    // Connect and complete handshake
    harness
        .connect_and_notify("owner", "node")
        .await
        .expect("Failed to connect");

    tokio::time::sleep(ACTOR_SPAWN_DELAY).await;

    complete_handshake(&harness, &owner_butler, &node_butler)
        .await
        .expect("Handshake failed");

    // Verify handshake completed - node should have owner info
    let owner_info = node_butler.get_owner().expect("Failed to get owner");
    assert!(owner_info.is_some(), "Node should have owner info after handshake");

    // Owner creates a space locally
    let user_info = owner_butler.user_info().await.expect("Failed to get user info");
    let space = owner_butler
        .create_space("My Test Space".to_string(), user_info.did.clone(), TEST_SPACE_TEMPLATE)
        .await
        .expect("Failed to create space");

    assert_eq!(space.name, "My Test Space");

    // Get node_id
    let node_node_id = harness.peer("node").unwrap().node_id;

    // Owner publishes space to node
    let owner_coordinator = &harness.peer("owner").unwrap().coordinator;
    owner_coordinator
        .cast(CoordinatorMessage::PublishSpace {
            node_id: node_node_id,
            space_id: space.id.clone(),
        })
        .expect("Failed to send PublishSpace");

    // Wait for publish to complete
    tokio::time::sleep(MESSAGE_DELIVERY_DELAY).await;

    // Verify node received and stored the space
    let node_space = node_butler
        .get_space(&space.id)
        .expect("Failed to get space from node");

    assert!(node_space.is_some(), "Node should have stored the space");
    let node_space = node_space.unwrap();
    assert_eq!(node_space.name, "My Test Space");
    assert_eq!(node_space.owner_did, user_info.did);

    harness.shutdown().await;
}

/// Test that space with pages returns page IDs in ack
#[tokio::test]
async fn test_publish_space_returns_empty_pages_initially() {
    init_tracing();

    let (owner_butler, _owner_signing_key, _owner_temp) = setup_butler_with_identity("bob", "password456")
        .await
        .expect("Failed to setup owner identity");

    let (node_butler, _node_signing_key, _node_temp) = setup_butler_with_identity("node2", "nodepass2")
        .await
        .expect("Failed to setup node identity");

    let mut harness = TestHarness::new();

    harness
        .add_peer_with_butler("owner", CourierMode::User, owner_butler.clone())
        .await
        .expect("Failed to create owner peer");

    harness
        .add_peer_with_butler("node", CourierMode::Node, node_butler.clone())
        .await
        .expect("Failed to create node peer");

    let _transport_handle = harness.spawn_transport();

    // Production flow: generate connection string, add sovereign node
    let connection_string = generate_connection_string(&node_butler).await
        .expect("Failed to generate connection string");
    owner_butler.add_sovereign_node(&connection_string)
        .expect("Failed to add sovereign node");

    harness
        .connect_and_notify("owner", "node")
        .await
        .expect("Failed to connect");

    tokio::time::sleep(ACTOR_SPAWN_DELAY).await;

    complete_handshake(&harness, &owner_butler, &node_butler)
        .await
        .expect("Handshake failed");

    // Owner creates a space
    let user_info = owner_butler.user_info().await.expect("Failed to get user info");
    let space = owner_butler
        .create_space("Space With No Pages".to_string(), user_info.did.clone(), TEST_SPACE_TEMPLATE)
        .await
        .expect("Failed to create space");

    // Publish space
    let node_node_id = harness.peer("node").unwrap().node_id;
    let owner_coordinator = &harness.peer("owner").unwrap().coordinator;
    owner_coordinator
        .cast(CoordinatorMessage::PublishSpace {
            node_id: node_node_id,
            space_id: space.id.clone(),
        })
        .expect("Failed to send PublishSpace");

    tokio::time::sleep(MESSAGE_DELIVERY_DELAY).await;

    // Node should have space stored
    let node_space = node_butler.get_space(&space.id).expect("Failed to get space");
    assert!(node_space.is_some());

    // Node should have no pages for this space (pages not published yet)
    let node_pages = node_butler.list_page_ids_for_space(&space.id).expect("Failed to list pages");
    assert!(node_pages.is_empty(), "Node should have no pages initially");

    harness.shutdown().await;
}

/// Test publishing without authentication fails
#[tokio::test]
async fn test_publish_without_auth_fails() {
    init_tracing();

    let (owner_butler, _owner_signing_key, _owner_temp) = setup_butler_with_identity("carol", "pass")
        .await
        .expect("Failed to setup owner identity");

    let (node_butler, _node_signing_key, _node_temp) = setup_butler_with_identity("node3", "nodepass3")
        .await
        .expect("Failed to setup node identity");

    let mut harness = TestHarness::new();

    harness
        .add_peer_with_butler("owner", CourierMode::User, owner_butler.clone())
        .await
        .expect("Failed to create owner peer");

    harness
        .add_peer_with_butler("node", CourierMode::Node, node_butler.clone())
        .await
        .expect("Failed to create node peer");

    let _transport_handle = harness.spawn_transport();

    harness
        .connect_and_notify("owner", "node")
        .await
        .expect("Failed to connect");

    tokio::time::sleep(ACTOR_SPAWN_DELAY).await;

    // DON'T complete handshake - try to publish without auth

    // Owner creates a space
    let user_info = owner_butler.user_info().await.expect("Failed to get user info");
    let space = owner_butler
        .create_space("Unauthorized Space".to_string(), user_info.did.clone(), TEST_SPACE_TEMPLATE)
        .await
        .expect("Failed to create space");

    // Try to publish space without completing handshake
    let node_node_id = harness.peer("node").unwrap().node_id;
    let owner_coordinator = &harness.peer("owner").unwrap().coordinator;

    // This should be ignored because peer is not authenticated
    owner_coordinator
        .cast(CoordinatorMessage::PublishSpace {
            node_id: node_node_id,
            space_id: space.id.clone(),
        })
        .expect("Failed to send PublishSpace");

    tokio::time::sleep(HANDSHAKE_DELAY).await;

    // Node should NOT have the space (publish should have been rejected)
    let node_space = node_butler.get_space(&space.id).expect("Failed to get space");
    assert!(node_space.is_none(), "Node should NOT have space when not authenticated");

    harness.shutdown().await;
}

/// Test publishing multiple spaces
#[tokio::test]
async fn test_publish_multiple_spaces() {
    init_tracing();

    let (owner_butler, _owner_signing_key, _owner_temp) = setup_butler_with_identity("dave", "pass123")
        .await
        .expect("Failed to setup owner identity");

    let (node_butler, _node_signing_key, _node_temp) = setup_butler_with_identity("node4", "nodepass4")
        .await
        .expect("Failed to setup node identity");

    let mut harness = TestHarness::new();

    harness
        .add_peer_with_butler("owner", CourierMode::User, owner_butler.clone())
        .await
        .expect("Failed to create owner peer");

    harness
        .add_peer_with_butler("node", CourierMode::Node, node_butler.clone())
        .await
        .expect("Failed to create node peer");

    let _transport_handle = harness.spawn_transport();

    // Production flow: generate connection string, add sovereign node
    let connection_string = generate_connection_string(&node_butler).await
        .expect("Failed to generate connection string");
    owner_butler.add_sovereign_node(&connection_string)
        .expect("Failed to add sovereign node");

    harness
        .connect_and_notify("owner", "node")
        .await
        .expect("Failed to connect");

    tokio::time::sleep(ACTOR_SPAWN_DELAY).await;

    complete_handshake(&harness, &owner_butler, &node_butler)
        .await
        .expect("Handshake failed");

    let user_info = owner_butler.user_info().await.expect("Failed to get user info");

    // Create multiple spaces
    let space1 = owner_butler
        .create_space("Space One".to_string(), user_info.did.clone(), TEST_SPACE_TEMPLATE)
        .await
        .expect("Failed to create space 1");

    let space2 = owner_butler
        .create_space("Space Two".to_string(), user_info.did.clone(), TEST_SPACE_TEMPLATE)
        .await
        .expect("Failed to create space 2");

    let space3 = owner_butler
        .create_space("Space Three".to_string(), user_info.did.clone(), TEST_SPACE_TEMPLATE)
        .await
        .expect("Failed to create space 3");

    // Publish all spaces
    let node_node_id = harness.peer("node").unwrap().node_id;
    let owner_coordinator = &harness.peer("owner").unwrap().coordinator;

    for space in [&space1, &space2, &space3] {
        owner_coordinator
            .cast(CoordinatorMessage::PublishSpace {
                node_id: node_node_id,
                space_id: space.id.clone(),
            })
            .expect("Failed to send PublishSpace");
    }

    // Wait for all publishes to complete
    tokio::time::sleep(PAGE_SYNC_DELAY).await;

    // Verify all spaces are stored on node
    for (space, name) in [(&space1, "Space One"), (&space2, "Space Two"), (&space3, "Space Three")] {
        let node_space = node_butler.get_space(&space.id).expect("Failed to get space");
        assert!(node_space.is_some(), "Node should have space: {}", name);
        assert_eq!(node_space.unwrap().name, name);
    }

    // Verify node has 3 spaces total
    let all_spaces = node_butler.list_spaces().expect("Failed to list spaces");
    assert_eq!(all_spaces.len(), 3, "Node should have exactly 3 spaces");

    harness.shutdown().await;
}

/// Test owner publishes space AND pages are automatically sent to node
///
/// Flow:
/// 1. Complete handshake
/// 2. Owner creates space and page locally
/// 3. Owner sends PublishSpace
/// 4. Node responds with PublishSpaceAck (with existing_pages=[])
/// 5. Coordinator automatically triggers PublishPage for all pages in space
/// 6. Node receives, decrypts transit, re-encrypts, stores each page
/// 7. Verify pages stored on Node (with local_only layer filtered out)
#[tokio::test]
async fn test_owner_publishes_space_and_pages_auto_sync() {
    init_tracing();

    let (owner_butler, _owner_signing_key, _owner_temp) = setup_butler_with_identity("page_owner", "password123")
        .await
        .expect("Failed to setup owner identity");

    let (node_butler, _node_signing_key, _node_temp) = setup_butler_with_identity("page_node", "nodepass")
        .await
        .expect("Failed to setup node identity");

    let mut harness = TestHarness::new();

    harness
        .add_peer_with_butler("owner", CourierMode::User, owner_butler.clone())
        .await
        .expect("Failed to create owner peer");

    harness
        .add_peer_with_butler("node", CourierMode::Node, node_butler.clone())
        .await
        .expect("Failed to create node peer");

    // Get node_id BEFORE connecting
    let node_node_id = harness.peer("node").unwrap().node_id;

    // Generate connection string from node
    let connection_string = generate_connection_string(&node_butler)
        .await
        .expect("Failed to generate connection string");

    // Owner adds sovereign node
    owner_butler
        .add_sovereign_node(&connection_string)
        .expect("Failed to add sovereign node");

    info!("Owner added sovereign node: {}", node_node_id);

    let _transport_handle = harness.spawn_transport();

    harness
        .connect_and_notify("owner", "node")
        .await
        .expect("Failed to connect");

    tokio::time::sleep(ACTOR_SPAWN_DELAY).await;

    complete_handshake(&harness, &owner_butler, &node_butler)
        .await
        .expect("Handshake failed");

    // Verify handshake completed
    let owner_info = node_butler.get_owner().expect("Failed to get owner");
    assert!(owner_info.is_some(), "Node should have owner info after handshake");

    // Owner creates a space locally
    let user_info = owner_butler.user_info().await.expect("Failed to get user info");
    let space = owner_butler
        .create_space("Auto-Sync Space".to_string(), user_info.did.clone(), TEST_SPACE_TEMPLATE)
        .await
        .expect("Failed to create space");

    // Owner creates multiple pages in the space
    let layer_names: Vec<String> = TEST_PAGE_LAYERS.iter().map(|s| s.to_string()).collect();

    let page1 = owner_butler
        .create_page(&space.id, "Page One", PageType::Content, layer_names.clone(), TEST_PAGE_TEMPLATE)
        .await
        .expect("Failed to create page 1");

    let page2 = owner_butler
        .create_page(&space.id, "Page Two", PageType::Content, layer_names.clone(), TEST_PAGE_TEMPLATE)
        .await
        .expect("Failed to create page 2");

    info!("Created pages: {}, {}", page1.id, page2.id);

    let owner_coordinator = &harness.peer("owner").unwrap().coordinator;

    // ONLY send PublishSpace - pages should be published automatically after SpaceAck
    owner_coordinator
        .cast(CoordinatorMessage::PublishSpace {
            node_id: node_node_id,
            space_id: space.id.clone(),
        })
        .expect("Failed to send PublishSpace");

    // Wait for space ack AND automatic page publishing
    tokio::time::sleep(EXTENDED_SYNC_DELAY).await;

    // Verify space is on node
    let node_space = node_butler.get_space(&space.id).expect("Failed to get space");
    assert!(node_space.is_some(), "Node should have the space");

    // Verify BOTH pages were automatically published
    let node_page1 = node_butler.get_page(&page1.id).expect("Failed to get page 1");
    assert!(node_page1.is_some(), "Node should have page 1 (auto-synced)");
    assert_eq!(node_page1.unwrap().meta.name, "Page One");

    let node_page2 = node_butler.get_page(&page2.id).expect("Failed to get page 2");
    assert!(node_page2.is_some(), "Node should have page 2 (auto-synced)");
    assert_eq!(node_page2.unwrap().meta.name, "Page Two");

    // Verify page count
    let node_pages = node_butler.list_pages(&space.id).expect("Failed to list pages");
    assert_eq!(node_pages.len(), 2, "Node should have 2 pages auto-synced");

    // Verify permit on one of the pages
    let node_page1 = node_butler.get_page(&page1.id).expect("Failed to get page").unwrap();
    assert!(node_page1.permit.is_some(), "Node should have permit for page");

    let permit = gurkha::Permit::from_token(node_page1.permit.as_ref().unwrap())
        .expect("Failed to parse page permit");
    let relationship = permit.get_fact("relationship")
        .and_then(|v| v.as_str())
        .expect("Missing relationship in permit");
    assert_eq!(relationship, "node", "Permit should have 'node' relationship");

    info!("Space and pages auto-synced successfully");

    harness.shutdown().await;
}

/// Test incremental sync - node already has some pages
///
/// When PublishSpaceAck returns existing_pages, we should only publish new ones.
#[tokio::test]
async fn test_incremental_page_sync() {
    init_tracing();

    let (owner_butler, _owner_signing_key, _owner_temp) = setup_butler_with_identity("incr_owner", "pass")
        .await
        .expect("Failed to setup owner identity");

    let (node_butler, _node_signing_key, _node_temp) = setup_butler_with_identity("incr_node", "nodepass")
        .await
        .expect("Failed to setup node identity");

    let mut harness = TestHarness::new();

    harness
        .add_peer_with_butler("owner", CourierMode::User, owner_butler.clone())
        .await
        .expect("Failed to create owner peer");

    harness
        .add_peer_with_butler("node", CourierMode::Node, node_butler.clone())
        .await
        .expect("Failed to create node peer");

    // Get node_id BEFORE connecting
    let node_node_id = harness.peer("node").unwrap().node_id;

    let connection_string = generate_connection_string(&node_butler)
        .await
        .expect("Failed to generate connection string");

    owner_butler
        .add_sovereign_node(&connection_string)
        .expect("Failed to add sovereign node");

    let _transport_handle = harness.spawn_transport();

    harness
        .connect_and_notify("owner", "node")
        .await
        .expect("Failed to connect");

    tokio::time::sleep(ACTOR_SPAWN_DELAY).await;

    complete_handshake(&harness, &owner_butler, &node_butler)
        .await
        .expect("Handshake failed");

    // Create space with one page
    let user_info = owner_butler.user_info().await.expect("Failed to get user info");
    let space = owner_butler
        .create_space("Incremental Space".to_string(), user_info.did.clone(), TEST_SPACE_TEMPLATE)
        .await
        .expect("Failed to create space");

    let layer_names: Vec<String> = TEST_PAGE_LAYERS.iter().map(|s| s.to_string()).collect();

    let page1 = owner_butler
        .create_page(&space.id, "Initial Page", PageType::Content, layer_names.clone(), TEST_PAGE_TEMPLATE)
        .await
        .expect("Failed to create page 1");

    let owner_coordinator = &harness.peer("owner").unwrap().coordinator;

    // First publish - space + page1
    owner_coordinator
        .cast(CoordinatorMessage::PublishSpace {
            node_id: node_node_id,
            space_id: space.id.clone(),
        })
        .expect("Failed to send PublishSpace");

    tokio::time::sleep(PAGE_SYNC_DELAY).await;

    // Verify page1 is on node
    let node_page1 = node_butler.get_page(&page1.id).expect("Failed to get page 1");
    assert!(node_page1.is_some(), "Node should have page 1");

    // Now create page2 locally
    let page2 = owner_butler
        .create_page(&space.id, "New Page", PageType::Content, layer_names.clone(), TEST_PAGE_TEMPLATE)
        .await
        .expect("Failed to create page 2");

    // Second publish - space already exists, should only sync page2
    owner_coordinator
        .cast(CoordinatorMessage::PublishSpace {
            node_id: node_node_id,
            space_id: space.id.clone(),
        })
        .expect("Failed to send PublishSpace again");

    tokio::time::sleep(PAGE_SYNC_DELAY).await;

    // Verify both pages are on node
    let node_pages = node_butler.list_pages(&space.id).expect("Failed to list pages");
    assert_eq!(node_pages.len(), 2, "Node should have 2 pages after incremental sync");

    let node_page2 = node_butler.get_page(&page2.id).expect("Failed to get page 2");
    assert!(node_page2.is_some(), "Node should have page 2 after incremental sync");

    info!("Incremental sync completed successfully");

    harness.shutdown().await;
}
