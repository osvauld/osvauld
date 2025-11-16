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
use crate::uri;
use base64::{Engine as _, engine::general_purpose};
use ed25519_dalek::VerifyingKey;
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use tracing::{debug, error, info, instrument};

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
}

impl TokenDecision {
    pub fn new(audience: &str) -> Self {
        Self {
            audience: audience.to_string(),
            capabilities: Vec::new(),
            facts: Map::new(),
            expiry: None,
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
}

// ==================== CONNECTION TOKEN DECISIONS ====================

/// Decide what should be in a one-time connection token
///
/// One-time tokens have:
/// - Wildcard audience (for first connection)
/// - user-connect capability
/// - 30 day expiry
pub fn decide_one_time_token(
    verifying_key: &VerifyingKey,
    capability_str: &str,
    role: &str,
) -> DecisionResult<TokenDecision> {
    let pub_key_b64 = general_purpose::STANDARD.encode(verifying_key.as_bytes());
    let capability = format!("{}:user-connect:{}", capability_str, pub_key_b64);

    let mut decision = TokenDecision::new("*"); // Wildcard audience

    decision.add_capability(capability, "use".to_string());

    decision.add_fact("token_type".into(), json!("one_time_connection"));
    decision.add_fact("role".into(), json!(role));
    decision.add_fact("first_connection".into(), json!(true));

    decision.set_expiry(30 * 24 * 60 * 60); // 30 days

    Ok(decision)
}

/// Decide what should be in a peer connection token
///
/// Peer tokens have:
/// - Specific peer's pubkey as audience
/// - Role-based capabilities (owner/node/user/viewer)
/// - Longer expiry
pub fn decide_peer_connection(
    verifying_key: &VerifyingKey,
    domain: &str,
    peer_pubkey: &str,
    role: &str,
) -> DecisionResult<TokenDecision> {
    let pub_key_b64 = general_purpose::STANDARD.encode(verifying_key.as_bytes());
    let user_id = &pub_key_b64;

    let mut decision = TokenDecision::new(peer_pubkey);

    // Role-based capabilities
    match role {
        "owner" => {
            decision.add_capability(
                format!("{}:peer-connection:*", domain),
                "admin".to_string(),
            );
        }
        "node" => {
            decision.add_capability(
                format!("{}:peer-connection:*", domain),
                "sync".to_string(),
            );
        }
        "user" => {
            decision.add_capability(
                format!("{}:peer-connection:*", domain),
                "read".to_string(),
            );
        }
        "viewer" => {
            decision.add_capability(
                format!("{}:peer-connection:*", domain),
                "read".to_string(),
            );
        }
        _ => return Err(GurkhaError::ValidationError(format!("Unknown role: {}", role))),
    }

    decision.add_fact("token_type".into(), json!(format!("{}_connection", role)));
    decision.add_fact("role".into(), json!(role));
    decision.add_fact("user_id".into(), json!(user_id));

    Ok(decision)
}

/// Decide what should be in a viewer authentication token
pub fn decide_viewer_auth(
    verifying_key: &VerifyingKey,
    resource_id: &str,
    domain: &str,
) -> DecisionResult<TokenDecision> {
    let pub_key_b64 = general_purpose::STANDARD.encode(verifying_key.as_bytes());

    let mut decision = TokenDecision::new(&pub_key_b64); // Self-signed initially

    let capability_uri = uri::resource_capability(domain, resource_id, "viewer");
    decision.add_capability(
        format!("{}:crud/readonly", capability_uri),
        String::new(),
    );

    decision.add_fact("token_type".into(), json!("viewer_auth"));
    decision.add_fact("role".into(), json!("viewer"));

    Ok(decision)
}

/// Decide what should be in a folder viewer authentication token
///
/// This is for shareable links - one-time use, wildcard audience.
/// The viewer uses this token to initiate connection to the node.
///
/// Folder viewer auth tokens have:
/// - Wildcard audience (for one-time shareable link)
/// - Folder connect capability (allows establishing connection)
/// - Folder access capability (allows accessing the folder)
/// - 30 day expiry
pub fn decide_folder_viewer_auth(
    verifying_key: &VerifyingKey,
    folder_id: &str,
    domain: &str,
) -> DecisionResult<TokenDecision> {
    let _pub_key_b64 = general_purpose::STANDARD.encode(verifying_key.as_bytes());

    let mut decision = TokenDecision::new("*"); // Wildcard audience for shareable link

    // Connect capability - allows viewer to establish connection
    let connect_capability = format!("{}:folder:{}:connect", domain, folder_id);
    decision.add_capability(connect_capability, "use".to_string());

    // Folder access capability - allows viewer to access folder
    let folder_capability = uri::folder_operation(domain, folder_id, "access");
    decision.add_capability(folder_capability, "allow".to_string());

    decision.add_fact("token_type".into(), json!("viewer_auth"));
    decision.add_fact("role".into(), json!("viewer"));
    decision.add_fact("folder_id".into(), json!(folder_id));

    // Short expiry for security (30 days like one-time connection)
    decision.set_expiry(30 * 24 * 60 * 60);

    Ok(decision)
}

// ==================== RESOURCE TOKEN DECISIONS ====================

/// Decide what should be in a resource owner token
///
/// Parses template JSON and decides:
/// - Which capabilities to include from template
/// - Which facts to copy
/// - Self-signed (audience is own pubkey)
pub fn decide_owner_token(
    verifying_key: &VerifyingKey,
    resource_id: &str,
    domain: &str,
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

    // Extract and build capabilities from template
    let caps = owner_template
        .get("capabilities")
        .and_then(|v| v.as_object())
        .ok_or_else(|| GurkhaError::InvalidTemplate("Missing capabilities in owner_template".to_string()))?;

    for (doc_name, cap_value) in caps.iter() {
        let cap_str = cap_value.as_str().ok_or_else(|| {
            GurkhaError::InvalidTemplate(format!("Invalid capability value for {}", doc_name))
        })?;
        let capability_uri = uri::resource_capability(domain, resource_id, doc_name);
        decision.add_capability(
            format!("{}:{}", capability_uri, cap_str),
            String::new(),
        );
    }

    // Build facts from template
    decision.add_fact("token_type".into(), json!("resource_owner"));
    decision.add_fact("role".into(), json!("owner"));

    // Copy sync facts if present
    if let Some(sync) = owner_template.get("sync") {
        decision.add_fact("sync".into(), sync.clone());
    }

    // Copy doc_metadata if present
    if let Some(doc_metadata) = owner_template.get("doc_metadata") {
        decision.add_fact("doc_metadata".into(), doc_metadata.clone());
    }

    // Copy delegation templates
    if let Some(delegation) = owner_template.get("delegation") {
        decision.add_fact("delegation".into(), delegation.clone());
    }

    Ok(decision)
}

/// Decide what capabilities should be delegated to node from owner
///
/// Extracts delegation template from owner token and determines:
/// - Which capabilities the node should receive
/// - Which facts to include
/// - Audience is the node's pubkey
pub fn decide_delegation_to_node(
    template: &DelegationTemplate,
    domain: &str,
    resource_id: &str,
    node_pubkey: &str,
) -> DecisionResult<DelegationDecision> {
    let mut decision = DelegationDecision {
        audience: node_pubkey.to_string(),
        capabilities: template.build_capabilities(domain, resource_id, "resource"),
        facts: template.to_facts(),
        template: Some(template.clone()),
    };

    decision.facts.insert("role".into(), json!("node"));

    Ok(decision)
}

/// Decide what capabilities should be delegated to user
pub fn decide_delegation_to_user(
    template: &DelegationTemplate,
    domain: &str,
    resource_id: &str,
    user_pubkey: &str,
) -> DecisionResult<DelegationDecision> {
    let mut decision = DelegationDecision {
        audience: user_pubkey.to_string(),
        capabilities: template.build_capabilities(domain, resource_id, "resource"),
        facts: template.to_facts(),
        template: Some(template.clone()),
    };

    decision.facts.insert("role".into(), json!("user"));

    Ok(decision)
}

/// Decide what capabilities should be delegated to viewer
pub fn decide_delegation_to_viewer(
    template: &DelegationTemplate,
    domain: &str,
    resource_id: &str,
    viewer_pubkey: &str,
) -> DecisionResult<DelegationDecision> {
    let mut decision = DelegationDecision {
        audience: viewer_pubkey.to_string(),
        capabilities: template.build_capabilities(domain, resource_id, "resource"),
        facts: template.to_facts(),
        template: Some(template.clone()),
    };

    decision.facts.insert("role".into(), json!("viewer"));

    Ok(decision)
}

// ==================== FOLDER TOKEN DECISIONS ====================

/// Decide what should be in a folder owner token
pub fn decide_folder_owner_token(
    verifying_key: &VerifyingKey,
    folder_id: &str,
    domain: &str,
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

    // Extract and build capabilities from template
    let caps = owner_template
        .get("capabilities")
        .and_then(|v| v.as_object())
        .ok_or_else(|| GurkhaError::InvalidTemplate("Missing capabilities in owner_template".to_string()))?;

    for (operation_name, cap_value) in caps.iter() {
        let cap_str = cap_value.as_str().ok_or_else(|| {
            GurkhaError::InvalidTemplate(format!("Invalid capability value for {}", operation_name))
        })?;
        let capability_uri = uri::folder_operation(domain, folder_id, operation_name);
        decision.add_capability(
            capability_uri,
            cap_str.to_string(),
        );
    }

    // Build facts from template
    decision.add_fact("token_type".into(), json!("folder_owner"));
    decision.add_fact("role".into(), json!("owner"));
    decision.add_fact("folder_id".into(), json!(folder_id));

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

/// Decide what capabilities should be delegated for folder
pub fn decide_folder_delegation(
    template: &DelegationTemplate,
    domain: &str,
    folder_id: &str,
    target_pubkey: &str,
    target_role: &str,
) -> DecisionResult<DelegationDecision> {
    let mut decision = DelegationDecision {
        audience: target_pubkey.to_string(),
        capabilities: template.build_capabilities(domain, folder_id, "folder"),
        facts: template.to_facts(),
        template: Some(template.clone()),
    };

    decision.facts.insert("role".into(), json!(target_role));
    decision.facts.insert("folder_id".into(), json!(folder_id));

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
    let ucan = crate::parser::GenericUcan::from_token(token_str)
        .map_err(|e| GurkhaError::ParseError(format!("Failed to parse token: {}", e)))?;

    // Extract template
    ucan.get_delegation_template(role)
        .cloned()
        .ok_or_else(|| GurkhaError::InvalidTemplate(format!("Failed to get template for role: {}", role)))
}

/// Extract domain from token
///
/// Domain is encoded in capability URIs, not in facts.
/// URI format: `domain:resource:id:doc_name` or `domain:folder:id:operation`
/// We extract the domain from the first capability URI.
#[instrument(skip(token_str))]
pub fn extract_domain_from_token(token_str: &str) -> DecisionResult<String> {
    debug!("🔍 Extracting domain from token");

    let ucan = crate::parser::GenericUcan::from_token(token_str)
        .map_err(|e| {
            error!("❌ Failed to parse token: {}", e);
            GurkhaError::ParseError(format!("Failed to parse token: {}", e))
        })?;
    debug!("✓ Token parsed successfully");

    // Extract domain from the first capability URI
    for cap in ucan.parsed().capabilities().iter() {
        let parts: Vec<&str> = cap.resource.split(':').collect();
        if !parts.is_empty() {
            let domain = parts[0];
            if !domain.is_empty() {
                info!("✓ Domain extracted from capability URI: {}", domain);
                return Ok(domain.to_string());
            }
        }
    }

    error!("❌ No valid domain found in token capabilities");
    Err(GurkhaError::ValidationError(
        "Cannot extract domain from token - no valid capabilities found".to_string()
    ))
}

/// Extract capabilities from token as human-readable map
pub fn extract_capabilities_from_token(token_str: &str) -> DecisionResult<HashMap<String, String>> {
    let ucan = crate::parser::GenericUcan::from_token(token_str)
        .map_err(|e| GurkhaError::ParseError(format!("Failed to parse token: {}", e)))?;

    let mut caps = HashMap::new();
    for cap in ucan.parsed().capabilities().iter() {
        caps.insert(cap.resource.clone(), cap.ability.clone());
    }

    Ok(caps)
}

/// Extract facts from token
pub fn extract_facts_from_token(token_str: &str) -> DecisionResult<Map<String, Value>> {
    let ucan = crate::parser::GenericUcan::from_token(token_str)
        .map_err(|e| GurkhaError::ParseError(format!("Failed to parse token: {}", e)))?;

    crate::extractors::get_facts(ucan.parsed())
        .ok_or_else(|| GurkhaError::ValidationError("No facts in token".to_string()))
}

/// Validate that a delegation template has required capabilities
pub fn validate_template_has_capability(
    template: &DelegationTemplate,
    required_ability: &str,
) -> DecisionResult<bool> {
    Ok(template.capabilities.values().any(|cap| cap.contains(required_ability)))
}

// ==================== SYNC DECISIONS ====================

/// Context for dual-UCAN sync decisions
#[derive(Debug, Clone)]
pub struct SyncContext {
    our_ucan: crate::parser::GenericUcan,
    peer_ucan: crate::parser::GenericUcan,
}

impl SyncContext {
    /// Create sync context from two raw tokens
    pub fn new(our_token: &str, peer_token: &str) -> Result<Self, String> {
        let our_ucan = crate::parser::GenericUcan::from_token(our_token)
            .map_err(|e| format!("Failed to parse our token: {}", e))?;
        let peer_ucan = crate::parser::GenericUcan::from_token(peer_token)
            .map_err(|e| format!("Failed to parse peer token: {}", e))?;

        Ok(Self { our_ucan, peer_ucan })
    }

    /// Create sync context from parsed UCANs
    pub fn from_ucans(our_ucan: crate::parser::GenericUcan, peer_ucan: crate::parser::GenericUcan) -> Self {
        Self { our_ucan, peer_ucan }
    }

    pub fn our_ucan(&self) -> &crate::parser::GenericUcan {
        &self.our_ucan
    }

    pub fn peer_ucan(&self) -> &crate::parser::GenericUcan {
        &self.peer_ucan
    }
}

/// Determine if we should send updates for a document
///
/// Algorithm:
/// 1. Check our local_only facts → DontSend
/// 2. Check our capability (can WE write?) → Viewer: DontSend, Collaborator: Continue, Submitter: SendFullSnapshot
/// 3. Check peer's no_incoming_updates → DontSend
/// 4. Check peer capability (can THEY receive?) → Collaborator/Viewer: SendIncrementalUpdates
pub fn should_send_updates(context: &SyncContext, doc_name: &str) -> crate::types::SyncDecision {
    use crate::types::SyncDecision;

    // 1. Check our local_only facts
    if context.our_ucan.is_local_only(doc_name) {
        return SyncDecision::DontSend;
    }

    // 2. Check our capability (can WE write?)
    match context.our_ucan.get_capability(doc_name) {
        None => return SyncDecision::DontSend,
        Some(our_cap) => {
            if !our_cap.can_write() {
                // We have Viewer capability → can't send
                return SyncDecision::DontSend;
            }

            // Check if we should send full snapshot (Submitter)
            if context.our_ucan.should_send_full_snapshot(doc_name) {
                return SyncDecision::SendFullSnapshot;
            }
        }
    }

    // 3. Check peer's no_incoming_updates
    if context.peer_ucan.has_no_incoming_updates(doc_name) {
        return SyncDecision::DontSend;
    }

    // 4. Check peer capability (can THEY receive?)
    match context.peer_ucan.get_capability(doc_name) {
        None => SyncDecision::DontSend,
        Some(peer_cap) => {
            // Peer can receive if they have Collaborator or Viewer capability
            if peer_cap.can_sync_bidirectional() || !peer_cap.can_write() {
                SyncDecision::SendIncrementalUpdates
            } else {
                SyncDecision::DontSend
            }
        }
    }
}

/// Check if we can receive updates for a document
///
/// Algorithm:
/// 1. Check our no_incoming_updates facts → false
/// 2. Check our capability (can WE receive?) → Collaborator/Viewer: true
pub fn can_receive_updates(context: &SyncContext, doc_name: &str) -> bool {
    // 1. Check our no_incoming_updates facts
    if context.our_ucan.has_no_incoming_updates(doc_name) {
        return false;
    }

    // 2. Check our capability (can WE receive?)
    match context.our_ucan.get_capability(doc_name) {
        None => false,
        Some(our_cap) => {
            // We can receive if we have Collaborator or Viewer capability
            our_cap.can_sync_bidirectional() || !our_cap.can_write()
        }
    }
}
