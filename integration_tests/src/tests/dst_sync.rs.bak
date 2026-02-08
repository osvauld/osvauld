//! DST Sync Tests - Strong Eventual Consistency and No Lost Writes
//!
//! Tests core CRDT synchronization invariants:
//! - INV-D1: Identical state after sync
//! - INV-D3: No updates lost during partition for authorized writes

use anyhow::Result;
use domains::Layer;
use tracing::info;

use crate::scenario::init_tracing;

// Test: Convergence (SEC + Concurrent Edits)

/// Validates INV-D1: After sync, two peers have identical state
///
/// Tests both same-ops-different-order and concurrent-different-ops scenarios
#[tokio::test]
async fn test_convergence() -> Result<()> {
    init_tracing();

    // Scenario 1: Same operations in different order
    let layer_a = Layer::new();
    let layer_b = Layer::new();

    layer_a.list_push("items", &serde_json::json!({"id": 1, "value": "first"}))?;
    layer_a.list_push("items", &serde_json::json!({"id": 2, "value": "second"}))?;
    layer_a.list_push("items", &serde_json::json!({"id": 3, "value": "third"}))?;

    layer_b.list_push("items", &serde_json::json!({"id": 3, "value": "third"}))?;
    layer_b.list_push("items", &serde_json::json!({"id": 1, "value": "first"}))?;
    layer_b.list_push("items", &serde_json::json!({"id": 2, "value": "second"}))?;

    let a_snapshot = layer_a.export_snapshot();
    let b_snapshot = layer_b.export_snapshot();

    layer_b.apply(&a_snapshot)?;
    layer_a.apply(&b_snapshot)?;

    assert_eq!(layer_a.to_json(), layer_b.to_json(), "After sync, layer content should match (INV-D1)");

    // Scenario 2: Concurrent different edits from shared baseline
    let baseline = Layer::new();
    baseline.list_push("messages", &serde_json::json!({"id": 0, "text": "baseline"}))?;
    let baseline_snapshot = baseline.export_snapshot();

    let peer_a = Layer::from_snapshot(&baseline_snapshot)?;
    let peer_b = Layer::from_snapshot(&baseline_snapshot)?;

    peer_a.list_push("messages", &serde_json::json!({"id": "a-1", "from": "peer_a"}))?;
    peer_a.list_push("messages", &serde_json::json!({"id": "a-2", "from": "peer_a"}))?;
    peer_b.list_push("messages", &serde_json::json!({"id": "b-1", "from": "peer_b"}))?;
    peer_b.list_push("messages", &serde_json::json!({"id": "b-2", "from": "peer_b"}))?;

    let a_updates = peer_a.export_snapshot();
    let b_updates = peer_b.export_snapshot();
    peer_a.apply(&b_updates)?;
    peer_b.apply(&a_updates)?;

    let a_json = peer_a.to_json();
    let b_json = peer_b.to_json();
    assert_eq!(a_json, b_json, "Peers should converge to same state");

    let messages = a_json["root"]["messages"].as_array().expect("messages array");
    assert_eq!(messages.len(), 5, "Should have all 5 messages after merge");

    info!("Convergence test passed");

    Ok(())
}

// Test: No Lost Writes (Authorized Operations)

/// Validates INV-D3: Authorized operations are never lost during sync
///
/// **Setup**: User makes edits while "offline" (before sync)
/// **Assert**: After sync, all authorized operations are preserved
#[tokio::test]
async fn test_no_lost_authorized_writes() -> Result<()> {
    init_tracing();

    let node_layer = Layer::new();
    node_layer.list_push("content", &serde_json::json!({"id": "node-1", "text": "node message"}))?;

    let node_snapshot = node_layer.export_snapshot();
    let viewer_layer = Layer::from_snapshot(&node_snapshot)?;

    viewer_layer.list_push("content", &serde_json::json!({"id": "viewer-1", "text": "viewer edit 1"}))?;
    viewer_layer.list_push("content", &serde_json::json!({"id": "viewer-2", "text": "viewer edit 2"}))?;
    node_layer.list_push("content", &serde_json::json!({"id": "node-2", "text": "node message 2"}))?;

    let viewer_updates = viewer_layer.export_snapshot();
    let node_updates = node_layer.export_snapshot();
    node_layer.apply(&viewer_updates)?;
    viewer_layer.apply(&node_updates)?;

    let node_json = node_layer.to_json();
    let viewer_json = viewer_layer.to_json();

    let node_content = node_json["root"]["content"].as_array().expect("content should be array");
    let viewer_content = viewer_json["root"]["content"].as_array().expect("content should be array");

    assert_eq!(node_content.len(), 4, "Node should have all 4 messages");
    assert_eq!(viewer_content.len(), 4, "Viewer should have all 4 messages");
    assert_eq!(
        node_layer.version_vector(),
        viewer_layer.version_vector(),
        "State vectors should match after sync (INV-D1)"
    );

    Ok(())
}

// Test: Typed Operations Converge (Map + Counter)

/// Tests that map and counter operations converge correctly
#[tokio::test]
async fn test_typed_operations_converge() -> Result<()> {
    init_tracing();

    // Map operations
    let baseline = Layer::new();
    baseline.map_insert("config", "version", &serde_json::json!(1))?;
    let baseline_snapshot = baseline.export_snapshot();

    let peer_a = Layer::from_snapshot(&baseline_snapshot)?;
    let peer_b = Layer::from_snapshot(&baseline_snapshot)?;

    peer_a.map_insert("config", "theme", &serde_json::json!("dark"))?;
    peer_a.map_insert("config", "lang", &serde_json::json!("en"))?;
    peer_b.map_insert("config", "timezone", &serde_json::json!("UTC"))?;
    peer_b.map_insert("config", "notifications", &serde_json::json!(true))?;

    let a_snapshot = peer_a.export_snapshot();
    let b_snapshot = peer_b.export_snapshot();
    peer_a.apply(&b_snapshot)?;
    peer_b.apply(&a_snapshot)?;

    let a_json = peer_a.to_json();
    let b_json = peer_b.to_json();
    assert_eq!(a_json, b_json, "Maps should converge");

    let config = &a_json["root"]["config"];
    assert_eq!(config["version"], 1);
    assert_eq!(config["theme"], "dark");
    assert_eq!(config["lang"], "en");
    assert_eq!(config["timezone"], "UTC");
    assert_eq!(config["notifications"], true);

    // Counter operations
    let baseline = Layer::new();
    baseline.counter_inc("likes", 0)?;
    let baseline_snapshot = baseline.export_snapshot();

    let peer_a = Layer::from_snapshot(&baseline_snapshot)?;
    let peer_b = Layer::from_snapshot(&baseline_snapshot)?;

    peer_a.counter_inc("likes", 5)?;
    peer_a.counter_inc("likes", 3)?;
    peer_b.counter_inc("likes", 2)?;
    peer_b.counter_inc("likes", 7)?;

    let a_snapshot = peer_a.export_snapshot();
    let b_snapshot = peer_b.export_snapshot();
    peer_a.apply(&b_snapshot)?;
    peer_b.apply(&a_snapshot)?;

    assert_eq!(peer_a.to_json(), peer_b.to_json(), "Counter state should converge");

    info!("Typed operations convergence test passed");

    Ok(())
}

// Test: Incremental Sync Works

/// Tests incremental sync using version vectors
///
/// **Setup**: Peer A has state, exports updates since peer B's vector
/// **Assert**: Peer B gets only new updates, not full snapshot
#[tokio::test]
async fn test_incremental_sync_works() -> Result<()> {
    init_tracing();

    let peer_a = Layer::new();
    peer_a.list_push("items", &serde_json::json!({"id": 1}))?;
    peer_a.list_push("items", &serde_json::json!({"id": 2}))?;

    let initial_snapshot = peer_a.export_snapshot();
    let peer_b = Layer::from_snapshot(&initial_snapshot)?;
    let b_vector = peer_b.version_vector();

    peer_a.list_push("items", &serde_json::json!({"id": 3}))?;
    peer_a.list_push("items", &serde_json::json!({"id": 4}))?;

    let incremental_updates = peer_a.export_updates(&b_vector)?;
    let full_snapshot = peer_a.export_snapshot();

    assert!(
        incremental_updates.len() < full_snapshot.len(),
        "Incremental updates should be smaller than full snapshot"
    );

    peer_b.apply(&incremental_updates)?;

    let b_json = peer_b.to_json();
    let items = b_json["root"]["items"].as_array().expect("items array");
    assert_eq!(items.len(), 4, "B should have all 4 items after incremental sync");

    assert_eq!(
        peer_a.version_vector(),
        peer_b.version_vector(),
        "Vectors should match after incremental sync"
    );

    Ok(())
}
