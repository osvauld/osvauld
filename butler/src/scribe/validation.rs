//! Lua-based validation for incoming updates
//!
//! **Context**: Same validation runs on owner, viewer, node, AI
//! **Pattern**: Fork → Import → Export ops → Validate → Apply or reject
//!
//! The validation.lua file lives in the app layer and defines a `validate_ops` function
//! that checks business rules before allowing writes.

use mlua::{Lua, Result as LuaResult, Value, MultiValue};
use tracing::{debug, warn, info};

use crate::models::Layer;

/// JSON operation extracted from Loro diff
///
/// **Context**: Represents a single operation from a Loro update
/// **Used by**: Lua validation to check field-level access control
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct JsonOp {
    /// Operation type: "insert", "delete", "update", "set"
    pub op: String,
    /// Path to the container (e.g., "orders", "orders.items")
    pub path: String,
    /// Key or index being modified (for maps/lists)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    /// Index for list operations
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index: Option<usize>,
    /// New value being set
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<serde_json::Value>,
    /// Previous value (for updates)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_value: Option<serde_json::Value>,
}

/// Lua-based validator for incoming updates
///
/// **Context**: Loaded from validation.lua in the app layer
/// **Runs**: Same code on owner, viewer, node, AI
///
/// **Validation flow**:
/// 1. Fork the current Loro doc
/// 2. Import the incoming update into the fork
/// 3. Export the diff as JSON operations
/// 4. Call Lua `validate_ops(layer_name, ops, from_did, role)`
/// 5. If valid, apply to real doc; if invalid, reject
pub struct LuaValidator {
    lua: Lua,
    page_id: String,
}

impl LuaValidator {
    /// Create a new validator from Lua code
    ///
    /// **Context**: Called when opening a page that has validation.lua
    /// **Code**: The validation.lua content from the app layer
    pub fn new(page_id: &str, validation_code: &str) -> Result<Self, String> {
        let lua = Lua::new();

        // Load the validation code
        lua.load(validation_code)
            .exec()
            .map_err(|e| format!("Failed to load validation code: {}", e))?;

        // Verify validate_ops function exists
        let globals = lua.globals();
        match globals.get::<Value>("validate_ops") {
            Ok(Value::Function(_)) => {
                info!(page_id = %page_id, "Loaded Lua validator with validate_ops function");
            }
            Ok(_) => {
                return Err("validate_ops is not a function".to_string());
            }
            Err(_) => {
                warn!(page_id = %page_id, "No validate_ops function found, validation will pass all ops");
            }
        }

        Ok(Self {
            lua,
            page_id: page_id.to_string(),
        })
    }

    /// Validate operations before applying an update
    ///
    /// **Context**: Remote peer sends update, we validate before applying
    /// **Returns**: Ok(true) if valid, Ok(false) if invalid, Err on Lua error
    ///
    /// **Parameters**:
    /// - `layer_name`: Which layer is being modified
    /// - `ops`: JSON operations extracted from the Loro diff
    /// - `from_did`: DID of the peer sending the update
    /// - `role`: Role of the peer (owner, viewer, customer, etc.)
    /// - `page_id`: The page ID (passed to Lua for layer pattern matching)
    pub fn validate_ops(
        &self,
        layer_name: &str,
        ops: &[JsonOp],
        from_did: &str,
        role: &str,
    ) -> Result<(bool, Option<String>), String> {
        let globals = self.lua.globals();

        // Check if validate_ops function exists
        let validate_fn = match globals.get::<Value>("validate_ops") {
            Ok(Value::Function(f)) => f,
            _ => {
                // No validation function, allow all
                debug!(layer = %layer_name, "No validate_ops function, allowing update");
                return Ok((true, None));
            }
        };

        // Convert ops to Lua table
        let ops_value = self.ops_to_lua(ops)?;

        // Call validate_ops(layer_name, ops, from_did, role, page_id)
        let result: LuaResult<MultiValue> = validate_fn.call((
            layer_name.to_string(),
            ops_value,
            from_did.to_string(),
            role.to_string(),
            self.page_id.clone(),
        ));

        match result {
            Ok(values) => {
                let mut iter = values.into_iter();

                // First return value: boolean (valid or not)
                let valid = match iter.next() {
                    Some(Value::Boolean(b)) => b,
                    Some(Value::Nil) => true, // nil means valid
                    _ => {
                        warn!("validate_ops returned non-boolean first value");
                        false
                    }
                };

                // Second return value: error message (optional)
                let error_msg = match iter.next() {
                    Some(Value::String(s)) => s.to_str().ok().map(|s| s.to_string()),
                    _ => None,
                };

                debug!(
                    layer = %layer_name,
                    from_did = %from_did,
                    role = %role,
                    valid = %valid,
                    error = ?error_msg,
                    "Validation result"
                );

                Ok((valid, error_msg))
            }
            Err(e) => {
                warn!(error = %e, "Lua validation error");
                Err(format!("Validation error: {}", e))
            }
        }
    }

    /// Convert JsonOp array to Lua table
    fn ops_to_lua(&self, ops: &[JsonOp]) -> Result<Value, String> {
        let lua_ops = self.lua.create_table()
            .map_err(|e| format!("Failed to create Lua table: {}", e))?;

        for (i, op) in ops.iter().enumerate() {
            let op_table = self.lua.create_table()
                .map_err(|e| format!("Failed to create op table: {}", e))?;

            op_table.set("op", op.op.clone())
                .map_err(|e| format!("Failed to set op: {}", e))?;
            op_table.set("path", op.path.clone())
                .map_err(|e| format!("Failed to set path: {}", e))?;

            if let Some(ref key) = op.key {
                op_table.set("key", key.clone())
                    .map_err(|e| format!("Failed to set key: {}", e))?;
            }

            if let Some(index) = op.index {
                op_table.set("index", index as i64)
                    .map_err(|e| format!("Failed to set index: {}", e))?;
            }

            if let Some(ref value) = op.value {
                let lua_value = self.json_to_lua(value)?;
                op_table.set("value", lua_value)
                    .map_err(|e| format!("Failed to set value: {}", e))?;
            }

            if let Some(ref old_value) = op.old_value {
                let lua_value = self.json_to_lua(old_value)?;
                op_table.set("old_value", lua_value)
                    .map_err(|e| format!("Failed to set old_value: {}", e))?;
            }

            lua_ops.set(i + 1, op_table)
                .map_err(|e| format!("Failed to set op in array: {}", e))?;
        }

        Ok(Value::Table(lua_ops))
    }

    /// Convert serde_json::Value to Lua Value
    fn json_to_lua(&self, json: &serde_json::Value) -> Result<Value, String> {
        match json {
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
                let lua_str = self.lua.create_string(s)
                    .map_err(|e| format!("Failed to create Lua string: {}", e))?;
                Ok(Value::String(lua_str))
            }
            serde_json::Value::Array(arr) => {
                let table = self.lua.create_table()
                    .map_err(|e| format!("Failed to create Lua table: {}", e))?;
                for (i, v) in arr.iter().enumerate() {
                    let lua_v = self.json_to_lua(v)?;
                    table.set(i + 1, lua_v)
                        .map_err(|e| format!("Failed to set array element: {}", e))?;
                }
                Ok(Value::Table(table))
            }
            serde_json::Value::Object(obj) => {
                let table = self.lua.create_table()
                    .map_err(|e| format!("Failed to create Lua table: {}", e))?;
                for (k, v) in obj {
                    let lua_v = self.json_to_lua(v)?;
                    table.set(k.clone(), lua_v)
                        .map_err(|e| format!("Failed to set object field: {}", e))?;
                }
                Ok(Value::Table(table))
            }
        }
    }
}

/// Extract operations from a Loro update by diffing
///
/// **Context**: We have an update (binary), need to extract what changed
/// **Method**: Create temp layer from current state → Get before → Apply update → Get after → Diff
///
/// **Returns**: List of JsonOp representing the changes
pub fn extract_ops_from_update(layer: &Layer, update: &[u8]) -> Result<Vec<JsonOp>, String> {
    // Get state before by creating a copy of current layer
    let before = layer.to_json();

    // Create a temporary layer from current snapshot to apply update
    let snapshot = layer.export_snapshot();
    let temp_layer = Layer::from_snapshot(&snapshot)
        .map_err(|e| format!("Failed to create temp layer: {}", e))?;

    // Import the update into temp layer
    temp_layer.apply(update)
        .map_err(|e| format!("Failed to apply update to temp layer: {}", e))?;

    // Get state after import
    let after = temp_layer.to_json();

    // Diff the before/after to extract operations
    let ops = diff_json_to_ops(&before, &after, "");

    debug!(op_count = ops.len(), "Extracted ops from update");

    Ok(ops)
}

/// Diff two JSON values and extract operations
///
/// **Context**: Compare before/after states to determine what changed
/// **Path**: Current path in the JSON tree (e.g., "orders.items")
fn diff_json_to_ops(before: &serde_json::Value, after: &serde_json::Value, path: &str) -> Vec<JsonOp> {
    let mut ops = Vec::new();

    match (before, after) {
        // Both are objects - diff keys
        (serde_json::Value::Object(before_map), serde_json::Value::Object(after_map)) => {
            // Check for removed keys
            for key in before_map.keys() {
                if !after_map.contains_key(key) {
                    let key_path = if path.is_empty() {
                        key.clone()
                    } else {
                        format!("{}.{}", path, key)
                    };
                    ops.push(JsonOp {
                        op: "delete".to_string(),
                        path: key_path,
                        key: Some(key.clone()),
                        index: None,
                        value: None,
                        old_value: Some(before_map[key].clone()),
                    });
                }
            }

            // Check for added or modified keys
            for (key, after_val) in after_map {
                let key_path = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{}.{}", path, key)
                };

                if let Some(before_val) = before_map.get(key) {
                    // Key exists in both - recurse for nested changes
                    if before_val != after_val {
                        let nested_ops = diff_json_to_ops(before_val, after_val, &key_path);
                        if nested_ops.is_empty() {
                            // Leaf value changed
                            ops.push(JsonOp {
                                op: "update".to_string(),
                                path: key_path,
                                key: Some(key.clone()),
                                index: None,
                                value: Some(after_val.clone()),
                                old_value: Some(before_val.clone()),
                            });
                        } else {
                            ops.extend(nested_ops);
                        }
                    }
                } else {
                    // Key is new
                    ops.push(JsonOp {
                        op: "insert".to_string(),
                        path: key_path,
                        key: Some(key.clone()),
                        index: None,
                        value: Some(after_val.clone()),
                        old_value: None,
                    });
                }
            }
        }

        // Both are arrays - diff elements
        (serde_json::Value::Array(before_arr), serde_json::Value::Array(after_arr)) => {
            let min_len = before_arr.len().min(after_arr.len());

            // Check common elements for changes
            for i in 0..min_len {
                let idx_path = if path.is_empty() {
                    format!("[{}]", i)
                } else {
                    format!("{}[{}]", path, i)
                };

                if before_arr[i] != after_arr[i] {
                    let nested_ops = diff_json_to_ops(&before_arr[i], &after_arr[i], &idx_path);
                    if nested_ops.is_empty() {
                        ops.push(JsonOp {
                            op: "update".to_string(),
                            path: idx_path,
                            key: None,
                            index: Some(i),
                            value: Some(after_arr[i].clone()),
                            old_value: Some(before_arr[i].clone()),
                        });
                    } else {
                        ops.extend(nested_ops);
                    }
                }
            }

            // Removed elements (after is shorter)
            for i in after_arr.len()..before_arr.len() {
                let idx_path = if path.is_empty() {
                    format!("[{}]", i)
                } else {
                    format!("{}[{}]", path, i)
                };
                ops.push(JsonOp {
                    op: "delete".to_string(),
                    path: idx_path,
                    key: None,
                    index: Some(i),
                    value: None,
                    old_value: Some(before_arr[i].clone()),
                });
            }

            // Added elements (after is longer)
            for i in before_arr.len()..after_arr.len() {
                let idx_path = if path.is_empty() {
                    format!("[{}]", i)
                } else {
                    format!("{}[{}]", path, i)
                };
                ops.push(JsonOp {
                    op: "insert".to_string(),
                    path: idx_path,
                    key: None,
                    index: Some(i),
                    value: Some(after_arr[i].clone()),
                    old_value: None,
                });
            }
        }

        // Different types or leaf values - treat as replacement
        _ => {
            if before != after {
                ops.push(JsonOp {
                    op: "set".to_string(),
                    path: path.to_string(),
                    key: None,
                    index: None,
                    value: Some(after.clone()),
                    old_value: Some(before.clone()),
                });
            }
        }
    }

    ops
}

// =============================================================================
// Path Parsing Helpers (for derivation)
// =============================================================================

/// Check if path indicates a field-level update within a list item
///
/// **Context**: Used by derivation to detect when we need to fetch the full item
/// **Examples**:
/// - "orders[0].status" -> true (field within list item)
/// - "orders[0]" -> false (entire list item, not a field)
/// - "products" -> false (not a list path)
pub fn is_nested_list_path(path: &str) -> bool {
    // Path contains [index].field pattern
    // We use a simple check: contains '].' which means there's a field after the index
    path.contains("].")
}

/// Extract list index from path like "orders[0].status" -> Some(0)
///
/// **Context**: Used by derivation to fetch the full item at this index
/// **Returns**: None if path doesn't contain a valid index
pub fn parse_list_index(path: &str) -> Option<usize> {
    // Find [ and ]
    let start = path.find('[')?;
    let end = path.find(']')?;
    if end <= start + 1 {
        return None;
    }
    // Extract and parse the number
    path[start + 1..end].parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lua_validator_basic() {
        let code = r#"
            function validate_ops(layer_name, ops, from_did, role, page_id)
                return true, nil
            end
        "#;

        let validator = LuaValidator::new("test_page", code).unwrap();
        let ops = vec![JsonOp {
            op: "insert".to_string(),
            path: "orders".to_string(),
            key: Some("order_1".to_string()),
            index: None,
            value: Some(serde_json::json!({"status": "pending"})),
            old_value: None,
        }];

        let (valid, error) = validator.validate_ops("orders", &ops, "did:key:customer", "customer").unwrap();
        assert!(valid);
        assert!(error.is_none());
    }

    #[test]
    fn test_lua_validator_rejects() {
        let code = r#"
            function validate_ops(layer_name, ops, from_did, role, page_id)
                if role == "viewer" then
                    return false, "Viewers cannot modify data"
                end
                return true, nil
            end
        "#;

        let validator = LuaValidator::new("test_page", code).unwrap();
        let ops = vec![];

        let (valid, error) = validator.validate_ops("orders", &ops, "did:key:viewer", "viewer").unwrap();
        assert!(!valid);
        assert_eq!(error, Some("Viewers cannot modify data".to_string()));
    }

    #[test]
    fn test_lua_validator_checks_field() {
        let code = r#"
            function validate_ops(layer_name, ops, from_did, role, page_id)
                for _, op in ipairs(ops) do
                    if op.key == "admin_only" and role ~= "owner" then
                        return false, "Cannot modify admin_only field"
                    end
                end
                return true, nil
            end
        "#;

        let validator = LuaValidator::new("test_page", code).unwrap();

        // Customer trying to modify admin_only field
        let ops = vec![JsonOp {
            op: "update".to_string(),
            path: "settings".to_string(),
            key: Some("admin_only".to_string()),
            index: None,
            value: Some(serde_json::json!(true)),
            old_value: Some(serde_json::json!(false)),
        }];

        let (valid, error) = validator.validate_ops("settings", &ops, "did:key:customer", "customer").unwrap();
        assert!(!valid);
        assert_eq!(error, Some("Cannot modify admin_only field".to_string()));

        // Owner can modify admin_only field
        let (valid, error) = validator.validate_ops("settings", &ops, "did:key:owner", "owner").unwrap();
        assert!(valid);
        assert!(error.is_none());
    }

    #[test]
    fn test_diff_json_to_ops_insert() {
        let before = serde_json::json!({});
        let after = serde_json::json!({"name": "test"});

        let ops = diff_json_to_ops(&before, &after, "");
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].op, "insert");
        assert_eq!(ops[0].key, Some("name".to_string()));
    }

    #[test]
    fn test_diff_json_to_ops_delete() {
        let before = serde_json::json!({"name": "test"});
        let after = serde_json::json!({});

        let ops = diff_json_to_ops(&before, &after, "");
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].op, "delete");
        assert_eq!(ops[0].key, Some("name".to_string()));
    }

    #[test]
    fn test_diff_json_to_ops_update() {
        let before = serde_json::json!({"status": "pending"});
        let after = serde_json::json!({"status": "confirmed"});

        let ops = diff_json_to_ops(&before, &after, "");
        assert_eq!(ops.len(), 1);
        // Leaf value changes within an object are reported as "set" operations
        assert_eq!(ops[0].op, "set");
        assert_eq!(ops[0].path, "status");
        assert_eq!(ops[0].value, Some(serde_json::json!("confirmed")));
        assert_eq!(ops[0].old_value, Some(serde_json::json!("pending")));
    }

    #[test]
    fn test_is_nested_list_path() {
        // Field within list item - should return true
        assert!(is_nested_list_path("orders[0].status"));
        assert!(is_nested_list_path("items[5].quantity"));
        assert!(is_nested_list_path("data[123].nested.field"));

        // Entire list item - should return false
        assert!(!is_nested_list_path("orders[0]"));
        assert!(!is_nested_list_path("items[5]"));

        // Not a list path - should return false
        assert!(!is_nested_list_path("products"));
        assert!(!is_nested_list_path("orders"));
        assert!(!is_nested_list_path("status"));
    }

    #[test]
    fn test_parse_list_index() {
        // Valid indices
        assert_eq!(parse_list_index("orders[0].status"), Some(0));
        assert_eq!(parse_list_index("items[5].quantity"), Some(5));
        assert_eq!(parse_list_index("data[123].field"), Some(123));
        assert_eq!(parse_list_index("orders[0]"), Some(0));

        // Invalid paths
        assert_eq!(parse_list_index("products"), None);
        assert_eq!(parse_list_index("orders"), None);
        assert_eq!(parse_list_index("orders[]"), None);
        assert_eq!(parse_list_index("orders[abc]"), None);
    }
}
