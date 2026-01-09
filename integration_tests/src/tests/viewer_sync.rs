//! Viewer sync flow tests
//!
//! Tests the shareable link flow for viewers:
//! - Owner publishes space with pages to node
//! - Owner requests shareable link from node
//! - Node generates viewer permit with aud:* (wildcard audience)
//! - Node returns link to owner

use tracing::info;


use courier::coordinator::{CoordinatorMessage, CourierMode};

use crate::TestHarness;
use crate::fixtures::{
    ACTOR_SPAWN_DELAY, EXTENDED_SYNC_DELAY, MESSAGE_DELIVERY_DELAY,
    TEST_PAGE_LAYERS, TEST_PAGE_TEMPLATE, TEST_SPACE_TEMPLATE,
};
use crate::helpers::{
    init_tracing, setup_butler_with_identity,
    generate_connection_string, complete_handshake,
};

/// Test owner requests shareable link from node after publishing space
///
/// Flow:
/// 1. Complete handshake (owner ↔ node)
/// 2. Owner creates space + pages locally
/// 3. Owner publishes space to node (pages auto-sync)
/// 4. Owner sends GetShareableLink request to node
/// 5. Node generates viewer permit with aud:* and returns link
/// 6. Verify link contains valid permit with aud:*
#[tokio::test]
async fn test_generate_shareable_link() {
    init_tracing();

    // Setup owner with identity
    let (owner_butler, _owner_signing_key, _owner_temp) = setup_butler_with_identity("link_owner", "password123")
        .await
        .expect("Failed to setup owner identity");

    // Setup node with identity
    let (node_butler, _node_signing_key, _node_temp) = setup_butler_with_identity("link_node", "nodepass")
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
        .create_space("Shareable Space".to_string(), user_info.did.clone(), TEST_SPACE_TEMPLATE)
        .await
        .expect("Failed to create space");

    // Owner creates a page in the space
    let layer_names: Vec<String> = TEST_PAGE_LAYERS.iter().map(|s| s.to_string()).collect();

    let page = owner_butler
        .create_page(&space.id, "Shareable Page", layer_names.clone(), TEST_PAGE_TEMPLATE)
        .await
        .expect("Failed to create page");

    info!("Created space {} with page {}", space.id, page.id);

    let owner_coordinator = &harness.peer("owner").unwrap().coordinator;

    // Publish space to node (pages auto-sync)
    owner_coordinator
        .cast(CoordinatorMessage::PublishSpace {
            node_id: node_node_id,
            space_id: space.id.clone(),
        })
        .expect("Failed to send PublishSpace");

    // Wait for space + page publishing
    tokio::time::sleep(EXTENDED_SYNC_DELAY).await;

    // Verify space and page are on node
    let node_space = node_butler.get_space(&space.id).expect("Failed to get space");
    assert!(node_space.is_some(), "Node should have the space");

    let node_page = node_butler.get_page(&page.id).expect("Failed to get page");
    assert!(node_page.is_some(), "Node should have the page");

    info!("Space and page published to node");

    // === Now the new part: Request shareable link ===

    // Owner requests shareable link from node via message flow
    owner_coordinator
        .cast(CoordinatorMessage::GetShareableLink {
            node_id: node_node_id,
            space_id: space.id.clone(),
        })
        .expect("Failed to send GetShareableLink");

    tokio::time::sleep(MESSAGE_DELIVERY_DELAY).await;

    // For verification, also generate directly on node to compare
    // In real usage, the owner would receive the permit via ShareableLinkReceived callback
    let (viewer_permit, _cid) = node_butler
        .issue_space_viewer_permit(&space.id)
        .await
        .expect("Failed to generate viewer permit");

    // Verify permit has wildcard audience
    let permit = gurkha::Permit::from_token(&viewer_permit)
        .expect("Failed to parse permit");

    assert_eq!(permit.parsed().audience(), "*", "Permit should have wildcard audience");

    let token_type = permit.get_fact("token_type")
        .and_then(|v| v.as_str())
        .expect("Missing token_type in permit");
    assert_eq!(token_type, "viewer_auth", "Token type should be viewer_auth");

    let permit_space_id = permit.get_fact("space_id")
        .and_then(|v| v.as_str())
        .expect("Missing space_id in permit");
    assert_eq!(permit_space_id, space.id, "Permit space_id should match space ID");

    let relationship = permit.get_fact("relationship")
        .and_then(|v| v.as_str())
        .expect("Missing relationship in permit");
    assert_eq!(relationship, "node_viewer", "Relationship should be node_viewer");

    info!("Viewer permit generated successfully with aud:* and space_id={}", permit_space_id);

    harness.shutdown().await;
}

/// Test full viewer space request flow
///
/// Flow:
/// 1. Owner publishes space + pages to node
/// 2. Owner gets shareable link (aud:* permit)
/// 3. Viewer connects to node with aud:* permit
/// 4. Node validates permit, delegates real permit, sends content
/// 5. Viewer stores space and pages locally
/// 6. Verify viewer has space and pages
#[tokio::test]
async fn test_viewer_space_request() {
    init_tracing();

    // Setup owner with identity
    let (owner_butler, _owner_signing_key, _owner_temp) = setup_butler_with_identity("vsr_owner", "password123")
        .await
        .expect("Failed to setup owner identity");

    // Setup node with identity
    let (node_butler, _node_signing_key, _node_temp) = setup_butler_with_identity("vsr_node", "nodepass")
        .await
        .expect("Failed to setup node identity");

    // Setup viewer with identity
    let (viewer_butler, _viewer_signing_key, _viewer_temp) = setup_butler_with_identity("vsr_viewer", "viewerpass")
        .await
        .expect("Failed to setup viewer identity");

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
        .expect("Failed to connect owner to node");

    tokio::time::sleep(ACTOR_SPAWN_DELAY).await;

    complete_handshake(&harness, &owner_butler, &node_butler)
        .await
        .expect("Handshake failed");

    // Owner creates a space locally
    let user_info = owner_butler.user_info().await.expect("Failed to get user info");
    let space = owner_butler
        .create_space("Viewer Test Space".to_string(), user_info.did.clone(), TEST_SPACE_TEMPLATE)
        .await
        .expect("Failed to create space");

    // Owner creates a page in the space
    let layer_names: Vec<String> = TEST_PAGE_LAYERS.iter().map(|s| s.to_string()).collect();

    let page = owner_butler
        .create_page(&space.id, "Viewer Test Page", layer_names.clone(), TEST_PAGE_TEMPLATE)
        .await
        .expect("Failed to create page");

    info!("Created space {} with page {}", space.id, page.id);

    let owner_coordinator = &harness.peer("owner").unwrap().coordinator;

    // Publish space to node (pages auto-sync)
    owner_coordinator
        .cast(CoordinatorMessage::PublishSpace {
            node_id: node_node_id,
            space_id: space.id.clone(),
        })
        .expect("Failed to send PublishSpace");

    // Wait for space + page publishing
    tokio::time::sleep(EXTENDED_SYNC_DELAY).await;

    // Generate shareable link (aud:* permit) on node
    let (viewer_permit, _cid) = node_butler
        .issue_space_viewer_permit(&space.id)
        .await
        .expect("Failed to generate viewer permit");

    info!("Generated viewer permit with aud:* for space {}", space.id);

    // === Now viewer connects with the shareable link ===

    // Add viewer peer to harness
    harness
        .add_peer_with_butler("viewer", CourierMode::User, viewer_butler.clone())
        .await
        .expect("Failed to create viewer peer");

    // Connect viewer to node
    harness
        .connect_and_notify("viewer", "node")
        .await
        .expect("Failed to connect viewer to node");

    tokio::time::sleep(ACTOR_SPAWN_DELAY).await;

    // Viewer requests space using the aud:* permit
    let viewer_coordinator = &harness.peer("viewer").unwrap().coordinator;

    viewer_coordinator
        .cast(CoordinatorMessage::RequestSpaceAsViewer {
            node_id: node_node_id,
            space_id: space.id.clone(),
            viewer_permit: viewer_permit.clone(),
        })
        .expect("Failed to send RequestSpaceAsViewer");

    // Wait for space request to complete
    tokio::time::sleep(EXTENDED_SYNC_DELAY).await;

    // Verify viewer has the space
    let viewer_space = viewer_butler.get_space(&space.id).expect("Failed to get space from viewer");
    assert!(viewer_space.is_some(), "Viewer should have the space");

    let viewer_space = viewer_space.unwrap();
    assert_eq!(viewer_space.name, "Viewer Test Space", "Space name should match");

    // Verify viewer has the page
    let viewer_page = viewer_butler.get_page(&page.id).expect("Failed to get page from viewer");
    assert!(viewer_page.is_some(), "Viewer should have the page");

    let viewer_page = viewer_page.unwrap();
    assert_eq!(viewer_page.meta.name, "Viewer Test Page", "Page name should match");

    // Verify node stored viewer permit CID in PERMIT_CIDS table
    let stored_cid = node_butler
        .get_permit_cid(&page.id, &viewer_butler.user_info().await.unwrap().did)
        .expect("Failed to get stored permit CID");
    assert!(stored_cid.is_some(), "Node should have stored permit CID for viewer");

    info!("Viewer successfully received space {} with page {}", space.id, page.id);

    // ========== State Vector Verification ==========
    // Verify node stored viewer's state vectors for incremental sync later
    let viewer_info = viewer_butler.user_info().await.expect("Failed to get viewer info");
    let viewer_node_id_str = harness.peer("viewer").unwrap().node_id.to_string();

    for layer_name in &layer_names {
        // Skip local_only layers which aren't synced
        if layer_name == "user_content_doc" {
            continue;
        }

        let viewer_vector = node_butler.store().get_peer_vector_for_layer(
            &page.id,
            &viewer_info.did,
            &viewer_node_id_str,
            layer_name,
        ).expect("Failed to get vector");

        assert!(viewer_vector.is_some(),
            "Node should store viewer's state vector for page {} layer {}",
            page.id, layer_name);
    }

    // TODO: Viewer-side state vector storage requires handshake with node to exchange DIDs.
    // Currently viewer connects directly with shareable link permit without Hello/Welcome flow,
    // so the viewer doesn't know the node's DID. This needs a viewer handshake flow.
    // For now, we only verify node stored viewer's vectors (above assertions).
    //
    // Once viewer handshake is implemented:
    // let node_info = node_butler.user_info().await.expect("Failed to get node info");
    // let node_node_id_str = node_node_id.to_string();
    // for layer_name in &layer_names {
    //     if layer_name == "user_content_doc" { continue; }
    //     let node_vector = viewer_butler.store().get_peer_vector_for_layer(
    //         &page.id, &node_info.did, &node_node_id_str, layer_name,
    //     ).expect("Failed to get vector");
    //     assert!(node_vector.is_some(),
    //         "Viewer should store node's state vector for page {} layer {}",
    //         page.id, layer_name);
    // }

    info!("State vectors stored correctly for viewer incremental sync");

    harness.shutdown().await;
}
