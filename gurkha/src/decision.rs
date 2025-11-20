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
use tracing::{debug, info, instrument};

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
    match relationship {
        "owner" => {
            // Owner: full admin capabilities
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
                "persist_share": "auth_capabilities.persist_share == true && relationship == 'owner'",
                "can_connect": "auth_capabilities.can_connect == true",
                "can_delegate": "auth_capabilities.can_delegate == true && operations.own == 'allow'",
                "sync_enabled": "auth_capabilities.sync_enabled == true && operations.read == 'allow'"
            }));
        }
        "viewer" => {
            // Viewer: restricted capabilities
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
                "persist_share": "auth_capabilities.persist_share == true && relationship == 'node'",
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
    match relationship {
        "owner" => {
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
        "node" => {
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
        "user" | "viewer" => {
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
        _ => return Err(GurkhaError::ValidationError(format!("Unknown relationship: {}", relationship))),
    }

    Ok(decision)
}

/// Decide what should be in a viewer authentication token
pub fn decide_viewer_auth(
    verifying_key: &VerifyingKey,
    resource_id: &str,
) -> DecisionResult<TokenDecision> {
    let pub_key_b64 = general_purpose::STANDARD.encode(verifying_key.as_bytes());

    let mut decision = TokenDecision::new(&pub_key_b64); // Self-signed initially

    // No URI capabilities - all authorization in facts
    decision.add_fact("token_type".into(), json!("viewer_auth"));
    decision.add_fact("relationship".into(), json!("viewer"));
    decision.add_fact("resource_id".into(), json!(resource_id));
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

/// Decide what should be in a folder viewer authentication token
///
/// This is for shareable links - one-time use, wildcard audience.
/// The viewer uses this token to initiate connection to the node.
///
/// Folder viewer auth tokens have:
/// - Wildcard audience (for one-time shareable link)
/// - Facts-based authorization (no URI capabilities)
/// - Folder context and viewer relationship
/// - 30 day expiry
pub fn decide_folder_viewer_auth(
    verifying_key: &VerifyingKey,
    folder_id: &str,
) -> DecisionResult<TokenDecision> {
    let pub_key_b64 = general_purpose::STANDARD.encode(verifying_key.as_bytes());

    let mut decision = TokenDecision::new("*"); // Wildcard audience for shareable link

    // No URI capabilities - all authorization in facts
    decision.add_fact("token_type".into(), json!("viewer_auth"));
    decision.add_fact("first_connection".into(), json!(true));
    decision.add_fact("relationship".into(), json!("viewer"));
    decision.add_fact("folder_id".into(), json!(folder_id));
    decision.add_fact("user_id".into(), json!(pub_key_b64));

    // Viewer capabilities for folder access
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
        "folder_access": "allow"
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

// ==================== RESOURCE TOKEN DECISIONS ====================

/// Decide what should be in a resource owner token
///
/// Parses template JSON and decides:
/// - Which documents and capabilities to include from template
/// - Which facts to copy
/// - Self-signed (audience is own pubkey)
/// - Facts-based (no URI capabilities)
pub fn decide_owner_token(
    verifying_key: &VerifyingKey,
    resource_id: &str,
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

    // Extract documents map from template
    // Format: { doc_name: { capability: "collaborator"|"viewer", type: "crdt"|"asset" } }
    let documents = owner_template
        .get("documents")
        .and_then(|v| v.as_object())
        .ok_or_else(|| GurkhaError::InvalidTemplate("Missing documents in owner_template".to_string()))?
        .clone();

    // Build facts from template (no URI capabilities)
    decision.add_fact("token_type".into(), json!("resource_owner"));
    decision.add_fact("relationship".into(), json!("owner"));
    decision.add_fact("resource_id".into(), json!(resource_id));
    decision.add_fact("user_id".into(), json!(pub_key_b64));
    decision.add_fact("documents".into(), json!(documents));

    // Extract operations from template
    let ops = owner_template
        .get("operations")
        .and_then(|v| v.as_object())
        .ok_or_else(|| GurkhaError::InvalidTemplate("Missing operations in owner_template".to_string()))?;

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

/// Decide what capabilities should be delegated to node from owner
///
/// Extracts delegation template from owner token and determines:
/// - Which capabilities the node should receive
/// - Which facts to include
/// - Audience is the node's pubkey

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
/// * `resource_type` - Type of resource ("resource" or "folder")
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

    // Get facts from template (includes documents, operations, CEL rules, etc.)
    let mut facts = template.to_facts();

    debug!("📋 Template converted to facts:");
    if let Some(documents) = facts.get("documents") {
        debug!("  Documents in facts: {:?}", documents);
    } else {
        debug!("  ⚠️ NO documents in facts!");
    }

    // Add resource ID based on type
    if resource_type == "resource" {
        facts.insert("resource_id".to_string(), json!(resource_id));
    } else if resource_type == "folder" {
        facts.insert("folder_id".to_string(), json!(resource_id));
    }

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

// ==================== FOLDER TOKEN DECISIONS ====================

/// Decide what should be in a folder owner token
pub fn decide_folder_owner_token(
    verifying_key: &VerifyingKey,
    folder_id: &str,
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
    decision.add_fact("token_type".into(), json!("folder_owner"));
    decision.add_fact("relationship".into(), json!("owner"));
    decision.add_fact("folder_id".into(), json!(folder_id));
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

/// Context for dual-UCAN sync decisions
#[derive(Debug, Clone)]
pub struct SyncContext {
    our_ucan: crate::parser::Permit,
    peer_ucan: crate::parser::Permit,
}

impl SyncContext {
    /// Create sync context from two raw tokens
    pub fn new(our_token: &str, peer_token: &str) -> Result<Self, String> {
        let our_ucan = crate::parser::Permit::from_token(our_token)
            .map_err(|e| format!("Failed to parse our token: {}", e))?;
        let peer_ucan = crate::parser::Permit::from_token(peer_token)
            .map_err(|e| format!("Failed to parse peer token: {}", e))?;

        Ok(Self { our_ucan, peer_ucan })
    }

    /// Create sync context from parsed UCANs
    pub fn from_ucans(our_ucan: crate::parser::Permit, peer_ucan: crate::parser::Permit) -> Self {
        Self { our_ucan, peer_ucan }
    }

    pub fn our_ucan(&self) -> &crate::parser::Permit {
        &self.our_ucan
    }

    pub fn peer_ucan(&self) -> &crate::parser::Permit {
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

    tracing::debug!("🔍 [should_send_updates] Checking document '{}'", doc_name);

    // 1. Check local_only facts (ours OR peer's)
    // Don't send if either we or the peer want to keep this doc local
    let our_local_only = context.our_ucan.is_local_only(doc_name);
    let peer_local_only = context.peer_ucan.is_local_only(doc_name);
    tracing::debug!("📋 [should_send_updates] '{}': our_local_only={}, peer_local_only={}", doc_name, our_local_only, peer_local_only);

    if our_local_only || peer_local_only {
        tracing::warn!("🚫 [should_send_updates] '{}' → DontSend (local_only)", doc_name);
        return SyncDecision::DontSend;
    }

    // 2. Check our capability (can WE write?)
    match context.our_ucan.get_capability(doc_name) {
        None => {
            tracing::warn!("🚫 [should_send_updates] '{}' → DontSend (no our_capability)", doc_name);
            return SyncDecision::DontSend;
        }
        Some(our_cap) => {
            tracing::debug!("📄 [should_send_updates] '{}': our_capability={:?}, can_write={}", doc_name, our_cap, our_cap.can_write());

            if !our_cap.can_write() {
                // We have Viewer capability → can't send
                tracing::warn!("🚫 [should_send_updates] '{}' → DontSend (our_capability={:?}, can't write)", doc_name, our_cap);
                return SyncDecision::DontSend;
            }

            // Check if we should send full snapshot (Submitter)
            if context.our_ucan.should_send_full_snapshot(doc_name) {
                tracing::info!("📤 [should_send_updates] '{}' → SendFullSnapshot (submitter)", doc_name);
                return SyncDecision::SendFullSnapshot;
            }
        }
    }

    // 3. Check peer's no_incoming_updates
    let peer_no_incoming = context.peer_ucan.has_no_incoming_updates(doc_name);
    tracing::debug!("📋 [should_send_updates] '{}': peer_no_incoming_updates={}", doc_name, peer_no_incoming);

    if peer_no_incoming {
        tracing::warn!("🚫 [should_send_updates] '{}' → DontSend (peer no_incoming_updates)", doc_name);
        return SyncDecision::DontSend;
    }

    // 4. Check peer capability (can THEY receive?)
    match context.peer_ucan.get_capability(doc_name) {
        None => {
            tracing::warn!("🚫 [should_send_updates] '{}' → DontSend (no peer_capability)", doc_name);
            SyncDecision::DontSend
        }
        Some(peer_cap) => {
            let can_sync_bidirectional = peer_cap.can_sync_bidirectional();
            let can_write = peer_cap.can_write();
            tracing::debug!("📄 [should_send_updates] '{}': peer_capability={:?}, can_sync_bidirectional={}, can_write={}", doc_name, peer_cap, can_sync_bidirectional, can_write);

            // Peer can receive if they have Collaborator or Viewer capability
            if can_sync_bidirectional || !can_write {
                tracing::info!("✅ [should_send_updates] '{}' → SendIncrementalUpdates", doc_name);
                SyncDecision::SendIncrementalUpdates
            } else {
                tracing::warn!("🚫 [should_send_updates] '{}' → DontSend (peer capability check failed)", doc_name);
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
    tracing::debug!("🔒 [can_receive_updates] Checking permissions for document '{}'", doc_name);

    // 1. Check our no_incoming_updates facts
    if context.our_ucan.has_no_incoming_updates(doc_name) {
        tracing::warn!("🚫 [can_receive_updates] Document '{}' has no_incoming_updates fact - DENYING", doc_name);
        return false;
    }

    // 2. Check our capability (can WE receive?)
    match context.our_ucan.get_capability(doc_name) {
        None => {
            tracing::warn!("🚫 [can_receive_updates] Document '{}' has no capability defined - DENYING", doc_name);
            false
        }
        Some(our_cap) => {
            let can_bidirectional = our_cap.can_sync_bidirectional();
            let can_write = our_cap.can_write();
            let result = can_bidirectional || !can_write;

            tracing::info!(
                "🔒 [can_receive_updates] Document '{}': capability={:?}, can_sync_bidirectional={}, can_write={}, result={}",
                doc_name, our_cap, can_bidirectional, can_write, result
            );

            if result {
                tracing::info!("✅ [can_receive_updates] Document '{}' - ALLOWING updates", doc_name);
            } else {
                tracing::warn!("🚫 [can_receive_updates] Document '{}' - DENYING updates (capability check failed)", doc_name);
            }

            result
        }
    }
}

/// Determine if we should REQUEST updates for a document (used in sync requests)
///
/// Different from should_send_updates - this checks if we want to RECEIVE updates
/// from the peer, so we send our state vector to enable incremental sync.
///
/// Algorithm:
/// 1. Check local_only → DontRequest (no sync for local-only docs)
/// 2. Check our no_incoming_updates → DontRequest (we don't want updates)
/// 3. Check our capability → if we can read (Viewer/Collaborator), RequestUpdates
/// 4. Check peer capability → if peer can write (Collaborator), RequestUpdates
///
/// # Returns
/// - RequestUpdates: Send state vector to request incremental updates
/// - RequestFullSnapshot: Request full snapshot (not used in current implementation)
/// - DontRequest: Don't request updates for this document
pub fn should_request_updates(context: &SyncContext, doc_name: &str) -> SyncDecision {
    tracing::debug!("🔍 [should_request_updates] Checking document '{}'", doc_name);

    // 1. Check local_only facts (ours OR peer's)
    let our_local_only = context.our_ucan.is_local_only(doc_name);
    let peer_local_only = context.peer_ucan.is_local_only(doc_name);
    tracing::debug!("📋 [should_request_updates] '{}': our_local_only={}, peer_local_only={}", doc_name, our_local_only, peer_local_only);

    if our_local_only || peer_local_only {
        tracing::warn!("🚫 [should_request_updates] '{}' → DontRequest (local_only)", doc_name);
        return SyncDecision::DontSend; // Reuse DontSend for "don't request"
    }

    // 2. Check our no_incoming_updates
    let our_no_incoming = context.our_ucan.has_no_incoming_updates(doc_name);
    tracing::debug!("📋 [should_request_updates] '{}': our_no_incoming_updates={}", doc_name, our_no_incoming);

    if our_no_incoming {
        tracing::warn!("🚫 [should_request_updates] '{}' → DontRequest (our no_incoming_updates)", doc_name);
        return SyncDecision::DontSend;
    }

    // 3. Check our capability (can WE read/receive?)
    match context.our_ucan.get_capability(doc_name) {
        None => {
            tracing::warn!("🚫 [should_request_updates] '{}' → DontRequest (no our_capability)", doc_name);
            SyncDecision::DontSend
        }
        Some(our_cap) => {
            tracing::debug!("📄 [should_request_updates] '{}': our_capability={:?}", doc_name, our_cap);

            // We can request updates if we have ANY capability (Viewer, Collaborator, Submitter)
            // because we want to receive data

            // Check peer capability - can they send to us?
            match context.peer_ucan.get_capability(doc_name) {
                None => {
                    tracing::warn!("🚫 [should_request_updates] '{}' → DontRequest (no peer_capability)", doc_name);
                    SyncDecision::DontSend
                }
                Some(peer_cap) => {
                    tracing::debug!("📄 [should_request_updates] '{}': peer_capability={:?}", doc_name, peer_cap);

                    // Peer can send if they can write (Collaborator/Submitter)
                    if peer_cap.can_write() {
                        tracing::info!("✅ [should_request_updates] '{}' → RequestUpdates (peer can write, we can receive)", doc_name);
                        SyncDecision::SendIncrementalUpdates // Reuse for "request incremental"
                    } else {
                        tracing::warn!("🚫 [should_request_updates] '{}' → DontRequest (peer can't write)", doc_name);
                        SyncDecision::DontSend
                    }
                }
            }
        }
    }
}
