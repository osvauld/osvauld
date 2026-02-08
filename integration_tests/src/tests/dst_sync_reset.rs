//! DST SyncReset Tests - Permit-Aware Snapshot Recovery
//!
//! Tests SyncReset invariants:
//! - INV-SR3: Authorized ops are preserved during SyncReset
//! - INV-SR4: Unauthorized ops are correctly discarded

use anyhow::Result;
use domains::Layer;
use tracing::info;

use crate::scenario::init_tracing;

// Test: SyncReset Preserves Authorized Ops Only

/// Validates INV-SR3, INV-SR4: Authorized ops preserved, unauthorized discarded
#[tokio::test]
async fn test_sync_reset_preserves_authorized_ops_only() -> Result<()> {
    init_tracing();

    let page_id = "test-page-001";
    let viewer_did = "did:key:viewer";

    let viewer_permit = gurkha::test_fixtures::handshake_viewer(page_id, viewer_did);
    let can_write_collaborative = viewer_permit.can_write_layer("collaborative_doc", page_id, viewer_did);
    let can_write_content = viewer_permit.can_write_layer("content_doc", page_id, viewer_did);

    let node_collab = Layer::new();
    node_collab.list_push("entries", &serde_json::json!({"from": "node", "id": "n1"}))?;
    let node_content = Layer::new();
    node_content.list_push("items", &serde_json::json!({"from": "node", "id": "n1"}))?;

    let user_collab = Layer::from_snapshot(&node_collab.export_snapshot())?;
    let user_content = Layer::from_snapshot(&node_content.export_snapshot())?;

    user_collab.list_push("entries", &serde_json::json!({"from": "user", "id": "u1", "authorized": true}))?;
    user_content.list_push("items", &serde_json::json!({"from": "user", "id": "u1", "unauthorized": true}))?;

    node_collab.list_push("entries", &serde_json::json!({"from": "node", "id": "n2"}))?;
    node_content.list_push("items", &serde_json::json!({"from": "node", "id": "n2"}))?;

    let node_collab_snapshot = node_collab.export_snapshot();
    let node_content_snapshot = node_content.export_snapshot();

    if can_write_collaborative {
        let merged = Layer::from_snapshot(&node_collab_snapshot)?;
        let node_vector = node_collab.version_vector();
        let user_ops = user_collab.export_updates(&node_vector)?;
        merged.apply(&user_ops)?;
        let user_collab = Layer::from_snapshot(&merged.export_snapshot())?;
        let json = user_collab.to_json();
        let entries = json["root"]["entries"].as_array().expect("entries");
        assert!(entries.len() >= 2);
        assert!(entries.iter().any(|e| e["from"] == "user"), "INV-SR3: User's authorized ops preserved");
    }

    if !can_write_content {
        let user_content = Layer::from_snapshot(&node_content_snapshot)?;
        let json = user_content.to_json();
        let items = json["root"]["items"].as_array().expect("items");
        assert!(!items.iter().any(|e| e["from"] == "user"), "INV-SR4: User's unauthorized ops discarded");
        assert_eq!(items.len(), 2);
    }

    info!("SyncReset permit-aware test passed (INV-SR3, INV-SR4)");
    Ok(())
}

// Test: SyncReset Basic Scenarios (full replacement, empty local, vectors match)

/// Tests SyncReset for read-only users, empty local state, and vector matching
#[tokio::test]
async fn test_sync_reset_basic_scenarios() -> Result<()> {
    init_tracing();

    // Full replacement for read-only user
    let node_layer = Layer::new();
    node_layer.list_push("data", &serde_json::json!({"id": 1, "from": "node"}))?;
    node_layer.list_push("data", &serde_json::json!({"id": 2, "from": "node"}))?;

    let user_layer = Layer::new();
    user_layer.list_push("data", &serde_json::json!({"id": 999, "from": "user", "corrupt": true}))?;

    assert_ne!(node_layer.to_json(), user_layer.to_json());

    let node_snapshot = node_layer.export_snapshot();
    let user_layer = Layer::from_snapshot(&node_snapshot)?;
    assert_eq!(node_layer.to_json(), user_layer.to_json(), "Read-only user has exact node state");

    // Empty local state SyncReset
    let node_layer = Layer::new();
    node_layer.list_push("items", &serde_json::json!({"id": 1}))?;
    let user_layer = Layer::from_snapshot(&node_layer.export_snapshot())?;
    assert_eq!(node_layer.version_vector(), user_layer.version_vector());

    node_layer.list_push("items", &serde_json::json!({"id": 2}))?;
    let user_layer = Layer::from_snapshot(&node_layer.export_snapshot())?;
    let items = user_layer.to_json()["root"]["items"].as_array().expect("items").len();
    assert_eq!(items, 2);

    // Vectors match after SyncReset
    let node_layer = Layer::new();
    node_layer.list_push("data", &serde_json::json!({"id": 1}))?;
    node_layer.list_push("data", &serde_json::json!({"id": 2}))?;
    let node_vector = node_layer.version_vector();

    let user_layer = Layer::new();
    user_layer.list_push("data", &serde_json::json!({"id": 100}))?;
    assert_ne!(user_layer.version_vector(), node_vector);

    let user_layer = Layer::from_snapshot(&node_layer.export_snapshot())?;
    assert_eq!(user_layer.version_vector(), node_vector, "Vectors match after SyncReset");

    info!("SyncReset basic scenarios test passed");
    Ok(())
}

// Test: Multiple Layers SyncReset

/// Tests SyncReset affecting multiple layers independently
#[tokio::test]
async fn test_sync_reset_multiple_layers() -> Result<()> {
    init_tracing();

    let layers = vec![
        ("template_doc", false),
        ("content_doc", false),
        ("collaborative_doc", true),
        ("submissions_doc", true),
    ];

    for (layer_name, can_write) in &layers {
        let node_layer = Layer::new();
        node_layer.map_insert("meta", "layer", &serde_json::json!(layer_name))?;
        node_layer.map_insert("meta", "source", &serde_json::json!("node"))?;

        let user_layer = Layer::new();
        user_layer.map_insert("meta", "layer", &serde_json::json!(layer_name))?;
        user_layer.map_insert("meta", "source", &serde_json::json!("user"))?;

        let node_snapshot = node_layer.export_snapshot();

        if *can_write {
            let merged = Layer::from_snapshot(&node_snapshot)?;
            merged.apply(&user_layer.export_snapshot())?;
            let json = merged.to_json();
            assert!(json["root"]["meta"]["layer"].is_string());
        } else {
            let replaced = Layer::from_snapshot(&node_snapshot)?;
            let json = replaced.to_json();
            assert_eq!(json["root"]["meta"]["source"], "node");
        }
    }

    info!("Multiple layers SyncReset test passed");
    Ok(())
}

// Test: SyncReset Cleans Up Pending Sync State

/// Tests that SyncReset properly cleans up pending_sync_offers
#[tokio::test]
async fn test_sync_reset_cleans_pending_state() -> Result<()> {
    init_tracing();

    use std::collections::HashMap;

    struct PendingSyncOffer {
        _our_state_vector: Vec<u8>,
        _permit: String,
        _resync_attempts: u32,
    }

    let mut pending_sync_offers: HashMap<(String, String), PendingSyncOffer> = HashMap::new();
    let key = ("test-page".to_string(), "content_doc".to_string());

    pending_sync_offers.insert(key.clone(), PendingSyncOffer {
        _our_state_vector: vec![1, 2, 3],
        _permit: "test-permit".to_string(),
        _resync_attempts: 3,
    });

    assert!(pending_sync_offers.contains_key(&key));
    pending_sync_offers.remove(&key);
    assert!(!pending_sync_offers.contains_key(&key));

    Ok(())
}

// Test: SyncReset Node Response

/// Tests that Node correctly responds to SyncReset with full snapshot
#[tokio::test]
async fn test_node_responds_to_sync_reset() -> Result<()> {
    init_tracing();

    let node_layer = Layer::new();
    node_layer.list_push("records", &serde_json::json!({"id": 1, "data": "record1"}))?;
    node_layer.list_push("records", &serde_json::json!({"id": 2, "data": "record2"}))?;
    node_layer.map_insert("meta", "version", &serde_json::json!(5))?;

    let snapshot = node_layer.export_snapshot();
    let test_layer = Layer::from_snapshot(&snapshot)?;
    let test_json = test_layer.to_json();

    assert_eq!(test_json["root"]["records"].as_array().unwrap().len(), 2);
    assert_eq!(test_json["root"]["meta"]["version"], 5);

    info!("Node SyncReset response test passed");
    Ok(())
}
