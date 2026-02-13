//! Resource Token Decisions
//!
//! Decision functions for resource owner tokens:
//! - Space owner tokens
//! - Space node-to-owner tokens
//! - Page owner tokens

use super::types::{DecisionResult, TokenDecision};
use crate::errors::GurkhaError;
use base64::{Engine as _, engine::general_purpose};
use ed25519_dalek::VerifyingKey;
use serde_json::{json, Value};

/// Decide what should be in a space owner token
///
/// Parses template JSON and decides which operations and delegation templates to include.
pub fn decide_space_owner_token(
    verifying_key: &VerifyingKey,
    space_id: &str,
    template_json: &str,
) -> DecisionResult<TokenDecision> {
    let pub_key_b64 = general_purpose::STANDARD.encode(verifying_key.as_bytes());

    let mut decision = TokenDecision::new(&pub_key_b64); // Self-signed

    // Parse template
    let template_data: Value = serde_json::from_str(template_json)
        .map_err(|e| GurkhaError::InvalidTemplate(format!("Invalid template JSON: {}", e)))?;

    let owner_template = template_data
        .get("owner_template")
        .ok_or_else(|| GurkhaError::InvalidTemplate("Missing owner_template".to_string()))?;

    // Extract operations from template
    let ops = owner_template
        .get("operations")
        .and_then(|v| v.as_object())
        .ok_or_else(|| GurkhaError::InvalidTemplate("Missing operations in owner_template".to_string()))?;

    // Build facts from template (no URI capabilities)
    decision.add_fact("token_type".into(), json!("space_owner"));
    decision.add_fact("relationship".into(), json!("owner"));
    decision.add_fact("space_id".into(), json!(space_id));
    decision.add_fact("user_id".into(), json!(pub_key_b64));
    // Keep operations as strings ("allow"/"deny") instead of converting to booleans
    decision.add_fact("operations".into(), json!(ops));

    // Copy sync facts if present
    if let Some(sync) = owner_template.get("sync") {
        decision.add_fact("sync".into(), sync.clone());
    }

    // Copy issue_on templates (self-describing permits)
    if let Some(issue_on) = owner_template.get("issue_on") {
        decision.add_fact("issue_on".into(), issue_on.clone());
    }

    Ok(decision)
}

/// Decide what should be in a space node-to-owner token
///
/// Used when node issues a permit back to the owner after receiving PublishSpace.
/// This permit proves the space is published to this node.
///
/// # Arguments
/// * `verifying_key` - Node's verifying key (issuer)
/// * `space_id` - Space identifier
/// * `owner_pubkey` - Owner's public key (audience)
pub fn decide_space_node_to_owner_token(
    verifying_key: &VerifyingKey,
    space_id: &str,
    owner_pubkey: &str,
) -> DecisionResult<TokenDecision> {
    let pub_key_b64 = general_purpose::STANDARD.encode(verifying_key.as_bytes());

    let mut decision = TokenDecision::new(owner_pubkey);

    // Build facts for node→owner permit
    // This is a simple permit proving space is published to this node
    decision.add_fact("token_type".into(), json!("space_node_share"));
    decision.add_fact("relationship".into(), json!("owner"));
    decision.add_fact("space_id".into(), json!(space_id));
    decision.add_fact("issuer_id".into(), json!(pub_key_b64));
    decision.add_fact("operations".into(), json!({
        "sync": "allow"
    }));
    decision.add_fact("auth_capabilities".into(), json!({
        "can_connect": true,
        "sync_enabled": true
    }));

    Ok(decision)
}

/// Decide what should be in a page owner token
///
/// Parses template JSON and decides which operations, layers, and delegation templates to include.
pub fn decide_page_owner_token(
    verifying_key: &VerifyingKey,
    page_id: &str,
    template_json: &str,
) -> DecisionResult<TokenDecision> {
    let pub_key_b64 = general_purpose::STANDARD.encode(verifying_key.as_bytes());

    let mut decision = TokenDecision::new(&pub_key_b64); // Self-signed

    // Parse template
    let template_data: Value = serde_json::from_str(template_json)
        .map_err(|e| GurkhaError::InvalidTemplate(format!("Invalid template JSON: {}", e)))?;

    let owner_template = template_data
        .get("owner_template")
        .ok_or_else(|| GurkhaError::InvalidTemplate("Missing owner_template".to_string()))?;

    // Extract operations from template
    let ops = owner_template
        .get("operations")
        .and_then(|v| v.as_object())
        .ok_or_else(|| GurkhaError::InvalidTemplate("Missing operations in owner_template".to_string()))?;

    // Build facts from template (no URI capabilities)
    decision.add_fact("token_type".into(), json!("page_owner"));
    decision.add_fact("relationship".into(), json!("owner"));
    decision.add_fact("page_id".into(), json!(page_id));
    decision.add_fact("user_id".into(), json!(pub_key_b64));
    // Keep operations as strings ("allow"/"deny") instead of converting to booleans
    decision.add_fact("operations".into(), json!(ops));

    // Copy all template fields to facts, then resolve {page_id}
    let template_fields = [
        "layers", "sync", "issue_on", "peer_capabilities",
        "presence", "ephemeral_funcs", "dynamic_layer_schemas",
    ];
    for field in &template_fields {
        if let Some(val) = owner_template.get(*field) {
            decision.add_fact((*field).to_string(), val.clone());
        }
    }
    // Note: layer_patterns intentionally NOT copied (replaced by dynamic_layer_schemas)

    // Resolve {page_id} in layer keys and nested issue_on templates
    crate::parser::resolve_page_id_in_facts(&mut decision.facts, page_id);

    Ok(decision)
}
