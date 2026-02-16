//! Connection Token Decisions
//!
//! Decision functions for connection-related tokens:
//! - One-time connection tokens
//! - Peer connection tokens
//! - Viewer authentication tokens
//!
//! **Architecture**: Template-driven, not role-based.
//! - Templates define capability sets (keyed by human-readable names like "node_owner")
//! - Template lookup extracts capabilities into permit
//! - Protocol code checks actual capabilities in permit, never the template key
//!
//! Note: `peer_capabilities` are for protocol-level decisions (capability-based, not role-based).

use super::types::{DecisionResult, TokenDecision};
use crate::errors::GurkhaError;
use base64::{engine::general_purpose, Engine as _};
use ed25519_dalek::VerifyingKey;
use serde_json::json;
use std::sync::LazyLock;

/// Embedded connection templates (loaded at compile time)
static CONNECTION_TEMPLATES: LazyLock<serde_json::Value> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../../templates/connection.json"))
        .expect("Invalid connection.json template")
});

/// Load connection template by key
///
/// Template keys ("node_owner", "viewer_node", etc.) are just lookup keys.
/// The protocol never checks these keys - it checks the capabilities inside.
fn load_connection_template(key: &str) -> Result<&'static serde_json::Value, GurkhaError> {
    CONNECTION_TEMPLATES.get(key).ok_or_else(|| {
        GurkhaError::ValidationError(format!("Unknown connection template: {}", key))
    })
}

/// Build TokenDecision from a connection template
///
/// **Context**: Extracts capabilities from template into permit facts.
/// No role matching happens here - just capability extraction.
fn build_decision_from_template(
    decision: &mut TokenDecision,
    template: &serde_json::Value,
    relationship: &str,
) {
    // Add relationship label (for logging/debugging only)
    decision.add_fact("relationship".into(), json!(relationship));

    // Extract peer_capabilities
    if let Some(peer_caps) = template.get("peer_capabilities") {
        decision.add_fact("peer_capabilities".into(), peer_caps.clone());
    }

    // Extract auth_capabilities
    if let Some(auth_caps) = template.get("auth_capabilities") {
        decision.add_fact("auth_capabilities".into(), auth_caps.clone());
    }

    // Extract operations
    if let Some(ops) = template.get("operations") {
        decision.add_fact("operations".into(), ops.clone());
    }

    // Extract presence config
    if let Some(presence) = template.get("presence") {
        decision.add_fact("presence".into(), presence.clone());
    }
}

/// Decide what should be in a one-time connection token
///
/// One-time tokens have:
/// - Wildcard audience (for first connection)
/// - Facts-based authorization (no URI capabilities)
/// - Template-driven capabilities
/// - 30 day expiry
pub fn decide_one_time_token(
    verifying_key: &VerifyingKey,
    relationship: &str,
) -> DecisionResult<TokenDecision> {
    let pub_key_b64 = general_purpose::STANDARD.encode(verifying_key.as_bytes());

    // Load template (validates relationship is known)
    let template = load_connection_template(relationship)?;

    let mut decision = TokenDecision::new("*"); // Wildcard audience

    // Core facts
    decision.add_fact("token_type".into(), json!("one_time_connection"));
    decision.add_fact("first_connection".into(), json!(true));
    decision.add_fact("user_id".into(), json!(pub_key_b64));

    // Extract capabilities from template
    build_decision_from_template(&mut decision, template, relationship);

    decision.set_expiry(30 * 24 * 60 * 60); // 30 days

    Ok(decision)
}

/// Decide what should be in a peer connection token
///
/// Peer tokens have:
/// - Specific peer's pubkey as audience
/// - Facts-based authorization (no URI capabilities)
/// - Template-driven capabilities
/// - No expiry (persistent connection)
pub fn decide_peer_connection(
    verifying_key: &VerifyingKey,
    peer_pubkey: &str,
    relationship: &str,
) -> DecisionResult<TokenDecision> {
    let pub_key_b64 = general_purpose::STANDARD.encode(verifying_key.as_bytes());

    // Load template (validates relationship is known)
    let template = load_connection_template(relationship)?;

    let mut decision = TokenDecision::new(peer_pubkey);

    // Core facts
    decision.add_fact(
        "token_type".into(),
        json!(format!("{}_connection", relationship)),
    );
    decision.add_fact("user_id".into(), json!(pub_key_b64));

    // Extract capabilities from template
    build_decision_from_template(&mut decision, template, relationship);

    Ok(decision)
}

/// Decide what should be in a page viewer authentication token
pub fn decide_page_viewer_auth(
    verifying_key: &VerifyingKey,
    page_id: &str,
) -> DecisionResult<TokenDecision> {
    let pub_key_b64 = general_purpose::STANDARD.encode(verifying_key.as_bytes());

    // Load viewer template
    let template = load_connection_template("node_viewer")?;

    let mut decision = TokenDecision::new(&pub_key_b64); // Self-signed initially

    // Core facts
    decision.add_fact("token_type".into(), json!("viewer_auth"));
    decision.add_fact("page_id".into(), json!(page_id));
    decision.add_fact("user_id".into(), json!(pub_key_b64));

    // Extract capabilities from template
    build_decision_from_template(&mut decision, template, "viewer");

    Ok(decision)
}

/// Decide what should be in a space viewer authentication token
///
/// This is for shareable links - one-time use, wildcard audience.
/// The viewer uses this token to initiate connection to the node.
///
/// Space viewer auth tokens have:
/// - Wildcard audience (for one-time shareable link)
/// - Facts-based authorization (no URI capabilities)
/// - Space context and viewer relationship
/// - 30 day expiry
pub fn decide_space_viewer_auth(
    verifying_key: &VerifyingKey,
    space_id: &str,
) -> DecisionResult<TokenDecision> {
    let pub_key_b64 = general_purpose::STANDARD.encode(verifying_key.as_bytes());

    // Load viewer template
    let template = load_connection_template("node_viewer")?;

    let mut decision = TokenDecision::new("*"); // Wildcard audience for shareable link

    // Core facts
    decision.add_fact("token_type".into(), json!("viewer_auth"));
    decision.add_fact("first_connection".into(), json!(false)); // Viewers never do first_connection
    decision.add_fact("space_id".into(), json!(space_id));
    decision.add_fact("user_id".into(), json!(pub_key_b64));

    // Extract capabilities from template
    build_decision_from_template(&mut decision, template, "node_viewer");

    // Add space_access operation
    decision.add_fact(
        "operations".into(),
        json!({
            "own": "deny",
            "read": "allow",
            "write": "deny",
            "space_access": "allow"
        }),
    );

    // Short expiry for security (30 days like one-time connection)
    decision.set_expiry(30 * 24 * 60 * 60);

    Ok(decision)
}
