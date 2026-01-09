//! Viewer Consent Permits for Sync
//!
//! Tests the viewer consent flow where viewers issue permits back to the node
//! to express consent for receiving sync updates.
//!
//! **Problem**: When a node sends sync updates to a viewer, the viewer never
//! explicitly consented to receive updates or new pages. The node-issued permit
//! authorizes the viewer to *read* - it doesn't represent viewer consent to
//! *receive writes*.
//!
//! **Solution**: Viewers issue permits back to the node to express consent:
//! - **Space permit**: Consent for space sync + ability to add new pages
//! - **Page permits**: Consent for specific page content updates (one per page)
//!
//! These viewer-issued permits use the node's original permit as `prf` (proof)
//! to derive permissions.

use tracing::info;


use courier::coordinator::{CoordinatorMessage, CourierMode};

use crate::TestHarness;
use crate::fixtures::{
    ACTOR_SPAWN_DELAY, EXTENDED_SYNC_DELAY,
    TEST_PAGE_LAYERS, TEST_PAGE_TEMPLATE, TEST_SPACE_TEMPLATE,
};
use crate::helpers::{
    init_tracing, setup_butler_with_identity,
    generate_connection_string, complete_handshake,
};

/// End-to-end test for viewer consent sync flow
///
/// This test is the "north star" - we're done implementing the feature when this passes.
///
/// **Flow**:
/// 1. Owner publishes space + pages to node
/// 2. Viewer connects and requests space
/// 3. Viewer receives SpaceSync with space + pages
/// 4. Viewer issues consent permits (space permit + page permits)
/// 5. Viewer sends permits to node
/// 6. Node stores viewer-issued permits keyed by (viewer_did, resource_id)
/// 7. Owner updates a page
/// 8. Node syncs to viewer using viewer-issued page permit
/// 9. Viewer verifies the permit and applies update
#[tokio::test]
async fn test_viewer_consent_sync_flow() {
    init_tracing();

    // === PHASE 1: Setup ===
    info!("=== PHASE 1: Setup ===");

    // Setup owner with identity
    let (owner_butler, _owner_signing_key, _owner_temp) = setup_butler_with_identity("consent_owner", "password123")
        .await
        .expect("Failed to setup owner identity");

    // Setup node with identity
    let (node_butler, _node_signing_key, _node_temp) = setup_butler_with_identity("consent_node", "nodepass")
        .await
        .expect("Failed to setup node identity");

    // Setup viewer with identity
    let (viewer_butler, _viewer_signing_key, _viewer_temp) = setup_butler_with_identity("consent_viewer", "viewerpass")
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

    let node_node_id = harness.peer("node").unwrap().node_id;

    // Generate connection string from node
    let connection_string = generate_connection_string(&node_butler)
        .await
        .expect("Failed to generate connection string");

    // Owner adds sovereign node
    owner_butler
        .add_sovereign_node(&connection_string)
        .expect("Failed to add sovereign node");

    let _transport_handle = harness.spawn_transport();

    harness
        .connect_and_notify("owner", "node")
        .await
        .expect("Failed to connect owner to node");

    tokio::time::sleep(ACTOR_SPAWN_DELAY).await;

    complete_handshake(&harness, &owner_butler, &node_butler)
        .await
        .expect("Owner-Node handshake failed");

    // Verify handshake completed
    let owner_info = node_butler.get_owner().expect("Failed to get owner");
    assert!(owner_info.is_some(), "Node should have owner info after handshake");

    // Owner creates a space locally
    let user_info = owner_butler.user_info().await.expect("Failed to get user info");
    let space = owner_butler
        .create_space("Consent Test Space".to_string(), user_info.did.clone(), TEST_SPACE_TEMPLATE)
        .await
        .expect("Failed to create space");

    // Owner creates multiple pages in the space (to test multiple page permits)
    let layer_names: Vec<String> = TEST_PAGE_LAYERS.iter().map(|s| s.to_string()).collect();

    let page1 = owner_butler
        .create_page(&space.id, "Consent Page 1", layer_names.clone(), TEST_PAGE_TEMPLATE)
        .await
        .expect("Failed to create page 1");

    let page2 = owner_butler
        .create_page(&space.id, "Consent Page 2", layer_names.clone(), TEST_PAGE_TEMPLATE)
        .await
        .expect("Failed to create page 2");

    info!("Created space {} with pages {} and {}", space.id, page1.id, page2.id);

    let owner_coordinator = &harness.peer("owner").unwrap().coordinator;

    // Publish space to node (pages auto-sync)
    owner_coordinator
        .cast(CoordinatorMessage::PublishSpace {
            node_id: node_node_id,
            space_id: space.id.clone(),
        })
        .expect("Failed to send PublishSpace");

    // Wait for space + pages to be published
    tokio::time::sleep(EXTENDED_SYNC_DELAY).await;

    // Verify space and pages are on node
    let node_space = node_butler.get_space(&space.id).expect("Failed to get space");
    assert!(node_space.is_some(), "Node should have the space");

    let node_page1 = node_butler.get_page(&page1.id).expect("Failed to get page1");
    assert!(node_page1.is_some(), "Node should have page 1");

    let node_page2 = node_butler.get_page(&page2.id).expect("Failed to get page2");
    assert!(node_page2.is_some(), "Node should have page 2");

    info!("Space and pages published to node successfully");

    // === PHASE 2: Viewer connects ===
    info!("=== PHASE 2: Viewer connects ===");

    // Generate viewer permit (aud:* for shareable link)
    let (viewer_permit, _cid) = node_butler
        .issue_space_viewer_permit(&space.id)
        .await
        .expect("Failed to generate viewer permit");

    info!("Generated viewer permit with aud:* for space {}", space.id);

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

    // === PHASE 3: First sync + consent permits ===
    info!("=== PHASE 3: First sync + consent permits ===");

    // Verify viewer has the space
    let viewer_space = viewer_butler.get_space(&space.id).expect("Failed to get space from viewer");
    assert!(viewer_space.is_some(), "Viewer should have the space");

    // Verify viewer has the pages
    let viewer_page1 = viewer_butler.get_page(&page1.id).expect("Failed to get page1 from viewer");
    assert!(viewer_page1.is_some(), "Viewer should have page 1");

    let viewer_page2 = viewer_butler.get_page(&page2.id).expect("Failed to get page2 from viewer");
    assert!(viewer_page2.is_some(), "Viewer should have page 2");

    info!("Viewer received space with {} pages", 2);

    // Get viewer's DID for assertions
    let viewer_info = viewer_butler.user_info().await.expect("Failed to get viewer info");
    let viewer_did = viewer_info.did.clone();
    info!("Viewer DID: {}", viewer_did);

    // Wait for consent permits to be issued and stored (happens automatically after last page)
    // The viewer issues consent permits in on_viewer_page when is_last=true
    tokio::time::sleep(EXTENDED_SYNC_DELAY).await;

    // === PHASE 4: Verify storage ===
    info!("=== PHASE 4: Verify storage ===");

    // Verify node has stored viewer-issued space consent permit
    let viewer_space_consent = node_butler
        .get_viewer_space_consent(&viewer_did, &space.id)
        .expect("Failed to get viewer space consent");
    assert!(viewer_space_consent.is_some(), "Node should have viewer-issued space consent permit");
    info!("✓ Node has space consent permit for viewer {}", viewer_did);

    // Verify node has stored viewer-issued page consent permits
    let viewer_page1_consent = node_butler
        .get_viewer_page_consent(&viewer_did, &page1.id)
        .expect("Failed to get viewer page1 consent");
    assert!(viewer_page1_consent.is_some(), "Node should have viewer-issued page1 consent permit");
    info!("✓ Node has page consent permit for page1 {}", page1.id);

    let viewer_page2_consent = node_butler
        .get_viewer_page_consent(&viewer_did, &page2.id)
        .expect("Failed to get viewer page2 consent");
    assert!(viewer_page2_consent.is_some(), "Node should have viewer-issued page2 consent permit");
    info!("✓ Node has page consent permit for page2 {}", page2.id);

    info!("=== All consent permits verified ===");

    harness.shutdown().await;

    // Mark test as incomplete - this should fail until implementation is done
    // Uncomment this assertion when ready to implement:
    // assert!(false, "Test incomplete: viewer consent permits not yet implemented");
}

/// Test that viewer issues correct permits after receiving SpaceSync
///
/// Verifies:
/// 1. Viewer receives space + pages from node
/// 2. Viewer issues one space permit (sync_space template)
/// 3. Viewer issues N page permits (sync_page template, one per page)
/// 4. All permits have correct prf (proof) field pointing to node's original permit
#[tokio::test]
#[ignore = "Not yet implemented - waiting for consent permit system"]
async fn test_viewer_issues_consent_permits() {
    init_tracing();

    // TODO: Implement after core consent permit system is in place
    // This test focuses on permit issuance logic specifically
}

/// Test that node stores viewer-issued permits correctly
///
/// Verifies:
/// 1. Node receives viewer-issued permits
/// 2. Node stores permits keyed by (viewer_did, space_id) and (viewer_did, page_id)
/// 3. Node can retrieve permits for a specific viewer + resource
#[tokio::test]
#[ignore = "Not yet implemented - waiting for consent permit system"]
async fn test_node_stores_viewer_permits() {
    init_tracing();

    // TODO: Implement after core consent permit system is in place
    // This test focuses on storage logic specifically
}

/// Test that node uses viewer-issued permit when sending LayerSync
///
/// Verifies:
/// 1. Node has viewer-issued page permit stored
/// 2. When sending LayerSync, node looks up permit by (viewer_did, page_id)
/// 3. LayerSync message contains the viewer-issued permit
#[tokio::test]
#[ignore = "Not yet implemented - waiting for consent permit system"]
async fn test_node_attaches_viewer_permit_to_sync() {
    init_tracing();

    // TODO: Implement after core consent permit system is in place
    // This test focuses on permit attachment logic specifically
}

/// Test that viewer rejects sync updates without valid consent permit
///
/// Verifies:
/// 1. Viewer rejects LayerSync with wrong permit
/// 2. Viewer rejects LayerSync with no permit
/// 3. Viewer accepts LayerSync with correct viewer-issued permit
#[tokio::test]
#[ignore = "Not yet implemented - waiting for consent permit system"]
async fn test_viewer_validates_consent_permit() {
    init_tracing();

    // TODO: Implement after core consent permit system is in place
    // This test focuses on viewer-side validation logic specifically
}
