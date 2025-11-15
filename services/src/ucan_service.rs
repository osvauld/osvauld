//! UCAN Token Service
//!
//! Handles ALL token generation and delegation for the UCAN authorization system.
//!
//! ## Core Principle: NEVER HARDCODE TEMPLATES
//!
//! This service follows a strict data-driven approach:
//! 1. Templates are ALWAYS extracted from delegator UCANs (never hardcoded)
//! 2. All delegation uses `DelegationTemplate::build_capabilities()` and `to_facts()`
//! 3. Typed tokens enforce correctness at compile time
//!
//! ## Architecture
//!
//! - **Private utilities**: Key management (get_decrypted_ucan_keys)
//! - **Public utilities**: Key access (get_public_ucan_key)
//! - **connection_tokens**: Device-to-device connection token issuance
//! - **resource_tokens**: Resource delegation (Owner → Node → User → Viewer)
//! - **folder_tokens**: Folder delegation (Owner → Node → User → Viewer)
//! - **validation**: Business-level validation combining crypto with app rules

use crate::errors::{ServiceError, ServiceResult};
use crypto_utils::CryptoUtils;
use osvauld_core::ucan::{
    parser::{ResourceUcan, DelegationTemplate},
    token::{
        ConnectionToken, FolderOwnerToken, FolderShareToken, FolderViewerToken,
        NodeConnectionToken, OneTimeConnectionToken, OwnerConnectionToken,
        ResourceOwnerToken, ResourceShareToken, ResourceViewerToken, UserConnectionToken,
        ViewerAuthToken,
    },
    uri,
};
use osvauld_core::ucan::prelude::*;
use persistance::database::RepositoryContext;
use serde_json::json;
use std::sync::Arc;
use tokio::sync::RwLock;

// ==================== PRIVATE UTILITIES ====================

/// Get encrypted UCAN key from repository
///
/// Internal utility for accessing the encrypted UCAN key.
/// Returns the encrypted key string which is passed to crypto_utils methods.
async fn get_decrypted_ucan_keys(repo_ctx: &Arc<RepositoryContext>) -> ServiceResult<String> {
    let encrypted_ucan_key = repo_ctx.store_repo.get_ucan_key().await?;
    Ok(encrypted_ucan_key)
}

// ==================== PUBLIC UTILITIES ====================

/// Get UCAN public key
///
/// Helper to retrieve the public key corresponding to the encrypted UCAN private key.
/// Used by callers who need the owner's public key for audience/issuer validation.
///
/// # Arguments
/// * `crypto_utils` - Crypto utilities instance
/// * `repo_ctx` - Repository context for accessing encrypted keys
///
/// # Returns
/// * `Ok(public_key)` - The UCAN public key as base64 string
pub async fn get_public_ucan_key(
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
    repo_ctx: &Arc<RepositoryContext>,
) -> ServiceResult<String> {
    let encrypted_key = get_decrypted_ucan_keys(repo_ctx).await?;
    let crypto = crypto_utils.read().await;
    Ok(crypto.get_public_ucan_key(&encrypted_key).await?)
}

// ==================== RE-EXPORTS ====================

// Re-export commonly used functions for easier access
pub use connection_tokens::{issue_one_time, issue_peer_connection, issue_viewer_auth};
pub use folder_tokens::{issue_folder_owner_token, issue_delegated_folder_token, delegate_to_node as delegate_folder_to_node, delegate_to_viewer as delegate_folder_to_viewer};
pub use resource_tokens::{issue_owner_token, delegate_to_node as delegate_resource_to_node, delegate_to_viewer as delegate_resource_to_viewer, delegate_by_role as delegate_resource_by_role};
pub use utilities::{extract_resource_id, extract_folder_id_from_viewer_token, extract_folder_id_with_add_resources, validate_folder_ucan_and_get_id, extract_doc_capabilities, extract_facts, extract_folder_capabilities, get_cid};
pub use validation::{validate_folder_access_for_resource, validate_ucan_structure, validate_peer_can_add_folder};

// ==================== CONNECTION TOKENS MODULE ====================

/// Connection token issuance functions
///
/// These tokens establish device-to-device connections and authentication.
/// Used for initial handshakes and peer relationship management.
///
/// Note: These functions use legacy crypto_utils methods for backwards compatibility.
/// They will be migrated to use generate_ucan_with_cid() in the future.
pub mod connection_tokens {
    use super::*;

    /// Issue a one-time connection token
    ///
    /// Used for initial device pairing (QR codes, connection strings).
    /// Single-use token that proves authorization to connect.
    ///
    /// # Arguments
    /// * `capability_str` - Domain (e.g., "sthalam")
    /// * `role` - Role for this connection ("owner", "node", "user")
    /// * `crypto_utils` - Crypto utilities instance
    /// * `repo_ctx` - Repository context for accessing encrypted keys
    ///
    /// # Returns
    /// * `Ok((token, public_key))` - Generated UCAN token and its public key
    pub async fn issue_one_time(
        capability_str: &str,
        role: &str,
        crypto_utils: &Arc<RwLock<CryptoUtils>>,
        repo_ctx: &Arc<RepositoryContext>,
    ) -> ServiceResult<(String, String)> {
        let encrypted_ucan_key = get_decrypted_ucan_keys(repo_ctx).await?;

        // Get public key (base64-encoded Ed25519 public key)
        let crypto = crypto_utils.read().await;
        let public_key = crypto.get_public_ucan_key(&encrypted_ucan_key).await?;

        // Use the public key itself as the user identifier
        let user_id = &public_key;

        // Build capability
        let capability = format!("{}:user-connect:{}", capability_str, user_id);
        let capabilities = vec![(capability, "use".to_string())];

        // Build facts
        let mut facts = serde_json::Map::new();
        facts.insert("token_type".to_string(), json!("one_time_connection"));
        facts.insert("role".to_string(), json!(role));
        facts.insert("first_connection".to_string(), json!(true));

        // Generate token with wildcard audience (single use)
        let (token, _cid) = crypto
            .generate_ucan_with_cid(
                &encrypted_ucan_key,
                "*", // Wildcard audience
                capabilities,
                Some(facts),
                Some(30 * 24 * 60 * 60), // 30 days expiry
            )
            .await?;

        Ok((token, public_key))
    }

    /// Issue a peer connection token with role-based capabilities
    ///
    /// Used when establishing long-lived connections between peers.
    /// Grants appropriate capabilities based on role (owner/node/user/viewer).
    ///
    /// # Arguments
    /// * `domain` - Application domain (e.g., "sthalam")
    /// * `peer_ucan_pub_key` - Peer's UCAN public key (audience)
    /// * `role` - Peer's role ("owner", "node", "user", "viewer")
    /// * `crypto_utils` - Crypto utilities instance
    /// * `repo_ctx` - Repository context for accessing encrypted keys
    ///
    /// # Returns
    /// * `Ok(token)` - Generated connection UCAN token
    pub async fn issue_peer_connection(
        domain: &str,
        peer_ucan_pub_key: &str,
        role: &str,
        crypto_utils: &Arc<RwLock<CryptoUtils>>,
        repo_ctx: &Arc<RepositoryContext>,
    ) -> ServiceResult<String> {
        let encrypted_key = get_decrypted_ucan_keys(repo_ctx).await?;

        // Get public key (base64-encoded Ed25519 public key)
        let crypto = crypto_utils.read().await;
        let public_key = crypto.get_public_ucan_key(&encrypted_key).await?;

        // Use the public key itself as the user identifier
        let user_id = &public_key;

        // Role-based capabilities
        let (token_type, capabilities) = match role {
            "owner" => (
                "owner_connection",
                vec![
                    (format!("{}:user:*", domain), "connect".to_string()),
                    (format!("{}:user:*", domain), "share".to_string()),
                    (format!("{}:folder:*", domain), "add_folder".to_string()),
                ],
            ),
            "node" => (
                "node_connection",
                vec![
                    (format!("{}:user:*", domain), "connect".to_string()),
                    (format!("{}:user:*", domain), "share".to_string()),
                    (format!("{}:folder:*", domain), "add_folder".to_string()),
                ],
            ),
            "user" => (
                "user_connection",
                vec![
                    (format!("{}:user:*", domain), "connect".to_string()),
                    (format!("{}:user:*", domain), "share".to_string()),
                ],
            ),
            "viewer" => (
                "viewer_connection",
                vec![
                    (format!("{}:user:*", domain), "connect".to_string()),
                    (format!("{}:user:*", domain), "share".to_string()),
                    (format!("{}:user:self", domain), "delete".to_string()),
                ],
            ),
            _ => {
                return Err(crate::errors::ServiceError::InvalidUcan(format!(
                    "Unknown role: {}",
                    role
                )));
            }
        };

        // Build facts
        let mut facts = serde_json::Map::new();
        facts.insert("token_type".to_string(), json!(token_type));
        facts.insert("role".to_string(), json!(role));
        facts.insert("first_connection".to_string(), json!(false));

        // Generate token with peer as audience (long-lived)
        let (token, _cid) = crypto
            .generate_ucan_with_cid(
                &encrypted_key,
                peer_ucan_pub_key,
                capabilities,
                Some(facts),
                Some(30 * 365 * 24 * 60 * 60), // 30 years expiry
            )
            .await?;

        Ok(token)
    }

    /// Issue viewer authentication token for folder access
    ///
    /// Generates a connection token with viewer capabilities for a specific folder.
    /// Used for shareable links - allows viewers to authenticate and request resources.
    ///
    /// # Arguments
    /// * `folder_id` - Folder ID to grant access to
    /// * `domain` - Domain prefix (e.g., "sthalam")
    /// * `crypto_utils` - Crypto utilities instance
    /// * `repo_ctx` - Repository context for accessing encrypted keys
    ///
    /// # Returns
    /// * `Ok(token)` - Viewer authentication token
    pub async fn issue_viewer_auth(
        folder_id: &str,
        domain: &str,
        crypto_utils: &Arc<RwLock<CryptoUtils>>,
        repo_ctx: &Arc<RepositoryContext>,
    ) -> ServiceResult<(String, String)> {
        let encrypted_ucan_key = get_decrypted_ucan_keys(repo_ctx).await?;

        // Build capabilities for viewer authentication
        let capabilities = vec![
            (format!("{}:user-connect:*", domain), "use".to_string()),
            (format!("{}:folder:{}", domain, folder_id), "request_resources".to_string()),
            (format!("{}:folder:{}", domain, folder_id), "get_share_link".to_string()),
            (format!("{}:resource:{}/*", domain, folder_id), "request_resources".to_string()),
            (format!("{}:resource:{}/*", domain, folder_id), "get_share_link".to_string()),
        ];

        // Build facts
        let mut facts = serde_json::Map::new();
        facts.insert("token_type".to_string(), json!("viewer_auth"));
        facts.insert("role".to_string(), json!("viewer"));
        facts.insert("folder_id".to_string(), json!(folder_id));
        facts.insert("first_connection".to_string(), json!(true));

        // Generate token (30 days expiry)
        let crypto = crypto_utils.read().await;
        let (viewer_token, cid) = crypto
            .generate_ucan_with_cid(
                &encrypted_ucan_key,
                "*", // Wildcard audience
                capabilities,
                Some(facts),
                Some(30 * 24 * 60 * 60), // 30 days
            )
            .await?;

        Ok((viewer_token, cid))
    }

    /// Issue one-time connection token (wrapper for backwards compatibility)
    ///
    /// Alias for issue_one_time() with more explicit naming.
    ///
    /// # Arguments
    /// * `capability_str` - Domain (e.g., "sthalam")
    /// * `role` - Role for this connection ("owner", "node", "viewer")
    /// * `crypto_utils` - Crypto utilities instance
    /// * `repo_ctx` - Repository context for accessing encrypted keys
    ///
    /// # Returns
    /// * `Ok((token, public_key))` - Generated UCAN token and its public key
    pub async fn issue_one_time_connection_token(
        capability_str: &str,
        role: &str,
        crypto_utils: &Arc<RwLock<CryptoUtils>>,
        repo_ctx: &Arc<RepositoryContext>,
    ) -> ServiceResult<(String, String)> {
        issue_one_time(capability_str, role, crypto_utils, repo_ctx).await
    }

    /// Generate public folder view token (alias for issue_viewer_auth)
    ///
    /// Wrapper for generating viewer authentication tokens via shareable links.
    ///
    /// # Arguments
    /// * `folder_id` - Folder ID to grant access to
    /// * `domain` - Domain prefix (e.g., "sthalam")
    /// * `crypto_utils` - Crypto utilities instance
    /// * `repo_ctx` - Repository context for accessing encrypted keys
    ///
    /// # Returns
    /// * `Ok((token, cid))` - Viewer authentication token and CID
    pub async fn generate_public_folder_view_token(
        folder_id: &str,
        domain: &str,
        crypto_utils: &Arc<RwLock<CryptoUtils>>,
        repo_ctx: &Arc<RepositoryContext>,
    ) -> ServiceResult<(String, String)> {
        issue_viewer_auth(folder_id, domain, crypto_utils, repo_ctx).await
    }

    /// Issue peer connection token (alias for issue_peer_connection)
    pub async fn issue_peer_connection_token(
        capability_str: &str,
        role: &str,
        peer_pub_key: &str,
        crypto_utils: &Arc<RwLock<CryptoUtils>>,
        repo_ctx: &Arc<RepositoryContext>,
    ) -> ServiceResult<String> {
        issue_peer_connection(capability_str, role, peer_pub_key, crypto_utils, repo_ctx).await
    }
}

// ==================== RESOURCE TOKENS MODULE ====================

/// Resource token delegation functions
///
/// These tokens control access to specific resources (documents).
/// Delegation chain: Owner → Node → User → Viewer
///
/// **CRITICAL**: All delegation MUST extract templates from delegator UCANs.
/// NEVER hardcode capabilities - always use `DelegationTemplate::build_capabilities()`.
pub mod resource_tokens {
    use super::*;

    /// Issue owner token for a new resource
    ///
    /// Creates the initial owner token with all delegation templates.
    /// Frontend sends the complete UCAN structure via ucan_template_json.
    ///
    /// # Arguments
    /// * `resource_id` - Resource ID
    /// * `domain` - Domain (e.g., "sthalam.com")
    /// * `ucan_template_json` - JSON containing owner_template with capabilities and delegation templates
    /// * `crypto_utils` - Crypto utilities instance
    /// * `repo_ctx` - Repository context
    ///
    /// # Returns
    /// * `Ok((token, cid))` - Generated UCAN token and its CID
    pub async fn issue_owner_token(
        resource_id: &str,
        domain: &str,
        ucan_template_json: &str,
        crypto_utils: &Arc<RwLock<CryptoUtils>>,
        repo_ctx: &Arc<RepositoryContext>,
    ) -> ServiceResult<(String, String)> {
        // Parse the template JSON from frontend
        let template_data: serde_json::Value = serde_json::from_str(ucan_template_json)
            .map_err(|e| crate::errors::ServiceError::InvalidUcan(format!("Invalid template JSON: {}", e)))?;

        let owner_template = template_data.get("owner_template")
            .ok_or_else(|| crate::errors::ServiceError::InvalidUcan("Missing owner_template".to_string()))?;

        // Extract capabilities from template
        let caps = owner_template.get("capabilities")
            .and_then(|v| v.as_object())
            .ok_or_else(|| crate::errors::ServiceError::InvalidUcan("Missing capabilities in owner_template".to_string()))?;

        // Build capability URIs using standard uri module
        // URI format: domain:resource:id:doc_name (capability value is in the URI)
        let mut capabilities = Vec::new();
        for (doc_name, cap_value) in caps.iter() {
            let cap_str = cap_value.as_str()
                .ok_or_else(|| crate::errors::ServiceError::InvalidUcan(format!("Invalid capability value for {}", doc_name)))?;
            let capability_uri = uri::resource_capability(domain, resource_id, doc_name);
            capabilities.push((
                format!("{}:{}", capability_uri, cap_str), // URI + capability level
                String::new(), // No caveat
            ));
        }

        // Build facts from template
        // NOTE: resource_id is NOT stored in facts - it's encoded in capability URIs
        let mut facts = serde_json::Map::new();
        facts.insert("token_type".to_string(), json!("resource_owner"));
        facts.insert("role".to_string(), json!("owner"));

        // Copy sync facts if present
        if let Some(sync) = owner_template.get("sync") {
            facts.insert("sync".to_string(), sync.clone());
        }

        // Copy doc_metadata if present
        if let Some(doc_metadata) = owner_template.get("doc_metadata") {
            facts.insert("doc_metadata".to_string(), doc_metadata.clone());
        }

        // Copy delegation templates
        if let Some(delegation) = owner_template.get("delegation") {
            facts.insert("delegation".to_string(), delegation.clone());
        }

        // Get keys
        let encrypted_key = get_decrypted_ucan_keys(repo_ctx).await?;
        let crypto = crypto_utils.read().await;
        let our_pub_key = crypto.get_public_ucan_key(&encrypted_key).await?;

        // Generate self-signed token
        let (token, cid) = crypto
            .generate_ucan_with_cid(
                &encrypted_key,
                &our_pub_key, // Self-signed
                capabilities,
                Some(facts),
                None, // Default 30 years
            )
            .await?;

        Ok((token, cid))
    }

    /// Delegate resource to node (Owner → Node)
    ///
    /// Extracts node delegation template from owner token and generates node share token.
    /// The template defines which capabilities the node receives (data-driven delegation).
    ///
    /// # Arguments
    /// * `owner_token` - Owner's resource token (contains delegation templates)
    /// * `node_pub_key` - Node's UCAN public key (becomes audience)
    /// * `resource_id` - Resource ID
    /// * `repo_ctx` - Repository context
    /// * `crypto_utils` - Crypto utilities instance
    ///
    /// # Returns
    /// * `Ok(ResourceShareToken)` - Node's delegated resource token
    pub async fn delegate_to_node(
        owner_token: &ResourceOwnerToken,
        node_pub_key: &str,
        resource_id: &str,
        repo_ctx: Arc<RepositoryContext>,
        crypto_utils: Arc<RwLock<CryptoUtils>>,
    ) -> ServiceResult<ResourceShareToken> {
        // 1. Extract template (NEVER HARDCODE!)
        let template = owner_token.get_delegation_template("node").map_err(|e| {
            crate::errors::ServiceError::Ucan(crypto_utils::errors::UcanError::FormatError(
                format!("Failed to get node template: {}", e),
            ))
        })?;

        // 2. Build capabilities from template using standard URI format (DATA-DRIVEN)
        // Extract domain from token's capability URIs (source of truth)
        let domain = owner_token
            .domain()
            .ok_or_else(|| {
                crate::errors::ServiceError::Ucan(crypto_utils::errors::UcanError::FormatError(
                    "Cannot extract domain from owner token".to_string(),
                ))
            })?;
        let capabilities = template.build_capabilities(&domain, resource_id, "resource");

        // 3. Convert template to facts (GENERIC)
        // NOTE: resource_id is NOT stored in facts - it's encoded in capability URIs
        let mut facts = template.to_facts();
        facts.insert("token_type".to_string(), json!("resource_share"));
        facts.insert("role".to_string(), json!("node"));

        // 4. Get encrypted key
        let encrypted_key = get_decrypted_ucan_keys(&repo_ctx).await?;

        // 5. Generate token via crypto layer
        let crypto = crypto_utils.read().await;
        let (token, _cid) = crypto
            .generate_ucan_with_cid(
                &encrypted_key,
                node_pub_key,
                capabilities,
                Some(facts),
                None, // Default 30 years
            )
            .await?;

        // 6. Return typed wrapper
        Ok(ResourceShareToken::from_token(&token).map_err(|e| {
            crate::errors::ServiceError::Ucan(crypto_utils::errors::UcanError::ValidationError(
                format!("Generated invalid ResourceShareToken: {}", e),
            ))
        })?)
    }

    /// Delegate resource to user (Node → User or Owner → User)
    ///
    /// Extracts user delegation template and generates user share token.
    ///
    /// # Arguments
    /// * `delegator_token` - Node or Owner's resource token (contains delegation templates)
    /// * `user_pub_key` - User's UCAN public key (becomes audience)
    /// * `resource_id` - Resource ID
    /// * `repo_ctx` - Repository context
    /// * `crypto_utils` - Crypto utilities instance
    ///
    /// # Returns
    /// * `Ok(ResourceShareToken)` - User's delegated resource token
    pub async fn delegate_to_user(
        delegator_token: &ResourceShareToken,
        user_pub_key: &str,
        resource_id: &str,
        repo_ctx: Arc<RepositoryContext>,
        crypto_utils: Arc<RwLock<CryptoUtils>>,
    ) -> ServiceResult<ResourceShareToken> {
        // 1. Extract template (NEVER HARDCODE!)
        let template = delegator_token.get_delegation_template("user").map_err(|e| {
            crate::errors::ServiceError::Ucan(crypto_utils::errors::UcanError::FormatError(
                format!("Failed to get user template: {}", e),
            ))
        })?;

        // 2. Build capabilities from template using standard URI format (DATA-DRIVEN)
        // Extract domain from delegator token's capability URIs (source of truth)
        let domain = delegator_token
            .domain()
            .ok_or_else(|| {
                crate::errors::ServiceError::Ucan(
                    crypto_utils::errors::UcanError::FormatError(
                        "Cannot extract domain from delegator token".to_string(),
                    ),
                )
            })?;
        let capabilities = template.build_capabilities(&domain, resource_id, "resource");

        // 3. Convert template to facts (GENERIC)
        // NOTE: resource_id is NOT stored in facts - it's encoded in capability URIs
        let mut facts = template.to_facts();
        facts.insert("token_type".to_string(), json!("resource_share"));
        facts.insert("role".to_string(), json!("user"));

        // 4. Get encrypted key
        let encrypted_key = get_decrypted_ucan_keys(&repo_ctx).await?;

        // 5. Generate token via crypto layer
        let crypto = crypto_utils.read().await;
        let (token, _cid) = crypto
            .generate_ucan_with_cid(
                &encrypted_key,
                user_pub_key,
                capabilities,
                Some(facts),
                None, // Default 30 years
            )
            .await?;

        // 6. Return typed wrapper
        Ok(ResourceShareToken::from_token(&token).map_err(|e| {
            crate::errors::ServiceError::Ucan(crypto_utils::errors::UcanError::ValidationError(
                format!("Generated invalid ResourceShareToken: {}", e),
            ))
        })?)
    }

    /// Delegate resource to viewer (Node → Viewer)
    ///
    /// Extracts viewer delegation template and generates viewer token.
    ///
    /// # Arguments
    /// * `node_token` - Node's resource token (contains delegation templates)
    /// * `viewer_pub_key` - Viewer's UCAN public key (becomes audience)
    /// * `resource_id` - Resource ID
    /// * `repo_ctx` - Repository context
    /// * `crypto_utils` - Crypto utilities instance
    ///
    /// # Returns
    /// * `Ok(ResourceViewerToken)` - Viewer's resource token
    pub async fn delegate_to_viewer(
        node_token: &ResourceShareToken,
        viewer_pub_key: &str,
        resource_id: &str,
        repo_ctx: Arc<RepositoryContext>,
        crypto_utils: Arc<RwLock<CryptoUtils>>,
    ) -> ServiceResult<ResourceViewerToken> {
        // 1. Extract template (NEVER HARDCODE!)
        let template = node_token.get_delegation_template("viewer").map_err(|e| {
            crate::errors::ServiceError::Ucan(crypto_utils::errors::UcanError::FormatError(
                format!("Failed to get viewer template: {}", e),
            ))
        })?;

        // 2. Build capabilities from template using standard URI format (DATA-DRIVEN)
        // Extract domain from node token's capability URIs (source of truth)
        let domain = node_token
            .domain()
            .ok_or_else(|| {
                crate::errors::ServiceError::Ucan(
                    crypto_utils::errors::UcanError::FormatError(
                        "Cannot extract domain from node token".to_string(),
                    ),
                )
            })?;
        let capabilities = template.build_capabilities(&domain, resource_id, "resource");

        // 3. Convert template to facts (GENERIC)
        // NOTE: resource_id is NOT stored in facts - it's encoded in capability URIs
        let mut facts = template.to_facts();
        facts.insert("token_type".to_string(), json!("resource_viewer"));
        facts.insert("role".to_string(), json!("viewer"));

        // 4. Get encrypted key
        let encrypted_key = get_decrypted_ucan_keys(&repo_ctx).await?;

        // 5. Generate token via crypto layer
        let crypto = crypto_utils.read().await;
        let (token, _cid) = crypto
            .generate_ucan_with_cid(
                &encrypted_key,
                viewer_pub_key,
                capabilities,
                Some(facts),
                None, // Default 30 years
            )
            .await?;

        // 6. Return typed wrapper
        Ok(ResourceViewerToken::from_token(&token).map_err(|e| {
            crate::errors::ServiceError::Ucan(crypto_utils::errors::UcanError::ValidationError(
                format!("Generated invalid ResourceViewerToken: {}", e),
            ))
        })?)
    }

    /// Delegate resource token by role (UNIFIED DELEGATION)
    ///
    /// Single function that handles delegation to any role based on the peer's role string.
    /// Extracts the appropriate delegation template from the delegator token and generates
    /// a new token with the peer's public key as audience.
    ///
    /// **UCAN-First Design**: All logic is driven by templates in the token, not hardcoded.
    ///
    /// # Arguments
    /// * `delegator_resource_ucan` - Delegator's resource UCAN (Owner or Share token)
    /// * `delegator_did` - Current user's DID/public key (added to facts as origin)
    /// * `peer_role` - Role to delegate to ("node", "viewer", "user")
    /// * `peer_pub_key` - Peer's UCAN public key (becomes audience)
    /// * `resource_id` - Resource ID
    /// * `repo_ctx` - Repository context
    /// * `crypto_utils` - Crypto utilities instance
    ///
    /// # Returns
    /// * `Ok((token_string, cid))` - Generated token string and its CID
    ///
    /// # Example
    /// ```ignore
    /// // Works for any role - logic is in the UCAN template!
    /// let (token, cid) = delegate_by_role(
    ///     &owner_ucan,
    ///     &current_user.public_key,  // Origin DID (who is delegating)
    ///     "viewer",                   // Could be "node", "user", "viewer"
    ///     viewer_pub_key,             // Audience (who receives)
    ///     resource_id,
    ///     repo_ctx,
    ///     crypto_utils
    /// ).await?;
    /// ```
    pub async fn delegate_by_role(
        delegator_resource_ucan: &str,
        delegator_did: &str,
        peer_role: &str,
        peer_pub_key: &str,
        resource_id: &str,
        repo_ctx: Arc<RepositoryContext>,
        crypto_utils: Arc<RwLock<CryptoUtils>>,
    ) -> ServiceResult<(String, String)> {
        use tracing::info;

        info!("Delegating resource UCAN to role: {}", peer_role);
        info!("  Origin DID (delegator): {}", delegator_did);

        // Parse delegator token
        let parsed_ucan = ResourceUcan::from_token(delegator_resource_ucan)?;

        // 1. Extract delegation template for peer's role (DATA-DRIVEN)
        let template = parsed_ucan.get_delegation_template(peer_role).ok_or_else(|| {
            crate::errors::ServiceError::Ucan(crypto_utils::errors::UcanError::FormatError(
                format!("No delegation template found for role: {}", peer_role),
            ))
        })?;

        // 2. Build capabilities from template using standard URI format (NEVER HARDCODED)
        // Extract domain from parsed UCAN's capability URIs (source of truth)
        let domain = parsed_ucan
            .parsed()
            .capabilities()
            .iter()
            .next()
            .and_then(|cap| {
                let parts: Vec<&str> = cap.resource.split(':').collect();
                parts.first().map(|d| d.to_string())
            })
            .ok_or_else(|| {
                crate::errors::ServiceError::Ucan(crypto_utils::errors::UcanError::FormatError(
                    "Cannot extract domain from delegator token".to_string(),
                ))
            })?;
        let capabilities = template.build_capabilities(&domain, resource_id, "resource");

        // 3. Convert template to facts + add origin DID
        // NOTE: resource_id is NOT stored in facts - it's encoded in capability URIs
        let mut facts = template.to_facts();
        facts.insert("token_type".to_string(), json!(format!("resource_{}", peer_role)));
        facts.insert("role".to_string(), json!(peer_role));
        facts.insert("origin".to_string(), json!(delegator_did)); // DID of delegator

        // 4. Get encrypted key
        let encrypted_key = get_decrypted_ucan_keys(&repo_ctx).await?;

        // 5. Generate token via crypto layer
        let crypto = crypto_utils.read().await;
        let (token, _cid) = crypto
            .generate_ucan_with_cid(
                &encrypted_key,
                peer_pub_key,
                capabilities,
                Some(facts),
                None, // Default 30 years
            )
            .await?;

        // 6. Get CID
        let cid = get_cid(&token)?;

        info!("✓ Generated {} resource token with origin DID", peer_role);

        Ok((token, cid))
    }
}

// ==================== FOLDER TOKENS MODULE ====================

/// Folder token delegation functions
///
/// These tokens control access to folders (collections of resources).
/// Delegation chain: Owner → Node → User → Viewer
///
/// **CRITICAL**: All delegation MUST extract templates from delegator UCANs.
/// NEVER hardcode capabilities - always use `DelegationTemplate::build_capabilities()`.
pub mod folder_tokens {
    use super::*;

    /// Delegate folder to node (Owner → Node)
    ///
    /// Extracts node delegation template from owner token and generates node share token.
    /// Includes both folder and resource wildcard capabilities.
    ///
    /// # Arguments
    /// * `owner_token` - Owner's folder token (contains delegation templates)
    /// * `node_pub_key` - Node's UCAN public key (becomes audience)
    /// * `folder_id` - Folder ID
    /// * `repo_ctx` - Repository context
    /// * `crypto_utils` - Crypto utilities instance
    ///
    /// # Returns
    /// * `Ok(FolderShareToken)` - Node's delegated folder token
    pub async fn delegate_to_node(
        owner_token: &FolderOwnerToken,
        node_pub_key: &str,
        folder_id: &str,
        repo_ctx: Arc<RepositoryContext>,
        crypto_utils: Arc<RwLock<CryptoUtils>>,
    ) -> ServiceResult<FolderShareToken> {
        // 1. Extract template (NEVER HARDCODE!)
        let template = owner_token.get_delegation_template("node").map_err(|e| {
            crate::errors::ServiceError::Ucan(crypto_utils::errors::UcanError::FormatError(
                format!("Failed to get node template: {}", e),
            ))
        })?;

        // 2. Build capabilities from template using standard URI format (DATA-DRIVEN)
        // Extract domain from owner token's capability URIs (source of truth)
        let domain = owner_token
            .domain()
            .ok_or_else(|| {
                crate::errors::ServiceError::Ucan(
                    crypto_utils::errors::UcanError::FormatError(
                        "Cannot extract domain from owner token".to_string(),
                    ),
                )
            })?;
        let mut capabilities = template.build_capabilities(&domain, folder_id, "folder");

        // Add resource wildcard capabilities (folder-scoped resources: folder_id/*)
        let resource_wildcard_capabilities =
            template.build_capabilities(&domain, &format!("{}/*", folder_id), "resource");
        capabilities.extend(resource_wildcard_capabilities);

        // 3. Convert template to facts (GENERIC)
        // NOTE: folder_id is NOT stored in facts - it's encoded in capability URIs
        let mut facts = template.to_facts();
        facts.insert("token_type".to_string(), json!("folder_share"));
        facts.insert("role".to_string(), json!("node"));

        // 4. Get encrypted key
        let encrypted_key = get_decrypted_ucan_keys(&repo_ctx).await?;

        // 5. Generate token via crypto layer
        let crypto = crypto_utils.read().await;
        let (token, _cid) = crypto
            .generate_ucan_with_cid(
                &encrypted_key,
                node_pub_key,
                capabilities,
                Some(facts),
                None, // Default 30 years
            )
            .await?;

        // 6. Return typed wrapper
        Ok(FolderShareToken::from_token(&token).map_err(|e| {
            crate::errors::ServiceError::Ucan(crypto_utils::errors::UcanError::ValidationError(
                format!("Generated invalid FolderShareToken: {}", e),
            ))
        })?)
    }

    /// Delegate folder to user (Node → User or Owner → User)
    ///
    /// Extracts user delegation template and generates user share token.
    ///
    /// # Arguments
    /// * `delegator_token` - Node or Owner's folder token (contains delegation templates)
    /// * `user_pub_key` - User's UCAN public key (becomes audience)
    /// * `folder_id` - Folder ID
    /// * `repo_ctx` - Repository context
    /// * `crypto_utils` - Crypto utilities instance
    ///
    /// # Returns
    /// * `Ok(FolderShareToken)` - User's delegated folder token
    pub async fn delegate_to_user(
        delegator_token: &FolderShareToken,
        user_pub_key: &str,
        folder_id: &str,
        repo_ctx: Arc<RepositoryContext>,
        crypto_utils: Arc<RwLock<CryptoUtils>>,
    ) -> ServiceResult<FolderShareToken> {
        // 1. Extract template (NEVER HARDCODE!)
        let template = delegator_token.get_delegation_template("user").map_err(|e| {
            crate::errors::ServiceError::Ucan(crypto_utils::errors::UcanError::FormatError(
                format!("Failed to get user template: {}", e),
            ))
        })?;

        // 2. Build capabilities from template using standard URI format (DATA-DRIVEN)
        // Extract domain from delegator token's capability URIs (source of truth)
        let domain = delegator_token
            .domain()
            .ok_or_else(|| {
                crate::errors::ServiceError::Ucan(
                    crypto_utils::errors::UcanError::FormatError(
                        "Cannot extract domain from delegator token".to_string(),
                    ),
                )
            })?;
        let mut capabilities = template.build_capabilities(&domain, folder_id, "folder");

        // Add resource wildcard capabilities (folder-scoped resources: folder_id/*)
        let resource_wildcard_capabilities =
            template.build_capabilities(&domain, &format!("{}/*", folder_id), "resource");
        capabilities.extend(resource_wildcard_capabilities);

        // 3. Convert template to facts (GENERIC)
        // NOTE: folder_id is NOT stored in facts - it's encoded in capability URIs
        let mut facts = template.to_facts();
        facts.insert("token_type".to_string(), json!("folder_share"));
        facts.insert("role".to_string(), json!("user"));

        // 4. Get encrypted key
        let encrypted_key = get_decrypted_ucan_keys(&repo_ctx).await?;

        // 5. Generate token via crypto layer
        let crypto = crypto_utils.read().await;
        let (token, _cid) = crypto
            .generate_ucan_with_cid(
                &encrypted_key,
                user_pub_key,
                capabilities,
                Some(facts),
                None, // Default 30 years
            )
            .await?;

        // 6. Return typed wrapper
        Ok(FolderShareToken::from_token(&token).map_err(|e| {
            crate::errors::ServiceError::Ucan(crypto_utils::errors::UcanError::ValidationError(
                format!("Generated invalid FolderShareToken: {}", e),
            ))
        })?)
    }

    /// Delegate folder to viewer (Node → Viewer)
    ///
    /// Extracts viewer delegation template and generates viewer token.
    ///
    /// # Arguments
    /// * `node_token` - Node's folder token (contains delegation templates)
    /// * `viewer_pub_key` - Viewer's UCAN public key (becomes audience)
    /// * `folder_id` - Folder ID
    /// * `repo_ctx` - Repository context
    /// * `crypto_utils` - Crypto utilities instance
    ///
    /// # Returns
    /// * `Ok(FolderViewerToken)` - Viewer's folder token
    pub async fn delegate_to_viewer(
        node_token: &FolderShareToken,
        viewer_pub_key: &str,
        folder_id: &str,
        repo_ctx: Arc<RepositoryContext>,
        crypto_utils: Arc<RwLock<CryptoUtils>>,
    ) -> ServiceResult<FolderViewerToken> {
        // 1. Extract template (NEVER HARDCODE!)
        let template = node_token.get_delegation_template("viewer").map_err(|e| {
            crate::errors::ServiceError::Ucan(crypto_utils::errors::UcanError::FormatError(
                format!("Failed to get viewer template: {}", e),
            ))
        })?;

        // 2. Build capabilities from template (DATA-DRIVEN)
        // Extract domain from node token's capability URIs (source of truth)
        let domain = node_token
            .domain()
            .ok_or_else(|| {
                crate::errors::ServiceError::Ucan(
                    crypto_utils::errors::UcanError::FormatError(
                        "Cannot extract domain from node token".to_string(),
                    ),
                )
            })?;
        let mut capabilities = template.build_capabilities(&domain, folder_id, "folder");

        // Add resource wildcard capabilities (folder pattern)
        let resource_wildcard_capabilities =
            template.build_capabilities(&domain, &format!("{}/*", folder_id), "resource");
        capabilities.extend(resource_wildcard_capabilities);

        // 3. Convert template to facts (GENERIC)
        let mut facts = template.to_facts();
        facts.insert("token_type".to_string(), json!("folder_viewer"));
        facts.insert("role".to_string(), json!("viewer"));
        facts.insert("folder_id".to_string(), json!(folder_id));

        // 4. Get encrypted key
        let encrypted_key = get_decrypted_ucan_keys(&repo_ctx).await?;

        // 5. Generate token via crypto layer
        let crypto = crypto_utils.read().await;
        let (token, _cid) = crypto
            .generate_ucan_with_cid(
                &encrypted_key,
                viewer_pub_key,
                capabilities,
                Some(facts),
                None, // Default 30 years
            )
            .await?;

        // 6. Return typed wrapper
        Ok(FolderViewerToken::from_token(&token).map_err(|e| {
            crate::errors::ServiceError::Ucan(crypto_utils::errors::UcanError::ValidationError(
                format!("Generated invalid FolderViewerToken: {}", e),
            ))
        })?)
    }

    /// Issue initial folder owner token
    ///
    /// Creates a self-signed folder owner token containing all delegation templates.
    /// This is called when a user creates a new folder - the frontend provides
    /// the complete UCAN structure with all permissions and templates.
    ///
    /// # Arguments
    /// * `folder_id` - Folder ID
    /// * `capabilities` - Vec of (resource, capability) pairs from frontend
    /// * `facts` - UCAN facts including delegation templates from frontend
    /// * `repo_ctx` - Repository context
    /// * `crypto_utils` - Crypto utilities instance
    ///
    /// # Returns
    /// * `Ok(FolderOwnerToken)` - Self-signed owner token
    /// Issue folder owner token from template JSON (high-level wrapper)
    ///
    /// This is the primary function for creating folder owner tokens from frontend templates.
    /// Parses the template JSON and generates the appropriate UCAN token.
    ///
    /// # Arguments
    /// * `folder_id` - Folder ID
    /// * `domain` - Domain (e.g., "sthalam.com")
    /// * `folder_template_json` - JSON containing folder capabilities
    /// * `crypto_utils` - Crypto utilities instance
    /// * `repo_ctx` - Repository context
    ///
    /// # Returns
    /// * `Ok((token, cid))` - Generated UCAN token string and its CID
    pub async fn issue_folder_owner_token(
        folder_id: &str,
        domain: &str,
        folder_template_json: &str,
        crypto_utils: &Arc<RwLock<CryptoUtils>>,
        repo_ctx: &Arc<RepositoryContext>,
    ) -> ServiceResult<(String, String)> {
        // Parse the template JSON
        let template_data: serde_json::Value = serde_json::from_str(folder_template_json)
            .map_err(|e| crate::errors::ServiceError::InvalidUcan(format!("Invalid folder template JSON: {}", e)))?;

        // Extract owner_template (consistent with resource token pattern)
        let owner_template = template_data.get("owner_template")
            .ok_or_else(|| crate::errors::ServiceError::InvalidUcan("Missing owner_template in folder template".to_string()))?;

        // Extract capabilities from owner_template
        let caps = owner_template.get("capabilities")
            .and_then(|v| v.as_object())
            .ok_or_else(|| crate::errors::ServiceError::InvalidUcan("Missing capabilities in owner_template".to_string()))?;

        // Build capability strings for folder operations
        let mut capabilities = Vec::new();
        for (op_name, _cap_value) in caps.iter() {
            capabilities.push((
                format!("{}:folder:{}:{}", domain, folder_id, op_name),
                String::new(), // No caveat
            ));
        }

        // Build facts
        let mut facts = serde_json::Map::new();
        facts.insert("token_type".to_string(), json!("folder_owner"));
        facts.insert("role".to_string(), json!("owner"));
        facts.insert("folder_id".to_string(), json!(folder_id));

        // Copy any additional facts from owner_template
        if let Some(metadata) = owner_template.get("metadata") {
            facts.insert("metadata".to_string(), metadata.clone());
        }

        // Copy delegation templates (enables delegation chain)
        if let Some(delegation) = owner_template.get("delegation") {
            facts.insert("delegation".to_string(), delegation.clone());
        }

        // Call the low-level function
        let folder_token = issue_folder_owner_token_internal(
            folder_id,
            capabilities,
            facts,
            repo_ctx.clone(),
            crypto_utils.clone(),
        ).await?;

        // Extract the raw token and generate CID
        let token_string = folder_token.ucan().raw_token().to_string();
        let cid = crate::ucan_service::get_cid(&token_string)?;

        Ok((token_string, cid))
    }

    /// Issue folder owner token (low-level function)
    ///
    /// Internal function that creates folder owner tokens from parsed capabilities.
    /// Use `issue_folder_owner_token` instead for most cases.
    async fn issue_folder_owner_token_internal(
        folder_id: &str,
        capabilities: Vec<(String, String)>,
        facts: serde_json::Map<String, serde_json::Value>,
        repo_ctx: Arc<RepositoryContext>,
        crypto_utils: Arc<RwLock<CryptoUtils>>,
    ) -> ServiceResult<FolderOwnerToken> {
        // Get encrypted key
        let encrypted_key = get_decrypted_ucan_keys(&repo_ctx).await?;

        // Get our own public key (self-signed)
        let crypto = crypto_utils.read().await;
        let our_pub_key = crypto.get_public_ucan_key(&encrypted_key).await?;

        // Generate token via crypto layer (self-signed)
        let (token, _cid) = crypto
            .generate_ucan_with_cid(
                &encrypted_key,
                &our_pub_key, // Self-signed
                capabilities,
                Some(facts),
                None, // Default 30 years
            )
            .await?;

        // Return typed wrapper
        Ok(FolderOwnerToken::from_token(&token).map_err(|e| {
            crate::errors::ServiceError::Ucan(crypto_utils::errors::UcanError::ValidationError(
                format!("Generated invalid FolderOwnerToken: {}", e),
            ))
        })?)
    }

    /// Issue delegated folder token (backwards compatibility wrapper)
    ///
    /// Legacy function that delegates folder access. Maps to delegate_to_node or delegate_to_viewer.
    ///
    /// # Arguments
    /// * `folder_owner_token` - Owner's folder token
    /// * `peer_pub_key` - Peer's UCAN public key
    /// * `folder_id` - Folder ID
    /// * `role` - Role to delegate ("node", "user", or "viewer")
    /// * `domain` - Domain (unused in new implementation)
    /// * `repo_ctx` - Repository context
    /// * `crypto_utils` - Crypto utilities instance
    ///
    /// # Returns
    /// * `Ok((token, cid))` - Generated folder token and its CID
    pub async fn issue_delegated_folder_token(
        folder_owner_token: &str,
        peer_pub_key: &str,
        folder_id: &str,
        role: &str,
        _domain: &str,
        repo_ctx: Arc<RepositoryContext>,
        crypto_utils: Arc<RwLock<CryptoUtils>>,
    ) -> ServiceResult<(String, String)> {
        // Parse owner token
        let owner_token = FolderOwnerToken::from_token(folder_owner_token)?;

        match role {
            "node" => {
                let node_token = delegate_to_node(&owner_token, peer_pub_key, folder_id, repo_ctx, crypto_utils).await?;
                Ok((node_token.ucan().raw_token().to_string(), String::new()))
            }
            "viewer" => {
                // For viewer, we need to go through node first (Owner → Node → Viewer delegation chain)
                // This is a simplification for backwards compatibility
                let node_token = delegate_to_node(&owner_token, peer_pub_key, folder_id, repo_ctx.clone(), crypto_utils.clone()).await?;
                let viewer_token = delegate_to_viewer(&node_token, peer_pub_key, folder_id, repo_ctx, crypto_utils).await?;
                Ok((viewer_token.ucan().raw_token().to_string(), String::new()))
            }
            _ => Err(crate::errors::ServiceError::InvalidUcan(format!("Unknown role: {}", role)))
        }
    }
}

// ==================== VALIDATION MODULE ====================

/// Validation functions combining crypto checks with business rules
///
/// These functions validate UCAN tokens and enforce application-level permissions.
pub mod validation {
    use super::*;

    /// Validate that a peer has the capability to add folders (UCAN-first)
    ///
    /// This validates based on capabilities in the token, not the token type or role.
    /// This follows the UCAN-first design principle: authorization is based on what
    /// capabilities the token has, not what role or type it claims to be.
    ///
    /// # Arguments
    /// * `peer_token` - Peer's connection token (any type - Owner, Node, User, etc.)
    /// * `domain` - Domain to check (e.g., "sthalam")
    ///
    /// # Returns
    /// * `Ok(())` if peer has add_folder capability
    /// * `Err` if peer lacks add_folder capability
    pub async fn validate_peer_can_add_folder(
        peer_token: &ConnectionToken,
        domain: &str,
    ) -> ServiceResult<()> {
        tracing::info!("🔍 Validating peer can add folder (UCAN-first)");
        tracing::info!("  - Domain: {}", domain);
        tracing::info!("  - Token role: {:?}", peer_token.role());

        // Check for add_folder capability using crypto utils
        // The capability is what matters, not the token type or role
        // The token capability structure is: "domain:folder:*" with ability "add_folder"
        let folder_resource = format!("{}:folder:*", domain);
        crypto_utils::ucan_utils::check_capability(
            peer_token.parsed(),
            &folder_resource,
            "add_folder",
        )
        .map_err(|_| {
            tracing::error!("❌ Peer lacks {}:add_folder capability", folder_resource);
            crate::errors::FolderServiceError::Validation(format!(
                "Peer lacks {}:add_folder capability",
                folder_resource
            ))
        })?;

        tracing::info!("  ✅ Peer has {}:add_folder capability (role: {:?})", folder_resource, peer_token.role());
        Ok(())
    }

    /// Validate that a peer can add resources to a folder
    ///
    /// This validates:
    /// - The folder UCAN structure is valid
    /// - The UCAN has `add_resources` capability for the specific folder
    /// - The folder_id in the UCAN matches the expected folder_id
    ///
    /// # Arguments
    /// * `folder_token` - Folder token (typed)
    /// * `expected_folder_id` - The folder_id that should match the UCAN
    /// * `domain` - Domain to check (e.g., "sthalam")
    ///
    /// # Returns
    /// * `Ok(())` if all validations pass
    /// * `Err` with descriptive error if any validation fails
    pub async fn validate_peer_can_add_resources(
        folder_token: &FolderShareToken,
        expected_folder_id: &str,
        domain: &str,
    ) -> ServiceResult<()> {
        // 1. Extract folder_id from token capabilities
        let folder_pattern = format!("{}:folder:", domain);
        let mut folder_id_from_ucan = None;

        for capability in folder_token.ucan().parsed().capabilities().iter() {
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
                "Folder UCAN lacks add_resources capability or no folder found".to_string(),
            )
        })?;

        // 2. Verify folder_id matches expected folder_id
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
    ///
    /// # Arguments
    /// * `folder_token` - Peer's folder token (typed)
    /// * `expected_folder_id` - The folder_id that should match
    /// * `domain` - Domain to check (e.g., "sthalam")
    ///
    /// # Returns
    /// * `Ok(())` if validation succeeds
    pub async fn validate_peer_can_request_link(
        folder_token: &FolderShareToken,
        expected_folder_id: &str,
        domain: &str,
    ) -> ServiceResult<()> {
        // Extract folder_id with get_share_link capability
        let folder_pattern = format!("{}:folder:", domain);
        let mut folder_id_from_ucan = None;

        for capability in folder_token.ucan().parsed().capabilities().iter() {
            let cap_resource = capability.resource;

            if cap_resource.starts_with(&folder_pattern) && capability.ability == "allow"
            {
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
                "Peer's folder UCAN lacks get_share_link capability or no folder found".to_string(),
            )
        })?;

        // Verify folder_id matches expected folder_id
        if folder_id_from_ucan != expected_folder_id {
            return Err(crate::errors::FolderServiceError::UcanError(format!(
                "Folder ID mismatch: UCAN has {}, expected {}",
                folder_id_from_ucan, expected_folder_id
            ))
            .into());
        }

        Ok(())
    }

    /// Validate folder access for resource operations
    ///
    /// Verifies that a folder token has the appropriate permissions for resource operations.
    /// This checks that the folder ID in the token matches the expected folder ID.
    ///
    /// # Arguments
    /// * `folder_token` - Folder token to validate (typed)
    /// * `expected_folder_id` - The folder_id that should match
    ///
    /// # Returns
    /// * `Ok(())` if validation succeeds
    /// * `Err` if folder ID mismatch or invalid token
    pub async fn validate_folder_access_for_resource(
        folder_token: &FolderShareToken,
        expected_folder_id: &str,
    ) -> ServiceResult<()> {
        // Extract folder_id from token
        let folder_id_from_ucan = folder_token.folder_id();

        // Verify folder_id matches expected folder_id
        if folder_id_from_ucan != expected_folder_id {
            return Err(crate::errors::ServiceError::InvalidUcan(format!(
                "Folder ID mismatch: UCAN has {}, expected {}",
                folder_id_from_ucan, expected_folder_id
            )));
        }

        Ok(())
    }

    /// Validate UCAN structure and basic fields
    ///
    /// Generic UCAN validation that checks for required fields and proper structure.
    /// This is a basic validation that can be applied to any UCAN token.
    ///
    /// # Arguments
    /// * `ucan_token` - Raw UCAN token string
    ///
    /// # Returns
    /// * `Ok(())` if UCAN structure is valid
    /// * `Err` if parsing fails or required fields missing
    pub async fn validate_ucan_structure(ucan_token: &str) -> ServiceResult<()> {
        // Attempt to parse the UCAN
        let _ucan = ResourceUcan::from_token(ucan_token).map_err(|e| {
            crate::errors::ServiceError::InvalidUcan(format!("UCAN parsing failed: {}", e))
        })?;

        // If parsing succeeds, the structure is valid
        // (ResourceUcan::from_token() validates token_type, role, capabilities, etc.)
        Ok(())
    }
}

// ==================== UTILITIES MODULE ====================

/// Extraction and parsing utilities for UCAN tokens
///
/// These functions provide backwards compatibility with the old ucan_service
/// extraction patterns while using the new typed token infrastructure.
pub mod utilities {
    use super::*;
    use osvauld_core::models::{ResourceUcan, Role};

    /// Extract resource ID from UCAN token
    ///
    /// Parses a resource UCAN token and extracts the resource ID from capabilities.
    ///
    /// # Arguments
    /// * `ucan_token` - Raw UCAN token string
    ///
    /// # Returns
    /// * `Ok(resource_id)` - Extracted resource ID
    /// * `Err` - If parsing fails or resource ID not found
    pub async fn extract_resource_id(ucan_token: &str) -> ServiceResult<String> {
        let ucan = ResourceUcan::from_token(ucan_token)
            .map_err(|e| crate::errors::ServiceError::InvalidUcan(e.to_string()))?;

        ucan.resource_id()
            .ok_or_else(|| {
                crate::errors::ServiceError::InvalidUcan(
                    "No resource ID found in UCAN capabilities".to_string(),
                )
            })
    }

    /// Extract folder ID from viewer token
    ///
    /// Viewer tokens use a special capability format for folders.
    /// This extracts the folder ID for viewer authentication.
    ///
    /// # Arguments
    /// * `viewer_ucan_token` - Viewer's UCAN token
    /// * `_domain` - Domain (unused, kept for backwards compatibility)
    ///
    /// # Returns
    /// * `Ok(folder_id)` - Extracted folder ID
    pub async fn extract_folder_id_from_viewer_token(
        viewer_ucan_token: &str,
        _domain: &str,
    ) -> ServiceResult<String> {
        let ucan = ResourceUcan::from_token(viewer_ucan_token)
            .map_err(|e| crate::errors::ServiceError::InvalidUcan(e.to_string()))?;

        ucan.folder_id()
            .ok_or_else(|| {
                crate::errors::ServiceError::InvalidUcan(
                    "No folder ID found in viewer UCAN capabilities".to_string(),
                )
            })
    }

    /// Extract folder ID with add_resources capability check
    ///
    /// Validates that the folder UCAN contains the add_resources capability
    /// by checking the actual UCAN capabilities (cap field) using check_capability.
    ///
    /// # Arguments
    /// * `ucan_token` - Folder UCAN token string
    /// * `domain` - Domain (e.g., "sthalam")
    ///
    /// # Returns
    /// * `Ok(folder_id)` - Extracted folder ID if has add_resources capability
    pub async fn extract_folder_id_with_add_resources(
        ucan_token: &str,
        domain: &str,
    ) -> ServiceResult<String> {
        use crypto_utils::ucan_utils::check_capability;

        // Parse token
        let ucan = ResourceUcan::from_token(ucan_token)
            .map_err(|e| crate::errors::ServiceError::InvalidUcan(e.to_string()))?;

        // Extract folder_id
        let folder_id = ucan.folder_id().ok_or_else(|| {
            crate::errors::ServiceError::InvalidUcan(
                "No folder ID found in UCAN".to_string(),
            )
        })?;

        // Validate add_resources capability using proper UCAN validation
        let folder_resource = format!("{}:folder:{}:add_resources", domain, folder_id);
        check_capability(ucan.parsed(), &folder_resource, "allow")
            .map_err(|_| crate::errors::ServiceError::InvalidUcan(
                "Folder token does not have add_resources capability".to_string(),
            ))?;

        Ok(folder_id)
    }

    /// Validate folder UCAN and extract folder_id
    ///
    /// Validates that a folder UCAN token is valid and extracts the folder_id from it.
    /// Works with any folder token type (Owner, Share, or Viewer).
    ///
    /// # Arguments
    /// * `folder_ucan` - Folder UCAN token string
    ///
    /// # Returns
    /// * `Ok(folder_id)` - The folder ID from the token
    /// * `Err` - If token is invalid or doesn't contain a folder ID
    pub async fn validate_folder_ucan_and_get_id(folder_ucan: &str) -> ServiceResult<String> {
        // Validate UCAN structure
        validate_ucan_structure(folder_ucan).await?;

        // Try to parse as FolderShareToken (works for Owner, Share, and Viewer tokens)
        let folder_token = FolderShareToken::from_token(folder_ucan)
            .map_err(|e| crate::errors::ServiceError::InvalidUcan(format!("Invalid folder token: {}", e)))?;

        Ok(folder_token.folder_id())
    }

    /// Extract document capabilities from UCAN token
    ///
    /// Returns a map of document names to their capability strings.
    ///
    /// # Arguments
    /// * `ucan_token` - UCAN token string
    ///
    /// # Returns
    /// * `Ok(HashMap<doc_name, capability_string>)` - Document capabilities
    pub async fn extract_doc_capabilities(
        ucan_token: &str,
    ) -> ServiceResult<std::collections::HashMap<String, String>> {
        let ucan = ResourceUcan::from_token(ucan_token)
            .map_err(|e| crate::errors::ServiceError::InvalidUcan(e.to_string()))?;

        let capabilities = ucan
            .capabilities()
            .iter()
            .map(|(doc_name, cap)| (doc_name.clone(), cap.as_str().to_string()))
            .collect();

        Ok(capabilities)
    }

    /// Extract UCAN facts
    ///
    /// Returns the raw facts JSON from the UCAN token.
    ///
    /// # Arguments
    /// * `ucan_token` - UCAN token string
    ///
    /// # Returns
    /// * `Ok(Some(facts))` - UCAN facts as JSON value
    /// * `Ok(None)` - If no facts present
    pub async fn extract_facts(
        ucan_token: &str,
    ) -> ServiceResult<Option<serde_json::Map<String, serde_json::Value>>> {
        let ucan = ResourceUcan::from_token(ucan_token)
            .map_err(|e| crate::errors::ServiceError::InvalidUcan(e.to_string()))?;

        // Convert BTreeMap to serde_json::Map
        Ok(ucan.parsed().facts().as_ref().map(|btree| {
            let mut map = serde_json::Map::new();
            for (k, v) in btree.iter() {
                map.insert(k.clone(), v.clone());
            }
            map
        }))
    }

    /// Extract role from UCAN token
    ///
    /// Extracts the role from any UCAN token's facts field.
    /// Works with connection tokens, folder tokens, and resource tokens.
    ///
    /// # Arguments
    /// * `ucan_token` - UCAN token string
    ///
    /// # Returns
    /// * `Ok(role_string)` - The role as a string ("owner", "node", "user", "viewer")
    /// * `Err` - If token is invalid or doesn't contain a role
    pub async fn extract_role_from_token(ucan_token: &str) -> ServiceResult<String> {
        let facts = extract_facts(ucan_token).await?;

        facts
            .and_then(|f| f.get("role").and_then(|v| v.as_str()).map(String::from))
            .ok_or_else(|| crate::errors::ServiceError::InvalidUcan("Role not found in token".to_string()))
    }

    /// Extract folder capabilities from UCAN token
    ///
    /// Similar to extract_doc_capabilities but for folder tokens.
    ///
    /// # Arguments
    /// * `ucan_token` - Folder UCAN token string
    ///
    /// # Returns
    /// * `Ok(Vec<capability_strings>)` - List of folder capabilities
    pub async fn extract_folder_capabilities(
        ucan_token: &str,
    ) -> ServiceResult<Vec<String>> {
        let ucan = ResourceUcan::from_token(ucan_token)
            .map_err(|e| crate::errors::ServiceError::InvalidUcan(e.to_string()))?;

        let capabilities = ucan
            .capabilities()
            .values()
            .map(|cap| cap.as_str().to_string())
            .collect();

        Ok(capabilities)
    }

    /// Get CID from UCAN token
    ///
    /// Extracts the CID (content identifier) from a UCAN token.
    /// Currently returns empty string as CID tracking is not yet implemented.
    ///
    /// # Arguments
    /// * `ucan_token` - UCAN token string
    ///
    /// # Returns
    /// * `Ok(cid)` - The CID string (empty for now)
    pub fn get_cid(_ucan_token: &str) -> ServiceResult<String> {
        // TODO: Implement actual CID extraction when we add CID tracking
        Ok(String::new())
    }

    // ==================== VIEWER-SPECIFIC STUBS (TODO) ====================

    /// Stub: Generate viewer token for folder
    pub async fn generate_viewer_token_for_folder(
        _folder_ucan: &str,
        _folder_id: &str,
        _domain: &str,
        _repo_ctx: Arc<RepositoryContext>,
        _crypto_utils: Arc<RwLock<CryptoUtils>>,
    ) -> ServiceResult<String> {
        Err(ServiceError::Internal {
            message: "Viewer support not yet implemented".to_string(),
        })
    }

    /// Stub: Extract folder ID from viewer token
    pub fn extract_folder_id(_token: &str) -> ServiceResult<String> {
        Err(ServiceError::Internal {
            message: "Viewer support not yet implemented".to_string(),
        })
    }

    /// Stub: Extract audience from UCAN token
    pub fn extract_audience(_token: &str) -> ServiceResult<String> {
        Err(ServiceError::Internal {
            message: "Viewer support not yet implemented".to_string(),
        })
    }
}
