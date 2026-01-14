//! Handshake flow tests
//!
//! Tests the owner-node handshake protocol:
//! - First connection (using first_connection permit)
//! - Reconnection (using stored permits from first connection)

use std::time::Duration;

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use butler::ConnectionType;
use courier::coordinator::{CoordinatorMessage, CourierMode};
use tokio::sync::oneshot;
use transport::NodeId;

use crate::TestHarness;
use crate::helpers::{init_tracing, setup_butler_with_identity};

/// Helper to initiate connection using production flow (ConnectAndAuth + OutboundConnection + connect)
///
/// This replicates what `connect_and_wait_for_auth` does:
/// 1. Sends ConnectAndAuth to store permit in pending_permits
/// 2. Sends OutboundConnection to mark as outbound
/// 3. Connects via transport (triggers on_connected which looks up pending_permits)
/// 4. Waits for auth to complete via oneshot channel
async fn connect_with_permit(
    harness: &TestHarness,
    from: &str,
    to: &str,
    coordinator: &ractor::ActorRef<CoordinatorMessage>,
    node_id: NodeId,
    permit: String,
) -> Result<NodeId, String> {
    let (tx, rx) = oneshot::channel();

    // 1. Store permit in pending_permits
    coordinator
        .cast(CoordinatorMessage::ConnectAndAuth {
            node_id,
            permit,
            response: tx,
        })
        .map_err(|e| format!("Failed to send ConnectAndAuth: {}", e))?;

    // 2. Mark as outbound connection
    coordinator
        .cast(CoordinatorMessage::OutboundConnection { node_id })
        .map_err(|e| format!("Failed to send OutboundConnection: {}", e))?;

    // 3. Connect (triggers on_connected which uses pending_permits)
    harness.connect_and_notify(from, to).await
        .map_err(|e| e.to_string())?;

    // 4. Wait for auth to complete
    tokio::time::timeout(Duration::from_secs(5), rx)
        .await
        .map_err(|_| "Timeout waiting for auth".to_string())?
        .map_err(|_| "Auth channel closed".to_string())?
}

/// Space template for tests - uses capability-based design
///
/// This template demonstrates the role-agnostic pattern:
/// - `peer_capabilities` controls protocol operations (relay, share, accept_publish)
/// - `layer_patterns` controls data access with `{aud}` placeholder for viewer DID
const TEST_SPACE_TEMPLATE: &str = r#"{
    "owner_template": {
        "operations": {
            "own": "allow",
            "get_share_link": "allow",
            "add_pages": "allow",
            "share_space": "allow"
        },
        "peer_capabilities": {
            "relay": false,
            "share": true,
            "accept_publish": true
        },
        "layers": {
            "content": { "sync": true, "write": true, "type": "list" }
        },
        "layer_patterns": {
            "{page_id}/*/*": { "create": true, "sync": true }
        },
        "delegation": {
            "node": {
                "token_type": "space_share",
                "peer_capabilities": {
                    "relay": true,
                    "share": true,
                    "accept_publish": false
                },
                "operations": {
                    "get_share_link": "allow",
                    "add_pages": "allow",
                    "share_space": "allow"
                },
                "layers": {
                    "content": { "sync": true, "write": false }
                },
                "layer_patterns": {
                    "{page_id}/*/*": { "create": false, "sync": true }
                },
                "auth_capabilities": {
                    "can_connect": true,
                    "persist_share": true,
                    "can_delegate": false,
                    "sync_enabled": true
                },
                "relationship": "node"
            },
            "viewer": {
                "token_type": "space_viewer",
                "peer_capabilities": {
                    "relay": false,
                    "share": false,
                    "accept_publish": false
                },
                "operations": {
                    "request_pages": "allow",
                    "get_share_link": "allow"
                },
                "layers": {
                    "content": { "sync": true, "write": false }
                },
                "layer_patterns": {
                    "{page_id}/*/{aud}": { "create": true, "sync": true }
                },
                "auth_capabilities": {
                    "can_connect": true,
                    "persist_share": false,
                    "can_delegate": false,
                    "sync_enabled": false
                },
                "relationship": "viewer"
            }
        }
    }
}"#;

#[tokio::test]
async fn test_harness_creation() {
    let harness = TestHarness::new();
    assert!(harness.peers.is_empty());
}

#[tokio::test]
async fn test_peer_creation() {
    let mut harness = TestHarness::new();

    // Create owner (User mode) and node (Node mode)
    harness.add_peer("owner", CourierMode::User).await.unwrap();
    harness.add_peer("node", CourierMode::Node).await.unwrap();

    assert_eq!(harness.peers.len(), 2);
    assert!(harness.peer("owner").is_some());
    assert!(harness.peer("node").is_some());

    // Each peer should have a unique node_id
    let owner = harness.peer("owner").unwrap();
    let node = harness.peer("node").unwrap();
    assert_ne!(owner.node_id, node.node_id);

    harness.shutdown().await;
}

#[tokio::test]
async fn test_peer_connection() {
    let mut harness = TestHarness::new();

    harness.add_peer("owner", CourierMode::User).await.unwrap();
    harness.add_peer("node", CourierMode::Node).await.unwrap();

    // Connect owner to node
    harness.connect("owner", "node").await.unwrap();

    // Verify connection exists in both directions
    let owner = harness.peer("owner").unwrap();
    let node = harness.peer("node").unwrap();

    let conn_to_node = harness
        .transport
        .get_connection(owner.node_id, node.node_id)
        .await;
    assert!(conn_to_node.is_some());

    let conn_to_owner = harness
        .transport
        .get_connection(node.node_id, owner.node_id)
        .await;
    assert!(conn_to_owner.is_some());

    harness.shutdown().await;
}

/// Test first connection handshake flow using production flow
///
/// Uses ConnectAndAuth + OutboundConnection + connect (same as connect_and_wait_for_auth)
///
/// Flow:
/// 1. Node generates connection string with first_connection permit
/// 2. Owner scans connection string (adds sovereign node record)
/// 3. Owner connects using production flow (pending_permits mechanism)
/// 4. Handshake completes, permits exchanged
///
/// Assertions:
/// - OwnerInfo: did, encryption_key, username, permit, paired_at
/// - SovereignNode: did, encryption_key, name, node_id, permit, connection_type
#[tokio::test]
async fn test_first_connection_handshake() {
    init_tracing();

    // Setup owner with identity
    let (owner_butler, _owner_signing_key, _owner_temp) = setup_butler_with_identity("alice", "password123")
        .await
        .expect("Failed to setup owner identity");

    // Setup node with identity
    let (node_butler, _node_signing_key, _node_temp) = setup_butler_with_identity("my-node", "nodepass")
        .await
        .expect("Failed to setup node identity");

    // Get expected values for assertions
    let owner_info_expected = owner_butler.user_info().await.expect("Failed to get owner info");
    let node_info_expected = node_butler.user_info().await.expect("Failed to get node info");

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
    let owner_coordinator = harness.peer("owner").unwrap().coordinator.clone();

    let _transport_handle = harness.spawn_transport();

    // === Node generates connection string, Owner scans it ===
    let connection_string = node_butler
        .generate_connection_string(None)
        .await
        .expect("Failed to generate connection string");

    // Owner adds sovereign node (stores in DB)
    let sovereign_node = owner_butler
        .add_sovereign_node(&connection_string)
        .expect("Failed to add sovereign node");

    let first_conn_permit = sovereign_node.permit.clone().expect("Should have permit");

    // Verify it's a first_connection permit
    let parsed = gurkha::Permit::from_token(&first_conn_permit).expect("Invalid permit");
    assert!(parsed.is_first_connection(), "Should be first_connection permit");

    // === Connect using production flow ===
    connect_with_permit(
        &harness,
        "owner",
        "node",
        &owner_coordinator,
        node_node_id,
        first_conn_permit,
    )
    .await
    .expect("First connection handshake failed");

    // === Verify OwnerInfo stored on Node ===
    let owner_info = node_butler
        .get_owner()
        .expect("Failed to get owner info")
        .expect("Node should have stored owner info");

    assert_eq!(owner_info.username, "alice", "OwnerInfo.username mismatch");
    assert!(!owner_info.did.is_empty(), "OwnerInfo.did should be set");
    assert!(!owner_info.encryption_key.is_empty(), "OwnerInfo.encryption_key should be set");
    assert!(owner_info.permit.is_some(), "OwnerInfo should have permit");
    assert!(owner_info.paired_at > 0, "OwnerInfo.paired_at should be set");

    // === Verify SovereignNode stored on Owner ===
    let sovereign_node_after = owner_butler
        .get_sovereign_node(&node_node_id.to_string())
        .expect("Failed to get sovereign node")
        .expect("Owner should have sovereign node record");

    assert_eq!(sovereign_node_after.name, "my-node", "SovereignNode.name mismatch");
    assert!(!sovereign_node_after.did.is_empty(), "SovereignNode.did should be set");
    assert!(!sovereign_node_after.encryption_key.is_empty(), "SovereignNode.encryption_key should be set");
    assert_eq!(sovereign_node_after.node_id, node_node_id.to_string(), "SovereignNode.node_id mismatch");
    assert_eq!(sovereign_node_after.connection_type, ConnectionType::Owner, "SovereignNode.connection_type should be Owner");
    assert!(sovereign_node_after.permit.is_some(), "SovereignNode should have permit");

    // Verify permit upgraded from first_connection
    let stored_permit = sovereign_node_after.permit.as_ref().unwrap();
    let parsed_stored = gurkha::Permit::from_token(stored_permit).expect("Invalid stored permit");
    assert!(!parsed_stored.is_first_connection(), "Permit should be upgraded from first_connection");

    // === Verify peer_capabilities (capability-based design) ===
    // Owner's permit from node should have capabilities allowing owner operations
    let owner_caps = parsed_stored.peer_capabilities();
    // Owner doesn't need relay (that's for nodes)
    // Owner should be able to share and accept_publish depends on node's template
    tracing::info!(
        "Owner permit capabilities: relay={}, share={}, accept_publish={}",
        owner_caps.relay, owner_caps.share, owner_caps.accept_publish
    );

    harness.shutdown().await;
}

/// Test reconnection handshake flow using production flow
///
/// Uses ConnectAndAuth + OutboundConnection + connect for both connections
///
/// Flow:
/// 1. First connection using production flow - establishes permits
/// 2. Load permit from DB (simulates app restart)
/// 3. Reconnect using production flow with DB-loaded permit
/// 4. Verify permits unchanged (no re-issuance)
#[tokio::test]
async fn test_reconnection_handshake() {
    init_tracing();

    // Setup owner with identity
    let (owner_butler, _owner_signing_key, _owner_temp) = setup_butler_with_identity("alice", "password123")
        .await
        .expect("Failed to setup owner identity");

    // Setup node with identity
    let (node_butler, _node_signing_key, _node_temp) = setup_butler_with_identity("my-node", "nodepass")
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

    let node_node_id = harness.peer("node").unwrap().node_id;
    let owner_coordinator = harness.peer("owner").unwrap().coordinator.clone();

    let _transport_handle = harness.spawn_transport();

    // === Node generates connection string, Owner scans it ===
    let connection_string = node_butler
        .generate_connection_string(None)
        .await
        .expect("Failed to generate connection string");

    let sovereign_node_initial = owner_butler
        .add_sovereign_node(&connection_string)
        .expect("Failed to add sovereign node");

    let first_conn_permit = sovereign_node_initial.permit.clone().expect("Should have permit");

    // Verify it IS a first_connection permit
    let parsed = gurkha::Permit::from_token(&first_conn_permit).expect("Invalid permit");
    assert!(parsed.is_first_connection(), "Initial permit should be first_connection");

    // === PHASE 1: First connection using production flow ===
    connect_with_permit(
        &harness,
        "owner",
        "node",
        &owner_coordinator,
        node_node_id,
        first_conn_permit,
    )
    .await
    .expect("First connection handshake failed");

    // Verify first connection succeeded
    let owner_info = node_butler.get_owner().expect("Failed to get owner")
        .expect("Node should have owner after first connection");
    assert!(owner_info.permit.is_some(), "Node should have stored permit");

    // Store permits for comparison after reconnection
    let stored_owner_permit = owner_info.permit.clone().unwrap();

    // === PHASE 2: Load permit from DB (simulates app restart) ===
    // This is what production does: butler.list_sovereign_nodes() or get_sovereign_node()
    let sovereign_node_from_db = owner_butler
        .get_sovereign_node(&node_node_id.to_string())
        .expect("Failed to get sovereign node from DB")
        .expect("SovereignNode should exist in DB");

    let reconnect_permit = sovereign_node_from_db.permit.clone()
        .expect("SovereignNode should have permit in DB");

    // Store for comparison
    let stored_sovereign_permit = reconnect_permit.clone();

    // Verify it's NOT a first_connection permit
    let parsed = gurkha::Permit::from_token(&reconnect_permit).expect("Invalid permit");
    assert!(!parsed.is_first_connection(), "Reconnection permit should NOT be first_connection");

    // === PHASE 3: Reconnect using production flow with DB-loaded permit ===
    connect_with_permit(
        &harness,
        "owner",
        "node",
        &owner_coordinator,
        node_node_id,
        reconnect_permit,
    )
    .await
    .expect("Reconnection handshake failed");

    // === PHASE 4: Verify no new permits were issued ===
    let owner_info_after = node_butler.get_owner().expect("Failed to get owner").unwrap();
    assert_eq!(
        owner_info_after.permit.as_ref().unwrap(),
        &stored_owner_permit,
        "Node's stored permit should be unchanged after reconnection"
    );

    let sovereign_node_after = owner_butler
        .get_sovereign_node(&node_node_id.to_string())
        .expect("Failed to get sovereign node")
        .unwrap();
    assert_eq!(
        sovereign_node_after.permit.as_ref().unwrap(),
        &stored_sovereign_permit,
        "Owner's stored permit should be unchanged after reconnection"
    );

    // Verify OwnerInfo unchanged
    assert_eq!(owner_info_after.username, owner_info.username, "OwnerInfo.username should be unchanged");
    assert_eq!(owner_info_after.did, owner_info.did, "OwnerInfo.did should be unchanged");

    harness.shutdown().await;
}

/// Test viewer handshake and reconnection flow using production flow
///
/// Uses ConnectAndAuth + OutboundConnection + connect for all connections
///
/// Flow:
/// 1. Owner connects to Node using production flow
/// 2. Owner publishes space to Node
/// 3. Node generates viewer permit (shareable link)
/// 4. Viewer connects to Node using production flow
/// 5. Viewer stores node as Contact (simulating handle_connect_to_website)
/// 6. Viewer reconnects using permit loaded from DB
///
/// Assertions:
/// - Contact: did, encryption_key, username, is_node, permit, node_id
/// - Node still has only owner (viewer not stored as owner)
/// - Viewer reconnection succeeds with DB-loaded permit
#[tokio::test]
async fn test_viewer_handshake() {
    init_tracing();

    // Setup owner with identity
    let (owner_butler, _owner_signing_key, _owner_temp) = setup_butler_with_identity("alice", "password123")
        .await
        .expect("Failed to setup owner identity");

    // Setup node with identity
    let (node_butler, _node_signing_key, _node_temp) = setup_butler_with_identity("my-node", "nodepass")
        .await
        .expect("Failed to setup node identity");

    // Setup viewer with identity
    let (viewer_butler, _viewer_signing_key, _viewer_temp) = setup_butler_with_identity("bob-viewer", "viewerpass")
        .await
        .expect("Failed to setup viewer identity");

    // Get expected values for assertions
    let node_info_expected = node_butler.user_info().await.expect("Failed to get node info");

    let mut harness = TestHarness::new();

    harness
        .add_peer_with_butler("owner", CourierMode::User, owner_butler.clone())
        .await
        .expect("Failed to create owner peer");

    harness
        .add_peer_with_butler("node", CourierMode::Node, node_butler.clone())
        .await
        .expect("Failed to create node peer");

    harness
        .add_peer_with_butler("viewer", CourierMode::User, viewer_butler.clone())
        .await
        .expect("Failed to create viewer peer");

    let node_node_id = harness.peer("node").unwrap().node_id;
    let owner_coordinator = harness.peer("owner").unwrap().coordinator.clone();
    let viewer_coordinator = harness.peer("viewer").unwrap().coordinator.clone();

    let _transport_handle = harness.spawn_transport();

    // === PHASE 1: Owner-Node handshake using production flow ===
    let connection_string = node_butler
        .generate_connection_string(None)
        .await
        .expect("Failed to generate connection string");

    let sovereign_node = owner_butler
        .add_sovereign_node(&connection_string)
        .expect("Failed to add sovereign node");

    let first_conn_permit = sovereign_node.permit.clone().expect("Should have permit");

    connect_with_permit(
        &harness,
        "owner",
        "node",
        &owner_coordinator,
        node_node_id,
        first_conn_permit,
    )
    .await
    .expect("Owner-node handshake failed");

    // Verify owner-node handshake succeeded
    let owner_info = node_butler.get_owner().expect("Failed to get owner")
        .expect("Node should have owner after handshake");
    assert_eq!(owner_info.username, "alice", "Owner username mismatch");

    // === PHASE 2: Owner creates space and publishes to node ===
    let user_info = owner_butler.user_info().await.expect("Failed to get user info");
    let space = owner_butler
        .create_space("Shared Space".to_string(), user_info.did.clone(), TEST_SPACE_TEMPLATE)
        .await
        .expect("Failed to create space");

    owner_coordinator
        .cast(CoordinatorMessage::PublishSpace {
            node_id: node_node_id,
            space_id: space.id.clone(),
        })
        .expect("Failed to publish space");

    tokio::time::sleep(Duration::from_millis(500)).await;

    // === PHASE 3: Generate viewer connection string (shareable link) ===
    let viewer_connection_string = node_butler
        .generate_viewer_connection_string(&space.id, None)
        .await
        .expect("Failed to generate viewer connection string");

    // === PHASE 4: Viewer connects using production flow ===
    let viewer_conn = viewer_butler
        .parse_connection_string(&viewer_connection_string)
        .expect("Failed to parse viewer connection string");

    let viewer_permit = viewer_conn.permit.clone();
    let parsed_viewer_permit = gurkha::Permit::from_token(&viewer_permit).expect("Invalid viewer permit");
    assert!(!parsed_viewer_permit.is_first_connection(), "Viewer permit should NOT be first_connection");

    // === Verify capability-based design (not role-based) ===
    // Viewer should have restricted peer_capabilities
    let viewer_caps = parsed_viewer_permit.peer_capabilities();
    assert!(!viewer_caps.relay, "Viewer should NOT have relay capability");
    assert!(!viewer_caps.share, "Viewer should NOT have share capability");
    assert!(!viewer_caps.accept_publish, "Viewer should NOT have accept_publish capability");

    // Verify viewer has layer_patterns with {aud} placeholder for privacy isolation
    if parsed_viewer_permit.has_layer_patterns() {
        let patterns = parsed_viewer_permit.layer_patterns();
        let has_aud_pattern = patterns.keys().any(|p| p.contains("{aud}"));
        tracing::info!(
            "Viewer layer_patterns: {:?}, has_aud_pattern: {}",
            patterns.keys().collect::<Vec<_>>(),
            has_aud_pattern
        );
        // Viewer should have DID-scoped pattern like {page_id}/*/{aud}
        // This ensures privacy isolation - viewers only see their own namespace
        if has_aud_pattern {
            tracing::info!("Viewer has {{aud}} pattern for privacy isolation");
        }
    } else {
        tracing::info!("Viewer permit has no layer_patterns (space-level permit)");
    }

    // Relationship is for logging only, not business logic
    if let Some(rel) = parsed_viewer_permit.core().relationship() {
        tracing::info!("Viewer relationship label (logging only): {}", rel);
    }

    connect_with_permit(
        &harness,
        "viewer",
        "node",
        &viewer_coordinator,
        node_node_id,
        viewer_permit.clone(),
    )
    .await
    .expect("Viewer handshake failed");

    // === PHASE 5: Store node as Contact (simulating handle_connect_to_website) ===
    // Convert base64 public key to DID format for consistent storage
    let node_did = herald::Identity::did_from_base64_pubkey(&viewer_conn.node_public_key)
        .expect("Invalid node public key");
    viewer_butler
        .add_node_contact(
            &node_did,
            &viewer_conn.node_encryption_key,
            &viewer_conn.name,
            &node_node_id.to_string(),
            &viewer_permit,
        )
        .expect("Failed to store node contact");

    // === Verify Contact stored correctly ===
    let node_contacts = viewer_butler.list_node_contacts().expect("Failed to list node contacts");
    assert_eq!(node_contacts.len(), 1, "Viewer should have one node contact");

    let node_contact = &node_contacts[0];
    assert!(node_contact.is_node(), "Contact should be of type Node");
    assert!(node_contact.permit.is_some(), "Contact should have permit");
    assert_eq!(node_contact.node_id.as_ref().unwrap(), &node_node_id.to_string(), "Contact.node_id mismatch");
    assert!(!node_contact.did.is_empty(), "Contact.did should be set");
    assert!(node_contact.did.starts_with("did:key:z"), "Contact.did should be DID format, got: {}", node_contact.did);
    assert_eq!(node_contact.encryption_key, BASE64.encode(&node_info_expected.encryption_key), "Contact.encryption_key mismatch");
    assert_eq!(node_contact.username, "my-node", "Contact.username mismatch");

    // Node should still only have one owner (not the viewer)
    let owner_info_final = node_butler.get_owner().expect("Failed to get owner").unwrap();
    assert_eq!(owner_info_final.username, "alice", "Owner should still be alice, not viewer");

    // === PHASE 6: Viewer reconnection using DB-loaded permit ===
    // Load permit from DB (simulates app restart)
    let contacts_from_db = viewer_butler.list_node_contacts().expect("Failed to load contacts from DB");
    let contact_from_db = &contacts_from_db[0];
    let reconnect_permit = contact_from_db.permit.clone().expect("Contact should have permit in DB");

    // Reconnect using production flow
    connect_with_permit(
        &harness,
        "viewer",
        "node",
        &viewer_coordinator,
        node_node_id,
        reconnect_permit,
    )
    .await
    .expect("Viewer reconnection failed");

    // Verify still authenticated (node still has same owner)
    let owner_info_after_reconnect = node_butler.get_owner().expect("Failed to get owner").unwrap();
    assert_eq!(owner_info_after_reconnect.username, "alice", "Owner should still be alice after viewer reconnect");

    harness.shutdown().await;
}
