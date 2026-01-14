//! Connection Token Decisions
//!
//! Decision functions for connection-related tokens:
//! - One-time connection tokens
//! - Peer connection tokens
//! - Viewer authentication tokens

use super::types::{DecisionResult, TokenDecision};
use crate::errors::GurkhaError;
use base64::{Engine as _, engine::general_purpose};
use ed25519_dalek::VerifyingKey;
use serde_json::json;

/// Decide what should be in a one-time connection token
///
/// One-time tokens have:
/// - Wildcard audience (for first connection)
/// - Facts-based authorization (no URI capabilities)
/// - Relationship determines permission level
/// - 30 day expiry
pub fn decide_one_time_token(
    verifying_key: &VerifyingKey,
    relationship: &str,
) -> DecisionResult<TokenDecision> {
    let pub_key_b64 = general_purpose::STANDARD.encode(verifying_key.as_bytes());

    let mut decision = TokenDecision::new("*"); // Wildcard audience

    // No URI capabilities - all authorization in facts
    decision.add_fact("token_type".into(), json!("one_time_connection"));
    decision.add_fact("first_connection".into(), json!(true));
    decision.add_fact("relationship".into(), json!(relationship));
    decision.add_fact("user_id".into(), json!(pub_key_b64));

    // Add CEL rules and operations based on relationship
    // Bidirectional naming: {issuer}_{recipient}
    match relationship {
        "node_owner" => {
            // Node issuing to owner: full admin capabilities
            // peer_capabilities for protocol-level decisions (capability-based, not role-based)
            decision.add_fact("peer_capabilities".into(), json!({
                "relay": false,
                "share": true,
                "accept_publish": true  // Owner can publish spaces to this node
            }));
            decision.add_fact("auth_capabilities".into(), json!({
                "can_connect": true,
                "persist_share": true,
                "can_delegate": true,
                "sync_enabled": true
            }));
            decision.add_fact("operations".into(), json!({
                "own": "allow",
                "read": "allow",
                "write": "allow"
            }));
            decision.add_fact("cel_rules".into(), json!({
                "persist_share": "auth_capabilities.persist_share == true && relationship == 'node_owner'",
                "can_connect": "auth_capabilities.can_connect == true",
                "can_delegate": "auth_capabilities.can_delegate == true && operations.own == 'allow'",
                "sync_enabled": "auth_capabilities.sync_enabled == true && operations.read == 'allow'"
            }));
        }
        "node_viewer" => {
            // Node issuing to viewer: restricted capabilities
            // peer_capabilities for protocol-level decisions (capability-based, not role-based)
            decision.add_fact("peer_capabilities".into(), json!({
                "relay": false,
                "share": false,
                "accept_publish": false  // Viewers cannot publish spaces
            }));
            decision.add_fact("auth_capabilities".into(), json!({
                "can_connect": true,
                "persist_share": false,
                "can_delegate": false,
                "sync_enabled": true
            }));
            decision.add_fact("operations".into(), json!({
                "own": "deny",
                "read": "allow",
                "write": "deny"
            }));
            decision.add_fact("cel_rules".into(), json!({
                "persist_share": "auth_capabilities.persist_share == true && relationship == 'owner_node'",
                "can_connect": "auth_capabilities.can_connect == true",
                "can_delegate": "auth_capabilities.can_delegate == true && operations.own == 'allow'",
                "sync_enabled": "auth_capabilities.sync_enabled == true && operations.read == 'allow'"
            }));
        }
        _ => return Err(GurkhaError::ValidationError(format!("Unknown relationship: {}", relationship))),
    }

    decision.set_expiry(30 * 24 * 60 * 60); // 30 days

    Ok(decision)
}

/// Decide what should be in a peer connection token
///
/// Peer tokens have:
/// - Specific peer's pubkey as audience
/// - Facts-based authorization (no URI capabilities)
/// - Relationship-based permissions
/// - No expiry (persistent connection)
pub fn decide_peer_connection(
    verifying_key: &VerifyingKey,
    peer_pubkey: &str,
    relationship: &str,
) -> DecisionResult<TokenDecision> {
    let pub_key_b64 = general_purpose::STANDARD.encode(verifying_key.as_bytes());

    let mut decision = TokenDecision::new(peer_pubkey);

    // No URI capabilities - all authorization in facts
    decision.add_fact("token_type".into(), json!(format!("{}_connection", relationship)));
    decision.add_fact("relationship".into(), json!(relationship));
    decision.add_fact("user_id".into(), json!(pub_key_b64));

    // Add relationship-based permissions
    // Bidirectional naming: {issuer}_{recipient}
    match relationship {
        "node_owner" => {
            // Node issuing to owner: full admin capabilities
            // peer_capabilities for protocol-level decisions (capability-based, not role-based)
            decision.add_fact("peer_capabilities".into(), json!({
                "relay": false,
                "share": true,
                "accept_publish": true  // Owner can publish spaces to this node
            }));
            decision.add_fact("auth_capabilities".into(), json!({
                "can_connect": true,
                "persist_share": true,
                "can_delegate": true,
                "sync_enabled": true
            }));
            decision.add_fact("operations".into(), json!({
                "own": "allow",
                "read": "allow",
                "write": "allow",
                "admin": "allow"
            }));
        }
        "node_viewer" => {
            // Node issuing to viewer: restricted capabilities
            // peer_capabilities for protocol-level decisions (capability-based, not role-based)
            decision.add_fact("peer_capabilities".into(), json!({
                "relay": false,
                "share": false,
                "accept_publish": false  // Viewers cannot publish spaces
            }));
            decision.add_fact("auth_capabilities".into(), json!({
                "can_connect": true,
                "persist_share": false,
                "can_delegate": false,
                "sync_enabled": false
            }));
            decision.add_fact("operations".into(), json!({
                "own": "deny",
                "read": "allow",
                "write": "deny"
            }));
        }
        "owner_node" => {
            // Owner issuing to node: node capabilities
            // peer_capabilities for protocol-level decisions (capability-based, not role-based)
            decision.add_fact("peer_capabilities".into(), json!({
                "relay": true,           // Node can relay data
                "share": true,           // Node can issue delegated permits
                "accept_publish": true   // Node accepts published spaces
            }));
            decision.add_fact("auth_capabilities".into(), json!({
                "can_connect": true,
                "persist_share": true,
                "can_delegate": false,
                "sync_enabled": true,
                "add_folder": true
            }));
            decision.add_fact("operations".into(), json!({
                "own": "deny",
                "read": "allow",
                "write": "allow",
                "sync": "allow"
            }));
        }
        "viewer_node" => {
            // Viewer issuing to node: limited node capabilities
            // peer_capabilities for protocol-level decisions (capability-based, not role-based)
            decision.add_fact("peer_capabilities".into(), json!({
                "relay": true,           // Node can relay data
                "share": false,          // Viewer's node can't further delegate
                "accept_publish": false  // Viewer's node doesn't accept publish
            }));
            decision.add_fact("auth_capabilities".into(), json!({
                "can_connect": true,
                "persist_share": false,
                "can_delegate": false,
                "sync_enabled": true
            }));
            decision.add_fact("operations".into(), json!({
                "own": "deny",
                "read": "allow",
                "write": "deny",
                "sync": "allow"
            }));
        }
        _ => return Err(GurkhaError::ValidationError(format!("Unknown relationship: {}", relationship))),
    }

    Ok(decision)
}

/// Decide what should be in a page viewer authentication token
pub fn decide_page_viewer_auth(
    verifying_key: &VerifyingKey,
    page_id: &str,
) -> DecisionResult<TokenDecision> {
    let pub_key_b64 = general_purpose::STANDARD.encode(verifying_key.as_bytes());

    let mut decision = TokenDecision::new(&pub_key_b64); // Self-signed initially

    // No URI capabilities - all authorization in facts
    decision.add_fact("token_type".into(), json!("viewer_auth"));
    decision.add_fact("relationship".into(), json!("viewer"));
    decision.add_fact("page_id".into(), json!(page_id));
    decision.add_fact("user_id".into(), json!(pub_key_b64));

    // peer_capabilities for protocol-level decisions (capability-based, not role-based)
    decision.add_fact("peer_capabilities".into(), json!({
        "relay": false,
        "share": false,
        "accept_publish": false  // Viewers cannot publish spaces
    }));
    // Viewer capabilities
    decision.add_fact("auth_capabilities".into(), json!({
        "can_connect": true,
        "persist_share": false,
        "can_delegate": false,
        "sync_enabled": true
    }));
    decision.add_fact("operations".into(), json!({
        "own": "deny",
        "read": "allow",
        "write": "deny"
    }));

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

    let mut decision = TokenDecision::new("*"); // Wildcard audience for shareable link

    // No URI capabilities - all authorization in facts
    decision.add_fact("token_type".into(), json!("viewer_auth"));
    decision.add_fact("first_connection".into(), json!(false)); // Viewers never do first_connection
    // Relationship is "node_viewer" = node issuing permit TO viewer (bidirectional naming)
    decision.add_fact("relationship".into(), json!("node_viewer"));
    decision.add_fact("space_id".into(), json!(space_id));
    decision.add_fact("user_id".into(), json!(pub_key_b64));

    // peer_capabilities for protocol-level decisions (capability-based, not role-based)
    decision.add_fact("peer_capabilities".into(), json!({
        "relay": false,
        "share": false,
        "accept_publish": false  // Viewers cannot publish spaces
    }));
    // Viewer capabilities for space access
    decision.add_fact("auth_capabilities".into(), json!({
        "can_connect": true,
        "persist_share": false,
        "can_delegate": false,
        "sync_enabled": true
    }));
    decision.add_fact("operations".into(), json!({
        "own": "deny",
        "read": "allow",
        "write": "deny",
        "space_access": "allow"
    }));
    decision.add_fact("cel_rules".into(), json!({
        "persist_share": "auth_capabilities.persist_share == true && relationship == 'node'",
        "can_connect": "auth_capabilities.can_connect == true",
        "can_delegate": "auth_capabilities.can_delegate == true && operations.own == 'allow'",
        "sync_enabled": "auth_capabilities.sync_enabled == true && operations.read == 'allow'"
    }));

    // Short expiry for security (30 days like one-time connection)
    decision.set_expiry(30 * 24 * 60 * 60);

    Ok(decision)
}
