//! Unified Delegation API
//!
//! Creates delegation decisions using templates from permits.
//! The template defines the capabilities and facts - no hardcoded role logic.

use super::types::{DecisionResult, DelegationDecision};
use crate::errors::GurkhaError;
use crate::parser::DelegationTemplate;
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use tracing::{debug, info, trace};

/// Unified delegation decision (replaces role-specific functions)
///
/// Creates a delegation decision using a template key to lookup the delegation pattern.
/// The template defines the capabilities and facts - no hardcoded role logic.
/// Facts-only approach - no URI capabilities generated.
///
/// # Arguments
/// * `template` - Delegation template defining capabilities and facts
/// * `resource_id` - ID of the resource being delegated
/// * `resource_type` - Type of resource ("resource" or "space")
/// * `audience_pubkey` - Public key of the delegatee (DID or base64)
///
/// # Returns
/// * `DelegationDecision` - Decision describing what should be in the delegated token
pub fn decide_delegation(
    template: &DelegationTemplate,
    resource_id: &str,
    resource_type: &str,
    audience_pubkey: &str,
) -> DecisionResult<DelegationDecision> {
    debug!(
        "📋 Deciding delegation: resource_type={}, id={}, audience={}",
        resource_type, resource_id, audience_pubkey
    );

    // Get facts from template (includes layers, operations, CEL rules, etc.)
    let mut facts = template.to_facts();

    trace!("📋 Template converted to facts:");
    if let Some(layers) = facts.get("layers") {
        trace!("  Layers in facts: {:?}", layers);
    } else {
        trace!("  ⚠️ NO layers in facts!");
    }

    // Add ID based on type (space or page only)
    match resource_type {
        "space" => facts.insert("space_id".to_string(), json!(resource_id)),
        "page" => facts.insert("page_id".to_string(), json!(resource_id)),
        _ => return Err(GurkhaError::ValidationError(format!("Unknown resource type: {}", resource_type))),
    };

    let decision = DelegationDecision {
        audience: audience_pubkey.to_string(),
        capabilities: Vec::new(), // No URI capabilities - facts-only!
        facts,
        template: Some(template.clone()),
        proofs: Vec::new(),
        proof_tokens: HashMap::new(),
    };

    info!("✓ Delegation decision created (facts-only)");
    Ok(decision)
}

/// Extract template from delegator token using `issue_on` (self-describing)
///
/// Self-describing permits carry embedded templates for what to issue next.
/// This eliminates role-based lookups - the permit knows what it can delegate.
///
/// # Arguments
/// * `token_str` - The delegator's permit token
/// * `action` - The action triggering delegation: "page_request", "share_link", etc.
///
/// # Returns
/// * `DelegationTemplate` - The template for this action from `issue_on`
pub fn extract_issue_template(
    token_str: &str,
    action: &str,
) -> DecisionResult<DelegationTemplate> {
    let permit = crate::parser::Permit::from_token(token_str)
        .map_err(|e| GurkhaError::ParseError(format!("Failed to parse token: {}", e)))?;

    permit.get_issue_template(action)
        .cloned()
        .ok_or_else(|| GurkhaError::InvalidTemplate(format!(
            "No issue_on template for action '{}'. Permit must define issue_on.{} to delegate.",
            action, action
        )))
}

/// Extract facts from token
///
/// Facts-only approach - all authorization info is in facts field.
pub fn extract_facts_from_token(token_str: &str) -> DecisionResult<Map<String, Value>> {
    let ucan = crate::parser::Permit::from_token(token_str)
        .map_err(|e| GurkhaError::ParseError(format!("Failed to parse token: {}", e)))?;

    Ok(ucan.facts().clone())
}
