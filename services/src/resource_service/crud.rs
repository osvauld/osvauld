//! CRUD operations for resources
//!
//! Resource lifecycle management: create, read, update, delete, share
//!
//! All functions use typed tokens and delegate common patterns to core.rs.

use super::core;
use crate::errors::{ResourceServiceError, ServiceError, ServiceResult};
use crypto_utils::{encrypt_data_for_user, CryptoUtils};
use log::{error, info};
use osvauld_core::models::{
    resource::{EncryptedResource, Resource},
    PermissionLevel, ResourceOwnerToken, ResourceShareToken, ShareOperation, ShareRecord, User,
};
use persistance::database::RepositoryContext;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

// =============================================================================
// CREATE
// =============================================================================

/// Create a new resource with encryption and UCAN token
///
/// # Arguments
/// * `resource_payload` - Initial document data (JSON string with doc snapshots)
/// * `ucan_template_json` - JSON containing owner_template and viewer_template
/// * `metadata_json` - Resource metadata (title, search config, etc.)
/// * `folder_id` - Folder UUID
/// * `user` - Current user
/// * `_current_device_id` - Device ID for tracking
/// * `domain` - Domain for UCAN (e.g., "sthalam.com")
/// * `repo_ctx` - Database repository context
/// * `crypto_utils` - Crypto utilities
///
/// # Returns
/// * `Resource` - The created resource (decrypted)
pub async fn create_resource(
    resource_payload: String,
    ucan_template_json: String,
    metadata_json: String,
    folder_id: String,
    user: &User,
    _current_device_id: &str,
    domain: &str,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
) -> ServiceResult<Resource> {
    info!(
        "Creating resource for user {} in folder {}",
        user.id, folder_id
    );

    // Generate resource ID
    let resource_id = Uuid::new_v4().to_string();

    // Parse metadata JSON
    let metadata: serde_json::Value = serde_json::from_str(&metadata_json).map_err(|e| {
        ResourceServiceError::InvalidResourceData(format!("Invalid metadata JSON: {}", e))
    })?;

    // Encrypt the resource payload
    let (encrypted_data, encrypted_key) =
        encrypt_data_for_user(&resource_payload, &user.public_key)
            .map_err(|e| ResourceServiceError::EncryptionFailed(e.to_string()))?;

    // Generate owner UCAN with templates from frontend
    let (ucan_token, ucan_cid) = crate::ucan_service::resource_tokens::issue_owner_token(
        &resource_id,
        domain,
        &ucan_template_json,
        crypto_utils,
        &repo_ctx,
    )
    .await
    .map_err(|e| {
        error!("Failed to generate owner UCAN: {}", e);
        ResourceServiceError::UcanError(e.to_string())
    })?;

    // Get current timestamp
    let now = chrono::Utc::now().timestamp();

    // Create EncryptedResource for database storage
    let encrypted_resource = EncryptedResource {
        id: resource_id.clone(),
        folder_id: folder_id.clone(),
        created_at: now,
        updated_at: now,
        encrypted_data,
        encrypted_key: encrypted_key.clone(),
        ucan_token: ucan_token.clone(),
        metadata: metadata.clone(),
    };

    // Save to database
    repo_ctx
        .resource_repo
        .save_encrypted(&encrypted_resource)
        .await
        .map_err(|e| {
            error!("Failed to create resource in database: {}", e);
            ResourceServiceError::DatabaseError(e.to_string())
        })?;

    // Create owner's ShareRecord
    let share_record = ShareRecord {
        id: Uuid::new_v4().to_string(),
        resource_id: resource_id.clone(),
        shared_by_user_id: user.id.clone(),
        recipient_user_id: user.id.clone(), // Owner shares with self
        ucan_token: ucan_token.clone(),
        ucan_cid: ucan_cid.clone(),
        operation_type: ShareOperation::Share,
        permission_level: PermissionLevel::Admin,
        created_at: now,
        updated_at: now,
    };

    repo_ctx.share_repo.save(&share_record).await.map_err(|e| {
        error!("Failed to create share record: {}", e);
        ResourceServiceError::DatabaseError(e.to_string())
    })?;

    info!("Successfully created resource {}", resource_id);

    // Return decrypted Resource
    let resource = Resource::from_decrypted_data(
        resource_id,
        folder_id,
        ucan_token,
        metadata,
        &resource_payload,
    )
    .map_err(|e| ResourceServiceError::InvalidResourceData(e))?;

    Ok(resource)
}

// =============================================================================
// READ
// =============================================================================

/// Get a single resource by ID (direct access, no token)
///
/// # Arguments
/// * `resource_id` - Resource UUID
/// * `repo_ctx` - Database repository context
/// * `crypto_utils` - Crypto utilities
///
/// # Returns
/// * `Resource` - The decrypted resource
pub async fn get_resource_by_id_direct(
    resource_id: &str,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
) -> ServiceResult<Resource> {
    info!("Fetching resource by ID: {}", resource_id);

    // Use core helper
    core::load_and_decrypt_resource(resource_id, repo_ctx, crypto_utils).await
}

/// Get a single resource by typed token
///
/// Uses the resource ID embedded in the token.
///
/// # Arguments
/// * `token` - Resource share token
/// * `repo_ctx` - Database repository context
/// * `crypto_utils` - Crypto utilities
///
/// # Returns
/// * `Resource` - The decrypted resource
pub async fn get_resource_by_token(
    token: &ResourceShareToken,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
) -> ServiceResult<Resource> {
    info!("Fetching resource by token");

    // Use core helper - eliminates Load-Decrypt-Parse pattern
    core::load_and_decrypt_by_share_token(token, repo_ctx, crypto_utils).await
}

/// Get metadata for all resources (no decryption, just metadata)
///
/// Returns lightweight metadata for all resources without decrypting their content.
/// Used for listing resources in the UI.
///
/// # Arguments
/// * `user_id` - User ID to fetch resources for
/// * `repo_ctx` - Repository context
///
/// # Returns
/// * `Vec<(EncryptedResource, String)>` - Encrypted resources with their encrypted keys
pub async fn get_all_resources_metadata(
    user_id: &str,
    repo_ctx: Arc<RepositoryContext>,
) -> ServiceResult<Vec<(EncryptedResource, String)>> {
    info!("Fetching all resources metadata for user: {}", user_id);

    // Get all encrypted resources from database
    let encrypted_resources = repo_ctx.resource_repo.get_all_resources(user_id).await?;

    // Convert to (EncryptedResource, encrypted_key) tuples
    let resources_with_keys: Vec<(EncryptedResource, String)> = encrypted_resources
        .into_iter()
        .map(|resource| {
            let key = resource.encrypted_key.clone();
            (resource, key)
        })
        .collect();

    info!("Found {} resources for user", resources_with_keys.len());
    Ok(resources_with_keys)
}

// =============================================================================
// UPDATE
// =============================================================================

/// Update a resource with new data and rotate AES key
///
/// Frontend sends complete new Loro snapshots, we replace the entire encrypted_data.
/// Generates a new AES key for forward secrecy on every update.
///
/// # Arguments
/// * `resource_id` - Resource UUID
/// * `data` - New Loro document data (JSON string with snapshots)
/// * `user` - Current user (for encryption)
/// * `repo_ctx` - Database repository context
///
/// # Returns
/// * `()` - Success
pub async fn update_resource(
    resource_id: &str,
    data: String,
    user: &User,
    repo_ctx: Arc<RepositoryContext>,
) -> ServiceResult<()> {
    info!("Updating resource {}", resource_id);

    // Parse data to extract metadata for validation
    let data_json: serde_json::Value = serde_json::from_str(&data).map_err(|e| {
        ResourceServiceError::InvalidResourceData(format!("Invalid data JSON: {}", e))
    })?;

    // Extract title from data for logging
    let title = data_json
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("Untitled");

    info!("Updating resource {} ({})", resource_id, title);

    // Generate NEW AES key and encrypt data (key rotation for forward secrecy)
    let (encrypted_data, encrypted_key) = encrypt_data_for_user(&data, &user.public_key)
        .map_err(|e| ResourceServiceError::EncryptionFailed(e.to_string()))?;

    // Update resource in database with new encrypted data and key
    repo_ctx
        .resource_repo
        .update_resource(&encrypted_data, &encrypted_key, resource_id)
        .await
        .map_err(|e| {
            error!("Failed to update resource in database: {}", e);
            ResourceServiceError::DatabaseError(e.to_string())
        })?;

    info!("Successfully updated resource {}", resource_id);
    Ok(())
}

// =============================================================================
// DELETE
// =============================================================================

/// Soft delete a resource
///
/// # Arguments
/// * `resource_id` - Resource UUID
/// * `repo_ctx` - Database repository context
///
/// # Returns
/// * `()` - Success
pub async fn delete_resource(
    resource_id: String,
    repo_ctx: Arc<RepositoryContext>,
) -> ServiceResult<()> {
    info!("Deleting resource {}", resource_id);

    repo_ctx
        .resource_repo
        .delete_resource(&resource_id)
        .await
        .map_err(|e| {
            error!("Failed to delete resource: {}", e);
            ResourceServiceError::DatabaseError(e.to_string())
        })?;

    info!("Successfully deleted resource {}", resource_id);
    Ok(())
}

// =============================================================================
// SHARE
// =============================================================================

/// Share a resource with another user
///
/// Creates a share record with delegated UCAN token. Does NOT create encrypted_data
/// (that's created on-demand during sync).
///
/// # Arguments
/// * `resource_id` - Resource ID to share
/// * `recipient_user_id` - User ID to share with
/// * `recipient_role` - Role for recipient ("owner", "node", or "viewer")
/// * `current_user` - Current user (owner) sharing the resource
/// * `domain` - Domain for UCAN verification
/// * `repo_ctx` - Database repository context
/// * `crypto_utils` - Crypto utilities
///
/// # Returns
/// * `()` - Success (share record created)
pub async fn share_resource(
    resource_id: &str,
    recipient_user_id: &str,
    recipient_role: &str,
    current_user: &User,
    domain: &str,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
) -> ServiceResult<()> {
    info!(
        "Sharing resource {} with user {} (role: {})",
        resource_id, recipient_user_id, recipient_role
    );

    // 1. Get recipient user to get their UCAN public key
    let recipient_user = repo_ctx
        .user_repo
        .get_user_by_id(recipient_user_id)
        .await
        .map_err(|e| {
            error!("Failed to find recipient user {}: {}", recipient_user_id, e);
            ResourceServiceError::UserNotFound(recipient_user_id.to_string())
        })?;

    // 2. Get resource from database to read its UCAN token (proof)
    let resource = repo_ctx
        .resource_repo
        .find_by_id(resource_id)
        .await
        .map_err(|e| {
            error!("Failed to fetch resource {}: {}", resource_id, e);
            ResourceServiceError::DatabaseError(e.to_string())
        })?;

    info!(
        "Owner's resource UCAN token length: {}",
        resource.ucan_token.len()
    );

    // 3. Parse owner token as typed token
    let owner_token = ResourceOwnerToken::from_token(&resource.ucan_token)
        .map_err(|e| {
            error!("Failed to parse owner UCAN token: {}", e);
            ResourceServiceError::UcanError(e.to_string())
        })?;

    // 4. Generate delegated UCAN for recipient using typed tokens
    info!(
        "Delegating resource UCAN to {} with role: {}",
        recipient_user_id, recipient_role
    );

    let delegated_token = crate::ucan_service::resource_tokens::delegate_to_node(
        &owner_token,
        &recipient_user.ucan_pub_key,
        resource_id,
        repo_ctx.clone(),
        crypto_utils.clone(),
    )
    .await
    .map_err(|e| {
        error!("Failed to generate delegated UCAN: {}", e);
        ResourceServiceError::UcanError(e.to_string())
    })?;

    let resource_ucan_token = delegated_token.ucan().raw_token().to_string();
    let resource_ucan_cid = String::new(); // TODO: Get CID from token

    info!(
        "Generated delegated UCAN token length: {}",
        resource_ucan_token.len()
    );

    // 5. Create share record
    let share_record = ShareRecord {
        id: Uuid::new_v4().to_string(),
        resource_id: resource_id.to_string(),
        shared_by_user_id: current_user.id.clone(),
        recipient_user_id: recipient_user_id.to_string(),
        ucan_token: resource_ucan_token,
        ucan_cid: resource_ucan_cid,
        permission_level: PermissionLevel::Admin,
        operation_type: ShareOperation::Share,
        created_at: chrono::Utc::now().timestamp(),
        updated_at: chrono::Utc::now().timestamp(),
    };

    // 6. Save share record to database
    repo_ctx.share_repo.save(&share_record).await.map_err(|e| {
        error!("Failed to save share record: {}", e);
        ResourceServiceError::DatabaseError(e.to_string())
    })?;

    info!(
        "Successfully shared resource {} with user {}",
        resource_id, recipient_user_id
    );

    Ok(())
}
