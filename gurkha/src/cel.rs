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

// ============================================================================
// Two-Permit Evaluation (for sync decisions)
// ============================================================================

/// Context for evaluating permit functions
///
/// Provides identity information for CEL expressions:
/// - `context.our_pubkey` - Who we are
/// - `context.their_pubkey` - Who the peer is
/// - `context.layer` - Current layer being synced
/// - `context.page_id` - Page being accessed
/// - `context.space_id` - Space being accessed
#[derive(Debug, Clone)]
pub struct EvalContext {
    pub our_pubkey: String,
    pub their_pubkey: String,
    pub layer: Option<String>,
    pub page_id: Option<String>,
    pub space_id: Option<String>,
}

impl EvalContext {
    pub fn new(our_pubkey: String, their_pubkey: String) -> Self {
        Self {
            our_pubkey,
            their_pubkey,
            layer: None,
            page_id: None,
            space_id: None,
        }
    }

    pub fn with_layer(mut self, layer: String) -> Self {
        self.layer = Some(layer);
        self
    }

    pub fn with_page_id(mut self, page_id: String) -> Self {
        self.page_id = Some(page_id);
        self
    }

    pub fn with_space_id(mut self, space_id: String) -> Self {
        self.space_id = Some(space_id);
        self
    }

    /// Convert to CEL value for context.* access
    fn to_cel_value(&self) -> cel_interpreter::objects::Value {
        let mut map = HashMap::new();
        map.insert("our_pubkey".to_string(),
            cel_interpreter::objects::Value::String(self.our_pubkey.clone().into()));
        map.insert("their_pubkey".to_string(),
            cel_interpreter::objects::Value::String(self.their_pubkey.clone().into()));

        if let Some(ref layer) = self.layer {
            map.insert("layer".to_string(),
                cel_interpreter::objects::Value::String(layer.clone().into()));
        }
        if let Some(ref page_id) = self.page_id {
            map.insert("page_id".to_string(),
                cel_interpreter::objects::Value::String(page_id.clone().into()));
        }
        if let Some(ref space_id) = self.space_id {
            map.insert("space_id".to_string(),
                cel_interpreter::objects::Value::String(space_id.clone().into()));
        }

        cel_interpreter::objects::Value::Map(map.into())
    }
}

/// Evaluate a permit function comparing two permits
///
/// Permits can contain CEL expressions in their `functions` field:
/// ```json
/// {
///   "fct": {
///     "functions": {
///       "should_send_layer": "self.page_id == peer.page_id && self.layers[context.layer].capability == 'collaborator'"
///     }
///   }
/// }
/// ```
///
/// CEL context provides:
/// - `self.*` - Our permit's facts (+ iss, aud)
/// - `peer.*` - Peer's permit's facts (+ iss, aud)
/// - `context.*` - Identity and operation context
pub fn evaluate_function(
    our_facts: &Map<String, Value>,
    our_iss: &str,
    our_aud: &str,
    peer_facts: &Map<String, Value>,
    peer_iss: &str,
    peer_aud: &str,
    function_name: &str,
    eval_ctx: &EvalContext,
) -> CelResult<bool> {
    // Get the function expression from our permit's facts.functions
    let expression = our_facts
        .get("functions")
        .and_then(|f| f.as_object())
        .and_then(|funcs| funcs.get(function_name))
        .and_then(|expr| expr.as_str())
        .ok_or_else(|| CelError::ExecutionError(
            format!("Function '{}' not found in permit", function_name)
        ))?;

    // Build self.* value (our permit facts + iss/aud)
    let self_value = build_permit_cel_value(our_facts, our_iss, our_aud)?;

    // Build peer.* value (peer permit facts + iss/aud)
    let peer_value = build_permit_cel_value(peer_facts, peer_iss, peer_aud)?;

    // Build context.* value
    let context_value = eval_ctx.to_cel_value();

    // Create CEL context with self, peer, context
    let mut context = Context::default();
    context.add_variable("self", self_value)
        .map_err(|e| CelError::ConversionError(format!("Failed to add self: {:?}", e)))?;
    context.add_variable("peer", peer_value)
        .map_err(|e| CelError::ConversionError(format!("Failed to add peer: {:?}", e)))?;
    context.add_variable("context", context_value)
        .map_err(|e| CelError::ConversionError(format!("Failed to add context: {:?}", e)))?;

    // Compile and execute
    let program = Program::compile(expression)
        .map_err(|e| CelError::CompilationError(format!("{:?}", e)))?;

    let result = program.execute(&context)
        .map_err(|e| CelError::ExecutionError(format!("{:?}", e)))?;

    match result {
        cel_interpreter::objects::Value::Bool(b) => Ok(b),
        other => Err(CelError::InvalidResult(format!("{:?}", other))),
    }
}

/// Build a CEL value for a permit (facts + iss + aud)
fn build_permit_cel_value(
    facts: &Map<String, Value>,
    iss: &str,
    aud: &str,
) -> CelResult<cel_interpreter::objects::Value> {
    let validator = OperationValidator::new();
    let mut map = HashMap::new();

    // Add iss and aud
    map.insert("iss".to_string(),
        cel_interpreter::objects::Value::String(iss.to_string().into()));
    map.insert("aud".to_string(),
        cel_interpreter::objects::Value::String(aud.to_string().into()));

    // Add all facts
    for (key, value) in facts {
        let cel_value = validator.json_to_cel_value(value)?;
        map.insert(key.clone(), cel_value);
    }

    Ok(cel_interpreter::objects::Value::Map(map.into()))
}

/// Simple version: evaluate with just our facts and context (no peer)
pub fn evaluate_single(
    facts: &Map<String, Value>,
    iss: &str,
    aud: &str,
    function_name: &str,
    eval_ctx: &EvalContext,
) -> CelResult<bool> {
    // For single permit evaluation, peer is empty
    let empty_facts = Map::new();
    evaluate_function(
        facts, iss, aud,
        &empty_facts, "", "",
        function_name,
        eval_ctx,
    )
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
