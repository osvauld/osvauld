//! Resource Token Decisions
//!
//! Decision functions for resource owner tokens:
//! - Space owner tokens
//! - Space node-to-owner tokens
//! - Page owner tokens

use super::types::{DecisionResult, TokenDecision};
use crate::errors::GurkhaError;
use base64::{engine::general_purpose, Engine as _};
use ed25519_dalek::VerifyingKey;
use serde_json::{json, Value};

/// Decide what should be in a space owner token using built-in defaults
pub fn decide_space_owner_token_from_defaults(
    verifying_key: &VerifyingKey,
    space_id: &str,
) -> DecisionResult<TokenDecision> {
    let pub_key_b64 = general_purpose::STANDARD.encode(verifying_key.as_bytes());

    let mut decision = TokenDecision::new(&pub_key_b64); // Self-signed

    // Build facts from template (no URI capabilities)
    decision.add_fact("token_type".into(), json!("space_owner"));
    decision.add_fact("relationship".into(), json!("owner"));
    decision.add_fact("space_id".into(), json!(space_id));
    decision.add_fact("user_id".into(), json!(pub_key_b64));
    decision.add_fact(
        "operations".into(),
        json!({
            "own": "allow",
            "get_share_link": "allow",
            "add_pages": "allow",
            "share_space": "allow"
        }),
    );
    decision.add_fact(
        "peer_capabilities".into(),
        json!({
            "relay": false,
            "share": true,
            "accept_publish": true
        }),
    );
    decision.add_fact(
        "issue_on".into(),
        json!({
            "node": {
                "token_type": "space_share",
                "peer_capabilities": {
                    "relay": true,
                    "share": true,
                    "accept_publish": true
                },
                "operations": {
                    "get_share_link": "allow",
                    "add_pages": "allow",
                    "share_space": "allow"
                },
                "auth_capabilities": {
                    "can_connect": true,
                    "persist_share": true,
                    "can_delegate": true,
                    "sync_enabled": true
                },
                "relationship": "node",
                "issue_on": {
                    "viewer": {
                        "token_type": "space_viewer",
                        "peer_capabilities": {
                            "relay": false,
                            "share": false,
                            "accept_publish": false
                        },
                        "operations": {
                            "request_pages": "allow",
                            "get_share_link": "allow"
                        },
                        "auth_capabilities": {
                            "can_connect": true,
                            "sync_enabled": false
                        },
                        "relationship": "viewer"
                    }
                }
            },
            "viewer": {
                "token_type": "space_viewer",
                "peer_capabilities": {
                    "relay": false,
                    "share": false,
                    "accept_publish": false
                },
                "operations": {
                    "request_pages": "allow",
                    "get_share_link": "allow"
                },
                "auth_capabilities": {
                    "can_connect": true,
                    "sync_enabled": false
                },
                "relationship": "viewer"
            }
        }),
    );

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
    decision.add_fact(
        "operations".into(),
        json!({
            "sync": "allow"
        }),
    );
    decision.add_fact(
        "auth_capabilities".into(),
        json!({
            "can_connect": true,
            "sync_enabled": true
        }),
    );

    Ok(decision)
}

/// Decide what should be in a page owner token from typed policy
///
/// **Context**: Constructs a page owner token directly from compiled `PolicyFacts`
/// without an intermediate JSON template round-trip.
///
/// Produces identical token facts to `decide_page_owner_token` with the equivalent
/// JSON template, including `layers`, `dynamic_layer_schemas`, `osv_policy`, and
/// `issue_on` templates (node / viewer / layer_authority).
///
/// # Arguments
/// * `verifying_key` - Owner's verifying key (self-signed)
/// * `page_id` - Page identifier (used to resolve `{page_id}` placeholders)
/// * `policy` - Compiled typed policy from `app.osv`
/// * `layer_names` - All layer names (data + app) to include in the permit
pub fn decide_page_owner_token_from_policy(
    verifying_key: &VerifyingKey,
    page_id: &str,
    policy: &policy_model::PolicyFacts,
    layer_names: &[String],
) -> DecisionResult<TokenDecision> {
    let pub_key_b64 = general_purpose::STANDARD.encode(verifying_key.as_bytes());
    let mut decision = TokenDecision::new(&pub_key_b64); // Self-signed

    // Standard owner operations
    decision.add_fact("token_type".into(), json!("page_owner"));
    decision.add_fact("relationship".into(), json!("owner"));
    decision.add_fact("page_id".into(), json!(page_id));
    decision.add_fact("user_id".into(), json!(pub_key_b64));
    decision.add_fact(
        "operations".into(),
        json!({
            "own": "allow",
            "read": "allow",
            "write": "allow",
            "share_page": "allow",
            "share": "allow",
            "grant": "allow",
            "revoke": "allow"
        }),
    );

    // Owner peer capabilities
    decision.add_fact(
        "peer_capabilities".into(),
        json!({
            "relay": false,
            "share": true,
            "accept_publish": true,
            "manage_layer_access": true
        }),
    );

    // Build layers map from layer_names
    let mut layers = serde_json::Map::new();
    for layer_name in layer_names {
        layers.insert(
            layer_name.clone(),
            json!({ "type": "map", "sync": true, "write": true }),
        );
    }
    decision.add_fact("layers".into(), Value::Object(layers.clone()));

    // Build dynamic_layer_schemas from policy
    let dynamic_schemas = build_dynamic_schemas_from_policy(policy);
    if !dynamic_schemas.is_empty() {
        decision.add_fact(
            "dynamic_layer_schemas".into(),
            Value::Object(dynamic_schemas.clone()),
        );
    }

    // Embed osv_policy
    let policy_json = serde_json::to_value(policy)
        .map_err(|e| GurkhaError::InvalidTemplate(format!("Failed to serialize policy: {}", e)))?;
    decision.add_fact("osv_policy".into(), policy_json.clone());

    // Build issue_on templates (node / viewer / layer_authority)
    let viewer_template = build_viewer_template(&layers, &dynamic_schemas, &policy_json);
    let node_template =
        build_node_template(&layers, &dynamic_schemas, &policy_json, &viewer_template);
    let layer_authority_template = json!({
        "token_type": "layer_authority",
        "relationship": "layer_authority",
        "osv_policy": policy_json
    });

    decision.add_fact(
        "issue_on".into(),
        json!({
            "node": node_template,
            "viewer": viewer_template,
            "layer_authority": layer_authority_template
        }),
    );

    // Resolve {page_id} in layer keys and nested issue_on templates
    crate::parser::resolve_page_id_in_facts(&mut decision.facts, page_id);

    Ok(decision)
}

/// Build dynamic_layer_schemas JSON map from PolicyFacts
fn build_dynamic_schemas_from_policy(
    policy: &policy_model::PolicyFacts,
) -> serde_json::Map<String, Value> {
    let mut map = serde_json::Map::new();
    for schema in &policy.dynamic_layer_schemas {
        let mut schema_obj = serde_json::Map::new();
        schema_obj.insert("type".to_string(), json!("map"));
        schema_obj.insert("grant".to_string(), json!(schema.grant));
        schema_obj.insert("namespace".to_string(), json!(schema.namespace));
        schema_obj.insert(
            "storage_strategy".to_string(),
            json!(schema.storage_strategy),
        );
        if let Some(resolution) = &schema.resolution {
            schema_obj.insert("resolution".to_string(), json!(resolution));
        }
        schema_obj.insert(
            "permissions".to_string(),
            json!({ "sync": true, "write": true }),
        );
        map.insert(schema.path.clone(), Value::Object(schema_obj));
    }
    map
}

/// Build viewer issue_on template
fn build_viewer_template(
    layers: &serde_json::Map<String, Value>,
    dynamic_schemas: &serde_json::Map<String, Value>,
    policy_json: &Value,
) -> Value {
    let mut template = serde_json::Map::new();
    template.insert("token_type".to_string(), json!("page_viewer"));
    template.insert(
        "peer_capabilities".to_string(),
        json!({
            "relay": false,
            "share": false,
            "accept_publish": false,
            "manage_layer_access": false
        }),
    );
    template.insert(
        "operations".to_string(),
        json!({
            "read": "allow",
            "write": "allow",
            "sync": "allow"
        }),
    );
    template.insert("layers".to_string(), Value::Object(layers.clone()));
    if !dynamic_schemas.is_empty() {
        template.insert(
            "dynamic_layer_schemas".to_string(),
            Value::Object(dynamic_schemas.clone()),
        );
    }
    template.insert("osv_policy".to_string(), policy_json.clone());
    template.insert("relationship".to_string(), json!("viewer"));
    Value::Object(template)
}

/// Build node issue_on template (includes nested viewer + layer_authority)
fn build_node_template(
    layers: &serde_json::Map<String, Value>,
    dynamic_schemas: &serde_json::Map<String, Value>,
    policy_json: &Value,
    viewer_template: &Value,
) -> Value {
    let layer_authority_template = json!({
        "token_type": "layer_authority",
        "relationship": "layer_authority",
        "osv_policy": policy_json
    });

    let mut template = serde_json::Map::new();
    template.insert("token_type".to_string(), json!("page_share"));
    template.insert(
        "peer_capabilities".to_string(),
        json!({
            "relay": true,
            "share": true,
            "accept_publish": true,
            "manage_layer_access": true
        }),
    );
    template.insert(
        "operations".to_string(),
        json!({
            "read": "allow",
            "write": "allow",
            "sync": "allow",
            "share_page": "allow",
            "grant": "allow",
            "revoke": "allow"
        }),
    );
    template.insert("layers".to_string(), Value::Object(layers.clone()));
    if !dynamic_schemas.is_empty() {
        template.insert(
            "dynamic_layer_schemas".to_string(),
            Value::Object(dynamic_schemas.clone()),
        );
    }
    template.insert("osv_policy".to_string(), policy_json.clone());
    template.insert("relationship".to_string(), json!("node"));
    template.insert(
        "issue_on".to_string(),
        json!({
            "viewer": viewer_template,
            "layer_authority": layer_authority_template
        }),
    );
    Value::Object(template)
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
        .ok_or_else(|| {
            GurkhaError::InvalidTemplate("Missing operations in owner_template".to_string())
        })?;

    // Build facts from template (no URI capabilities)
    decision.add_fact("token_type".into(), json!("page_owner"));
    decision.add_fact("relationship".into(), json!("owner"));
    decision.add_fact("page_id".into(), json!(page_id));
    decision.add_fact("user_id".into(), json!(pub_key_b64));
    // Keep operations as strings ("allow"/"deny") instead of converting to booleans
    decision.add_fact("operations".into(), json!(ops));

    // Copy all template fields to facts, then resolve {page_id}
    let template_fields = [
        "layers",
        "sync",
        "issue_on",
        "peer_capabilities",
        "presence",
        "ephemeral_funcs",
        "dynamic_layer_schemas",
        "osv_policy",
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
