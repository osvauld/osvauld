//! Sync Consent Decisions
//!
//! Decision functions for sync consent permits:
//! - Space sync consent (viewer → node)
//! - Page sync consent (viewer → node)

use super::types::{DecisionResult, TokenDecision};
use crate::errors::GurkhaError;
use base64::{Engine as _, engine::general_purpose};
use ed25519_dalek::VerifyingKey;
use serde_json::{json, Value};
use tracing::trace;

/// Decide what should be in a space sync consent permit
///
/// **Context**: Viewer issues this permit back to the node after receiving SpaceSync.
/// It expresses the viewer's consent to receive sync updates for the space.
///
/// **Issued by**: Viewer
/// **Audience**: Node (the specific node DID)
/// **Proof**: Node's original viewer permit (establishes delegation chain)
///
/// Space consent permits have:
/// - Specific node pubkey as audience (not wildcard)
/// - Facts expressing consent to receive updates and new pages
/// - Proof chain to node's original permit
/// - Same space_id as the original permit
pub fn decide_sync_space_consent(
    viewer_verifying_key: &VerifyingKey,
    node_pubkey: &str,
    space_id: &str,
    node_viewer_permit: &str,
    template_json: &str,
) -> DecisionResult<TokenDecision> {
    let viewer_pub_key_b64 = general_purpose::STANDARD.encode(viewer_verifying_key.as_bytes());

    let mut decision = TokenDecision::new(node_pubkey); // Specific node as audience

    // Parse consent template
    let template_data: Value = serde_json::from_str(template_json)
        .map_err(|e| GurkhaError::InvalidTemplate(format!("Invalid consent template JSON: {}", e)))?;

    let consent_template = template_data
        .get("consent_template")
        .ok_or_else(|| GurkhaError::InvalidTemplate("Missing consent_template".to_string()))?;

    // Core consent facts
    decision.add_fact("token_type".into(), json!("sync_space_consent"));
    decision.add_fact("relationship".into(), json!("sync_consent"));
    decision.add_fact("space_id".into(), json!(space_id));
    decision.add_fact("user_id".into(), json!(viewer_pub_key_b64));

    // Operations from template
    if let Some(operations) = consent_template.get("operations") {
        decision.add_fact("operations".into(), operations.clone());
    }

    // Auth capabilities from template
    if let Some(auth_caps) = consent_template.get("auth_capabilities") {
        decision.add_fact("auth_capabilities".into(), auth_caps.clone());
    }

    // Calculate CID of the proof token and add to proof chain
    let proof_cid = crate::crypto::get_permit_cid(node_viewer_permit)?;
    decision.proofs.push(proof_cid.clone());
    decision.proof_tokens.insert(proof_cid, node_viewer_permit.to_string());

    trace!("Space sync consent decision created for space {} -> node {}", space_id, node_pubkey);
    Ok(decision)
}

/// Decide what should be in a page sync consent permit
///
/// **Context**: Viewer issues this permit back to the node after receiving PageSync.
/// It expresses the viewer's consent to receive layer updates for a specific page.
///
/// **Issued by**: Viewer
/// **Audience**: Node (the specific node DID)
/// **Proof**: Node's original page_viewer permit (establishes delegation chain)
///
/// Page consent permits have:
/// - Specific node pubkey as audience
/// - Facts expressing consent to receive layer updates
/// - Layers with "receive" capability (mirroring original viewer layers)
/// - Proof chain to node's original permit
/// - Same page_id as the original permit
pub fn decide_sync_page_consent(
    viewer_verifying_key: &VerifyingKey,
    node_pubkey: &str,
    page_id: &str,
    node_viewer_permit: &str,
    template_json: &str,
) -> DecisionResult<TokenDecision> {
    let viewer_pub_key_b64 = general_purpose::STANDARD.encode(viewer_verifying_key.as_bytes());

    let mut decision = TokenDecision::new(node_pubkey); // Specific node as audience

    // Parse consent template
    let template_data: Value = serde_json::from_str(template_json)
        .map_err(|e| GurkhaError::InvalidTemplate(format!("Invalid consent template JSON: {}", e)))?;

    let consent_template = template_data
        .get("consent_template")
        .ok_or_else(|| GurkhaError::InvalidTemplate("Missing consent_template".to_string()))?;

    // Core consent facts
    decision.add_fact("token_type".into(), json!("sync_page_consent"));
    decision.add_fact("relationship".into(), json!("sync_consent"));
    decision.add_fact("page_id".into(), json!(page_id));
    decision.add_fact("user_id".into(), json!(viewer_pub_key_b64));

    // Operations from template
    if let Some(operations) = consent_template.get("operations") {
        decision.add_fact("operations".into(), operations.clone());
    }

    // Layers from template (with "receive" capability)
    if let Some(layers) = consent_template.get("layers") {
        decision.add_fact("layers".into(), layers.clone());
    }

    // Sync rules from template
    if let Some(sync) = consent_template.get("sync") {
        decision.add_fact("sync".into(), sync.clone());
    }

    // Auth capabilities from template
    if let Some(auth_caps) = consent_template.get("auth_capabilities") {
        decision.add_fact("auth_capabilities".into(), auth_caps.clone());
    }

    // Calculate CID of the proof token and add to proof chain
    let proof_cid = crate::crypto::get_permit_cid(node_viewer_permit)?;
    decision.proofs.push(proof_cid.clone());
    decision.proof_tokens.insert(proof_cid, node_viewer_permit.to_string());

    trace!("Page sync consent decision created for page {} -> node {}", page_id, node_pubkey);
    Ok(decision)
}

/// Decide what should be in a layer sync consent permit
///
/// **Context**: Subscriber wants to subscribe to a dynamic layer, issues consent.
/// Sent inside LayerSubscribe to authorize the responder to sync this layer.
///
/// **Issued by**: Subscriber (user or node)
/// **Audience**: Responder (node or creator)
/// **Proof**: Page permit (establishes delegation chain — subscriber doesn't have layer permit yet)
///
/// Layer consent permits have:
/// - Specific responder pubkey as audience
/// - Facts expressing consent to receive updates for a specific layer
/// - The layer name identifying which dynamic layer
/// - Proof chain to the page permit
pub fn decide_sync_layer_consent(
    viewer_verifying_key: &VerifyingKey,
    node_pubkey: &str,
    page_id: &str,
    layer_name: &str,
    page_permit_token: &str,
    template_json: &str,
) -> DecisionResult<TokenDecision> {
    let viewer_pub_key_b64 = general_purpose::STANDARD.encode(viewer_verifying_key.as_bytes());

    let mut decision = TokenDecision::new(node_pubkey);

    // Parse consent template (reuse page consent template structure)
    let template_data: Value = serde_json::from_str(template_json)
        .map_err(|e| GurkhaError::InvalidTemplate(format!("Invalid consent template JSON: {}", e)))?;

    let consent_template = template_data
        .get("consent_template")
        .ok_or_else(|| GurkhaError::InvalidTemplate("Missing consent_template".to_string()))?;

    // Core consent facts
    decision.add_fact("token_type".into(), json!("sync_layer_consent"));
    decision.add_fact("relationship".into(), json!("sync_consent"));
    decision.add_fact("page_id".into(), json!(page_id));
    decision.add_fact("layer_name".into(), json!(layer_name));
    decision.add_fact("user_id".into(), json!(viewer_pub_key_b64));

    // Operations from template
    if let Some(operations) = consent_template.get("operations") {
        decision.add_fact("operations".into(), operations.clone());
    }

    // Auth capabilities from template
    if let Some(auth_caps) = consent_template.get("auth_capabilities") {
        decision.add_fact("auth_capabilities".into(), auth_caps.clone());
    }

    // Calculate CID of the page permit and add to proof chain
    let proof_cid = crate::crypto::get_permit_cid(page_permit_token)?;
    decision.proofs.push(proof_cid.clone());
    decision.proof_tokens.insert(proof_cid, page_permit_token.to_string());

    trace!("Layer sync consent decision created for page {} layer {} -> node {}", page_id, layer_name, node_pubkey);
    Ok(decision)
}
