//! UCAN service - centralized UCAN validation, issuance, and extraction
//!
//! This module provides:
//! - **Validation**: Business-level validation functions combining crypto_utils with app rules
//! - **Issuance**: Common token generation workflows to eliminate boilerplate
//! - **Extraction**: Utilities for parsing UCAN claims and capabilities

use crate::errors::ServiceResult;
use crypto_utils;
use ed25519_dalek::{SigningKey, VerifyingKey};
use persistance::database::RepositoryContext;
use std::sync::Arc;
use tokio::sync::RwLock;

// ==================== PRIVATE HELPERS ====================

/// Get decrypted UCAN keys from repository
async fn get_decrypted_ucan_keys(
    repo_ctx: &Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<crypto_utils::CryptoUtils>>,
) -> ServiceResult<(String, SigningKey, VerifyingKey)> {
    let encrypted_ucan_key = repo_ctx.store_repo.get_ucan_key().await?;

    let crypto = crypto_utils.read().await;
    let (signing_key, verifying_key) = crypto
        .decrypt_ucan_key(&encrypted_ucan_key)?;

    Ok((encrypted_ucan_key, signing_key, verifying_key))
}

// ==================== VALIDATION FUNCTIONS ====================

/// Validate that a peer has the capability to add folders
///
/// This validates:
/// - The peer connection token structure is valid
/// - The peer has `{domain}:add_folder` capability with `use` ability
///
/// # Arguments
/// * `peer_connection_token` - The peer's connection UCAN token
/// * `domain` - The domain to check (e.g., "sthalam")
///
/// # Returns
/// * `Ok(())` if validation succeeds
/// * `Err` with descriptive error if validation fails
pub async fn validate_peer_can_add_folder(
    peer_connection_token: &str,
    domain: &str,
) -> ServiceResult<()> {
    tracing::info!("🔍 Validating peer can add folder");
    tracing::info!("  - Token length: {} chars", peer_connection_token.len());
    tracing::info!("  - Domain: {}", domain);

    // 1. Validate token structure
    tracing::debug!("  - Step 1: Parsing UCAN token structure...");
    let peer_ucan = crypto_utils::ucan_utils::validate_structure(peer_connection_token)
        .await
        .map_err(|e| {
            tracing::error!("❌ Failed to parse peer connection token: {}", e);
            tracing::error!("   Token preview: {}",
                if peer_connection_token.len() > 50 {
                    &peer_connection_token[..50]
                } else {
                    peer_connection_token
                }
            );
            crate::errors::FolderServiceError::Validation(format!(
                "Invalid peer connection token: {}",
                e
            ))
        })?;

    tracing::info!("  ✓ Token structure valid");

    // 2. Check for add_folder capability
    let add_folder_resource = format!("{}:add_folder", domain);
    tracing::debug!("  - Step 2: Checking for capability: {}", add_folder_resource);

    crypto_utils::ucan_utils::check_capability(&peer_ucan, &add_folder_resource, "use")
        .map_err(|_| {
            tracing::error!("❌ Peer lacks {} capability", add_folder_resource);
            tracing::error!("   Available capabilities: {:?}", peer_ucan.capabilities());
            crate::errors::FolderServiceError::Validation(format!(
                "Peer lacks {} capability",
                add_folder_resource
            ))
        })?;

    tracing::info!("  ✅ Peer has {} capability", add_folder_resource);
    Ok(())
}

/// Validate that a peer can add resources to a folder
///
/// This validates:
/// - The owner's folder UCAN structure is valid
/// - The owner has `add_resources` capability for the specific folder
/// - The folder_id in the UCAN matches the expected folder_id
///
/// # Arguments
/// * `owner_folder_ucan` - The owner's folder UCAN token
/// * `expected_folder_id` - The folder_id that should match the UCAN
/// * `domain` - The domain to check (e.g., "sthalam")
///
/// # Returns
/// * `Ok(())` if all validations pass
/// * `Err` with descriptive error if any validation fails
pub async fn validate_peer_can_add_resources(
    owner_folder_ucan: &str,
    expected_folder_id: &str,
    domain: &str,
) -> ServiceResult<()> {
    // 1. Validate owner's folder UCAN structure
    let folder_ucan = crypto_utils::ucan_utils::validate_structure(owner_folder_ucan)
        .await
        .map_err(|e| {
            crate::errors::ResourceServiceError::UcanError(format!(
                "Invalid owner folder UCAN: {}",
                e
            ))
        })?;

    // 2. Extract folder_id with add_resources capability (business logic)
    let folder_pattern = format!("{}:folder:", domain);
    let mut folder_id_from_ucan = None;

    for capability in folder_ucan.capabilities().iter() {
        let cap_resource = capability.resource;

        if cap_resource.starts_with(&folder_pattern) && capability.ability == "add_resources" {
            if let Some(folder_id) = cap_resource.strip_prefix(&folder_pattern) {
                if !folder_id.is_empty() && folder_id != "*" {
                    folder_id_from_ucan = Some(folder_id.to_string());
                    break;
                }
            }
        }
    }

    let folder_id_from_ucan = folder_id_from_ucan.ok_or_else(|| {
        crate::errors::ResourceServiceError::UcanError(
            "Owner's folder UCAN lacks add_resources capability or no folder found".to_string()
        )
    })?;

    // 3. Verify folder_id matches expected folder_id
    if folder_id_from_ucan != expected_folder_id {
        return Err(crate::errors::ResourceServiceError::UcanError(format!(
            "Folder ID mismatch: UCAN has {}, expected {}",
            folder_id_from_ucan, expected_folder_id
        ))
        .into());
    }

    Ok(())
}

/// Validate that a peer can request a share link for a folder
///
/// This validates:
/// - Peer's folder UCAN is valid
/// - Peer has get_share_link capability for the specific folder
/// - Folder ID in UCAN matches expected folder ID
pub async fn validate_peer_can_request_link(
    peer_folder_ucan: &str,
    expected_folder_id: &str,
    domain: &str,
) -> ServiceResult<()> {
    // 1. Validate peer's folder UCAN structure
    let folder_ucan = crypto_utils::ucan_utils::validate_structure(peer_folder_ucan)
        .await
        .map_err(|e| {
            crate::errors::FolderServiceError::UcanError(format!(
                "Invalid peer folder UCAN: {}",
                e
            ))
        })?;

    // 2. Extract folder_id with get_share_link capability
    let folder_pattern = format!("{}:folder:", domain);
    let mut folder_id_from_ucan = None;

    for capability in folder_ucan.capabilities().iter() {
        let cap_resource = capability.resource;

        if cap_resource.starts_with(&folder_pattern) && capability.ability == "get_share_link" {
            if let Some(folder_id) = cap_resource.strip_prefix(&folder_pattern) {
                if !folder_id.is_empty() && folder_id != "*" {
                    folder_id_from_ucan = Some(folder_id.to_string());
                    break;
                }
            }
        }
    }

    let folder_id_from_ucan = folder_id_from_ucan.ok_or_else(|| {
        crate::errors::FolderServiceError::UcanError(
            "Peer's folder UCAN lacks get_share_link capability or no folder found".to_string()
        )
    })?;

    // 3. Verify folder_id matches expected folder_id
    if folder_id_from_ucan != expected_folder_id {
        return Err(crate::errors::FolderServiceError::UcanError(format!(
            "Folder ID mismatch: UCAN has {}, expected {}",
            folder_id_from_ucan, expected_folder_id
        ))
        .into());
    }

    Ok(())
}

/// Generate a viewer connection token for folder access
///
/// This function:
/// 1. Validates the requester has get_share_link capability for the folder
/// 2. Generates a viewer connection token with appropriate capabilities
/// 3. Returns the token string for the requester to share with viewers
///
/// # Arguments
/// * `requester_folder_ucan` - The requester's folder UCAN (must have get_share_link capability)
/// * `folder_id` - The folder ID to generate viewer access for
/// * `domain` - The domain prefix (e.g., "sthalam")
/// * `repo_ctx` - Repository context for accessing encrypted UCAN keys
/// * `crypto_utils` - Crypto utilities for token generation
///
/// # Returns
/// * `Ok(token_string)` - The generated viewer connection token
/// * `Err` if validation fails or token generation fails
pub async fn generate_viewer_token_for_folder(
    requester_folder_ucan: &str,
    folder_id: &str,
    domain: &str,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: Arc<RwLock<crypto_utils::CryptoUtils>>,
) -> ServiceResult<String> {
    // 1. Validate requester has get_share_link capability
    validate_peer_can_request_link(requester_folder_ucan, folder_id, domain).await?;

    // 2. Get encrypted UCAN key from store
    let encrypted_ucan_key = repo_ctx.store_repo.get_ucan_key().await?;

    // 3. Build capabilities for viewer token
    let capabilities = vec![
        // Universal connection capability
        (format!("{}:user-connect:*", domain), "use".to_string()),
        // Folder-level capabilities
        (
            format!("{}:folder:{}", domain, folder_id),
            "request_resources".to_string(),
        ),
        (
            format!("{}:folder:{}", domain, folder_id),
            "get_share_link".to_string(),
        ),
        // Resource-level wildcard capabilities
        (
            format!("{}:resource:{}/*", domain, folder_id),
            "request_resources".to_string(),
        ),
        (
            format!("{}:resource:{}/*", domain, folder_id),
            "get_share_link".to_string(),
        ),
    ];

    // 4. Build facts for viewer token
    let mut facts = serde_json::Map::new();
    facts.insert("role".to_string(), serde_json::json!("viewer"));
    facts.insert("folder_id".to_string(), serde_json::json!(folder_id));

    // 5. Generate viewer token (30 days expiry)
    let crypto = crypto_utils.read().await;
    let viewer_token = crypto
        .generate_viewer_connection_token(
            &encrypted_ucan_key,
            capabilities,
            Some(facts),
            "*", // Wildcard audience
            Some(30 * 24 * 60 * 60), // 30 days
        )
        .await
        .map_err(|e| {
            crate::errors::FolderServiceError::UcanError(format!(
                "Failed to generate viewer token: {}",
                e
            ))
        })?;

    Ok(viewer_token)
}

/// Validate that a requester has access to a resource via their folder UCAN
///
/// This validates:
/// - The folder UCAN structure is valid
/// - The folder UCAN has proper capabilities (add_resources)
/// - The resource's folder_id matches the folder_id in the UCAN
///
/// Used when a peer requests a resource - validates they should have access
/// based on folder permissions.
///
/// # Arguments
/// * `folder_ucan` - Requester's folder UCAN token
/// * `resource_folder_id` - The folder_id that the resource belongs to
/// * `domain` - The domain to check (e.g., "sthalam")
///
/// # Returns
/// * `Ok(())` if requester has valid folder access
/// * `Err` with descriptive error if validation fails
pub async fn validate_folder_access_for_resource(
    folder_ucan: &str,
    resource_folder_id: &str,
    domain: &str,
) -> ServiceResult<()> {
    // Reuse existing validation - folder UCAN must have add_resources capability
    // and folder_id must match the resource's folder
    validate_peer_can_add_resources(folder_ucan, resource_folder_id, domain).await
}

/// Validate a connect token (moved from crypto_utils::ucan_operations)
///
/// This validates:
/// - One-time tokens: issuer matches verifier, has user-connect capability
/// - Delegated tokens: validates audience and embedded proof chain
///
/// # Arguments
/// * `token` - The connect token to validate
/// * `presenter_ucan_pub` - The presenter's UCAN public key
/// * `verifier_ucan_pub` - The verifier's UCAN public key
/// * `verifier_user_id` - The verifier's user ID
/// * `domain` - The domain to check
///
/// # Returns
/// * `Ok(())` if validation succeeds
/// * `Err` with descriptive error if validation fails
pub async fn validate_connect_token(
    token: &str,
    presenter_ucan_pub: &str,
    verifier_ucan_pub: &str,
    verifier_user_id: &str,
    domain: &str,
) -> ServiceResult<()> {
    let ucan = crypto_utils::ucan_utils::validate_structure(token).await?;
    let required_resource = format!("{}:user-connect:{}", domain, verifier_user_id);

    if crypto_utils::ucan_utils::is_one_time_connect_token(&ucan, domain) {
        crypto_utils::ucan_utils::verify_did_key_match(ucan.issuer(), verifier_ucan_pub)?;
        crypto_utils::ucan_utils::check_capability(&ucan, &required_resource, "use")?;
    } else {
        crypto_utils::ucan_utils::validate_audience(&ucan, presenter_ucan_pub)?;
        crypto_utils::ucan_utils::validate_embedded_proof_chain(
            token,
            verifier_user_id,
            verifier_ucan_pub,
            domain,
            None,
        )
        .await?;
    }

    Ok(())
}

/// Validate authority for update operations (moved from crypto_utils::ucan_operations)
///
/// This validates:
/// - Token structure is valid
/// - Audience matches peer's UCAN public key
/// - Has proper "crud/update" permission via proof chain
///
/// # Arguments
/// * `ucan_token` - The UCAN token to validate
/// * `peer_ucan_pub` - The peer's UCAN public key (audience)
/// * `root_ucan_pub` - The root authority's UCAN public key
/// * `resource_id` - The resource ID to check permissions for
/// * `domain` - The domain to check
/// * `proof_resolver` - Function to resolve proof CIDs to UCAN tokens
///
/// # Returns
/// * `Ok(())` if validation succeeds
/// * `Err` with descriptive error if validation fails
pub async fn validate_authority_for_update<F, Fut>(
    ucan_token: &str,
    peer_ucan_pub: &str,
    root_ucan_pub: &str,
    resource_id: &str,
    domain: &str,
    proof_resolver: &F,
) -> ServiceResult<()>
where
    F: Fn(&str) -> Fut + Send + Sync,
    Fut: std::future::Future<Output = Result<String, crypto_utils::errors::UcanError>> + Send + 'static,
{
    let ucan = crypto_utils::ucan_utils::validate_structure(ucan_token).await?;
    crypto_utils::ucan_utils::validate_audience(&ucan, peer_ucan_pub)?;

    let resource_uri = format!("{}:resource:{}", domain, resource_id);
    crypto_utils::ucan_utils::validate_ucan_permission(
        &ucan,
        root_ucan_pub,
        proof_resolver,
        &resource_uri,
        "crud/update",
    )
    .await?;

    Ok(())
}

// ==================== EXTRACTION FUNCTIONS ====================

/// Extract folder_id that has add_resources capability from UCAN token
///
/// This enforces the business rule that folder access requires add_resources capability.
///
/// # Arguments
/// * `ucan_token` - The UCAN token to extract from
/// * `domain` - The domain (e.g., "sthalam")
///
/// # Returns
/// * `Ok(folder_id)` - The extracted folder_id
/// * `Err` if no folder with add_resources capability found
pub async fn extract_folder_id_with_add_resources(
    ucan_token: &str,
    domain: &str,
) -> ServiceResult<String> {
    let ucan = crypto_utils::ucan_utils::validate_structure(ucan_token).await?;
    crypto_utils::ucan_extractors::extract_id_with_capability(
        &ucan,
        domain,
        "folder",
        "add_resources"
    ).map_err(|e| e.into())
}

/// Extract folder_id from viewer UCAN token (uses request_resources capability)
///
/// Viewer tokens have request_resources instead of add_resources.
/// This function extracts the folder_id from viewer connection tokens.
///
/// # Arguments
/// * `ucan_token` - The viewer UCAN token to extract from
/// * `domain` - The domain (e.g., "sthalam")
///
/// # Returns
/// * `Ok(folder_id)` - The extracted folder_id
/// * `Err` if no folder with request_resources capability found
pub async fn extract_folder_id_from_viewer_token(
    ucan_token: &str,
    domain: &str,
) -> ServiceResult<String> {
    let ucan = crypto_utils::ucan_utils::validate_structure(ucan_token).await?;
    crypto_utils::ucan_extractors::extract_id_with_capability(
        &ucan,
        domain,
        "folder",
        "request_resources"
    ).map_err(|e| e.into())
}

/// Extract resource_id from UCAN token (business logic)
///
/// Parses the UCAN and extracts the first resource ID found in capabilities.
/// Supports multiple formats:
/// - "domain:resource:resource_id" (standard)
/// - "domain:resource:resource_id:doc_type" (3-doc architecture)
/// - "domain:resource:folder_id/resource_id" (folder-scoped)
///
/// # Arguments
/// * `ucan_token` - The UCAN token to extract from
///
/// # Returns
/// * `Ok(resource_id)` - The extracted resource ID
/// * `Err` if no resource capability found
pub async fn extract_resource_id(ucan_token: &str) -> ServiceResult<String> {
    let ucan = crypto_utils::ucan_utils::validate_structure(ucan_token).await?;

    for capability in ucan.capabilities().iter() {
        let cap_resource = capability.resource;

        if cap_resource.contains(":resource:") {
            let parts: Vec<&str> = cap_resource.split(':').collect();

            if parts.len() >= 3 && parts[1] == "resource" {
                let resource_id = parts[2];

                // Handle folder-scoped pattern: "folder_id/resource_id"
                let final_resource_id = if let Some(slash_pos) = resource_id.find('/') {
                    &resource_id[slash_pos + 1..]
                } else {
                    resource_id
                };

                // Make sure it's not wildcard or empty
                if !final_resource_id.is_empty() && final_resource_id != "*" && final_resource_id != "**" {
                    return Ok(final_resource_id.to_string());
                }
            }
        }
    }

    Err(crate::errors::ServiceError::Ucan(
        crypto_utils::errors::UcanError::CapabilityNotFound
    ))
}

/// Get role from UCAN token (convenience wrapper)
///
/// # Arguments
/// * `ucan_token` - The UCAN token to extract role from
///
/// # Returns
/// * `Ok(role)` - The role string from the UCAN facts
pub async fn get_role(ucan_token: &str) -> ServiceResult<String> {
    let ucan = crypto_utils::ucan_utils::validate_structure(ucan_token).await?;
    Ok(crypto_utils::ucan_utils::get_role_from_token(&ucan))
}

/// Get CID from UCAN token (convenience wrapper)
///
/// # Arguments
/// * `ucan_token` - The UCAN token to get CID from
///
/// # Returns
/// * `Ok(cid)` - The CID string
pub fn get_cid(ucan_token: &str) -> ServiceResult<String> {
    Ok(crypto_utils::ucan_utils::get_ucan_cid(ucan_token)?)
}

// ==================== ISSUANCE FUNCTIONS ====================

/// Issue a one-time connection token
///
/// Used for initial user connection setup (QR codes, connection strings, etc.)
///
/// # Arguments
/// * `capability_str` - The capability/domain string (e.g., "sthalam")
/// * `role` - The role for this connection ("owner", "node", "viewer")
/// * `crypto_utils` - Crypto utilities instance
/// * `repo_ctx` - Repository context for accessing encrypted keys
///
/// # Returns
/// * `Ok((token, public_key))` - The generated UCAN token and its public key
pub async fn issue_one_time_connection_token(
    capability_str: &str,
    role: &str,
    crypto_utils: &Arc<RwLock<crypto_utils::CryptoUtils>>,
    repo_ctx: &Arc<RepositoryContext>,
) -> ServiceResult<(String, String)> {
    let encrypted_ucan_key = repo_ctx.store_repo.get_ucan_key().await?;

    let crypto = crypto_utils.read().await;
    let (token, pub_key) = crypto
        .generate_one_time_user_connect_token(&encrypted_ucan_key, capability_str, role)
        .await?;

    Ok((token, pub_key))
}

/// Issue a peer connection token with role-based capabilities
///
/// Used when establishing connections between peers (owner ↔ node, owner ↔ viewer, etc.)
///
/// # Arguments
/// * `domain` - The application domain (e.g., "sthalam")
/// * `peer_ucan_pub_key` - The peer's UCAN public key (audience)
/// * `role` - The peer's role ("node", "viewer", etc.)
/// * `crypto_utils` - Crypto utilities instance
/// * `repo_ctx` - Repository context for accessing encrypted keys
///
/// # Returns
/// * `Ok(token)` - The generated connection UCAN token
pub async fn issue_peer_connection_token(
    domain: &str,
    peer_ucan_pub_key: &str,
    role: &str,
    crypto_utils: &Arc<RwLock<crypto_utils::CryptoUtils>>,
    repo_ctx: &Arc<RepositoryContext>,
) -> ServiceResult<String> {
    let encrypted_key = repo_ctx.store_repo.get_ucan_key().await?;

    // Determine role-based additional capabilities
    let additional_capabilities = match role {
        "node" => vec![(format!("{}:add_folder", domain), "use".to_string())],
        "viewer" => vec![],
        _ => vec![],
    };

    let crypto = crypto_utils.read().await;
    let token = crypto
        .issue_connect_and_share_user_token(
            &encrypted_key,
            domain,
            peer_ucan_pub_key,
            role,
            additional_capabilities,
        )
        .await?;

    Ok(token)
}

/// Issue a folder owner UCAN token with template
///
/// Used when creating a new folder - generates the root owner UCAN
///
/// # Arguments
/// * `folder_id` - The folder ID
/// * `domain` - The application domain
/// * `folder_template_json` - JSON template defining folder capabilities
/// * `crypto_utils` - Crypto utilities instance
/// * `repo_ctx` - Repository context for accessing encrypted keys
///
/// # Returns
/// * `Ok((token, cid))` - The generated UCAN token and its CID
pub async fn issue_folder_owner_token(
    folder_id: &str,
    domain: &str,
    folder_template_json: &str,
    crypto_utils: &Arc<RwLock<crypto_utils::CryptoUtils>>,
    repo_ctx: &Arc<RepositoryContext>,
) -> ServiceResult<(String, String)> {
    let encrypted_key = repo_ctx.store_repo.get_ucan_key().await?;

    let crypto = crypto_utils.read().await;
    let (token, cid) = crypto
        .generate_folder_ucan_with_template(&encrypted_key, folder_id, domain, folder_template_json)
        .await?;

    Ok((token, cid))
}

/// Issue a delegated folder UCAN token
///
/// Used when sharing a folder with another user - creates delegated token with subset of capabilities
///
/// # Arguments
/// * `folder_id` - The folder ID
/// * `domain` - The application domain
/// * `folder_capabilities` - List of capability resources (just the keys)
/// * `recipient_ucan_pub_key` - The recipient's UCAN public key
/// * `recipient_role` - The recipient's role
/// * `crypto_utils` - Crypto utilities instance
/// * `repo_ctx` - Repository context for accessing encrypted keys
///
/// # Returns
/// * `Ok(token)` - The generated delegated UCAN token
pub async fn issue_delegated_folder_token(
    folder_id: &str,
    domain: &str,
    folder_capabilities: Vec<String>,
    recipient_ucan_pub_key: &str,
    recipient_role: &str,
    crypto_utils: &Arc<RwLock<crypto_utils::CryptoUtils>>,
    repo_ctx: &Arc<RepositoryContext>,
) -> ServiceResult<String> {
    let (_, signing_key, verifying_key) = get_decrypted_ucan_keys(repo_ctx, crypto_utils).await?;

    // Convert Vec<String> to Vec<&str> for the crypto_utils function
    let cap_refs: Vec<&str> = folder_capabilities.iter().map(|s| s.as_str()).collect();

    let token = crypto_utils::ucan_utils::generate_flexible_folder_token(
        &signing_key,
        &verifying_key,
        folder_id,
        domain,
        None, // 30 year expiry
        cap_refs,
        recipient_ucan_pub_key,
        recipient_role,
    )
    .await?;

    Ok(token)
}

/// Issue a resource owner UCAN token with template
///
/// Used when creating a new resource - generates the root owner UCAN
///
/// # Arguments
/// * `resource_id` - The resource ID
/// * `domain` - The application domain
/// * `ucan_template_json` - JSON template defining resource capabilities
/// * `crypto_utils` - Crypto utilities instance
/// * `repo_ctx` - Repository context for accessing encrypted keys
///
/// # Returns
/// * `Ok((token, cid))` - The generated UCAN token and its CID
pub async fn issue_resource_owner_token(
    resource_id: &str,
    domain: &str,
    ucan_template_json: &str,
    crypto_utils: &Arc<RwLock<crypto_utils::CryptoUtils>>,
    repo_ctx: &Arc<RepositoryContext>,
) -> ServiceResult<(String, String)> {
    let encrypted_key = repo_ctx.store_repo.get_ucan_key().await?;

    let crypto = crypto_utils.read().await;
    let (token, cid) = crypto
        .generate_flexible_resource_owner_ucan(
            &encrypted_key,
            resource_id,
            domain,
            ucan_template_json,
            None, // Default 30-year expiry
        )
        .await?;

    Ok((token, cid))
}

/// Get UCAN public key
///
/// Helper to retrieve the public key corresponding to the encrypted UCAN private key
///
/// # Arguments
/// * `crypto_utils` - Crypto utilities instance
/// * `repo_ctx` - Repository context for accessing encrypted keys
///
/// # Returns
/// * `Ok(public_key)` - The UCAN public key as base64 string
pub async fn get_ucan_public_key(
    crypto_utils: &Arc<RwLock<crypto_utils::CryptoUtils>>,
    repo_ctx: &Arc<RepositoryContext>,
) -> ServiceResult<String> {
    let encrypted_key = repo_ctx.store_repo.get_ucan_key().await?;
    let crypto = crypto_utils.read().await;
    Ok(crypto.get_public_ucan_key(&encrypted_key).await?)
}

// ==================== EXTRACTION FUNCTIONS ====================
// Note: Extraction functions that work with parsed UCAN objects are domain-specific
// and remain in crypto_utils::ucan_utils. Services can call them directly as needed.

/// Extract folder capabilities from folder UCAN template based on role
///
/// Parses the folder UCAN, extracts facts, finds the appropriate template
/// (owner_template or node_template), and returns the capabilities list.
///
/// # Arguments
/// * `folder_ucan_token` - The folder owner's UCAN token
/// * `recipient_role` - Role for recipient ("owner" or "node")
///
/// # Returns
/// * `Vec<String>` - List of capability keys (e.g., ["add_resources", "view_folder"])
pub async fn extract_folder_capabilities(
    folder_ucan_token: &str,
    recipient_role: &str,
) -> ServiceResult<Vec<String>> {
    // Determine template key based on role
    let template_key = match recipient_role {
        "owner" => "owner_template",
        "node" => "node_template",
        _ => return Err(crate::errors::FolderServiceError::InvalidRole {
            role: recipient_role.to_string(),
        }.into()),
    };

    // Parse and validate UCAN structure
    let folder_ucan_parsed = crypto_utils::ucan_utils::validate_structure(folder_ucan_token)
        .await
        .map_err(|e| crate::errors::FolderServiceError::UcanError(
            format!("Failed to parse folder UCAN: {}", e)
        ))?;

    // Extract facts
    let facts = folder_ucan_parsed.facts().clone()
        .ok_or_else(|| crate::errors::FolderServiceError::UcanError(
            "Missing facts in folder UCAN".into()
        ))?;

    // Get template by key
    let template = facts.get(template_key)
        .ok_or_else(|| crate::errors::FolderServiceError::UcanError(
            format!("Missing {} in folder UCAN", template_key)
        ))?;

    // Extract capabilities map
    let capabilities_map = template.get("capabilities")
        .and_then(|c| c.as_object())
        .ok_or_else(|| crate::errors::FolderServiceError::UcanError(
            "Missing capabilities in template".into()
        ))?;

    // Convert keys to Vec<String>
    let folder_capabilities: Vec<String> = capabilities_map
        .keys()
        .map(|k| k.clone())
        .collect();

    Ok(folder_capabilities)
}

/// Validate UCAN token structure and signature
///
/// Checks that the UCAN is well-formed and has a valid signature.
/// Discards the parsed result - only used for validation.
///
/// # Arguments
/// * `ucan_token` - The UCAN token to validate
///
/// # Returns
/// * `Ok(())` - UCAN is valid
/// * `Err` - UCAN is invalid or malformed
pub async fn validate_ucan_structure(ucan_token: &str) -> ServiceResult<()> {
    crypto_utils::ucan_utils::validate_structure(ucan_token)
        .await
        .map_err(|e| crate::errors::FolderServiceError::UcanError(
            format!("Invalid UCAN structure: {}", e)
        ))?;
    Ok(())
}

/// Extract document capabilities from UCAN token
///
/// Wrapper for crypto_utils function - returns doc_name -> ability map.
/// Used by merge_service for CRDT sync permission checking.
///
/// # Arguments
/// * `ucan_token` - The UCAN token to parse
///
/// # Returns
/// HashMap of doc_name to ability (e.g., "main_doc" -> "crud/merge")
pub async fn extract_doc_capabilities(
    ucan_token: &str,
) -> ServiceResult<std::collections::HashMap<String, String>> {
    let ucan = crypto_utils::ucan_utils::validate_structure(ucan_token).await?;

    let mut doc_capabilities = std::collections::HashMap::new();

    for capability in ucan.capabilities().iter() {
        let cap_resource = capability.resource;

        // Look for resource pattern with doc name: "domain:resource:resource_id:doc_name"
        if cap_resource.contains(":resource:") {
            let parts: Vec<&str> = cap_resource.split(':').collect();

            // Must have exactly 4 parts: [domain, "resource", resource_id, doc_name]
            if parts.len() == 4 && parts[1] == "resource" {
                let doc_name = parts[3];
                let ability = capability.ability;

                doc_capabilities.insert(doc_name.to_string(), ability.to_string());
            }
        }
    }

    if doc_capabilities.is_empty() {
        return Err(crate::errors::FolderServiceError::UcanError(
            "No document capabilities found".to_string()
        ).into());
    }

    Ok(doc_capabilities)
}

/// Extract facts from UCAN token
///
/// Returns facts JSON map directly from parsed UCAN.
/// Used by merge_service to check dont_send_to_node and no_update_from_node rules.
///
/// # Arguments
/// * `ucan_token` - The UCAN token to parse
///
/// # Returns
/// Optional JSON map with facts section
pub async fn extract_facts(
    ucan_token: &str,
) -> ServiceResult<Option<serde_json::Map<String, serde_json::Value>>> {
    let ucan = crypto_utils::ucan_utils::validate_structure(ucan_token).await?;

    Ok(ucan.facts().as_ref().map(|facts| {
        // Convert BTreeMap to serde_json::Map
        facts.iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }))
}
