//! Permit Service - Stateless permit token operations
//!
//! Provides stateless functions for permit token generation and delegation.
//! All functions take signing key bytes and return signed permits.
//!
//! Usage:
//! ```ignore
//! let secret_key: [u8; 32] = identity.secret_signing_key();
//! let (token, cid) = gurkha::issue_one_time(&secret_key, "owner").await?;
//! ```

use crate::crypto;
use crate::decision;
use crate::errors::{ServiceError, ServiceResult};
use crate::parser::Permit;
use base64::{engine::general_purpose, Engine as _};
use ed25519_dalek::SigningKey;
use tracing::{info, instrument, trace};

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
    trace!("Issuing one-time connection token");

    let signing_key = SigningKey::from_bytes(signing_key_bytes);
    let verifying_key = signing_key.verifying_key();

    let decision = decision::decide_one_time_token(&verifying_key, relationship)?;
    trace!("Token decision created");

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
    trace!("Issuing peer connection token");

    let signing_key = SigningKey::from_bytes(signing_key_bytes);
    let verifying_key = signing_key.verifying_key();

    let decision = decision::decide_peer_connection(&verifying_key, peer_pubkey, relationship)?;

    let (token, _cid) = crypto::sign_permit(signing_key_bytes, &decision).await?;

    info!("Peer connection token generated");
    Ok(token)
}

/// Issue a page viewer authentication token
///
/// Facts-only approach - no domain needed.
///
/// # Arguments
/// * `signing_key_bytes` - 32-byte Ed25519 secret key
/// * `page_id` - Page identifier
#[instrument(skip(signing_key_bytes), fields(page_id = %page_id, token_type = "viewer_auth"))]
pub async fn issue_page_viewer_auth(
    signing_key_bytes: &[u8; 32],
    page_id: &str,
) -> ServiceResult<String> {
    trace!("Issuing page viewer authentication token");

    let signing_key = SigningKey::from_bytes(signing_key_bytes);
    let verifying_key = signing_key.verifying_key();

    let decision = decision::decide_page_viewer_auth(&verifying_key, page_id)?;

    let (token, _cid) = crypto::sign_permit(signing_key_bytes, &decision).await?;

    info!("Page viewer auth token generated");
    Ok(token)
}

/// Issue a space viewer authentication token (for shareable links)
///
/// This is a one-time token with wildcard audience that viewers use to connect.
/// Used in the SpaceTokenRequest flow to generate shareable connection strings.
/// Facts-only approach - no domain needed.
///
/// # Arguments
/// * `signing_key_bytes` - 32-byte Ed25519 secret key
/// * `space_id` - Space identifier
#[instrument(skip(signing_key_bytes), fields(space_id = %space_id, token_type = "space_viewer_auth"))]
pub async fn issue_space_viewer_auth(
    signing_key_bytes: &[u8; 32],
    space_id: &str,
) -> ServiceResult<(String, String)> {
    trace!("Issuing space viewer authentication token");

    let signing_key = SigningKey::from_bytes(signing_key_bytes);
    let verifying_key = signing_key.verifying_key();

    let decision = decision::decide_space_viewer_auth(&verifying_key, space_id)?;
    trace!("Token decision created");

    let (token, cid) = crypto::sign_permit(signing_key_bytes, &decision).await?;

    info!("Space viewer auth token generated: cid={}", cid);
    Ok((token, cid))
}

// ==================== PAGE TOKENS ====================

/// Issue page owner token
///
/// Creates a self-signed permit for the page owner with full permissions.
/// Facts-only approach - all permissions stored in token facts.
///
/// # Arguments
/// * `signing_key_bytes` - 32-byte Ed25519 secret key
/// * `page_id` - Page identifier
/// * `permit_template_json` - JSON template for the permit (PAGE_TEMPLATE)
#[instrument(skip(signing_key_bytes, permit_template_json), fields(page_id = %page_id, token_type = "page_owner"))]
pub async fn issue_page_owner_token(
    signing_key_bytes: &[u8; 32],
    page_id: &str,
    permit_template_json: &str,
) -> ServiceResult<(String, String)> {
    trace!("Issuing page owner token");

    let signing_key = SigningKey::from_bytes(signing_key_bytes);
    let verifying_key = signing_key.verifying_key();

    trace!("Parsing page template");
    let decision =
        decision::decide_page_owner_token(&verifying_key, page_id, permit_template_json)?;
    trace!("Token decision created");

    let (token, cid) = crypto::sign_permit(signing_key_bytes, &decision).await?;

    info!("Page owner token generated: cid={}", cid);
    Ok((token, cid))
}

/// Unified page delegation
///
/// Delegates a page to any audience using a template key.
/// Template key determines the delegation pattern (e.g., "node", "viewer", "page_request").
///
/// # Arguments
/// * `signing_key_bytes` - 32-byte Ed25519 secret key
/// * `delegator_token` - Token of the delegator (must have share_page capability)
/// * `action` - The issue_on action to use (e.g., "node", "viewer", "page_request")
/// * `audience_pubkey` - Public key of the delegatee
///
/// # Returns
/// * `Ok((token_string, cid))` - The delegated token and its CID
///
/// # Self-Describing Permits
/// Permit must have `issue_on.{action}` defining what to issue.
/// No role-based lookup - the permit carries its own delegation template.
pub async fn delegate_page(
    signing_key_bytes: &[u8; 32],
    delegator_token: &str,
    action: &str,
    audience_pubkey: &str,
) -> ServiceResult<(String, String)> {
    let _signing_key = SigningKey::from_bytes(signing_key_bytes);

    // Self-describing: permit must have issue_on.{action}
    let template = decision::extract_issue_template(delegator_token, action)?;

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
    trace!("Issuing space owner token");

    let signing_key = SigningKey::from_bytes(signing_key_bytes);
    let verifying_key = signing_key.verifying_key();

    trace!("Parsing space template");
    let decision =
        decision::decide_space_owner_token(&verifying_key, space_id, permit_template_json)?;
    trace!("Token decision created");

    let (token, cid) = crypto::sign_permit(signing_key_bytes, &decision).await?;

    info!("Space owner token generated: cid={}", cid);
    Ok((token, cid))
}

/// Issue space node-to-owner token
///
/// Used when node issues a permit back to owner after receiving PublishSpace.
/// This permit proves the space is published to this node.
///
/// # Arguments
/// * `signing_key_bytes` - Node's 32-byte Ed25519 secret key
/// * `space_id` - Space identifier
/// * `owner_pubkey` - Owner's public key (audience)
#[instrument(skip(signing_key_bytes), fields(space_id = %space_id, token_type = "space_node_share"))]
pub async fn issue_space_node_to_owner(
    signing_key_bytes: &[u8; 32],
    space_id: &str,
    owner_pubkey: &str,
) -> ServiceResult<(String, String)> {
    trace!("Issuing space node-to-owner token");

    let signing_key = SigningKey::from_bytes(signing_key_bytes);
    let verifying_key = signing_key.verifying_key();

    let decision = decision::decide_space_node_to_owner_token(
        &verifying_key,
        space_id,
        owner_pubkey,
    )?;

    let (token, cid) = crypto::sign_permit(signing_key_bytes, &decision).await?;

    info!("Space node-to-owner token generated: cid={}", cid);
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
/// * `action` - The issue_on action to use (e.g., "node", "viewer", "space_request")
/// * `audience_pubkey` - Public key of the delegatee
///
/// # Returns
/// * `Ok((token_string, cid))` - The delegated token and its CID
///
/// # Self-Describing Permits
/// Permit must have `issue_on.{action}` defining what to issue.
/// No role-based lookup - the permit carries its own delegation template.
pub async fn delegate_space(
    signing_key_bytes: &[u8; 32],
    delegator_token: &str,
    action: &str,
    audience_pubkey: &str,
) -> ServiceResult<(String, String)> {
    let _signing_key = SigningKey::from_bytes(signing_key_bytes);

    // Self-describing: permit must have issue_on.{action}
    let template = decision::extract_issue_template(delegator_token, action)?;

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
    trace!("Extracting space ID from token");
    let permit = Permit::from_token(permit_token)?;

    let space_id = permit.get_fact("space_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            ServiceError::InvalidPermit("Cannot extract space_id".to_string())
        })?;

    trace!("Space ID extracted: {}", space_id);
    Ok(space_id.to_string())
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

// ==================== SYNC CONSENT TOKENS ====================

/// Issue a space sync consent permit
///
/// **Context**: Viewer issues this permit back to the node after receiving SpaceSync.
/// It expresses the viewer's consent to receive sync updates and new pages for this space.
///
/// **Issued by**: Viewer
/// **Audience**: Node (specific node pubkey/DID)
/// **Proof**: Node's original viewer permit (establishes delegation chain)
///
/// # Arguments
/// * `signing_key_bytes` - Viewer's 32-byte Ed25519 secret key
/// * `node_pubkey` - Node's public key (audience of the consent permit)
/// * `space_id` - Space identifier
/// * `node_viewer_permit` - Node's original permit issued to the viewer (used as proof)
/// * `template_json` - JSON template for the consent permit (SYNC_SPACE_CONSENT_TEMPLATE)
///
/// # Returns
/// * `Ok((token, cid))` - The consent permit token and its CID
#[instrument(skip(signing_key_bytes, node_viewer_permit, template_json), fields(space_id = %space_id, token_type = "sync_space_consent"))]
pub async fn issue_sync_space_consent(
    signing_key_bytes: &[u8; 32],
    node_pubkey: &str,
    space_id: &str,
    node_viewer_permit: &str,
    template_json: &str,
) -> ServiceResult<(String, String)> {
    trace!("Issuing space sync consent permit");

    let signing_key = SigningKey::from_bytes(signing_key_bytes);
    let verifying_key = signing_key.verifying_key();

    let token_decision = decision::decide_sync_space_consent(
        &verifying_key,
        node_pubkey,
        space_id,
        node_viewer_permit,
        template_json,
    )?;

    let (token, cid) = crypto::sign_permit(signing_key_bytes, &token_decision).await?;

    info!("Space sync consent permit issued: cid={}", cid);
    Ok((token, cid))
}

/// Issue a page sync consent permit
///
/// **Context**: Viewer issues this permit back to the node after receiving PageSync.
/// It expresses the viewer's consent to receive layer updates for this specific page.
///
/// **Issued by**: Viewer
/// **Audience**: Node (specific node pubkey/DID)
/// **Proof**: Node's original page_viewer permit (establishes delegation chain)
///
/// # Arguments
/// * `signing_key_bytes` - Viewer's 32-byte Ed25519 secret key
/// * `node_pubkey` - Node's public key (audience of the consent permit)
/// * `page_id` - Page identifier
/// * `node_viewer_permit` - Node's original permit issued to the viewer (used as proof)
/// * `template_json` - JSON template for the consent permit (SYNC_PAGE_CONSENT_TEMPLATE)
///
/// # Returns
/// * `Ok((token, cid))` - The consent permit token and its CID
#[instrument(skip(signing_key_bytes, node_viewer_permit, template_json), fields(page_id = %page_id, token_type = "sync_page_consent"))]
pub async fn issue_sync_page_consent(
    signing_key_bytes: &[u8; 32],
    node_pubkey: &str,
    page_id: &str,
    node_viewer_permit: &str,
    template_json: &str,
) -> ServiceResult<(String, String)> {
    trace!("Issuing page sync consent permit");

    let signing_key = SigningKey::from_bytes(signing_key_bytes);
    let verifying_key = signing_key.verifying_key();

    let token_decision = decision::decide_sync_page_consent(
        &verifying_key,
        node_pubkey,
        page_id,
        node_viewer_permit,
        template_json,
    )?;

    let (token, cid) = crypto::sign_permit(signing_key_bytes, &token_decision).await?;

    info!("Page sync consent permit issued: cid={}", cid);
    Ok((token, cid))
}
