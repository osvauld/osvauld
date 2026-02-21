//! Protocol-level validation tests using sample app validation.lua files
//!
//! Tests the validation pipeline with real validation logic from sample apps:
//! - my-shop: Order state machine, role-based access
//! - my-booking: Booking state machine, owner-only layers
//! - osvauld-demos: Presence validation, message validation
//!
//! These are standalone Lua tests — no P2P, just Layer + mlua.

use anyhow::Result;
use domains::Layer;
use mlua::{Function, Lua, Value};
use std::fs;

use crate::fixtures::{init_tracing, workspace_root};

/// Validation runtime — loads real validation.lua files from sample apps
struct AppValidationRuntime {
    lua: Lua,
}

impl AppValidationRuntime {
    /// Create runtime and load validation.lua from sample app
    fn new(app_name: &str) -> Result<Self> {
        let lua = Lua::new();

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

        Ok(Self { lua })
    }

    fn has_validate_ops(&self) -> bool {
        self.lua
            .globals()
            .get::<Value>("validate_ops")
            .map(|v| matches!(v, Value::Function(_)))
            .unwrap_or(false)
    }

    fn validate_ops(
        &self,
        layer_name: &str,
        ops: &[serde_json::Value],
        from_did: &str,
        role: &str,
        page_id: &str,
    ) -> Result<bool> {
        let globals = self.lua.globals();

        let validate_fn: Function = globals
            .get("validate_ops")
            .map_err(|e| anyhow::anyhow!("validate_ops not found: {}", e))?;

        let ops_table = self.lua.create_table()?;
        for (i, op) in ops.iter().enumerate() {
            let op_lua = json_to_lua(&self.lua, op)?;
            ops_table.set(i + 1, op_lua)?;
        }

        let result: bool = validate_fn
            .call((layer_name, ops_table, from_did, role, page_id))
            .map_err(|e| anyhow::anyhow!("validate_ops error: {}", e))?;

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
        serde_json::Value::String(s) => Ok(Value::String(lua.create_string(s)?)),
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

// Layer::extract_ops_from_bytes

/// extract_ops_from_bytes produces JsonOp format compatible with validation.lua
#[test]
fn test_extract_ops_format_matches_validation_expectations() -> Result<()> {
    init_tracing();

    let layer = Layer::new();
    let root = layer.loro().get_map("root");
    root.insert("messages", loro::LoroValue::List(vec![].into()))?;
    layer.loro().commit();

    let messages = layer.loro().get_list("messages");
    let msg = loro::LoroValue::Map(
        [
            ("id".to_string(), loro::LoroValue::String("msg1".into())),
            ("text".to_string(), loro::LoroValue::String("hello".into())),
            (
                "sender_did".to_string(),
                loro::LoroValue::String("did:key:alice".into()),
            ),
        ]
        .into_iter()
        .collect::<std::collections::HashMap<_, _>>()
        .into(),
    );
    messages.push(msg)?;
    layer.loro().commit();

    let update = layer
        .loro()
        .export(loro::ExportMode::updates(&loro::VersionVector::new()))
        .expect("export");

    let ops = Layer::extract_ops_from_bytes(&update)?;

    assert!(!ops.is_empty(), "Should have ops");

    for op in &ops {
        assert!(!op.op.is_empty(), "op.op should not be empty");
        assert!(!op.path.is_empty(), "op.path should not be empty");
    }

    Ok(())
}

// my-shop validation tests

/// Products layer only allows owner writes
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

    let result = runtime.validate_ops("products", &ops, "did:key:owner", "owner", page_id)?;
    assert!(result, "Owner should be able to modify products");

    let result = runtime.validate_ops("products", &ops, "did:key:customer", "customer", page_id)?;
    assert!(!result, "Customer should not be able to modify products");

    let result = runtime.validate_ops("products", &ops, "did:key:viewer", "viewer", page_id)?;
    assert!(!result, "Viewer should not be able to modify products");

    Ok(())
}

/// Customers can only modify their own orders layer
#[test]
fn test_myshop_orders_ownership() -> Result<()> {
    init_tracing();

    let runtime = AppValidationRuntime::new("my-shop")?;

    let page_id = "shop123";
    let customer_did = "did:key:customer_a";
    let other_customer_did = "did:key:customer_b";
    let layer_name = format!("{}/orders/{}", page_id, customer_did);

    let ops = vec![serde_json::json!({
        "op": "insert",
        "path": "[0]",
        "value": {"id": "order1", "status": "pending", "items": []}
    })];

    let result = runtime.validate_ops(&layer_name, &ops, customer_did, "customer", page_id)?;
    assert!(result, "Customer should be able to modify their own orders");

    let result =
        runtime.validate_ops(&layer_name, &ops, other_customer_did, "customer", page_id)?;
    assert!(
        !result,
        "Customer should not be able to modify other's orders"
    );

    let result = runtime.validate_ops(&layer_name, &ops, "did:key:owner", "owner", page_id)?;
    assert!(result, "Owner should be able to modify any orders");

    Ok(())
}

/// Order state machine — synced orders must start pending
#[test]
fn test_myshop_order_initial_state() -> Result<()> {
    init_tracing();

    let runtime = AppValidationRuntime::new("my-shop")?;

    let page_id = "shop123";
    let customer_did = "did:key:customer";
    let layer_name = format!("{}/orders/{}", page_id, customer_did);

    let valid_ops = vec![serde_json::json!({
        "op": "insert",
        "path": "[0]",
        "value": {"id": "order1", "status": "pending"}
    })];

    let result =
        runtime.validate_ops(&layer_name, &valid_ops, customer_did, "customer", page_id)?;
    assert!(result, "Pending order should be allowed");

    let invalid_ops = vec![serde_json::json!({
        "op": "insert",
        "path": "[0]",
        "value": {"id": "order2", "status": "draft"}
    })];

    let result =
        runtime.validate_ops(&layer_name, &invalid_ops, customer_did, "customer", page_id)?;
    assert!(
        !result,
        "Draft order should be rejected (drafts are local-only)"
    );

    Ok(())
}

// my-booking validation tests

/// Schedule layer only allows owner writes
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

    let result = runtime.validate_ops(&layer_name, &ops, "did:key:owner", "owner", page_id)?;
    assert!(result, "Owner should be able to modify schedule");

    let result =
        runtime.validate_ops(&layer_name, &ops, "did:key:customer", "customer", page_id)?;
    assert!(!result, "Customer should not be able to modify schedule");

    Ok(())
}

/// Derived calendar layer only allows node writes
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

    let result = runtime.validate_ops(&layer_name, &ops, "did:key:node", "node", page_id)?;
    assert!(result, "Node should be able to modify derived calendar");

    let result = runtime.validate_ops(&layer_name, &ops, "did:key:owner", "owner", page_id)?;
    assert!(
        !result,
        "Owner should not be able to modify derived calendar"
    );

    let result =
        runtime.validate_ops(&layer_name, &ops, "did:key:customer", "customer", page_id)?;
    assert!(
        !result,
        "Customer should not be able to modify derived calendar"
    );

    Ok(())
}

/// Booking state machine — synced bookings must start pending
#[test]
fn test_mybooking_booking_initial_state() -> Result<()> {
    init_tracing();

    let runtime = AppValidationRuntime::new("my-booking")?;

    let page_id = "booking123";
    let customer_did = "did:key:customer";
    let layer_name = format!("{}/bookings/{}", page_id, customer_did);

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

    let result =
        runtime.validate_ops(&layer_name, &valid_ops, customer_did, "customer", page_id)?;
    assert!(result, "Pending booking should be allowed");

    let invalid_ops = vec![serde_json::json!({
        "op": "insert",
        "path": "[0]",
        "value": {
            "id": "booking2",
            "status": "pending"
        }
    })];

    let result =
        runtime.validate_ops(&layer_name, &invalid_ops, customer_did, "customer", page_id)?;
    assert!(
        !result,
        "Booking without required fields should be rejected"
    );

    Ok(())
}
