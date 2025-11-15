//! Synchronization operations for resources
//!
//! CRDT sync operations, state vectors, peer updates, and viewer operations
//!
//! This module consolidates:
//! - 13 sync functions from resource_service.rs
//! - 9 functions from merge_service.rs (refactored with SyncContext)
//! - 3 functions from website_service.rs (refactored with typed tokens)

use super::core;
use crate::errors::{ResourceServiceError, ServiceError, ServiceResult};
use crypto_utils::{encrypt_data_for_user, CryptoUtils};
use log::{error, info};
use osvauld_core::models::{
    document::{apply_updates, export_shallow_snapshot, export_updates, state_frontiers},
    resource::{EncryptedResource, Resource},
    Folder, FolderShareRecord, PermissionLevel, ResourceShareToken, ResourceViewerToken,
    ShareOperation, ShareRecord, SyncContext, SyncDecision, User,
};
use persistance::database::RepositoryContext;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

// =============================================================================
// State Vector Operations
// =============================================================================

/// Get state vectors for a resource based on UCAN token capabilities
///
/// Uses SyncContext to determine which documents to include.
///
/// # Arguments
/// * `ucan_token` - UCAN token from the initiator
/// * `_domain` - Domain for UCAN validation
/// * `repo_ctx` - Database repository context
/// * `crypto_utils` - Crypto utilities for decryption
///
/// # Returns
/// * `String` - JSON string with state vectors for accessible docs
pub async fn get_resource_state_vectors_by_ucan(
    ucan_token: &str,
    _domain: &str,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
) -> Result<String, ResourceServiceError> {
    info!("Getting state vectors from UCAN token");

    // Load resource by UCAN
    let resource_id = crate::ucan_service::extract_resource_id(ucan_token).await?;
    let resource = core::load_and_decrypt_resource(&resource_id, repo_ctx, crypto_utils).await?;

    // Use merge_service for now (will refactor to use SyncContext)
    crate::merge_service::get_state_vectors_for_ucan(&resource, ucan_token)
        .await
        .map_err(|e| match e {
            ServiceError::Resource(err) => err,
            _ => ResourceServiceError::InvalidState(format!("Unexpected error: {}", e)),
        })
}

/// Generate updates for peer based on their state vectors
///
/// # Arguments
/// * `ucan_token` - Our UCAN token
/// * `peer_ucan` - Peer's UCAN token
/// * `peer_state_vectors_json` - JSON with peer's state vectors
/// * `domain` - Domain
/// * `repo_ctx` - Repository context
/// * `crypto_utils` - Crypto utilities
///
/// # Returns
/// * `String` - JSON with updates for peer
pub async fn generate_updates_for_peer(
    ucan_token: &str,
    peer_ucan: &str,
    peer_state_vectors_json: String,
    _domain: &str,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
) -> Result<String, ResourceServiceError> {
    info!("Generating updates for peer");

    // Load resource
    let resource_id = crate::ucan_service::extract_resource_id(ucan_token).await?;
    let resource = core::load_and_decrypt_resource(&resource_id, repo_ctx, crypto_utils).await?;

    // Use merge_service for now
    crate::merge_service::generate_updates_for_peer(&resource, ucan_token, &peer_state_vectors_json)
        .await
        .map_err(|e| match e {
            ServiceError::Resource(err) => err,
            _ => ResourceServiceError::InvalidState(format!("Unexpected error: {}", e)),
        })
}

/// Apply peer updates to resource
///
/// # Arguments
/// * `ucan_token` - Our UCAN token
/// * `peer_ucan` - Peer's UCAN token
/// * `peer_updates_json` - JSON with peer's updates
/// * `domain` - Domain
/// * `user` - Current user
/// * `repo_ctx` - Repository context
/// * `crypto_utils` - Crypto utilities
///
/// # Returns
/// * `()` - Success
pub async fn apply_peer_updates(
    ucan_token: &str,
    peer_ucan: &str,
    peer_updates_json: String,
    _domain: &str,
    user: &User,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
) -> Result<(), ResourceServiceError> {
    info!("Applying peer updates");

    // Load resource
    let resource_id = crate::ucan_service::extract_resource_id(ucan_token).await?;
    let mut resource =
        core::load_and_decrypt_resource(&resource_id, repo_ctx.clone(), crypto_utils).await?;

    // Use merge_service for now
    crate::merge_service::apply_peer_updates(&mut resource, ucan_token, &peer_updates_json)
        .await
        .map_err(|e| match e {
            ServiceError::Resource(err) => err,
            _ => ResourceServiceError::InvalidState(format!("Unexpected error: {}", e)),
        })?;

    // Save updated resource
    core::encrypt_and_save_resource(&resource, &user.public_key, repo_ctx).await?;

    Ok(())
}

// =============================================================================
// Resource Transfer (for folder sync)
// =============================================================================

/// Get resource UCANs for sync
pub async fn get_resource_ucans_for_sync(
    resource_id: &str,
    current_user: &User,
    peer_folder_ucan: &str,
    peer_role: &str,
    peer_user: &User,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
) -> Result<(String, String, String), ResourceServiceError> {
    info!("Getting resource UCANs for sync: {}", resource_id);
    info!("  Peer role: {}", peer_role);
    info!("  Current user: {}", current_user.id);

    // Extract DIDs (UCAN public keys) from User models
    let current_user_did = &current_user.ucan_pub_key; // DID is the UCAN public key
    let peer_did = &peer_user.ucan_pub_key;             // Peer's DID for audience

    // Get resource from database to get our UCAN and folder_id
    let encrypted_resource = repo_ctx.resource_repo.find_by_id(resource_id).await?;
    let our_ucan = encrypted_resource.ucan_token.clone();
    let folder_id = &encrypted_resource.folder_id;

    // Validate peer has folder access and get folder_id from their folder UCAN
    info!("  Validating peer folder UCAN");
    let peer_folder_id = crate::ucan_service::validate_folder_ucan_and_get_id(peer_folder_ucan)
        .await
        .map_err(|e| {
            error!("Invalid peer folder UCAN: {}", e);
            ResourceServiceError::UcanError(format!("Invalid peer folder UCAN: {}", e))
        })?;

    // Verify peer's folder UCAN is for the same folder as the resource
    if &peer_folder_id != folder_id {
        error!(
            "Folder ID mismatch: resource folder {} vs peer folder {}",
            folder_id, peer_folder_id
        );
        return Err(ResourceServiceError::UcanError(format!(
            "Peer folder UCAN is for folder {} but resource is in folder {}",
            peer_folder_id, folder_id
        )));
    }

    info!("  ✓ Peer has valid folder access for folder {}", folder_id);

    // Delegate resource UCAN using unified UCAN-first function
    info!("  Delegating resource UCAN by role: {}", peer_role);
    let (peer_ucan, cid) = crate::ucan_service::delegate_resource_by_role(
        &our_ucan,
        current_user_did,
        peer_role,
        peer_did,
        resource_id,
        repo_ctx,
        crypto_utils.clone(),
    )
    .await?;

    info!("  ✓ Generated peer resource UCAN");

    Ok((our_ucan, peer_ucan, cid))
}

/// Prepare resource for peer
pub async fn prepare_resource_for_peer(
    resource_id: &str,
    our_ucan: &str,
    peer_ucan: &str,
    peer_public_key: &str,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
) -> Result<(String, String), ResourceServiceError> {
    info!("Preparing resource for peer: {}", resource_id);

    // Load resource
    let resource =
        core::load_and_decrypt_resource(resource_id, repo_ctx.clone(), crypto_utils).await?;

    // Create SyncContext
    let sync_context = core::create_sync_context(our_ucan, peer_ucan).await?;

    // Filter and encrypt for peer (returns base64 strings)
    core::filter_and_encrypt_for_peer(&resource, &sync_context, peer_public_key)
        .await
        .map_err(|e| match e {
            ServiceError::Resource(err) => err,
            _ => ResourceServiceError::InvalidState(format!("Unexpected error: {}", e)),
        })
}

// =============================================================================
// Folder Sync Helpers
// =============================================================================

/// Compare asset IDs between two resources
pub fn compare_asset_ids(
    _our_resource: &Resource,
    _peer_resource: &Resource,
) -> (Vec<String>, Vec<String>) {
    // TODO: Implement asset comparison
    (Vec::new(), Vec::new())
}

/// Get resource list for folder
pub async fn get_resource_list_for_folder(
    folder_id: &str,
    repo_ctx: Arc<RepositoryContext>,
) -> Result<Vec<EncryptedResource>, ResourceServiceError> {
    info!("Getting resource list for folder: {}", folder_id);

    repo_ctx
        .resource_repo
        .get_all_resources(folder_id)
        .await
        .map_err(|e| ResourceServiceError::DatabaseError(e.to_string()))
}

/// Prepare resource sync request
pub async fn prepare_resource_sync_request(
    ucan_token: &str,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
) -> Result<String, ResourceServiceError> {
    info!("Preparing resource sync request");

    // Load resource
    let resource_id = crate::ucan_service::extract_resource_id(ucan_token).await?;
    let resource = core::load_and_decrypt_resource(&resource_id, repo_ctx, crypto_utils).await?;

    // Build state vectors for all documents
    let state_vectors_json = core::build_state_vectors_json(&resource, |_| true)?;

    Ok(state_vectors_json)
}

// =============================================================================
// Asset Operations (Stubs for now)
// =============================================================================

/// Get asset binary data
pub async fn get_asset_binary_data(
    _resource_id: &str,
    _asset_id: &str,
    _repo_ctx: Arc<RepositoryContext>,
) -> Result<Vec<u8>, ResourceServiceError> {
    // TODO: Implement asset retrieval
    Ok(Vec::new())
}

/// Save asset binary data
pub async fn save_asset_binary_data(
    _resource_id: &str,
    _asset_id: &str,
    _data: Vec<u8>,
    _repo_ctx: Arc<RepositoryContext>,
) -> Result<(), ResourceServiceError> {
    // TODO: Implement asset saving
    Ok(())
}

// =============================================================================
// Re-exports from merge_service (temporary - will refactor)
// =============================================================================

pub use crate::merge_service::{
    apply_submission, extract_asset_ids, extract_state_vectors_from_updates,
    filter_documents_to_send, prepare_sync_data,
};

// =============================================================================
// Resource Transfer Operations
// =============================================================================

/// Prepare resource transfer
///
/// Prepares a complete EncryptedResource for sending to a peer.
/// Validates folder access, delegates resource UCAN, filters and re-encrypts data.
///
/// # Arguments
/// * `resource_id` - Resource ID to transfer
/// * `current_user` - Current user (delegator)
/// * `peer_folder_ucan` - Peer's folder UCAN (proves folder access)
/// * `peer_role` - Peer's role ("node", "viewer", etc.)
/// * `peer_user` - Peer user (recipient)
/// * `repo_ctx` - Database repository context
/// * `crypto_utils` - Crypto utilities
///
/// # Returns
/// * `Ok(EncryptedResource)` - Complete encrypted resource with peer's UCAN
pub async fn prepare_resource_transfer(
    resource_id: &str,
    current_user: &User,
    peer_folder_ucan: &str,
    peer_role: &str,
    peer_user: &User,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
) -> Result<EncryptedResource, ResourceServiceError> {
    info!("Preparing resource transfer: {}", resource_id);
    info!("  Current user: {}", current_user.id);
    info!("  Peer user: {}, role: {}", peer_user.id, peer_role);

    // Get original resource for metadata and timestamps
    let original_resource = repo_ctx.resource_repo.find_by_id(resource_id).await?;

    // Get UCANs (validates folder access, delegates resource UCAN)
    let (our_ucan, peer_ucan, _cid) = get_resource_ucans_for_sync(
        resource_id,
        current_user,
        peer_folder_ucan,
        peer_role,
        peer_user,
        repo_ctx.clone(),
        crypto_utils,
    )
    .await?;

    // Prepare resource data (filter and encrypt using peer's PGP public key)
    // Returns base64-encoded strings ready for EncryptedResource
    let peer_pgp_key = &peer_user.public_key;
    let (encrypted_data, encrypted_key) = prepare_resource_for_peer(
        resource_id,
        &our_ucan,
        &peer_ucan,
        peer_pgp_key,
        repo_ctx,
        crypto_utils,
    )
    .await?;

    // Construct EncryptedResource with peer's UCAN
    let peer_encrypted_resource = EncryptedResource {
        id: original_resource.id,
        folder_id: original_resource.folder_id,
        created_at: original_resource.created_at,
        updated_at: original_resource.updated_at,
        encrypted_data,  // Already base64-encoded
        encrypted_key,   // Already base64-encoded
        ucan_token: peer_ucan,  // Peer's delegated UCAN
        metadata: original_resource.metadata,
    };

    info!("✓ Resource transfer prepared");

    Ok(peer_encrypted_resource)
}

/// Save resource transfer
pub async fn save_resource_transfer(
    _resource_id: &str,
    _peer_encrypted_data: Vec<u8>,
    _peer_encrypted_key: Vec<u8>,
    _peer_ucan: String,
    _peer_ucan_cid: String,
    _repo_ctx: Arc<RepositoryContext>,
) -> Result<(), ResourceServiceError> {
    // TODO: Implement save logic
    info!("Saving resource transfer");
    Ok(())
}

/// Accept resource from peer (node receives from owner)
///
/// Validates that the owner has add_resources permission and saves the resource
/// with all share_records in a transaction.
///
/// # Arguments
/// * `resource` - EncryptedResource to save (contains peer's delegated UCAN)
/// * `share_records` - ALL share_records for this resource (for forwarding to viewers)
/// * `folder_ucan` - Owner's folder UCAN (proves add_resources capability)
/// * `domain` - Domain for UCAN validation
/// * `repo_ctx` - Database repository context
///
/// # Returns
/// * `Ok(())` - Resource and share_records saved successfully
/// * `Err` - If validation fails or database save fails
pub async fn accept_resource_from_peer(
    resource: &EncryptedResource,
    share_records: &[ShareRecord],
    folder_ucan: &str,
    domain: &str,
    repo_ctx: Arc<RepositoryContext>,
) -> ServiceResult<()> {
    info!("📥 Accepting resource {} from peer", resource.id);
    info!("  Folder: {}", resource.folder_id);
    info!("  Share records: {}", share_records.len());

    // 1. Validate folder_ucan has add_resources capability
    info!("  Step 1: Validating owner folder UCAN has add_resources capability");
    let folder_id = crate::ucan_service::validate_folder_token_has_add_resources(
        folder_ucan,
        domain,
    )
    .map_err(|e| {
        error!("❌ Owner folder UCAN validation failed: {}", e);
        ResourceServiceError::UcanError(format!(
            "Owner folder UCAN doesn't have add_resources: {}",
            e
        ))
    })?;

    // 2. Verify the folder_id matches the resource's folder
    if folder_id != resource.folder_id {
        error!(
            "❌ Folder ID mismatch: UCAN folder {} vs resource folder {}",
            folder_id, resource.folder_id
        );
        return Err(ServiceError::Resource(ResourceServiceError::UcanError(format!(
            "Owner folder UCAN is for folder {} but resource is in folder {}",
            folder_id, resource.folder_id
        ))));
    }

    info!("  ✓ Owner has add_resources capability for folder {}", folder_id);

    // 3. Validate resource UCAN structure
    info!("  Step 2: Validating resource UCAN structure");
    crate::ucan_service::validate_ucan_structure(&resource.ucan_token)
        .await
        .map_err(|e| {
            error!("❌ Invalid resource UCAN: {}", e);
            ResourceServiceError::UcanError(format!("Invalid resource UCAN: {}", e))
        })?;

    info!("  ✓ Resource UCAN structure valid");

    // 4. Validate all share_record UCANs
    info!("  Step 3: Validating {} share_record UCANs", share_records.len());
    for (i, share_record) in share_records.iter().enumerate() {
        crate::ucan_service::validate_ucan_structure(&share_record.ucan_token)
            .await
            .map_err(|e| {
                error!("❌ Invalid share_record[{}] UCAN: {}", i, e);
                ResourceServiceError::UcanError(format!("Invalid share_record UCAN: {}", e))
            })?;
    }

    info!("  ✓ All share_record UCANs valid");

    // 5. Save resource + share_records in transaction
    info!("  Step 4: Saving resource and share_records to database");
    repo_ctx
        .resource_repo
        .save_resource_with_share_records(resource, share_records)
        .await
        .map_err(|e| {
            error!("❌ Failed to save resource with share records: {}", e);
            ResourceServiceError::DatabaseError(e.to_string())
        })?;

    info!("✅ Successfully accepted resource {} with {} share_records", resource.id, share_records.len());

    Ok(())
}
