use mlua::{Function, Lua, Value};

use crate::bindings::convert::json_to_lua;

/// Test-only validation runtime that doesn't require a full Scribe actor.
///
/// Purpose: enables unit testing of validate_ops without actor setup.
pub struct TestValidationRuntime {
    lua: Lua,
    page_id: String,
}

impl TestValidationRuntime {
    /// Create a test runtime for validation testing.
    pub fn new(page_id: &str) -> Result<Self, String> {
        let lua = Lua::new();
        Ok(Self {
            lua,
            page_id: page_id.to_string(),
        })
    }

    /// Load validation code.
    pub fn load_validation_code(&self, code: &str) -> Result<(), String> {
        self.lua
            .load(code)
            .exec()
            .map_err(|e| format!("Failed to load validation code: {}", e))?;
        Ok(())
    }

    /// Check if a function exists.
    pub fn has_function(&self, name: &str) -> bool {
        self.lua
            .globals()
            .get::<Value>(name)
            .map(|v| matches!(v, Value::Function(_)))
            .unwrap_or(false)
    }

    /// Validate operations.
    pub fn validate_ops(
        &self,
        layer_name: &str,
        ops: &[serde_json::Value],
        from_did: &str,
        role: &str,
    ) -> Result<(bool, Option<String>), String> {
        let globals = self.lua.globals();

        let validate_fn: Function = match globals.get("validate_ops") {
            Ok(f) => f,
            Err(_) => {
                return Ok((true, None));
            }
        };

        let ops_table = self
            .lua
            .create_table()
            .map_err(|e| format!("Failed to create ops table: {}", e))?;

        for (i, op_json) in ops.iter().enumerate() {
            let op_lua = json_to_lua(&self.lua, op_json)
                .map_err(|e| format!("Failed to convert op to Lua: {}", e))?;
            ops_table
                .set(i + 1, op_lua)
                .map_err(|e| format!("Failed to add op to table: {}", e))?;
        }

        let result: bool = validate_fn
            .call((layer_name, ops_table, from_did, role, self.page_id.as_str()))
            .map_err(|e| format!("validate_ops error: {}", e))?;

        Ok((
            result,
            if result {
                None
            } else {
                Some("Validation failed".to_string())
            },
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_ops_basic() {
        let rt = TestValidationRuntime::new("test-page").unwrap();
        assert!(!rt.has_function("validate_ops"));
        let ops = vec![
            serde_json::json!({"op": "insert", "path": "messages", "value": {"text": "hello"}}),
        ];
        let (passed, _) = rt
            .validate_ops("messages", &ops, "did:key:user", "viewer")
            .unwrap();
        assert!(passed, "Should allow ops when no validate_ops function");

        let rt = TestValidationRuntime::new("test-page").unwrap();
        rt.load_validation_code(r#"function validate_ops(l, o, f, r, p) return true end"#)
            .unwrap();
        let (passed, error) = rt
            .validate_ops("messages", &ops, "did:key:user", "viewer")
            .unwrap();
        assert!(passed);
        assert!(error.is_none());

        let rt = TestValidationRuntime::new("test-page").unwrap();
        rt.load_validation_code(r#"function validate_ops(l, o, f, r, p) return false end"#)
            .unwrap();
        let (passed, error) = rt
            .validate_ops("messages", &ops, "did:key:user", "viewer")
            .unwrap();
        assert!(!passed);
        assert!(error.is_some());

        let rt = TestValidationRuntime::new("test-page").unwrap();
        rt.load_validation_code(r#"function validate_ops(l, ops, f, r, p) return #ops == 0 end"#)
            .unwrap();
        let (passed, _) = rt
            .validate_ops("layer", &[], "did:key:user", "viewer")
            .unwrap();
        assert!(passed, "Empty ops should pass");
        let (passed, _) = rt
            .validate_ops("layer", &ops, "did:key:user", "viewer")
            .unwrap();
        assert!(!passed, "Non-empty ops should fail");
    }

    #[test]
    fn test_validate_ops_args_and_logic() {
        let rt = TestValidationRuntime::new("test-page-123").unwrap();
        rt.load_validation_code(
            r#"
            function validate_ops(layer_name, ops, from_did, role, page_id)
                if layer_name ~= "orders" then return false end
                if from_did ~= "did:key:alice" then return false end
                if role ~= "owner" then return false end
                if page_id ~= "test-page-123" then return false end
                if #ops ~= 2 then return false end
                return true
            end
        "#,
        )
        .unwrap();
        let ops = vec![
            serde_json::json!({"op": "insert", "value": 1}),
            serde_json::json!({"op": "insert", "value": 2}),
        ];
        let (passed, _) = rt
            .validate_ops("orders", &ops, "did:key:alice", "owner")
            .unwrap();
        assert!(passed, "All arguments should match");

        let rt = TestValidationRuntime::new("test-page").unwrap();
        rt.load_validation_code(
            r#"
            function validate_ops(layer_name, ops, from_did, role, page_id)
                for _, op in ipairs(ops) do
                    if op.op == "insert" and op.path == "messages" then
                        if op.value and op.value.text == "forbidden" then return false end
                    end
                end
                return true
            end
        "#,
        )
        .unwrap();
        let allowed = vec![
            serde_json::json!({"op": "insert", "path": "messages", "value": {"text": "hello"}}),
        ];
        let (passed, _) = rt
            .validate_ops("messages", &allowed, "did:key:user", "viewer")
            .unwrap();
        assert!(passed, "Normal messages allowed");
        let forbidden = vec![
            serde_json::json!({"op": "insert", "path": "messages", "value": {"text": "forbidden"}}),
        ];
        let (passed, _) = rt
            .validate_ops("messages", &forbidden, "did:key:user", "viewer")
            .unwrap();
        assert!(!passed, "Forbidden messages rejected");

        let rt = TestValidationRuntime::new("test-page").unwrap();
        rt.load_validation_code(
            r#"
            function validate_ops(layer_name, ops, from_did, role, page_id)
                if layer_name == "orders" and role ~= "owner" then return false end
                return true
            end
        "#,
        )
        .unwrap();
        let ops = vec![serde_json::json!({"op": "insert"})];
        let (passed, _) = rt
            .validate_ops("orders", &ops, "did:key:owner", "owner")
            .unwrap();
        assert!(passed, "Owner can modify orders");
        let (passed, _) = rt
            .validate_ops("orders", &ops, "did:key:viewer", "viewer")
            .unwrap();
        assert!(!passed, "Viewer cannot modify orders");
    }

    #[test]
    fn test_validate_ops_error_handling() {
        let rt = TestValidationRuntime::new("test-page").unwrap();
        rt.load_validation_code(
            r#"
            function validate_ops(l, o, f, r, p)
                error("Something went wrong!")
            end
        "#,
        )
        .unwrap();
        let ops = vec![serde_json::json!({"op": "insert"})];
        let result = rt.validate_ops("layer", &ops, "did:key:user", "viewer");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Something went wrong"));

        let rt = TestValidationRuntime::new("test-page").unwrap();
        let result = rt.load_validation_code("function validate_ops( -- missing closing paren");
        assert!(result.is_err(), "Invalid Lua should fail to load");
    }
}
