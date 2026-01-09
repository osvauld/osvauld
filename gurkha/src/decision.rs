//! Decision Logic Module
//!
//! Pure decision/inference functions that determine WHAT should be in a token.
//! Returns data structures (TokenDecision, DelegationDecision) that describe the decision.
//! NO crypto calls - just logic for determining capabilities, facts, expiry, audience.
//!
//! These functions are replaceable - they could later be implemented in OCaml
//! and called via FFI, but would still return the same data structures.
//! The crypto layer (crypto.rs) then takes these decisions and signs them.

use crate::errors::GurkhaError;
use crate::parser::DelegationTemplate;
use crate::types::SyncDecision;
use base64::{Engine as _, engine::general_purpose};
use ed25519_dalek::VerifyingKey;
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use tracing::{debug, info};

pub type DecisionResult<T> = Result<T, GurkhaError>;

// ==================== DECISION DATA STRUCTURES ====================

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

// ==================== CONNECTION TOKEN DECISIONS ====================

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

// ==================== UNIFIED DELEGATION API ====================

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
///
/// # Example
/// ```rust
/// let template = extract_template_from_token(owner_token, "node")?;
/// let decision = decide_delegation(&template, "res_123", "resource", "did:key:z...")?;
/// ```
pub fn decide_delegation(
    template: &DelegationTemplate,
    resource_id: &str,
    resource_type: &str,
    audience_pubkey: &str,
    parent_token: Option<&str>,
) -> DecisionResult<DelegationDecision> {
    debug!(
        "📋 Deciding delegation: resource_type={}, id={}, audience={}",
        resource_type, resource_id, audience_pubkey
    );

    // Get facts from template (includes layers, operations, CEL rules, etc.)
    let mut facts = template.to_facts();

    debug!("📋 Template converted to facts:");
    if let Some(layers) = facts.get("layers") {
        debug!("  Layers in facts: {:?}", layers);
    } else {
        debug!("  ⚠️ NO layers in facts!");
    }

    // Add ID based on type (space or page only)
    match resource_type {
        "space" => facts.insert("space_id".to_string(), json!(resource_id)),
        "page" => facts.insert("page_id".to_string(), json!(resource_id)),
        _ => return Err(GurkhaError::ValidationError(format!("Unknown resource type: {}", resource_type))),
    };

    // Copy delegation templates from parent token (for further delegation)
    if let Some(parent_token_str) = parent_token {
        let parent_ucan = crate::parser::Permit::from_token(parent_token_str)
            .map_err(|e| GurkhaError::ParseError(format!("Failed to parse parent token: {}", e)))?;

        let delegation_templates = parent_ucan.delegation_templates();
        if !delegation_templates.is_empty() {
            // Convert delegation templates to JSON
            let mut delegation_json = serde_json::Map::new();
            for (key, template_obj) in delegation_templates {
                let template_facts = template_obj.to_facts();
                delegation_json.insert(key.clone(), json!(template_facts));
            }
            facts.insert("delegation".to_string(), json!(delegation_json));
            debug!("✓ Copied {} delegation templates from parent token", delegation_templates.len());
        }
    }

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

// ==================== SPACE TOKEN DECISIONS ====================

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

    // Copy delegation templates
    if let Some(delegation) = owner_template.get("delegation") {
        decision.add_fact("delegation".into(), delegation.clone());
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

// ==================== PAGE TOKEN DECISIONS ====================

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

    // Copy layers if present
    if let Some(layers) = owner_template.get("layers") {
        decision.add_fact("layers".into(), layers.clone());
    }

    // Copy sync facts if present
    if let Some(sync) = owner_template.get("sync") {
        decision.add_fact("sync".into(), sync.clone());
    }

    // Copy delegation templates
    if let Some(delegation) = owner_template.get("delegation") {
        decision.add_fact("delegation".into(), delegation.clone());
    }

    Ok(decision)
}

// ==================== EXTRACTION/INFERENCE FUNCTIONS ====================

/// Extract template from delegator token (NEVER HARDCODE!)
///
/// This is critical inference logic - extract the rules from the delegator's token
pub fn extract_template_from_token(
    token_str: &str,
    role: &str,
) -> DecisionResult<DelegationTemplate> {
    // Parse the token
    let ucan = crate::parser::Permit::from_token(token_str)
        .map_err(|e| GurkhaError::ParseError(format!("Failed to parse token: {}", e)))?;

    // Extract template
    ucan.get_delegation_template(role)
        .cloned()
        .ok_or_else(|| GurkhaError::InvalidTemplate(format!("Failed to get template for role: {}", role)))
}

/// Extract facts from token
///
/// Facts-only approach - all authorization info is in facts field.
pub fn extract_facts_from_token(token_str: &str) -> DecisionResult<Map<String, Value>> {
    let ucan = crate::parser::Permit::from_token(token_str)
        .map_err(|e| GurkhaError::ParseError(format!("Failed to parse token: {}", e)))?;

    Ok(ucan.facts().clone())
}

/// Validate that a delegation template has required capabilities
pub fn validate_template_has_capability(
    template: &DelegationTemplate,
    required_ability: &str,
) -> DecisionResult<bool> {
    Ok(template.capabilities.values().any(|cap| cap.contains(required_ability)))
}

// ==================== SYNC DECISIONS ====================

/// Context for dual-permit sync decisions (CEL-based)
///
/// Uses CEL functions embedded in permits for authorization decisions.
/// Falls back to hardcoded logic if functions are not present.
#[derive(Debug, Clone)]
pub struct SyncContext {
    our_permit: crate::parser::Permit,
    peer_permit: crate::parser::Permit,
}

impl SyncContext {
    /// Create sync context from two raw tokens
    pub fn new(our_token: &str, peer_token: &str) -> Result<Self, String> {
        let our_permit = crate::parser::Permit::from_token(our_token)
            .map_err(|e| format!("Failed to parse our token: {}", e))?;
        let peer_permit = crate::parser::Permit::from_token(peer_token)
            .map_err(|e| format!("Failed to parse peer token: {}", e))?;

        Ok(Self { our_permit, peer_permit })
    }

    /// Create sync context from parsed permits
    pub fn from_permits(our_permit: crate::parser::Permit, peer_permit: crate::parser::Permit) -> Self {
        Self { our_permit, peer_permit }
    }

    pub fn our_permit(&self) -> &crate::parser::Permit {
        &self.our_permit
    }

    pub fn peer_permit(&self) -> &crate::parser::Permit {
        &self.peer_permit
    }

}

/// Determine if we should send updates for a layer
///
/// Uses simple sync rules from permit facts.
/// Returns DontSend if function not found or evaluates to false.
pub fn should_send_updates(context: &SyncContext, layer_name: &str) -> crate::types::SyncDecision {
    use crate::types::SyncDecision;

    tracing::debug!("🔍 [should_send_updates] Checking layer '{}'", layer_name);

    // TODO: Implement simple permit-based logic in Phase 4
    // For now, check if we have capability for this layer
    if context.our_permit.has_capability(layer_name) {
        if context.our_permit.should_send_full_snapshot(layer_name) {
            tracing::info!("📤 [should_send_updates] '{}' → SendFullSnapshot", layer_name);
            SyncDecision::SendFullSnapshot
        } else {
            tracing::info!("✅ [should_send_updates] '{}' → SendIncrementalUpdates", layer_name);
            SyncDecision::SendIncrementalUpdates
        }
    } else {
        tracing::info!("🚫 [should_send_updates] '{}' → DontSend (no capability)", layer_name);
        SyncDecision::DontSend
    }
}

/// Check if we can receive updates for a layer
///
/// Uses simple sync rules from permit facts.
/// Returns false if no capability or sync disabled.
pub fn can_receive_updates(context: &SyncContext, layer_name: &str) -> bool {
    tracing::debug!("🔒 [can_receive_updates] Checking layer '{}'", layer_name);

    // TODO: Implement simple permit-based logic in Phase 4
    // For now, check if we have capability and sync is not disabled
    let has_capability = context.our_permit.has_capability(layer_name);
    let can_receive = has_capability && !context.our_permit.has_no_incoming_updates(layer_name);

    tracing::info!("🎯 [can_receive_updates] '{}' → {}", layer_name, can_receive);
    can_receive
}

/// Determine if we should REQUEST updates for a layer (used in sync requests)
///
/// Uses simple sync rules - if we can receive, we should request.
/// Returns DontSend if no capability or sync disabled.
pub fn should_request_updates(context: &SyncContext, layer_name: &str) -> SyncDecision {
    tracing::debug!("🔍 [should_request_updates] Checking layer '{}'", layer_name);

    // TODO: Implement simple permit-based logic in Phase 4
    if can_receive_updates(context, layer_name) {
        tracing::info!("✅ [should_request_updates] '{}' → RequestUpdates", layer_name);
        SyncDecision::SendIncrementalUpdates
    } else {
        tracing::info!("🚫 [should_request_updates] '{}' → DontRequest", layer_name);
        SyncDecision::DontSend
    }
}

// ==================== SYNC CONSENT DECISIONS ====================

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

    // CEL rules from template
    if let Some(cel_rules) = consent_template.get("cel_rules") {
        decision.add_fact("cel_rules".into(), cel_rules.clone());
    }

    // CEL functions from template
    if let Some(functions) = consent_template.get("functions") {
        decision.add_fact("functions".into(), functions.clone());
    }

    // Calculate CID of the proof token and add to proof chain
    let proof_cid = crate::crypto::get_permit_cid(node_viewer_permit)?;
    decision.proofs.push(proof_cid.clone());
    decision.proof_tokens.insert(proof_cid, node_viewer_permit.to_string());

    debug!("Space sync consent decision created for space {} -> node {}", space_id, node_pubkey);
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

    // CEL rules from template
    if let Some(cel_rules) = consent_template.get("cel_rules") {
        decision.add_fact("cel_rules".into(), cel_rules.clone());
    }

    // CEL functions from template
    if let Some(functions) = consent_template.get("functions") {
        decision.add_fact("functions".into(), functions.clone());
    }

    // Calculate CID of the proof token and add to proof chain
    let proof_cid = crate::crypto::get_permit_cid(node_viewer_permit)?;
    decision.proofs.push(proof_cid.clone());
    decision.proof_tokens.insert(proof_cid, node_viewer_permit.to_string());

    debug!("Page sync consent decision created for page {} -> node {}", page_id, node_pubkey);
    Ok(decision)
}

// ==================== LAYER PATTERN AUTHORIZATION ====================

/// Check if permit authorizes access to a layer (IDENTITY-based)
///
/// **Note**: This only checks if the viewer CAN access the layer based on identity.
/// State-based checks (can_write, can_delete) happen via auth_lua in AuthSandbox.
///
/// # Arguments
/// * `permit` - The viewer's permit
/// * `layer_name` - Name of the layer to access
/// * `operation` - Operation to check: "create", "sync", "read", "write"
///
/// # Returns
/// `true` if the permit authorizes this layer access
pub fn can_access_layer(permit: &crate::parser::Permit, layer_name: &str, operation: &str) -> bool {
    let aud = permit.core().audience().unwrap_or("");
    let iss = permit.core().issuer().unwrap_or("");

    // Check static capabilities first (existing behavior)
    if permit.has_capability(layer_name) {
        tracing::debug!("✅ [can_access_layer] '{}' has static capability", layer_name);
        return true;
    }

    // Check layer_patterns with placeholder expansion
    for (pattern, config) in permit.layer_patterns() {
        let expanded = pattern
            .replace("{aud}", aud)
            .replace("{iss}", iss);

        if matches_layer_pattern(layer_name, &expanded) {
            let allowed = match operation {
                "create" => config.create,
                "sync" => config.sync,
                "read" | "write" => true, // State-based checks in AuthSandbox
                _ => false,
            };
            tracing::debug!(
                "🔍 [can_access_layer] Pattern '{}' → '{}' matches '{}': {} = {}",
                pattern, expanded, layer_name, operation, allowed
            );
            return allowed;
        }
    }

    tracing::debug!("🚫 [can_access_layer] No pattern matches '{}'", layer_name);
    false
}

/// Match a layer name against a pattern
///
/// Supports:
/// - Exact match: "layer_name" matches "layer_name"
/// - Prefix wildcard: "did:key:abc:*" matches "did:key:abc:orders"
/// - Suffix wildcard: "*:orders" matches "did:key:abc:orders"
fn matches_layer_pattern(layer_name: &str, pattern: &str) -> bool {
    if pattern.ends_with(":*") {
        // Prefix match: "did:key:abc:*" matches "did:key:abc:anything"
        layer_name.starts_with(&pattern[..pattern.len() - 1])
    } else if pattern.ends_with("*") {
        // General prefix: "prefix*" matches "prefixanything"
        layer_name.starts_with(&pattern[..pattern.len() - 1])
    } else if pattern.starts_with("*:") {
        // Suffix match: "*:orders" matches "anything:orders"
        layer_name.ends_with(&pattern[1..])
    } else {
        // Exact match
        layer_name == pattern
    }
}
