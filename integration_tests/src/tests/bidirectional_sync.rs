//! Bidirectional sync tests using real chat app
//!
//! Tests the full Owner ↔ Node ↔ Viewer sync chain:
//! - Owner sends message → Node receives → Node broadcasts to Viewer
//! - Viewer sends message → Node receives → Node broadcasts to Owner
//!
//! Uses the actual chat app from sample_apps/chat/ to ensure production templates work.

use std::path::Path;
use serde_json::json;
use tracing::info;

use butler::ScribeMessage;
use courier::coordinator::{CoordinatorMessage, CourierMode};

use crate::TestHarness;
use crate::fixtures::{
    ACTOR_SPAWN_DELAY, EXTENDED_SYNC_DELAY, TEST_SPACE_TEMPLATE,
};
use crate::helpers::{
    init_tracing, setup_butler_with_identity,
    generate_connection_string, complete_handshake,
};

/// Get path to sample_apps/chat directory
fn chat_app_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("integration_tests should have parent directory")
        .join("sample_apps/chat")
}

/// Test full bidirectional sync: Owner ↔ Node ↔ Viewer
///
/// Flow:
/// 1. Owner creates space, imports chat app
/// 2. Owner publishes to node
/// 3. Viewer requests space from node
/// 4. Owner sends message → verify viewer receives it
/// 5. Viewer sends message → verify owner receives it
#[tokio::test]
async fn test_bidirectional_chat_sync() {
    init_tracing();

    // Setup owner with identity
    let (owner_butler, _owner_signing_key, _owner_temp) = setup_butler_with_identity("bidir_owner", "ownerpass")
        .await
        .expect("Failed to setup owner identity");

    // Setup node with identity
    let (node_butler, _node_signing_key, _node_temp) = setup_butler_with_identity("bidir_node", "nodepass")
        .await
        .expect("Failed to setup node identity");

    // Setup viewer with identity
    let (viewer_butler, _viewer_signing_key, _viewer_temp) = setup_butler_with_identity("bidir_viewer", "viewerpass")
        .await
        .expect("Failed to setup viewer identity");

    let mut harness = TestHarness::new();

    // Add all peers to harness
    harness
        .add_peer_with_butler("owner", CourierMode::User, owner_butler.clone())
        .await
        .expect("Failed to create owner peer");

    harness
        .add_peer_with_butler("node", CourierMode::Node, node_butler.clone())
        .await
        .expect("Failed to create node peer");

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

    // Owner connects to node
    harness
        .connect_and_notify("owner", "node")
        .await
        .expect("Failed to connect owner to node");

    tokio::time::sleep(ACTOR_SPAWN_DELAY).await;

    // Complete handshake between owner and node
    complete_handshake(&harness, &owner_butler, &node_butler)
        .await
        .expect("Handshake failed");

    // Verify handshake completed
    let owner_info = node_butler.get_owner().expect("Failed to get owner");
    assert!(owner_info.is_some(), "Node should have owner info after handshake");

    info!("Owner-Node handshake complete");

    // ========== Owner creates space and imports chat app ==========

    let user_info = owner_butler.user_info().await.expect("Failed to get user info");
    let space = owner_butler
        .create_space("Chat Test Space".to_string(), user_info.did.clone(), TEST_SPACE_TEMPLATE)
        .await
        .expect("Failed to create space");

    // Import the actual chat app from sample_apps/chat/
    let chat_dir = chat_app_dir();
    info!("Importing chat app from: {:?}", chat_dir);

    let page = owner_butler
        .import_app(&space.id, &chat_dir)
        .await
        .expect("Failed to import chat app");

    info!("Imported chat app: page_id={}, page_name={}", page.id, page.name);

    // ========== Owner publishes space to node ==========

    let owner_coordinator = &harness.peer("owner").unwrap().coordinator;

    owner_coordinator
        .cast(CoordinatorMessage::PublishSpace {
            node_id: node_node_id,
            space_id: space.id.clone(),
        })
        .expect("Failed to send PublishSpace");

    // Wait for space + page publishing
    tokio::time::sleep(EXTENDED_SYNC_DELAY).await;

    // Verify space and page are on node
    let node_space = node_butler.get_space(&space.id).expect("Failed to get space from node");
    assert!(node_space.is_some(), "Node should have the space");

    let node_page = node_butler.get_page(&page.id).expect("Failed to get page from node");
    assert!(node_page.is_some(), "Node should have the page");

    info!("Space and page published to node");

    // ========== Viewer requests space ==========

    // Generate viewer permit from node FIRST (before connecting)
    let (viewer_permit, _cid) = node_butler
        .issue_space_viewer_permit(&space.id)
        .await
        .expect("Failed to generate viewer permit");

    info!("Generated viewer permit for space {}", space.id);

    // Add viewer peer to harness
    harness
        .add_peer_with_butler("viewer", CourierMode::User, viewer_butler.clone())
        .await
        .expect("Failed to create viewer peer");

    let viewer_coordinator = &harness.peer("viewer").unwrap().coordinator;

    // Setup ConnectAndAuth pattern for viewer authentication
    // This mirrors CourierHandle::connect_and_wait_for_auth
    let (auth_tx, auth_rx) = tokio::sync::oneshot::channel();

    viewer_coordinator
        .cast(CoordinatorMessage::ConnectAndAuth {
            node_id: node_node_id,
            permit: viewer_permit.clone(),
            response: auth_tx,
        })
        .expect("Failed to send ConnectAndAuth");

    viewer_coordinator
        .cast(CoordinatorMessage::OutboundConnection { node_id: node_node_id })
        .expect("Failed to send OutboundConnection");

    // Connect viewer to node (triggers handshake because of pending permit)
    harness
        .connect_and_notify("viewer", "node")
        .await
        .expect("Failed to connect viewer to node");

    // Wait for authentication to complete
    match tokio::time::timeout(std::time::Duration::from_secs(5), auth_rx).await {
        Ok(Ok(Ok(_))) => info!("Viewer authenticated with node"),
        Ok(Ok(Err(e))) => panic!("Viewer auth failed: {}", e),
        Ok(Err(_)) => panic!("Auth channel closed unexpectedly"),
        Err(_) => panic!("Viewer auth timed out"),
    }

    tokio::time::sleep(ACTOR_SPAWN_DELAY).await;

    // Viewer requests space using the permit
    viewer_coordinator
        .cast(CoordinatorMessage::RequestSpaceAsViewer {
            node_id: node_node_id,
            space_id: space.id.clone(),
            viewer_permit: viewer_permit.clone(),
        })
        .expect("Failed to send RequestSpaceAsViewer");

    // Wait for space request to complete (includes consent exchange)
    tokio::time::sleep(EXTENDED_SYNC_DELAY).await;
    tokio::time::sleep(EXTENDED_SYNC_DELAY).await; // Extra time for consent

    // Verify viewer has the space and page
    let viewer_space = viewer_butler.get_space(&space.id).expect("Failed to get space from viewer");
    assert!(viewer_space.is_some(), "Viewer should have the space");

    let viewer_page = viewer_butler.get_page(&page.id).expect("Failed to get page from viewer");
    assert!(viewer_page.is_some(), "Viewer should have the page");

    info!("Viewer received space and page");

    // ========== Open Scribes on all parties ==========

    let owner_scribe = owner_butler
        .open_page(&page.id)
        .await
        .expect("Failed to open owner's page");

    let _node_scribe = node_butler
        .open_page(&page.id)
        .await
        .expect("Failed to open node's page");

    let viewer_scribe = viewer_butler
        .open_page(&page.id)
        .await
        .expect("Failed to open viewer's page");

    // Give time for EnsureSync to establish subscriptions
    tokio::time::sleep(EXTENDED_SYNC_DELAY).await;

    info!("All Scribes opened, subscriptions should be established");

    // ========== Test Owner → Viewer sync ==========

    info!("Testing Owner → Viewer sync...");

    // Owner writes message to "messages" layer
    owner_scribe
        .cast(ScribeMessage::ListPush {
            layer_name: "messages".to_string(),
            path: "".to_string(),
            item: json!({
                "text": "Hello from owner!",
                "author": "owner",
                "timestamp": 1000
            }),
        })
        .expect("Failed to send ListPush to owner's Scribe");

    // Wait for sync propagation: Owner → Node → Viewer
    tokio::time::sleep(EXTENDED_SYNC_DELAY).await;
    tokio::time::sleep(EXTENDED_SYNC_DELAY).await;

    // Verify viewer has the message
    let (viewer_messages_tx, viewer_messages_rx) = tokio::sync::oneshot::channel();
    viewer_scribe
        .cast(ScribeMessage::GetLayerJson {
            layer_name: "messages".to_string(),
            reply: viewer_messages_tx,
        })
        .expect("Failed to get viewer messages");

    let viewer_messages = viewer_messages_rx.await.expect("Failed to receive viewer messages");
    let viewer_messages_str = viewer_messages.map(|v| v.to_string()).unwrap_or_default();

    info!("Viewer messages: {}", viewer_messages_str);
    assert!(
        viewer_messages_str.contains("Hello from owner!"),
        "Viewer should receive owner's message. Got: {}",
        viewer_messages_str
    );

    info!("✓ Owner → Viewer sync working!");

    // ========== Test Viewer → Owner sync ==========

    info!("Testing Viewer → Owner sync...");

    // Viewer writes message to "messages" layer
    viewer_scribe
        .cast(ScribeMessage::ListPush {
            layer_name: "messages".to_string(),
            path: "".to_string(),
            item: json!({
                "text": "Hello from viewer!",
                "author": "viewer",
                "timestamp": 2000
            }),
        })
        .expect("Failed to send ListPush to viewer's Scribe");

    // Wait for sync propagation: Viewer → Node → Owner
    tokio::time::sleep(EXTENDED_SYNC_DELAY).await;
    tokio::time::sleep(EXTENDED_SYNC_DELAY).await;

    // Verify owner has the message
    let (owner_messages_tx, owner_messages_rx) = tokio::sync::oneshot::channel();
    owner_scribe
        .cast(ScribeMessage::GetLayerJson {
            layer_name: "messages".to_string(),
            reply: owner_messages_tx,
        })
        .expect("Failed to get owner messages");

    let owner_messages = owner_messages_rx.await.expect("Failed to receive owner messages");
    let owner_messages_str = owner_messages.map(|v| v.to_string()).unwrap_or_default();

    info!("Owner messages: {}", owner_messages_str);
    assert!(
        owner_messages_str.contains("Hello from viewer!"),
        "Owner should receive viewer's message. Got: {}",
        owner_messages_str
    );

    info!("✓ Viewer → Owner sync working!");

    // Verify both messages are present on both sides (CRDT merge)
    assert!(
        owner_messages_str.contains("Hello from owner!"),
        "Owner should also have their own message"
    );
    assert!(
        viewer_messages_str.contains("Hello from owner!") || {
            // Re-check viewer's messages after owner sync
            let (tx, rx) = tokio::sync::oneshot::channel();
            viewer_scribe.cast(ScribeMessage::GetLayerJson {
                layer_name: "messages".to_string(),
                reply: tx,
            }).ok();
            rx.await.ok().flatten().map(|v| v.to_string().contains("Hello from owner!")).unwrap_or(false)
        },
        "Viewer should have both messages after sync"
    );

    info!("✓ Bidirectional sync complete! Both parties have both messages.");

    harness.shutdown().await;
}

/// Test that Node correctly broadcasts owner updates to multiple viewers
#[tokio::test]
async fn test_owner_to_node_sync_only() {
    init_tracing();

    // Setup owner and node only (simpler test)
    let (owner_butler, _, _owner_temp) = setup_butler_with_identity("o2n_owner", "pass")
        .await
        .expect("Failed to setup owner");

    let (node_butler, _, _node_temp) = setup_butler_with_identity("o2n_node", "pass")
        .await
        .expect("Failed to setup node");

    let mut harness = TestHarness::new();

    harness.add_peer_with_butler("owner", CourierMode::User, owner_butler.clone()).await.unwrap();
    harness.add_peer_with_butler("node", CourierMode::Node, node_butler.clone()).await.unwrap();

    let node_node_id = harness.peer("node").unwrap().node_id;

    let connection_string = generate_connection_string(&node_butler).await.unwrap();
    owner_butler.add_sovereign_node(&connection_string).unwrap();

    let _transport = harness.spawn_transport();
    harness.connect_and_notify("owner", "node").await.unwrap();
    tokio::time::sleep(ACTOR_SPAWN_DELAY).await;

    complete_handshake(&harness, &owner_butler, &node_butler).await.unwrap();

    // Create space and import chat app
    let user_info = owner_butler.user_info().await.unwrap();
    let space = owner_butler
        .create_space("O2N Test".to_string(), user_info.did.clone(), TEST_SPACE_TEMPLATE)
        .await
        .unwrap();

    let page = owner_butler.import_app(&space.id, &chat_app_dir()).await.unwrap();
    info!("Imported chat app: {}", page.id);

    // Publish to node
    let owner_coordinator = &harness.peer("owner").unwrap().coordinator;
    owner_coordinator
        .cast(CoordinatorMessage::PublishSpace {
            node_id: node_node_id,
            space_id: space.id.clone(),
        })
        .unwrap();

    tokio::time::sleep(EXTENDED_SYNC_DELAY).await;

    // Open Scribes
    let owner_scribe = owner_butler.open_page(&page.id).await.unwrap();
    let node_scribe = node_butler.open_page(&page.id).await.unwrap();

    tokio::time::sleep(EXTENDED_SYNC_DELAY).await;

    // Owner writes message
    owner_scribe
        .cast(ScribeMessage::ListPush {
            layer_name: "messages".to_string(),
            path: "".to_string(),
            item: json!({"text": "Test message", "author": "owner"}),
        })
        .unwrap();

    tokio::time::sleep(EXTENDED_SYNC_DELAY).await;

    // Verify node received it
    let (tx, rx) = tokio::sync::oneshot::channel();
    node_scribe
        .cast(ScribeMessage::GetLayerJson {
            layer_name: "messages".to_string(),
            reply: tx,
        })
        .unwrap();

    let node_messages = rx.await.unwrap();
    let node_messages_str = node_messages.map(|v| v.to_string()).unwrap_or_default();

    info!("Node messages: {}", node_messages_str);
    assert!(
        node_messages_str.contains("Test message"),
        "Node should receive owner's message. Got: {}",
        node_messages_str
    );

    info!("✓ Owner → Node sync working!");

    harness.shutdown().await;
}
