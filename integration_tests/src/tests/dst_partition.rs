//! DST Partition Tests - Convergence After Network Partition
//!
//! Tests partition recovery invariants:
//! - INV-D1: States converge after partition heals
//! - INV-D3: No authorized edits lost during partition

use anyhow::Result;
use domains::Layer;
use tracing::info;

use crate::scenario::init_tracing;

// Test: Partition Convergence (Basic + Asymmetric)

/// Validates INV-D1, INV-D3: Convergence after partition (symmetric and asymmetric)
#[tokio::test]
async fn test_partition_convergence() -> Result<()> {
    init_tracing();

    // Symmetric: both sides make edits during partition
    let initial = Layer::new();
    initial.list_push("messages", &serde_json::json!({"id": "initial", "text": "hello"}))?;
    let initial_snapshot = initial.export_snapshot();

    let owner_layer = Layer::from_snapshot(&initial_snapshot)?;
    let node_layer = Layer::from_snapshot(&initial_snapshot)?;

    owner_layer.list_push("messages", &serde_json::json!({"id": "owner-1", "text": "from owner during partition"}))?;
    owner_layer.list_push("messages", &serde_json::json!({"id": "owner-2", "text": "another owner message"}))?;
    node_layer.list_push("messages", &serde_json::json!({"id": "node-1", "text": "from node during partition"}))?;
    node_layer.map_insert("metadata", "last_active", &serde_json::json!("node-timestamp"))?;

    let owner_updates = owner_layer.export_snapshot();
    let node_updates = node_layer.export_snapshot();
    owner_layer.apply(&node_updates)?;
    node_layer.apply(&owner_updates)?;

    let owner_json = owner_layer.to_json();
    let node_json = node_layer.to_json();
    assert_eq!(owner_json, node_json, "Content should match after partition heals (INV-D1)");

    let messages = owner_json["root"]["messages"].as_array().expect("messages array");
    assert_eq!(messages.len(), 4, "Should have initial + 2 owner + 1 node messages");
    assert_eq!(owner_json["root"]["metadata"]["last_active"], "node-timestamp");

    // Asymmetric: only one side makes edits
    let initial = Layer::new();
    let initial_snap = initial.export_snapshot();
    let owner_layer = Layer::from_snapshot(&initial_snap)?;
    let node_layer = Layer::from_snapshot(&initial_snap)?;

    for i in 1..=10 {
        owner_layer.list_push("tasks", &serde_json::json!({"id": i, "title": format!("Task {}", i)}))?;
    }

    let owner_snap = owner_layer.export_snapshot();
    node_layer.apply(&owner_snap)?;

    let node_json = node_layer.to_json();
    let tasks = node_json["root"]["tasks"].as_array().expect("tasks array");
    assert_eq!(tasks.len(), 10, "Node should have all 10 tasks from owner");
    assert_eq!(owner_layer.version_vector(), node_layer.version_vector());

    info!("Partition convergence test passed");
    Ok(())
}

// Test: Extended Partitions (Multiple + Long with Many Edits)

/// Tests recovery from multiple sequential partitions and extended partition with many edits
#[tokio::test]
async fn test_extended_partitions() -> Result<()> {
    init_tracing();

    // Multiple sequential partitions
    let owner_layer = Layer::new();
    let node_layer = Layer::new();

    // Phase 1: Connected
    owner_layer.list_push("log", &serde_json::json!({"phase": 1, "from": "owner"}))?;
    let owner_snap = owner_layer.export_snapshot();
    node_layer.apply(&owner_snap)?;

    // Phase 2: First Partition
    owner_layer.list_push("log", &serde_json::json!({"phase": 2, "from": "owner", "during": "partition1"}))?;
    node_layer.list_push("log", &serde_json::json!({"phase": 2, "from": "node", "during": "partition1"}))?;

    // Phase 3: Heal
    let owner_snap2 = owner_layer.export_snapshot();
    let node_snap2 = node_layer.export_snapshot();
    owner_layer.apply(&node_snap2)?;
    node_layer.apply(&owner_snap2)?;
    assert_eq!(owner_layer.version_vector(), node_layer.version_vector(), "Should converge after first partition");

    // Phase 4: Second Partition
    owner_layer.list_push("log", &serde_json::json!({"phase": 4, "from": "owner", "during": "partition2"}))?;
    node_layer.list_push("log", &serde_json::json!({"phase": 4, "from": "node", "during": "partition2"}))?;

    // Phase 5: Heal
    let owner_snap3 = owner_layer.export_snapshot();
    let node_snap3 = node_layer.export_snapshot();
    owner_layer.apply(&node_snap3)?;
    node_layer.apply(&owner_snap3)?;
    assert_eq!(owner_layer.version_vector(), node_layer.version_vector(), "Should converge after second partition");

    let final_json = owner_layer.to_json();
    let log = final_json["root"]["log"].as_array().expect("log array");
    assert_eq!(log.len(), 5, "Should have all log entries from all phases");

    // Long partition with many edits
    let initial = Layer::new();
    initial.list_push("items", &serde_json::json!({"id": 0, "baseline": true}))?;
    let initial_snap = initial.export_snapshot();

    let owner_layer = Layer::from_snapshot(&initial_snap)?;
    let node_layer = Layer::from_snapshot(&initial_snap)?;

    for i in 1..=50 {
        owner_layer.list_push("items", &serde_json::json!({"id": format!("owner-{}", i), "from": "owner"}))?;
        node_layer.list_push("items", &serde_json::json!({"id": format!("node-{}", i), "from": "node"}))?;
    }

    let owner_snap = owner_layer.export_snapshot();
    let node_snap = node_layer.export_snapshot();
    owner_layer.apply(&node_snap)?;
    node_layer.apply(&owner_snap)?;

    let owner_json = owner_layer.to_json();
    let node_json = node_layer.to_json();
    assert_eq!(owner_json, node_json, "Content should converge after long partition");

    let items = owner_json["root"]["items"].as_array().expect("items array");
    assert_eq!(items.len(), 101, "Should have all 101 items");
    assert_eq!(items.iter().filter(|i| i["from"] == "owner").count(), 50);
    assert_eq!(items.iter().filter(|i| i["from"] == "node").count(), 50);

    info!("Extended partitions test passed");
    Ok(())
}

// Test: Partition During Complex Operations

/// Tests partition recovery with different operation types
#[tokio::test]
async fn test_partition_mixed_operations() -> Result<()> {
    init_tracing();

    let baseline = Layer::new();
    baseline.list_push("events", &serde_json::json!({"event": "init"}))?;
    baseline.map_insert("config", "version", &serde_json::json!(1))?;
    let baseline_snapshot = baseline.export_snapshot();

    let owner_layer = Layer::from_snapshot(&baseline_snapshot)?;
    let node_layer = Layer::from_snapshot(&baseline_snapshot)?;

    owner_layer.list_push("events", &serde_json::json!({"event": "owner-login"}))?;
    owner_layer.map_insert("config", "owner_setting", &serde_json::json!("value1"))?;
    owner_layer.counter_inc("owner_counter", 5)?;

    node_layer.list_push("events", &serde_json::json!({"event": "node-startup"}))?;
    node_layer.map_insert("config", "node_setting", &serde_json::json!("value2"))?;
    node_layer.counter_inc("node_counter", 10)?;

    let owner_snap = owner_layer.export_snapshot();
    let node_snap = node_layer.export_snapshot();
    owner_layer.apply(&node_snap)?;
    node_layer.apply(&owner_snap)?;

    let owner_json = owner_layer.to_json();
    let node_json = node_layer.to_json();
    assert_eq!(owner_json, node_json, "Mixed operations should converge");

    let events = owner_json["root"]["events"].as_array().expect("events array");
    assert_eq!(events.len(), 3, "Should have init + owner + node events");

    let config = &owner_json["root"]["config"];
    assert!(config["owner_setting"].is_string());
    assert!(config["node_setting"].is_string());
    assert_eq!(config["version"], 1);

    info!("Mixed operations partition recovery successful");
    Ok(())
}

// Test: Partition with Conflicting Keys

/// Tests partition recovery when both sides modify the same keys
#[tokio::test]
async fn test_partition_conflicting_keys() -> Result<()> {
    init_tracing();

    let initial = Layer::new();
    initial.map_insert("state", "status", &serde_json::json!("initial"))?;
    let initial_snap = initial.export_snapshot();

    let owner_layer = Layer::from_snapshot(&initial_snap)?;
    let node_layer = Layer::from_snapshot(&initial_snap)?;

    owner_layer.map_insert("state", "status", &serde_json::json!("owner-updated"))?;
    node_layer.map_insert("state", "status", &serde_json::json!("node-updated"))?;

    let owner_snap = owner_layer.export_snapshot();
    let node_snap = node_layer.export_snapshot();
    owner_layer.apply(&node_snap)?;
    node_layer.apply(&owner_snap)?;

    assert_eq!(owner_layer.to_json(), node_layer.to_json(), "Conflicting keys should converge to same state");

    info!("Conflicting key resolution test passed");
    Ok(())
}
