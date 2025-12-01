//! Permit Service - Stateless permit token operations
//!
//! Provides stateless functions for permit token generation and delegation.
//! All functions take signing key bytes and return signed permits.
//!
//! Usage:
//! ```rust
//! let secret_key: [u8; 32] = identity.secret_signing_key();
//! let (token, cid) = gurkha::issue_one_time(&secret_key, "owner").await?;
//! ```

use crate::crypto;
use crate::decision;
use crate::errors::{ServiceError, ServiceResult};
use crate::parser::Permit;
use base64::{engine::general_purpose, Engine as _};
use ed25519_dalek::SigningKey;
use tracing::{debug, info, instrument};

// ==================== CONNECTION TOKENS ====================

/// Issue a one-time connection token
///
/// Used for initial device pairing (QR codes, connection strings).
/// Facts-only approach - relationship determines permissions.
///
/// # Arguments
/// * `signing_key_bytes` - 32-byte Ed25519 secret key
/// * `relationship` - "owner", "node", "peer_user", or "peer_node"
#[instrument(skip(signing_key_bytes), fields(relationship = %relationship, token_type = "one_time"))]
pub async fn issue_one_time(
    signing_key_bytes: &[u8; 32],
    relationship: &str,
) -> ServiceResult<(String, String)> {
    debug!("Issuing one-time connection token");

    let signing_key = SigningKey::from_bytes(signing_key_bytes);
    let verifying_key = signing_key.verifying_key();

    let decision = decision::decide_one_time_token(&verifying_key, relationship)?;
    debug!("Token decision created");

    let (token, cid) = crypto::sign_permit(signing_key_bytes, &decision).await?;

    info!("One-time token generated: cid={}", cid);
    Ok((token, cid))
}

/// Issue a peer connection token
///
/// Establishes bidirectional peer connection with signed permit.
/// Facts-only approach - no domain needed, relationship determines permissions.
///
/// # Arguments
/// * `signing_key_bytes` - 32-byte Ed25519 secret key
/// * `peer_pubkey` - Base64-encoded public key of the peer
/// * `relationship` - "owner", "node", "peer_user", or "peer_node"
#[instrument(skip(signing_key_bytes, peer_pubkey), fields(relationship = %relationship, token_type = "peer_connection"))]
pub async fn issue_peer_connection(
    signing_key_bytes: &[u8; 32],
    peer_pubkey: &str,
    relationship: &str,
) -> ServiceResult<String> {
    debug!("Issuing peer connection token");

    let signing_key = SigningKey::from_bytes(signing_key_bytes);
    let verifying_key = signing_key.verifying_key();

    let decision = decision::decide_peer_connection(&verifying_key, peer_pubkey, relationship)?;

    let (token, _cid) = crypto::sign_permit(signing_key_bytes, &decision).await?;

    info!("Peer connection token generated");
    Ok(token)
}

/// Issue a viewer authentication token
///
/// Facts-only approach - no domain needed.
///
/// # Arguments
/// * `signing_key_bytes` - 32-byte Ed25519 secret key
/// * `resource_id` - Resource identifier
#[instrument(skip(signing_key_bytes), fields(resource_id = %resource_id, token_type = "viewer_auth"))]
pub async fn issue_viewer_auth(
    signing_key_bytes: &[u8; 32],
    resource_id: &str,
) -> ServiceResult<String> {
    debug!("Issuing viewer authentication token");

    let signing_key = SigningKey::from_bytes(signing_key_bytes);
    let verifying_key = signing_key.verifying_key();

    let decision = decision::decide_viewer_auth(&verifying_key, resource_id)?;

    let (token, _cid) = crypto::sign_permit(signing_key_bytes, &decision).await?;

    info!("Viewer auth token generated");
    Ok(token)
}

/// Issue a folder viewer authentication token (for shareable links)
///
/// This is a one-time token with wildcard audience that viewers use to connect.
/// Used in the FolderTokenRequest flow to generate shareable connection strings.
/// Facts-only approach - no domain needed.
///
/// # Arguments
/// * `signing_key_bytes` - 32-byte Ed25519 secret key
/// * `folder_id` - Folder identifier
#[instrument(skip(signing_key_bytes), fields(folder_id = %folder_id, token_type = "folder_viewer_auth"))]
pub async fn issue_folder_viewer_auth(
    signing_key_bytes: &[u8; 32],
    folder_id: &str,
) -> ServiceResult<(String, String)> {
    debug!("Issuing folder viewer authentication token");

    let signing_key = SigningKey::from_bytes(signing_key_bytes);
    let verifying_key = signing_key.verifying_key();

    let decision = decision::decide_folder_viewer_auth(&verifying_key, folder_id)?;
    debug!("Token decision created");

    let (token, cid) = crypto::sign_permit(signing_key_bytes, &decision).await?;

    info!("Folder viewer auth token generated: cid={}", cid);
    Ok((token, cid))
}

// ==================== PAGE TOKENS ====================

/// Unified page delegation
///
/// Delegates a page to any audience using a template key.
/// Template key determines the delegation pattern (e.g., "node", "viewer").
///
/// # Arguments
/// * `signing_key_bytes` - 32-byte Ed25519 secret key
/// * `delegator_token` - Token of the delegator (must have share_page capability)
/// * `template_key` - Template key to use for delegation ("node", "viewer", etc.)
/// * `audience_pubkey` - Public key of the delegatee
///
/// # Returns
/// * `Ok((token_string, cid))` - The delegated token and its CID
pub async fn delegate_page(
    signing_key_bytes: &[u8; 32],
    delegator_token: &str,
    template_key: &str,
    audience_pubkey: &str,
) -> ServiceResult<(String, String)> {
    let _signing_key = SigningKey::from_bytes(signing_key_bytes);

    // Extract delegation template from delegator token
    let template = decision::extract_template_from_token(delegator_token, template_key)?;

    let parsed_permit = Permit::from_token(delegator_token)?;

    // Extract page ID from facts (new terminology)
    let page_id = parsed_permit
        .get_fact("page_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            ServiceError::InvalidPermit("Missing page_id in token facts".to_string())
        })?
        .to_string();

    // Validate token has share_page permission (from facts.operations)
    let has_share = parsed_permit
        .get_fact("operations")
        .and_then(|ops| ops.as_object())
        .and_then(|ops| ops.get("share_page"))
        .and_then(|v| v.as_str())
        .map(|s| s == "allow")
        .unwrap_or(false);

    if !has_share {
        return Err(ServiceError::ValidationError(
            "Token does not have share_page permission".to_string(),
        ));
    }

    // Create delegation decision
    let delegation_decision = decision::decide_delegation(
        &template,
        &page_id,
        "page",
        audience_pubkey,
        Some(delegator_token),
    )?;

    // Use builder to create token
    let builder = crate::builder::GurkhaPermitBuilder::from_bytes(signing_key_bytes);
    let (token, cid) = builder.build(delegation_decision.into()).await?;

    Ok((token, cid))
}

// ==================== SPACE TOKENS ====================

/// Issue space owner token
///
/// Facts-only approach - no domain needed.
///
/// # Arguments
/// * `signing_key_bytes` - 32-byte Ed25519 secret key
/// * `space_id` - Space identifier
/// * `permit_template_json` - JSON template for the permit
#[instrument(skip(signing_key_bytes, permit_template_json), fields(space_id = %space_id, token_type = "space_owner"))]
pub async fn issue_space_owner_token(
    signing_key_bytes: &[u8; 32],
    space_id: &str,
    permit_template_json: &str,
) -> ServiceResult<(String, String)> {
    debug!("Issuing space owner token");

    let signing_key = SigningKey::from_bytes(signing_key_bytes);
    let verifying_key = signing_key.verifying_key();

    debug!("Parsing space template");
    let decision =
        decision::decide_space_owner_token(&verifying_key, space_id, permit_template_json)?;
    debug!("Token decision created");

    let (token, cid) = crypto::sign_permit(signing_key_bytes, &decision).await?;

    info!("Space owner token generated: cid={}", cid);
    Ok((token, cid))
}

/// Unified space delegation
///
/// Delegates a space to any audience using a template key.
/// Template key determines the delegation pattern (e.g., "node", "viewer").
///
/// # Arguments
/// * `signing_key_bytes` - 32-byte Ed25519 secret key
/// * `delegator_token` - Token of the delegator (must have add_pages capability)
/// * `template_key` - Template key to use for delegation ("node", "viewer", etc.)
/// * `audience_pubkey` - Public key of the delegatee
///
/// # Returns
/// * `Ok((token_string, cid))` - The delegated token and its CID
pub async fn delegate_space(
    signing_key_bytes: &[u8; 32],
    delegator_token: &str,
    template_key: &str,
    audience_pubkey: &str,
) -> ServiceResult<(String, String)> {
    let _signing_key = SigningKey::from_bytes(signing_key_bytes);

    // Extract delegation template from delegator token
    let template = decision::extract_template_from_token(delegator_token, template_key)?;

    let parsed_permit = Permit::from_token(delegator_token)?;

    // Extract space ID from facts (new terminology)
    let space_id = parsed_permit
        .get_fact("space_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            ServiceError::InvalidPermit("Missing space_id in token facts".to_string())
        })?
        .to_string();

    // Validate token has add_pages permission (from facts.operations)
    let has_add_pages = parsed_permit
        .get_fact("operations")
        .and_then(|ops| ops.as_object())
        .and_then(|ops| ops.get("add_pages"))
        .and_then(|v| v.as_str())
        .map(|s| s == "allow")
        .unwrap_or(false);

    if !has_add_pages {
        return Err(ServiceError::ValidationError(
            "Token does not have add_pages permission".to_string(),
        ));
    }

    // Create delegation decision
    let delegation_decision = decision::decide_delegation(
        &template,
        &space_id,
        "space",
        audience_pubkey,
        Some(delegator_token),
    )?;

    // Use builder to create token
    let builder = crate::builder::GurkhaPermitBuilder::from_bytes(signing_key_bytes);
    let (token, cid) = builder.build(delegation_decision.into()).await?;

    Ok((token, cid))
}

// ==================== EXTRACTION & UTILITIES ====================

/// Get public key from signing key bytes
pub fn get_public_key(signing_key_bytes: &[u8; 32]) -> String {
    let signing_key = SigningKey::from_bytes(signing_key_bytes);
    let verifying_key = signing_key.verifying_key();
    general_purpose::STANDARD.encode(verifying_key.as_bytes())
}

/// Extract space ID from token
#[instrument(skip(permit_token))]
pub fn extract_space_id(permit_token: &str) -> ServiceResult<String> {
    debug!("Extracting space ID from token");
    let permit = Permit::from_token(permit_token)?;

    let space_id = permit.get_fact("space_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            ServiceError::InvalidPermit("Cannot extract space_id".to_string())
        })?;

    debug!("Space ID extracted: {}", space_id);
    Ok(space_id.to_string())
}

/// Extract page ID from token
#[instrument(skip(permit_token))]
pub fn extract_page_id(permit_token: &str) -> ServiceResult<String> {
    debug!("Extracting page ID from token");
    let permit = Permit::from_token(permit_token)?;

    let page_id = permit.get_fact("page_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            ServiceError::InvalidPermit("Cannot extract page_id".to_string())
        })?;

    debug!("Page ID extracted: {}", page_id);
    Ok(page_id.to_string())
}

/// Extract layer capabilities from token
///
/// Reads from facts.layers map
#[instrument(skip(permit_token))]
pub async fn extract_capabilities(permit_token: &str) -> ServiceResult<Vec<(String, String)>> {
    debug!("Extracting document capabilities from token facts");
    let permit = Permit::from_token(permit_token)?;

    let mut doc_capabilities = Vec::new();

    // V3: Extract from facts.documents map
    // Format: { doc_name: { type: "crdt"|"asset", capability: "collaborator"|"viewer" } }
    if let Some(documents) = permit.get_fact("documents").and_then(|v| v.as_object()) {
        for (doc_name, doc_info) in documents {
            if let Some(capability) = doc_info
                .as_object()
                .and_then(|obj| obj.get("capability"))
                .and_then(|v| v.as_str())
            {
                doc_capabilities.push((doc_name.clone(), capability.to_string()));
            }
        }
    }

    debug!("Extracted {} document capabilities", doc_capabilities.len());
    Ok(doc_capabilities)
}

/// Validate permit structure
#[instrument(skip(permit_token))]
pub async fn validate_permit_structure(permit_token: &str) -> ServiceResult<()> {
    debug!("Validating permit structure");
    Permit::from_token(permit_token)?;
    debug!("Permit structure valid");
    Ok(())
}

// Helper trait to convert DelegationDecision to TokenDecision
impl From<crate::decision::DelegationDecision> for crate::decision::TokenDecision {
    fn from(dd: crate::decision::DelegationDecision) -> Self {
        crate::decision::TokenDecision {
            audience: dd.audience,
            capabilities: dd.capabilities,
            facts: dd.facts,
            expiry: None,
            proofs: dd.proofs,
            proof_tokens: dd.proof_tokens,
        }
    }
}
