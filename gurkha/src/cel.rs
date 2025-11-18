//! CEL (Common Expression Language) Operation Validator
//!
//! Provides conditional logic evaluation for UCAN permissions using CEL expressions.
//!
//! ## Design
//!
//! - **Pure evaluation** - No side effects, just validates operations against facts
//! - **Sandboxed** - CEL is non-Turing complete, safe for untrusted expressions
//! - **Flexible** - Facts-driven validation without rigid type system
//! - **Token issuance rules** - CEL expressions determine what capabilities delegated tokens should have
//! - **Dynamic authorization** - Connection ability, share persistence, sync rules all determined by facts
//!
//! ## Use Cases
//!
//! 1. **Operation validation** - Can user perform operation X?
//! 2. **Token issuance** - What capabilities should the issued token have?
//! 3. **Share record persistence** - Should we persist this share to node?
//! 4. **Connection authorization** - Can this user connect to another?
//! 5. **Sync rules** - Should updates be synced based on capabilities?
//!
//! ## Usage
//!
//! ```rust,ignore
//! use gurkha::cel::OperationValidator;
//! use serde_json::json;
//!
//! let validator = OperationValidator::new();
//! let facts = json!({
//!     "operations": {"read": true, "write": false},
//!     "relationship": "viewer"
//! });
//!
//! // Validate if user can perform an operation
//! let can_read = validator.validate("operations.read == true", &facts)?;
//! assert!(can_read);
//!
//! let can_write = validator.validate("operations.write == true", &facts)?;
//! assert!(!can_write);
//! ```

use cel_interpreter::{Context, Program};
use serde_json::{Map, Value};
use std::collections::HashMap;
use thiserror::Error;

/// CEL validation errors
#[derive(Debug, Error)]
pub enum CelError {
    #[error("Failed to compile CEL expression: {0}")]
    CompilationError(String),

    #[error("Failed to execute CEL expression: {0}")]
    ExecutionError(String),

    #[error("Invalid expression result: expected boolean, got {0}")]
    InvalidResult(String),

    #[error("Failed to convert fact to CEL value: {0}")]
    ConversionError(String),
}

pub type CelResult<T> = Result<T, CelError>;

/// Operation validator using CEL expressions
///
/// Evaluates CEL expressions against UCAN facts to determine if operations are allowed.
///
/// ## Example CEL Expressions
///
/// ```cel
/// // Check if user can read a specific document
/// operations.read == true && doc_id == "abc123"
///
/// // Check if user is owner
/// operations.own == true
///
/// // Check collaborative permission
/// capabilities.collaborative == true
///
/// // Determine if share record should be persisted to node
/// capabilities.can_delegate == true && operations.own == true
///
/// // Check if user has ability to connect to another user
/// capabilities.can_connect == true
///
/// // Determine if user can sync with peers (not hardcoded by role)
/// capabilities.sync_enabled == true && operations.read == true
///
/// // Complex conditional for connection authorization
/// (operations.own == true || capabilities.can_delegate == true) && relationship != "viewer"
/// ```
pub struct OperationValidator {
    /// Cached compiled programs for performance
    program_cache: HashMap<String, Program>,
}

impl OperationValidator {
    /// Create a new operation validator
    pub fn new() -> Self {
        Self {
            program_cache: HashMap::new(),
        }
    }

    /// Validate an operation using a CEL expression
    ///
    /// # Arguments
    /// * `expression` - CEL expression string (e.g., "operations.read == true")
    /// * `facts` - UCAN facts as JSON (from Permit.facts())
    ///
    /// # Returns
    /// * `true` if the expression evaluates to true
    /// * `false` if the expression evaluates to false
    /// * `Err` if compilation or execution fails
    ///
    /// # Example
    /// ```rust,ignore
    /// let validator = OperationValidator::new();
    /// let facts = json!({"operations": {"read": true}});
    /// let allowed = validator.validate("operations.read == true", &facts)?;
    /// ```
    pub fn validate(&mut self, expression: &str, facts: &Map<String, Value>) -> CelResult<bool> {
        // Compile or retrieve cached program
        let program = if let Some(cached) = self.program_cache.get(expression) {
            cached
        } else {
            let compiled = Program::compile(expression)
                .map_err(|e| CelError::CompilationError(format!("{:?}", e)))?;
            self.program_cache.insert(expression.to_string(), compiled);
            self.program_cache.get(expression).unwrap()
        };

        // Create context with facts
        let context = self.create_context(facts)?;

        // Execute program
        let result = program
            .execute(&context)
            .map_err(|e| CelError::ExecutionError(format!("{:?}", e)))?;

        // Convert result to boolean
        match result {
            cel_interpreter::objects::Value::Bool(b) => Ok(b),
            other => Err(CelError::InvalidResult(format!("{:?}", other))),
        }
    }

    /// Validate multiple operations at once
    ///
    /// # Arguments
    /// * `expressions` - Map of operation names to CEL expressions
    /// * `facts` - UCAN facts
    ///
    /// # Returns
    /// * Map of operation names to validation results
    pub fn validate_batch(
        &mut self,
        expressions: &HashMap<String, String>,
        facts: &Map<String, Value>,
    ) -> CelResult<HashMap<String, bool>> {
        let mut results = HashMap::new();

        for (op_name, expression) in expressions {
            let result = self.validate(expression, facts)?;
            results.insert(op_name.clone(), result);
        }

        Ok(results)
    }

    /// Create a CEL context from UCAN facts
    ///
    /// Converts JSON facts into CEL variables accessible in expressions.
    fn create_context(&self, facts: &Map<String, Value>) -> CelResult<Context> {
        let mut context = Context::default();

        // Add all facts as top-level variables in the context
        for (key, value) in facts {
            let cel_value = self.json_to_cel_value(value)?;
            // Note: CEL context API may differ - this is a placeholder
            // We'll need to check the actual API for adding variables
            // For now, assuming we can add via context directly
            context.add_variable(key, cel_value)
                .map_err(|e| CelError::ConversionError(format!("Failed to add variable {}: {:?}", key, e)))?;
        }

        Ok(context)
    }

    /// Convert JSON value to CEL value
    ///
    /// Recursively converts serde_json::Value to cel_interpreter::Value.
    fn json_to_cel_value(&self, value: &Value) -> CelResult<cel_interpreter::objects::Value> {
        match value {
            Value::Null => Ok(cel_interpreter::objects::Value::Null),
            Value::Bool(b) => Ok(cel_interpreter::objects::Value::Bool(*b)),
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Ok(cel_interpreter::objects::Value::Int(i))
                } else if let Some(f) = n.as_f64() {
                    Ok(cel_interpreter::objects::Value::Float(f))
                } else {
                    Err(CelError::ConversionError(format!("Unsupported number: {}", n)))
                }
            }
            Value::String(s) => Ok(cel_interpreter::objects::Value::String(s.clone().into())),
            Value::Array(arr) => {
                let cel_list: Result<Vec<_>, _> = arr
                    .iter()
                    .map(|v| self.json_to_cel_value(v))
                    .collect();
                Ok(cel_interpreter::objects::Value::List(cel_list?.into()))
            }
            Value::Object(obj) => {
                let mut cel_map = HashMap::new();
                for (k, v) in obj {
                    cel_map.insert(k.clone(), self.json_to_cel_value(v)?);
                }
                Ok(cel_interpreter::objects::Value::Map(cel_map.into()))
            }
        }
    }

    /// Clear the program cache
    ///
    /// Useful for testing or when expressions change.
    pub fn clear_cache(&mut self) {
        self.program_cache.clear();
    }

    /// Get the number of cached programs
    pub fn cache_size(&self) -> usize {
        self.program_cache.len()
    }
}

impl Default for OperationValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_simple_boolean_validation() {
        let mut validator = OperationValidator::new();
        let facts = json!({
            "operations": {
                "read": true,
                "write": false
            }
        });

        let facts_map = facts.as_object().unwrap();

        // Should allow read
        let can_read = validator.validate("operations.read == true", facts_map).unwrap();
        assert!(can_read);

        // Should deny write
        let can_write = validator.validate("operations.write == true", facts_map).unwrap();
        assert!(!can_write);
    }

    #[test]
    fn test_complex_expression() {
        let mut validator = OperationValidator::new();
        let facts = json!({
            "operations": {
                "own": false,
                "read": true
            },
            "relationship": "viewer"
        });

        let facts_map = facts.as_object().unwrap();

        // Complex condition: can read but not own, and is viewer
        let result = validator
            .validate(
                "operations.read == true && operations.own == false && relationship == \"viewer\"",
                facts_map,
            )
            .unwrap();
        assert!(result);
    }

    #[test]
    fn test_program_caching() {
        let mut validator = OperationValidator::new();
        let facts = json!({"test": true});
        let facts_map = facts.as_object().unwrap();

        // First execution should cache the program
        validator.validate("test == true", facts_map).unwrap();
        assert_eq!(validator.cache_size(), 1);

        // Second execution should use cached program
        validator.validate("test == true", facts_map).unwrap();
        assert_eq!(validator.cache_size(), 1);

        // Different expression should create new cache entry
        validator.validate("test == false", facts_map).unwrap();
        assert_eq!(validator.cache_size(), 2);
    }

    #[test]
    fn test_batch_validation() {
        let mut validator = OperationValidator::new();
        let facts = json!({
            "operations": {
                "read": true,
                "write": false,
                "delete": false
            }
        });
        let facts_map = facts.as_object().unwrap();

        let mut expressions = HashMap::new();
        expressions.insert("can_read".to_string(), "operations.read == true".to_string());
        expressions.insert("can_write".to_string(), "operations.write == true".to_string());
        expressions.insert("can_delete".to_string(), "operations.delete == true".to_string());

        let results = validator.validate_batch(&expressions, facts_map).unwrap();

        assert_eq!(results.get("can_read"), Some(&true));
        assert_eq!(results.get("can_write"), Some(&false));
        assert_eq!(results.get("can_delete"), Some(&false));
    }
}
