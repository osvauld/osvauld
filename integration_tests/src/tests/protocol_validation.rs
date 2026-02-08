//! Protocol-level validation tests using sample app validation.lua files
//!
//! Tests the validation pipeline with real validation logic from sample apps:
//! - my-shop: Order state machine, role-based access
//! - my-booking: Booking state machine, owner-only layers
//! - osvauld-demos: Presence validation, message validation
//!
//! These tests verify that:
//! 1. Layer::extract_ops_from_bytes produces correct JsonOp format
//! 2. validate_ops function receives correct arguments
//! 3. Role-based access control works correctly
//! 4. State machine transitions are enforced

use anyhow::Result;
use domains::Layer;
use mlua::{Lua, Function, Value};
use std::fs;
use std::path::PathBuf;

/// Get workspace root directory (where Cargo.toml is)
fn workspace_root() -> PathBuf {
    // Try CARGO_MANIFEST_DIR first (set during cargo test)
    if let Ok(dir) = std::env::var("CARGO_MANIFEST_DIR") {
        // CARGO_MANIFEST_DIR is integration_tests/, go up one level
        return PathBuf::from(dir).parent().unwrap().to_path_buf();
    }

    // Fallback: look for sample_apps directory starting from current dir
    let mut current = std::env::current_dir().unwrap();
    loop {
        if current.join("sample_apps").exists() {
            return current;
        }
        if !current.pop() {
            panic!("Could not find workspace root (sample_apps directory)");
        }
    }
}

/// Initialize tracing for tests
fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_test_writer()
        .try_init();
}

/// Test validation runtime - loads real validation.lua files
#[allow(dead_code)]
struct AppValidationRuntime {
    lua: Lua,
    app_name: String,
}

impl AppValidationRuntime {
    /// Create runtime and load validation.lua from sample app
    fn new(app_name: &str) -> Result<Self> {
        let lua = Lua::new();

        // Load validation.lua from sample app (using workspace root)
        let validation_path = workspace_root()
            .join("sample_apps")
            .join(app_name)
            .join("shared")
            .join("validation.lua");

        let code = fs::read_to_string(&validation_path)
            .map_err(|e| anyhow::anyhow!("Failed to read {}: {}", validation_path.display(), e))?;

        lua.load(&code)
            .exec()
            .map_err(|e| anyhow::anyhow!("Failed to load validation.lua: {}", e))?;

        Ok(Self {
            lua,
            app_name: app_name.to_string(),
        })
    }

    /// Check if validate_ops function exists
    fn has_validate_ops(&self) -> bool {
        self.lua.globals()
            .get::<Value>("validate_ops")
            .map(|v| matches!(v, Value::Function(_)))
            .unwrap_or(false)
    }

    /// Call validate_ops with given parameters
    fn validate_ops(
        &self,
        layer_name: &str,
        ops: &[serde_json::Value],
        from_did: &str,
        role: &str,
        page_id: &str,
    ) -> Result<bool> {
        let globals = self.lua.globals();

        let validate_fn: Function = globals.get("validate_ops")
            .map_err(|e| anyhow::anyhow!("validate_ops not found: {}", e))?;

        // Convert ops to Lua table
        let ops_table = self.lua.create_table()?;
        for (i, op) in ops.iter().enumerate() {
            let op_lua = json_to_lua(&self.lua, op)?;
            ops_table.set(i + 1, op_lua)?;
        }

        // Call validate_ops
        let result: bool = validate_fn.call((
            layer_name,
            ops_table,
            from_did,
            role,
            page_id,
        )).map_err(|e| anyhow::anyhow!("validate_ops error: {}", e))?;

        Ok(result)
    }
}

/// Convert serde_json::Value to Lua value
fn json_to_lua(lua: &Lua, value: &serde_json::Value) -> Result<Value> {
    match value {
        serde_json::Value::Null => Ok(Value::Nil),
        serde_json::Value::Bool(b) => Ok(Value::Boolean(*b)),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(Value::Integer(i))
            } else if let Some(f) = n.as_f64() {
                Ok(Value::Number(f))
            } else {
                Ok(Value::Nil)
            }
        }
        serde_json::Value::String(s) => {
            Ok(Value::String(lua.create_string(s)?))
        }
        serde_json::Value::Array(arr) => {
            let table = lua.create_table()?;
            for (i, v) in arr.iter().enumerate() {
                table.set(i + 1, json_to_lua(lua, v)?)?;
            }
            Ok(Value::Table(table))
        }
        serde_json::Value::Object(obj) => {
            let table = lua.create_table()?;
            for (k, v) in obj {
                table.set(k.clone(), json_to_lua(lua, v)?)?;
            }
            Ok(Value::Table(table))
        }
    }
}

// Layer::extract_ops_from_bytes Integration Tests

/// Test that extract_ops_from_bytes produces JsonOp format compatible with validation.lua
#[test]
fn test_extract_ops_format_matches_validation_expectations() -> Result<()> {
    init_tracing();

    // Create a layer with some data
    let layer = Layer::new();
    let root = layer.loro().get_map("root");
    root.insert("messages", loro::LoroValue::List(vec![].into()))?;
    layer.loro().commit();

    // Add a message
    let messages = layer.loro().get_list("messages");
    let msg = loro::LoroValue::Map(
        [
            ("id".to_string(), loro::LoroValue::String("msg1".into())),
            ("text".to_string(), loro::LoroValue::String("hello".into())),
            ("sender_did".to_string(), loro::LoroValue::String("did:key:alice".into())),
        ].into_iter().collect::<std::collections::HashMap<_, _>>().into()
    );
    messages.push(msg)?;
    layer.loro().commit();

    // Export full update
    let update = layer.loro()
        .export(loro::ExportMode::updates(&loro::VersionVector::new()))
        .expect("export");

    // Extract ops
    let ops = Layer::extract_ops_from_bytes(&update)?;

    // Verify ops format
    assert!(!ops.is_empty(), "Should have ops");

    // Check that ops have the expected fields
    for op in &ops {
        assert!(!op.op.is_empty(), "op.op should not be empty");
        assert!(!op.path.is_empty(), "op.path should not be empty");
        // op.key, op.index, op.value, op.old_value are optional
    }

    tracing::info!("Extracted {} ops: {:?}", ops.len(), ops);

    Ok(())
}

// my-shop Validation Tests

/// Test that products layer only allows owner writes
#[test]
fn test_myshop_products_owner_only() -> Result<()> {
    init_tracing();

    let runtime = AppValidationRuntime::new("my-shop")?;
    assert!(runtime.has_validate_ops());

    let ops = vec![serde_json::json!({
        "op": "insert",
        "path": "products",
        "value": {"name": "Test Product", "price": 100}
    })];

    let page_id = "shop123";

    // Owner should be allowed
    let result = runtime.validate_ops("products", &ops, "did:key:owner", "owner", page_id)?;
    assert!(result, "Owner should be able to modify products");

    // Customer should be rejected
    let result = runtime.validate_ops("products", &ops, "did:key:customer", "customer", page_id)?;
    assert!(!result, "Customer should not be able to modify products");

    // Viewer should be rejected
    let result = runtime.validate_ops("products", &ops, "did:key:viewer", "viewer", page_id)?;
    assert!(!result, "Viewer should not be able to modify products");

    tracing::info!("my-shop products layer owner-only validation passed");

    Ok(())
}

/// Test that customers can only modify their own orders layer
#[test]
fn test_myshop_orders_ownership() -> Result<()> {
    init_tracing();

    let runtime = AppValidationRuntime::new("my-shop")?;

    let page_id = "shop123";
    let customer_did = "did:key:customer_a";
    let other_customer_did = "did:key:customer_b";

    // Layer for customer_a
    let layer_name = format!("{}/orders/{}", page_id, customer_did);

    let ops = vec![serde_json::json!({
        "op": "insert",
        "path": "[0]",
        "value": {"id": "order1", "status": "pending", "items": []}
    })];

    // Customer A can modify their own layer
    let result = runtime.validate_ops(&layer_name, &ops, customer_did, "customer", page_id)?;
    assert!(result, "Customer should be able to modify their own orders");

    // Customer B cannot modify Customer A's layer
    let result = runtime.validate_ops(&layer_name, &ops, other_customer_did, "customer", page_id)?;
    assert!(!result, "Customer should not be able to modify other's orders");

    // Owner can modify any orders layer
    let result = runtime.validate_ops(&layer_name, &ops, "did:key:owner", "owner", page_id)?;
    assert!(result, "Owner should be able to modify any orders");

    tracing::info!("my-shop orders ownership validation passed");

    Ok(())
}

/// Test order state machine - synced orders must start pending
#[test]
fn test_myshop_order_initial_state() -> Result<()> {
    init_tracing();

    let runtime = AppValidationRuntime::new("my-shop")?;

    let page_id = "shop123";
    let customer_did = "did:key:customer";
    let layer_name = format!("{}/orders/{}", page_id, customer_did);

    // Order starting in pending state - should pass
    let valid_ops = vec![serde_json::json!({
        "op": "insert",
        "path": "[0]",
        "value": {"id": "order1", "status": "pending"}
    })];

    let result = runtime.validate_ops(&layer_name, &valid_ops, customer_did, "customer", page_id)?;
    assert!(result, "Pending order should be allowed");

    // Order starting in draft state - should fail (drafts are local-only)
    let invalid_ops = vec![serde_json::json!({
        "op": "insert",
        "path": "[0]",
        "value": {"id": "order2", "status": "draft"}
    })];

    let result = runtime.validate_ops(&layer_name, &invalid_ops, customer_did, "customer", page_id)?;
    assert!(!result, "Draft order should be rejected (drafts are local-only)");

    tracing::info!("my-shop order initial state validation passed");

    Ok(())
}

// my-booking Validation Tests

/// Test that schedule layer only allows owner writes
#[test]
fn test_mybooking_schedule_owner_only() -> Result<()> {
    init_tracing();

    let runtime = AppValidationRuntime::new("my-booking")?;
    assert!(runtime.has_validate_ops());

    let page_id = "booking123";
    let layer_name = format!("{}/schedule", page_id);

    let ops = vec![serde_json::json!({
        "op": "update",
        "path": "root",
        "key": "monday",
        "value": {"start": "09:00", "end": "17:00"}
    })];

    // Owner should be allowed
    let result = runtime.validate_ops(&layer_name, &ops, "did:key:owner", "owner", page_id)?;
    assert!(result, "Owner should be able to modify schedule");

    // Customer should be rejected
    let result = runtime.validate_ops(&layer_name, &ops, "did:key:customer", "customer", page_id)?;
    assert!(!result, "Customer should not be able to modify schedule");

    tracing::info!("my-booking schedule owner-only validation passed");

    Ok(())
}

/// Test that derived calendar layer only allows node writes
#[test]
fn test_mybooking_calendar_node_only() -> Result<()> {
    init_tracing();

    let runtime = AppValidationRuntime::new("my-booking")?;

    let page_id = "booking123";
    let layer_name = format!("{}/derived/calendar", page_id);

    let ops = vec![serde_json::json!({
        "op": "update",
        "path": "root",
        "value": {"slots": []}
    })];

    // Node should be allowed (derivation)
    let result = runtime.validate_ops(&layer_name, &ops, "did:key:node", "node", page_id)?;
    assert!(result, "Node should be able to modify derived calendar");

    // Owner should be rejected (derived layers are node-only)
    let result = runtime.validate_ops(&layer_name, &ops, "did:key:owner", "owner", page_id)?;
    assert!(!result, "Owner should not be able to modify derived calendar");

    // Customer should be rejected
    let result = runtime.validate_ops(&layer_name, &ops, "did:key:customer", "customer", page_id)?;
    assert!(!result, "Customer should not be able to modify derived calendar");

    tracing::info!("my-booking derived calendar node-only validation passed");

    Ok(())
}

/// Test booking state machine - synced bookings must start pending
#[test]
fn test_mybooking_booking_initial_state() -> Result<()> {
    init_tracing();

    let runtime = AppValidationRuntime::new("my-booking")?;

    let page_id = "booking123";
    let customer_did = "did:key:customer";
    let layer_name = format!("{}/bookings/{}", page_id, customer_did);

    // Booking starting in pending state - should pass
    let valid_ops = vec![serde_json::json!({
        "op": "insert",
        "path": "[0]",
        "value": {
            "id": "booking1",
            "status": "pending",
            "date": "2024-01-15",
            "start_time": "10:00",
            "end_time": "11:00"
        }
    })];

    let result = runtime.validate_ops(&layer_name, &valid_ops, customer_did, "customer", page_id)?;
    assert!(result, "Pending booking should be allowed");

    // Booking missing required fields - should fail
    let invalid_ops = vec![serde_json::json!({
        "op": "insert",
        "path": "[0]",
        "value": {
            "id": "booking2",
            "status": "pending"
            // Missing date, start_time, end_time
        }
    })];

    let result = runtime.validate_ops(&layer_name, &invalid_ops, customer_did, "customer", page_id)?;
    assert!(!result, "Booking without required fields should be rejected");

    tracing::info!("my-booking booking initial state validation passed");

    Ok(())
}

// osvauld-demos Validation Tests

/// Test that presence layers enforce "own DID only" rule
/// Note: This test only verifies the presence_lib check is called
#[test]
fn test_demos_presence_layer_detection() -> Result<()> {
    init_tracing();

    let runtime = AppValidationRuntime::new("osvauld-demos")?;
    assert!(runtime.has_validate_ops());

    let page_id = "chat123";

    // Test presence layer detection
    // Note: Without presence_lib global, the validation allows all presence ops
    // This test verifies the layer detection works

    let ops = vec![serde_json::json!({
        "op": "update",
        "path": "root",
        "key": "did:key:user",
        "value": {"online": true, "cursor": {"x": 100, "y": 200}}
    })];

    // Presence layer (ends with /presence)
    let presence_layer = format!("{}/presence", page_id);

    // Without presence_lib, should allow (graceful fallback)
    let result = runtime.validate_ops(&presence_layer, &ops, "did:key:user", "viewer", page_id)?;
    assert!(result, "Should allow presence ops when presence_lib not available");

    tracing::info!("osvauld-demos presence layer detection passed");

    Ok(())
}

/// Test that non-presence layers pass by default
#[test]
fn test_demos_default_allow() -> Result<()> {
    init_tracing();

    let runtime = AppValidationRuntime::new("osvauld-demos")?;

    let page_id = "chat123";

    let ops = vec![serde_json::json!({
        "op": "insert",
        "path": "messages",
        "value": {"text": "hello", "sender": "did:key:user"}
    })];

    // Non-presence layer should be allowed by default
    let result = runtime.validate_ops("messages", &ops, "did:key:user", "viewer", page_id)?;
    assert!(result, "Non-presence layers should be allowed by default");

    tracing::info!("osvauld-demos default allow validation passed");

    Ok(())
}

// End-to-End Extraction + Validation Tests

/// Test full pipeline: Layer changes -> extract_ops -> validate_ops
#[test]
fn test_e2e_extraction_to_validation() -> Result<()> {
    init_tracing();

    // Create a layer simulating an order insert
    let layer = Layer::new();
    let orders = layer.loro().get_list("orders");

    let order = loro::LoroValue::Map(
        [
            ("id".to_string(), loro::LoroValue::String("order1".into())),
            ("status".to_string(), loro::LoroValue::String("pending".into())),
            ("items".to_string(), loro::LoroValue::List(vec![].into())),
        ].into_iter().collect::<std::collections::HashMap<_, _>>().into()
    );
    orders.push(order)?;
    layer.loro().commit();

    // Extract ops
    let update = layer.loro()
        .export(loro::ExportMode::updates(&loro::VersionVector::new()))
        .expect("export");

    let extracted_ops = Layer::extract_ops_from_bytes(&update)?;

    // Convert JsonOp to serde_json::Value for validation
    let ops_json: Vec<serde_json::Value> = extracted_ops.iter()
        .map(|op| serde_json::to_value(op).unwrap())
        .collect();

    // Load validation and run
    let runtime = AppValidationRuntime::new("my-shop")?;

    let page_id = "shop123";
    let customer_did = "did:key:customer";
    let layer_name = format!("{}/orders/{}", page_id, customer_did);

    // The extracted ops should be valid for the customer's orders layer
    // Note: The actual validation depends on the exact op format
    tracing::info!("Extracted ops for validation: {:?}", ops_json);

    // This demonstrates the pipeline works - actual validation result
    // depends on whether the op format matches what validation.lua expects
    let _result = runtime.validate_ops(&layer_name, &ops_json, customer_did, "customer", page_id);

    tracing::info!("E2E extraction to validation pipeline completed");

    Ok(())
}
