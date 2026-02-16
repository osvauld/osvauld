//! Decision Data Structures
//!
//! Core types that decision functions return - pure data describing token content.
//! The crypto layer then takes these decisions and signs them.

use crate::errors::GurkhaError;
use crate::parser::DelegationTemplate;
use serde_json::{Map, Value};
use std::collections::HashMap;

pub type DecisionResult<T> = Result<T, GurkhaError>;

/// Represents a decision about what should go in a token (before signing)
///
/// This is what decision functions return - pure data describing the token content.
/// The crypto layer then takes this and signs it.
#[derive(Debug, Clone)]
pub struct TokenDecision {
    pub audience: String,
    pub capabilities: Vec<(String, String)>,
    pub facts: Map<String, Value>,
    pub expiry: Option<u64>,
    /// Proof chain: Array of CIDs for the `prf` field (UCAN standard)
    pub proofs: Vec<String>,
    /// Embedded proof tokens: CID -> full JWT token mapping for `fct.prf_tokens` (extension)
    /// Enables stateless proof chain validation without database lookups
    pub proof_tokens: HashMap<String, String>,
}

impl TokenDecision {
    pub fn new(audience: &str) -> Self {
        Self {
            audience: audience.to_string(),
            capabilities: Vec::new(),
            facts: Map::new(),
            expiry: None,
            proofs: Vec::new(),
            proof_tokens: HashMap::new(),
        }
    }

    pub fn add_capability(&mut self, resource: String, ability: String) {
        self.capabilities.push((resource, ability));
    }

    pub fn add_fact(&mut self, key: String, value: Value) {
        self.facts.insert(key, value);
    }

    pub fn set_expiry(&mut self, seconds: u64) {
        self.expiry = Some(seconds);
    }

    /// Set presence configuration (visibility, display name)
    ///
    /// **Context**: Controls whether this peer appears in /users layer and presence events
    pub fn with_presence(mut self, visible: bool, name: Option<String>) -> Self {
        let mut presence_obj = serde_json::Map::new();
        presence_obj.insert("visible".to_string(), Value::Bool(visible));
        if let Some(n) = name {
            presence_obj.insert("name".to_string(), Value::String(n));
        }
        self.facts
            .insert("presence".to_string(), Value::Object(presence_obj));
        self
    }

    /// Set allowed ephemeral function names
    ///
    /// **Context**: Controls which ephemeral functions this peer can send
    /// **Empty list** = all functions allowed
    pub fn with_ephemeral_funcs(mut self, funcs: Vec<String>) -> Self {
        if !funcs.is_empty() {
            let funcs_arr: Vec<Value> = funcs.into_iter().map(Value::String).collect();
            self.facts
                .insert("ephemeral_funcs".to_string(), Value::Array(funcs_arr));
        }
        self
    }
}

/// Represents a decision about a delegation (what to give to delegatee)
#[derive(Debug, Clone)]
pub struct DelegationDecision {
    pub audience: String,
    pub capabilities: Vec<(String, String)>,
    pub facts: Map<String, Value>,
    pub template: Option<DelegationTemplate>,
    /// Proof chain: Array of CIDs for the `prf` field (UCAN standard)
    pub proofs: Vec<String>,
    /// Embedded proof tokens: CID -> full JWT token mapping for `fct.prf_tokens` (extension)
    pub proof_tokens: HashMap<String, String>,
}
