// =============================================================================
// Loro Migration - Phase 2: Resource Service Layer
// =============================================================================
//
// This module provides service-layer operations for encrypted resources using
// Loro CRDTs. It handles:
// - CRUD operations with encryption/decryption
// - UCAN-based sharing with template-driven permissions
// - Role-based delegation (owner, node, viewer)
//
// Major changes from Yrs version:
// - Removed all sync methods (deferred to Phase 3 network module)
// - Removed vector clock operations
// - Removed UI helpers (toggle_fav, update_last_accessed)
// - Simplified to 14 core methods using new Resource struct
// - UCAN templates control permissions (owner_template, viewer_template)

use crate::errors::{ResourceServiceError, ServiceError, ServiceResult};
use crypto_utils::{CryptoUtils, encrypt_data_for_user, errors::UcanError};
use log::{error, info};
use osvauld_core::models::{
    PermissionLevel, ShareOperation, ShareRecord, User,
    resource::{EncryptedResource, Resource},
    document::create_doc,
};
use persistance::database::RepositoryContext;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

// =============================================================================
// Helper Functions
// =============================================================================

/// Load and decrypt a resource by extracting resource_id from UCAN token
///
/// Common helper used by sync operations to load a resource based on peer's UCAN.
///
/// # Arguments
/// * `ucan_token` - UCAN token containing resource_id
/// * `repo_ctx` - Database repository context
/// * `crypto_utils` - Crypto utilities for decryption
///
/// # Returns
/// * `Resource` - Decrypted resource ready for sync operations
async fn load_resource_by_ucan(
    ucan_token: &str,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
) -> Result<Resource, ResourceServiceError> {
    // 1. Extract resource_id from UCAN token
    let resource_id = crate::ucan_service::extract_resource_id(ucan_token)
        .await
        .map_err(|e| {
            error!("Failed to extract resource_id from UCAN: {}", e);
            ResourceServiceError::UcanError(format!("Invalid UCAN token: {}", e))
        })?;

    info!("Loading resource: {}", resource_id);

    // 2. Use common helper to fetch, decrypt, and parse resource
    get_resource_by_id_direct(&resource_id, repo_ctx, crypto_utils)
        .await
        .map_err(|e| match e {
            ServiceError::Resource(err) => err,
            _ => ResourceServiceError::InvalidState(format!("Unexpected error: {}", e)),
        })
}

/// Decrypt one or more resources from database format to in-memory Resource structs
///
/// Unified helper that handles both single resource and batch decryption.
///
/// # Arguments
/// * `resources_with_keys` - Vector of encrypted resources with their keys
/// * `crypto_utils` - Crypto utilities for decryption
///
/// # Returns
/// * `Vec<Resource>` - Decrypted resources ready for use
async fn decrypt_resources(
    encrypted_resources: Vec<EncryptedResource>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
) -> ServiceResult<Vec<Resource>> {
    let mut decrypted_resources = Vec::new();

    for encrypted_resource in encrypted_resources {
        let crypto = crypto_utils.read().await;

        // Decrypt the encrypted_data field
        let decrypted_json = crypto
            .decrypt_resource(
                &encrypted_resource.encrypted_data,
                &encrypted_resource.encrypted_key,
            )
            .map_err(|e| {
                error!(
                    "Failed to decrypt resource {}: {}",
                    encrypted_resource.id, e
                );
                ResourceServiceError::DecryptionFailed(encrypted_resource.id.clone())
            })?;

        // Parse decrypted JSON and create Resource
        let resource = Resource::from_decrypted_data(
            encrypted_resource.id.clone(),
            encrypted_resource.folder_id.clone(),
            encrypted_resource.ucan_token.clone(),
            encrypted_resource.metadata.clone(),
            &decrypted_json,
        )
        .map_err(|e| {
            error!("Failed to parse resource {}: {}", encrypted_resource.id, e);
            ResourceServiceError::InvalidResourceData(e)
        })?;

        decrypted_resources.push(resource);
    }

    Ok(decrypted_resources)
}

/// Decrypt a single EncryptedResource to Resource
///
/// Helper that decrypts and parses a single encrypted resource.
///
/// # Arguments
/// * `encrypted_resource` - The encrypted resource to decrypt
/// * `crypto_utils` - Crypto utilities for decryption
///
/// # Returns
/// * `Resource` - Decrypted resource ready for use
async fn decrypt_encrypted_resource(
    encrypted_resource: &EncryptedResource,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
) -> Result<Resource, ResourceServiceError> {
    let crypto = crypto_utils.read().await;

    // Decrypt the encrypted_data field
    let decrypted_json = crypto
        .decrypt_resource(
            &encrypted_resource.encrypted_data,
            &encrypted_resource.encrypted_key,
        )
        .map_err(|e| {
            error!(
                "Failed to decrypt resource {}: {}",
                encrypted_resource.id, e
            );
            ResourceServiceError::DecryptionFailed(encrypted_resource.id.clone())
        })?;

    // Parse decrypted JSON and create Resource
    let resource = Resource::from_decrypted_data(
        encrypted_resource.id.clone(),
        encrypted_resource.folder_id.clone(),
        encrypted_resource.ucan_token.clone(),
        encrypted_resource.metadata.clone(),
        &decrypted_json,
    )
    .map_err(|e| {
        error!("Failed to parse resource {}: {}", encrypted_resource.id, e);
        ResourceServiceError::InvalidResourceData(e)
    })?;

    Ok(resource)
}

// =============================================================================
// CRUD Operations
// =============================================================================

/// Create a new resource with encryption and UCAN token
///
/// # Arguments
/// * `resource_payload` - Initial document data (JSON string with doc snapshots)
/// * `ucan_template_json` - JSON containing owner_template and viewer_template
/// * `metadata_json` - Resource metadata (title, search config, etc.)
/// * `folder_id` - Folder UUID
/// * `user` - Current user
/// * `current_device_id` - Device ID for tracking
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
    let (ucan_token, ucan_cid) = crate::ucan_service::issue_resource_owner_token(
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

    // Save to database using new save_encrypted method
    repo_ctx
        .resource_repo
        .save_encrypted(&encrypted_resource)
        .await
        .map_err(|e| {
            error!("Failed to create resource in database: {}", e);
            ResourceServiceError::DatabaseError(e.to_string())
        })?;

    // NOTE: ResourceKey is now part of EncryptedResource.encrypted_key
    // No separate table needed

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

/// Get a single resource by ID
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

    // Fetch encrypted resource from database
    let encrypted_resource = repo_ctx.resource_repo.find_by_id(resource_id).await?;

    // Decrypt using existing helper
    let resource = decrypt_encrypted_resource(&encrypted_resource, crypto_utils).await?;

    // Return single resource

    Ok(resource)
}

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
    // EncryptedResource already contains encrypted_key field
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

    // 3. Generate delegated UCAN for recipient using ucan_service
    info!(
        "Calling issue_resource_ucan_for_node with role: {}",
        recipient_role
    );

    let (resource_ucan_token, resource_ucan_cid) = crate::ucan_service::issue_resource_ucan_for_node(
        resource_id,
        &resource.ucan_token, // Owner's resource UCAN (contains templates)
        &recipient_user.ucan_pub_key,
        domain,
        crypto_utils.clone(),
        &repo_ctx,
    )
    .await
    .map_err(|e| {
        error!("Failed to generate delegated UCAN: {}", e);
        ResourceServiceError::UcanError(e.to_string())
    })?;

    info!(
        "Generated delegated UCAN token length: {}",
        resource_ucan_token.len()
    );

    // 5. Create share record (NO encrypted_data - created during sync)
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

// =============================================================================
// Resource Sync Operations
// =============================================================================

/// Get state vectors for a resource based on UCAN token capabilities
///
/// Extracts resource_id from the UCAN token, loads the resource, and returns
/// state vectors only for documents that the token holder has access to.
/// This ensures that owner/node get all docs, while viewers only get docs
/// they have permissions for (respects dont_send_to_node rules).
///
/// # Arguments
/// * `ucan_token` - UCAN token from the initiator (contains resource_id and capabilities)
/// * `domain` - Domain for UCAN validation (e.g., "sthalam")
/// * `repo_ctx` - Database repository context
/// * `crypto_utils` - Crypto utilities for decryption
///
/// # Returns
/// * `String` - JSON string with state vectors: {"doc_name": {"state_vector": [...]}, ...}
///           Only includes docs that the UCAN token has access to
pub async fn get_resource_state_vectors_by_ucan(
    ucan_token: &str,
    _domain: &str,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
) -> Result<String, ResourceServiceError> {
    info!("Getting state vectors from UCAN token");

    // TODO: Validate UCAN token (signature, proof chain, expiry)
    // For now, we trust the token since it came from authenticated peer connection

    // Load resource using common helper
    let resource = load_resource_by_ucan(ucan_token, repo_ctx, crypto_utils).await?;

    // Get state vectors filtered by UCAN capabilities
    let state_vectors = crate::merge_service::get_state_vectors_for_ucan(&resource, ucan_token)
        .await
        .map_err(|e| {
            error!("Failed to get filtered state vectors: {}", e);
            ResourceServiceError::InvalidResourceData(e.to_string())
        })?;

    info!("Successfully got state vectors (filtered by UCAN)");
    Ok(state_vectors)
}

/// Generate incremental updates for a peer based on their state vectors
///
/// Loads resource, compares peer's state vectors with our current state,
/// and generates incremental updates filtered by peer's UCAN capabilities.
///
/// # Arguments
/// * `peer_ucan` - Peer's UCAN token (contains resource_id and capabilities)
/// * `peer_state_vectors` - JSON string with peer's current state vectors
/// * `repo_ctx` - Database repository context
/// * `crypto_utils` - Crypto utilities for decryption
///
/// # Returns
/// * `String` - JSON string: {"doc_name": {"updates": [...], "state_vector": [...]}, ...}
pub async fn generate_updates_for_peer(
    peer_ucan: &str,
    peer_state_vectors: &str,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
) -> Result<(String, String), ResourceServiceError> {
    info!("Generating updates for peer");

    // Load resource using common helper
    let resource = load_resource_by_ucan(peer_ucan, repo_ctx, crypto_utils).await?;

    // Generate updates based on peer's state vectors and capabilities
    let updates =
        crate::merge_service::generate_updates_for_peer(&resource, peer_ucan, peer_state_vectors)
            .await
            .map_err(|e| {
                error!("Failed to generate updates for peer: {}", e);
                ResourceServiceError::InvalidResourceData(e.to_string())
            })?;

    // Extract state vectors from updates
    let state_vectors = crate::merge_service::extract_state_vectors_from_updates(&updates)
        .map_err(|e| {
            error!("Failed to extract state vectors from updates: {}", e);
            ResourceServiceError::InvalidResourceData(e.to_string())
        })?;

    info!("Successfully generated updates and state vectors for peer");
    Ok((updates, state_vectors))
}

/// Apply updates from a peer and generate our updates back
///
/// This function:
/// 1. Applies peer's updates to our local resource (validates UCAN permissions)
/// 2. Extracts peer's state vectors from the updates
/// 3. Generates our updates for peer based on their state
/// 4. Saves updated resource to DB with new AES key (key rotation)
/// 5. Returns our updates + our state vectors
///
/// # Arguments
/// * `peer_ucan` - Peer's UCAN token (contains resource_id and capabilities)
/// * `peer_updates_json` - JSON string with peer's updates and state vectors
/// * `user` - Current user (for encryption)
/// * `repo_ctx` - Database repository context
/// * `crypto_utils` - Crypto utilities for encryption/decryption
///
/// # Returns
/// * `String` - JSON with our updates: {"doc_name": {"updates": [...], "state_vector": [...]}, ...}
pub async fn apply_peer_updates(
    peer_ucan: &str,
    peer_updates_json: &str,
    user: &User,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
) -> Result<String, ResourceServiceError> {
    info!("Applying peer updates");

    // 1. Load resource using common helper
    let mut resource = load_resource_by_ucan(peer_ucan, repo_ctx.clone(), crypto_utils).await?;

    // 2. Apply peer's updates (validates UCAN permissions)
    crate::merge_service::apply_peer_updates(&mut resource, peer_ucan, peer_updates_json)
        .await
        .map_err(|e| {
            error!("Failed to apply peer updates: {}", e);
            ResourceServiceError::InvalidResourceData(format!("Failed to apply updates: {}", e))
        })?;

    info!("Successfully applied peer updates");

    // 3. Extract peer's state vectors from their updates to know what they have
    let peer_state_vectors_json = crate::merge_service::extract_state_vectors_from_updates(
        peer_updates_json,
    )
    .map_err(|e| {
        error!("Failed to extract peer state vectors: {}", e);
        ResourceServiceError::InvalidResourceData(e.to_string())
    })?;

    // 4. Generate our updates for peer based on their state
    let our_updates = crate::merge_service::generate_updates_for_peer(
        &resource,
        peer_ucan,
        &peer_state_vectors_json,
    )
    .await
    .map_err(|e| {
        error!("Failed to generate our updates for peer: {}", e);
        ResourceServiceError::InvalidResourceData(e.to_string())
    })?;

    // 5. Serialize and save updated resource to database
    let updated_json = resource.to_json().map_err(|e| {
        error!("Failed to serialize updated resource: {}", e);
        ResourceServiceError::InvalidResourceData(e)
    })?;

    update_resource(&resource.id, updated_json, user, repo_ctx)
        .await
        .map_err(|e| match e {
            ServiceError::Resource(err) => err,
            _ => ResourceServiceError::InvalidState(format!("Failed to update resource: {}", e)),
        })?;

    info!("Successfully applied updates and generated response");
    Ok(our_updates)
}

/// Get resource UCANs for sync
///
/// Retrieves the resource UCAN and folder UCAN needed to initiate sync.
/// Used by the sync handler to prepare ResourceSyncRequest message.
///
/// NOTE: Share records are self-referencing (node→node) by design (duct tape solution).
/// We get ALL share records for the resource/folder and take the first one.
///
/// # Arguments
/// * `resource_id` - ID of the resource to sync
/// * `repo_ctx` - Database repository context
///
/// # Returns
/// * `(resource_ucan, folder_ucan)` - Tuple of UCAN tokens
pub async fn get_resource_ucans_for_sync(
    resource_id: &str,
    repo_ctx: Arc<RepositoryContext>,
) -> Result<(String, String), ResourceServiceError> {
    info!("Getting UCANs for resource sync: {}", resource_id);

    // Get resource to find folder_id
    let resource = repo_ctx
        .resource_repo
        .find_by_id(resource_id)
        .await
        .map_err(|e| {
            error!("Failed to find resource {}: {}", resource_id, e);
            ResourceServiceError::DatabaseError(e.to_string())
        })?;

    // Get ALL share records for this resource (operation="share")
    // NOTE: Share records are self-referencing (node→node) by design
    let share_records = repo_ctx
        .share_repo
        .find_by_resource_and_operation(resource_id, "share")
        .await
        .map_err(|e| {
            error!(
                "Failed to find share records for resource {}: {}",
                resource_id, e
            );
            ResourceServiceError::DatabaseError(format!("No share records found: {}", e))
        })?;

    // Take first share record (should be only one for now)
    let share_record = share_records.first().ok_or_else(|| {
        error!("No share records found for resource {}", resource_id);
        ResourceServiceError::DatabaseError("No share records found".to_string())
    })?;

    // Get ALL folder share records for this folder (operation="share")
    // NOTE: get_records_by_folder_id already filters by operation="share"
    let folder_share_records = repo_ctx
        .folder_share_repo
        .get_records_by_folder_id(&resource.folder_id)
        .await
        .map_err(|e| {
            error!(
                "Failed to find folder share records for folder {}: {}",
                resource.folder_id, e
            );
            ResourceServiceError::DatabaseError(format!("No folder share records found: {}", e))
        })?;

    // Take first folder share record (should be only one for now)
    let folder_share_record = folder_share_records.first().ok_or_else(|| {
        error!(
            "No folder share records found for folder {}",
            resource.folder_id
        );
        ResourceServiceError::DatabaseError("No folder share records found".to_string())
    })?;

    info!("✓ Found UCANs for resource sync");
    Ok((share_record.ucan_token.clone(), folder_share_record.ucan_token.clone()))
}

/// Prepare resource for sending to a peer
///
/// Filters documents based on UCANs and re-encrypts for peer.
///
/// # Arguments
/// * `resource_id` - Resource ID
/// * `user_id` - User ID preparing resource
/// * `peer_ucan` - Peer's UCAN token for capability filtering
/// * `peer_public_key` - Peer's public key for encryption
/// * `repo_ctx` - Database repository context
/// * `crypto_utils` - Crypto utilities
///
/// # Returns
/// * `EncryptedResource` - Resource filtered and encrypted for peer
pub async fn prepare_resource_for_peer(
    resource_id: &str,
    _user_id: &str,
    peer_ucan: &str,
    peer_public_key: &str,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
) -> Result<EncryptedResource, ResourceServiceError> {
    info!("Preparing resource {} for peer", resource_id);

    // 1. Fetch original encrypted resource (needed for timestamps and re_encrypt_for_recipient)
    let original_encrypted = repo_ctx
        .resource_repo
        .find_by_id(resource_id)
        .await
        .map_err(|e| {
            error!("Failed to fetch resource {}: {}", resource_id, e);
            ResourceServiceError::DatabaseError(e.to_string())
        })?;

    // 2. Decrypt and parse resource using helper
    let resource = decrypt_encrypted_resource(&original_encrypted, crypto_utils).await?;

    // 3. Filter documents based on UCANs (returns unencrypted HashMap)
    let filtered_snapshots =
        crate::merge_service::filter_documents_to_send(&resource, &resource.ucan_token, peer_ucan)
            .await
            .map_err(|e| {
                error!("Failed to filter resource {}: {}", resource_id, e);
                ResourceServiceError::InvalidResourceData(e.to_string())
            })?;

    // 4. Convert filtered HashMap to JSON string
    let filtered_json = serde_json::to_string(&filtered_snapshots).map_err(|e| {
        error!("Failed to serialize filtered data: {}", e);
        ResourceServiceError::InvalidResourceData(format!("Serialization failed: {}", e))
    })?;

    // 5. Encrypt filtered data for peer
    let (new_encrypted_data, new_encrypted_key) =
        encrypt_data_for_user(&filtered_json, peer_public_key).map_err(|e| {
            error!("Failed to encrypt for peer: {}", e);
            ResourceServiceError::EncryptionFailed(e.to_string())
        })?;

    // 6. Create new EncryptedResource for peer
    let peer_encrypted_resource = original_encrypted.re_encrypt_for_recipient(
        new_encrypted_data,
        new_encrypted_key,
        peer_ucan.to_string(),
    );

    info!("Successfully prepared resource {} for peer", resource_id);
    Ok(peer_encrypted_resource)
}

/// Prepare resource transfer in response to ResourceNotFoundRequest
///
/// This function validates folder access, fetches share records, and prepares
/// the resource for sending to a peer who doesn't have it.
///
/// # Arguments
/// * `resource_id` - ID of the resource to transfer
/// * `requester_folder_ucan` - Requester's folder UCAN token (proves they should have access)
/// * `requester_user_id` - Requester's user ID
/// * `requester_public_key` - Requester's public key for encryption
/// * `domain` - Domain for UCAN validation
/// * `repo_ctx` - Database repository context
/// * `crypto_utils` - Crypto utilities for encryption
///
/// # Returns
/// * `(EncryptedResource, Vec<ShareRecord>)` - Resource encrypted for requester and all share records
pub async fn prepare_resource_transfer(
    resource_id: &str,
    requester_folder_ucan: &str,
    requester_user_id: &str,
    requester_public_key: &str,
    domain: &str,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
) -> Result<(EncryptedResource, Vec<ShareRecord>), ResourceServiceError> {
    info!("Preparing resource transfer for resource {}", resource_id);

    // Get resource to find its folder_id
    let resource = repo_ctx
        .resource_repo
        .find_by_id(resource_id)
        .await
        .map_err(|e| {
            error!("Failed to find resource {}: {}", resource_id, e);
            ResourceServiceError::InvalidState(format!("Resource not found: {}", e))
        })?;

    // Validate requester's folder_ucan proves they should have access
    crate::validate_folder_access_for_resource(
        requester_folder_ucan,
        &resource.folder_id,
        domain,
    )
    .await
    .map_err(|e| {
        error!(
            "Failed to validate folder access for resource {}: {}",
            resource_id, e
        );
        ResourceServiceError::UcanError(format!("Access denied: {}", e))
    })?;

    info!("✓ Validated folder access");

    // Get share record for requester to get their resource UCAN
    // Note: Using "view" operation as default - this gives read access
    let peer_share_record = repo_ctx
        .share_repo
        .find_by_resource_and_operation_and_user(resource_id, "view", requester_user_id)
        .await
        .map_err(|e| {
            error!(
                "Failed to find share record for resource {} and user {}: {}",
                resource_id, requester_user_id, e
            );
            ResourceServiceError::InvalidState(format!(
                "No share record found for requester: {}",
                e
            ))
        })?;

    // Get ALL share records for this resource
    let all_share_records = crate::get_all_share_records_for_resource(resource_id, repo_ctx.clone())
        .await
        .map_err(|e| {
            error!(
                "Failed to get all share records for resource {}: {}",
                resource_id, e
            );
            ResourceServiceError::InvalidState(format!(
                "Failed to get share records: {}",
                e
            ))
        })?;

    // Prepare resource for peer (decrypt, filter, re-encrypt)
    let peer_encrypted_resource = prepare_resource_for_peer(
        resource_id,
        requester_user_id,
        &peer_share_record.ucan_token,
        requester_public_key,
        repo_ctx.clone(),
        crypto_utils,
    )
    .await
    .map_err(|e| {
        error!(
            "Failed to prepare resource {} for peer: {}",
            resource_id, e
        );
        ResourceServiceError::InvalidState(format!("Failed to prepare resource: {}", e))
    })?;

    info!("✓ Resource prepared for transfer");
    Ok((peer_encrypted_resource, all_share_records))
}

/// Save resource transfer from initiator
///
/// This function saves a resource and its share records received during
/// ResourceTransfer protocol. Note: We skip folder_ucan validation here because:
/// 1. Responder sent their folder_ucan in the request
/// 2. Initiator already validated it before sending
///
/// # Arguments
/// * `resource` - Encrypted resource to save
/// * `share_records` - All share records for this resource
/// * `repo_ctx` - Database repository context
///
/// # Returns
/// * `()` - Success (resource and share records saved)
pub async fn save_resource_transfer(
    resource: &EncryptedResource,
    share_records: &[ShareRecord],
    repo_ctx: Arc<RepositoryContext>,
) -> Result<(), ResourceServiceError> {
    info!("Saving resource transfer for resource {}", resource.id);

    // Save resource with all share records in transaction
    repo_ctx
        .resource_repo
        .save_resource_with_share_records(resource, share_records)
        .await
        .map_err(|e| {
            error!(
                "Failed to save resource {} with share records: {}",
                resource.id, e
            );
            ResourceServiceError::InvalidState(format!("Failed to save resource: {}", e))
        })?;

    info!(
        "✓ Saved resource {} with {} share records",
        resource.id,
        share_records.len()
    );
    Ok(())
}

/// Accept and save a resource from a peer after validating capabilities
///
/// Validates that:
/// 1. Owner has `add_resources` capability for the folder in folder UCAN
/// 2. Folder ID in folder UCAN matches resource.folder_id
///
/// # Arguments
/// * `resource` - Encrypted resource to save
/// * `share_records` - All share records for this resource
/// * `owner_folder_ucan` - Owner's folder UCAN token
/// * `domain` - Domain for UCAN verification
/// * `repo_ctx` - Database repository context
///
/// # Returns
/// * `()` - Success (resource and share records saved)
pub async fn accept_resource_from_peer(
    resource: &EncryptedResource,
    share_records: &[ShareRecord],
    owner_folder_ucan: &str,
    domain: &str,
    repo_ctx: Arc<RepositoryContext>,
) -> ServiceResult<()> {
    info!("Accepting resource {} from peer", resource.id);

    // Validate owner's folder UCAN has add_resources capability for this folder
    let folder_token = osvauld_core::models::FolderShareToken::from_token(owner_folder_ucan)
        .map_err(|e| ResourceServiceError::UcanError(format!("Failed to parse folder token: {}", e)))?;
    crate::ucan_service::validation::validate_peer_can_add_resources(&folder_token, &resource.folder_id, domain).await?;

    // Save resource with all share records in transaction
    repo_ctx
        .resource_repo
        .save_resource_with_share_records(resource, share_records)
        .await
        .map_err(|e| {
            error!(
                "Failed to save resource {} with share records: {}",
                resource.id, e
            );
            ResourceServiceError::DatabaseError(e.to_string())
        })?;

    info!(
        "Successfully accepted resource {} with {} share records",
        resource.id,
        share_records.len()
    );
    Ok(())
}

/// Compare asset IDs using set operations
///
/// Determines which assets are missing on each side by performing
/// HashSet difference operations.
///
/// # Arguments
/// * `local_asset_ids` - Asset IDs we have locally
/// * `peer_asset_ids` - Asset IDs the peer has
///
/// # Returns
/// * `(missing_on_peer, missing_on_local)` - Tuple of asset ID vectors
pub fn compare_asset_ids(
    local_asset_ids: &[String],
    peer_asset_ids: &[String],
) -> (Vec<String>, Vec<String>) {
    use std::collections::HashSet;

    let local_set: HashSet<&String> = local_asset_ids.iter().collect();
    let peer_set: HashSet<&String> = peer_asset_ids.iter().collect();

    // Assets we have that peer doesn't
    let missing_on_peer: Vec<String> = local_set
        .difference(&peer_set)
        .map(|&id| id.clone())
        .collect();

    // Assets peer has that we don't
    let missing_on_local: Vec<String> = peer_set
        .difference(&local_set)
        .map(|&id| id.clone())
        .collect();

    (missing_on_peer, missing_on_local)
}

/// Get list of resources in folder for folder sync
///
/// Returns minimal resource information needed for folder sync discovery phase.
/// Includes resource_id, state_vectors, and asset_ids for each resource.
///
/// # Arguments
/// * `folder_id` - ID of the folder
/// * `user_id` - ID of the user
/// * `repo_ctx` - Database repository context
/// * `crypto_utils` - Crypto utilities for decryption
///
/// # Returns
/// * `Vec<ResourceSyncInfo>` - List of resource sync information
pub async fn get_resource_list_for_folder(
    folder_id: &str,
    user_id: &str,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
) -> ServiceResult<Vec<serde_json::Value>> {
    info!("Getting resource list for folder: {}", folder_id);

    // 1. Get all resource IDs in folder
    let resource_ids = repo_ctx
        .resource_repo
        .get_resource_ids_by_folder_id(folder_id)
        .await
        .map_err(|e| ResourceServiceError::DatabaseError(e.to_string()))?;

    info!("Found {} resources in folder", resource_ids.len());

    // 2. For each resource, get share record and prepare sync info
    let mut resource_list = Vec::new();

    for resource_id in resource_ids {
        // Get share record to get UCAN token
        let share_records = repo_ctx
            .share_repo
            .find_by_resources_and_user(&[resource_id.clone()], user_id, "read")
            .await
            .map_err(|e| {
                error!("Failed to get share record for resource {}: {}", resource_id, e);
                ResourceServiceError::DatabaseError(e.to_string())
            })?;

        let share_record = share_records.into_iter().next().ok_or_else(|| {
            error!("No share record found for resource {} and user {}", resource_id, user_id);
            ResourceServiceError::InvalidState(format!("No share record found for resource {}", resource_id))
        })?;

        // Get resource for last_modified timestamp
        let resource = repo_ctx
            .resource_repo
            .find_by_id(&resource_id)
            .await
            .map_err(|e| ResourceServiceError::DatabaseError(e.to_string()))?;

        // Prepare resource info
        let resource_info = serde_json::json!({
            "resource_id": resource_id,
            "resource_ucan": share_record.ucan_token,
            "last_modified": resource.updated_at,
        });

        resource_list.push(resource_info);
    }

    Ok(resource_list)
}

/// Prepare all data needed for ResourceSyncRequest
///
/// Aggregates UCANs, state vectors, and full docs for a resource sync request.
///
/// # Arguments
/// * `resource_id` - ID of the resource
/// * `user_id` - ID of the user
/// * `repo_ctx` - Database repository context
/// * `crypto_utils` - Crypto utilities for decryption
///
/// # Returns
/// * `(resource_ucan, folder_ucan, state_vectors, full_docs)` - Tuple of sync data
pub async fn prepare_resource_sync_request(
    resource_id: &str,
    user_id: &str,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
) -> ServiceResult<(String, String, String, String)> {
    info!("Preparing ResourceSyncRequest for resource: {}", resource_id);

    // 1. Get UCANs (user_id no longer needed - share records are self-referencing)
    let (resource_ucan, folder_ucan) =
        get_resource_ucans_for_sync(resource_id, repo_ctx.clone()).await?;

    // 2. Load and decrypt resource
    let resource = get_resource_by_id_direct(resource_id, repo_ctx.clone(), crypto_utils).await?;

    // 3. Prepare sync data (state vectors + full docs)
    let (state_vectors, full_docs) =
        crate::merge_service::prepare_sync_data(&resource, &resource_ucan).await?;

    info!("Successfully prepared ResourceSyncRequest data");
    Ok((resource_ucan, folder_ucan, state_vectors, full_docs))
}

/// Get asset binary data from local storage
///
/// Retrieves the binary data for an asset stored in the filesystem.
/// Assets are stored at: <data_dir>/resources/<resource_id>/assets/<asset_id>.<ext>
///
/// # Arguments
/// * `resource_id` - ID of the resource containing the asset
/// * `asset_id` - ID of the asset to retrieve
/// * `repo_ctx` - Repository context (provides data_dir path)
///
/// # Returns
/// * `Vec<u8>` - Binary asset data
pub async fn get_asset_binary_data(
    resource_id: &str,
    asset_id: &str,
    repo_ctx: Arc<RepositoryContext>,
) -> ServiceResult<Vec<u8>> {
    // TODO: Implement asset retrieval from encrypted resource blob
    // Assets are stored inside EncryptedResource.encrypted_data as JSON
    // Not as separate files on filesystem
    info!("get_asset_binary_data called for asset {} in resource {} (STUB)", asset_id, resource_id);
    Ok(Vec::new())
}

/// Save asset binary data to local storage
///
/// Saves binary data for an asset to the filesystem and updates the
/// static_assets Loro document with metadata.
///
/// # Arguments
/// * `resource_id` - ID of the resource to save asset to
/// * `asset_id` - ID of the asset
/// * `asset_data` - Binary asset data
/// * `metadata_json` - Asset metadata JSON (mime_type, size, filename)
/// * `user` - Current user (for encryption when saving resource)
/// * `repo_ctx` - Repository context
/// * `crypto_utils` - Crypto utilities for decryption/encryption
///
/// # Returns
/// * `()` - Success
pub async fn save_asset_binary_data(
    resource_id: &str,
    asset_id: &str,
    asset_data: &[u8],
    metadata_json: &str,
    user: &User,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
) -> ServiceResult<()> {
    // TODO: Implement asset saving to encrypted resource blob
    // Assets should be stored inside EncryptedResource.encrypted_data as JSON
    // This function should call merge_service to handle the asset addition
    info!("save_asset_binary_data called for asset {} in resource {} (STUB)", asset_id, resource_id);
    Ok(())
}

// ==================== Re-export merge_service functions ====================
// These are thin wrappers to maintain architectural boundaries:
// P2P layer → resource_service → merge_service

/// Apply viewer submission (re-exported from merge_service)
pub use crate::merge_service::apply_submission;

/// Extract asset IDs (re-exported from merge_service)
pub use crate::merge_service::extract_asset_ids;
