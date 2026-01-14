//! Decision Data Structures
//!
//! Core types that decision functions return - pure data describing token content.
//! The crypto layer then takes these decisions and signs them.

use crate::parser::DelegationTemplate;
use crate::errors::GurkhaError;
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
