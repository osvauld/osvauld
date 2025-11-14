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
    _folder_ucan: &str,
    peer_ucan_pub_key: &str,
    domain: &str,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
) -> Result<(String, String, String), ResourceServiceError> {
    info!("Getting resource UCANs for sync: {}", resource_id);

    // Get resource from database
    let encrypted_resource = repo_ctx.resource_repo.find_by_id(resource_id).await?;

    // Get our UCAN token (owner's token)
    let our_ucan = encrypted_resource.ucan_token.clone();

    // Generate delegated UCAN for peer
    // TODO: Use typed tokens and proper delegation
    let peer_ucan = our_ucan.clone(); // Placeholder
    let cid = String::new();

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
) -> Result<(Vec<u8>, Vec<u8>), ResourceServiceError> {
    info!("Preparing resource for peer: {}", resource_id);

    // Load resource
    let resource =
        core::load_and_decrypt_resource(resource_id, repo_ctx.clone(), crypto_utils).await?;

    // Create SyncContext
    let sync_context = core::create_sync_context(our_ucan, peer_ucan).await?;

    // Filter and encrypt for peer
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
// Viewer Connection Helper Functions (Temporary - P2P layer needs refactoring)
// =============================================================================
//
// TODO: These functions exist only for backwards compatibility with the current P2P layer.
// The P2P layer (website_handler.rs) should be refactored to use the standard sync pipeline
// functions above (prepare_resource_for_peer, etc.) which already handle permissions via UCAN.
//
// The website_handler should:
// 1. Use standard folder_service functions for folder operations
// 2. Use prepare_resource_for_peer() for resource sync
// 3. Stop treating "viewer mode" as special - it's just another peer with UCAN permissions

/// Check user and folder status for viewer connection
///
/// TODO: Remove this - P2P layer should use standard folder queries
pub async fn check_user_and_folder_status(
    viewer_ucan_token: &str,
    node_user_id: &str,
    domain: &str,
    repo_ctx: Arc<RepositoryContext>,
) -> ServiceResult<(bool, bool)> {
    info!("Checking user and folder status for viewer connection");

    // Extract folder_id from UCAN token
    let folder_id = crate::ucan_service::extract_folder_id_from_viewer_token(viewer_ucan_token, domain)
        .await?;

    // Check if folder exists
    let folder_exists = repo_ctx.folder_repo.find_by_id(&folder_id).await.is_ok();

    // Check first_sync status
    let first_sync_done = repo_ctx
        .user_repo
        .get_user_by_id(node_user_id)
        .await
        .map(|user| user.first_sync)
        .unwrap_or(false);

    Ok((first_sync_done, folder_exists))
}

/// Get folder to send to viewer
///
/// TODO: Remove this - Use standard folder delegation via folder_service
pub async fn get_folder_to_send(
    folder_id: &str,
    node_user_id: &str,
    viewer_public_key: &str,
    domain: &str,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: Arc<RwLock<CryptoUtils>>,
) -> ServiceResult<(Folder, Vec<u8>, Vec<u8>, String)> {
    info!("Preparing folder {} for viewer", folder_id);

    // Get folder
    let folder = repo_ctx.folder_repo.find_by_id(folder_id).await?;

    // Issue delegated folder UCAN for viewer
    let (folder_ucan_token, _folder_ucan_cid) = crate::ucan_service::issue_delegated_folder_token(
        &folder.ucan,
        viewer_public_key,
        folder_id,
        "viewer",
        domain,
        repo_ctx.clone(),
        crypto_utils.clone(),
    )
    .await?;

    // TODO: Properly serialize FolderShareRecord
    let encrypted_data = vec![];
    let encrypted_key = vec![];

    Ok((folder, encrypted_data, encrypted_key, folder_ucan_token))
}

/// Prepare resource for viewer
///
/// TODO: Remove this - Use prepare_resource_for_peer() which already handles UCAN permissions
pub async fn prepare_resource_for_viewer(
    resource_id: &str,
    node_user_id: &str,
    viewer_public_key: &str,
    _domain: &str,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: Arc<RwLock<CryptoUtils>>,
) -> ServiceResult<(Vec<u8>, Vec<u8>, String)> {
    info!("Preparing resource {} for viewer", resource_id);

    // Get node user's share record to get their UCAN
    let node_share = repo_ctx
        .share_repo
        .find_by_resource_and_operation_and_user(resource_id, "share", node_user_id)
        .await?;

    let node_ucan = &node_share.ucan_token;

    // Get viewer user
    let viewer_user_id = crypto_utils::get_key_id(viewer_public_key)?;
    let viewer_user = repo_ctx.user_repo.get_user_by_id(&viewer_user_id).await?;

    // Issue viewer UCAN
    let (viewer_ucan, _cid) = crate::ucan_service::create_viewer_resource_token(
        node_ucan,
        &viewer_user.ucan_pub_key,
        resource_id,
        "",
        repo_ctx.clone(),
        crypto_utils.clone(),
    )
    .await?;

    // Use standard prepare_resource_for_peer with proper UCAN context
    let (encrypted_data, encrypted_key) =
        prepare_resource_for_peer(resource_id, node_ucan, &viewer_ucan, viewer_public_key, repo_ctx, &crypto_utils)
            .await?;

    Ok((encrypted_data, encrypted_key, viewer_ucan))
}

// =============================================================================
// Resource Transfer Operations
// =============================================================================

/// Prepare resource transfer
pub async fn prepare_resource_transfer(
    resource_id: &str,
    folder_ucan: &str,
    peer_ucan_pub_key: &str,
    domain: &str,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
) -> Result<(String, String, String, Vec<u8>, Vec<u8>), ResourceServiceError> {
    info!("Preparing resource transfer: {}", resource_id);

    // Get UCANs
    let (our_ucan, peer_ucan, cid) = get_resource_ucans_for_sync(
        resource_id,
        folder_ucan,
        peer_ucan_pub_key,
        domain,
        repo_ctx.clone(),
        crypto_utils,
    )
    .await?;

    // Prepare resource data
    let (encrypted_data, encrypted_key) = prepare_resource_for_peer(
        resource_id,
        &our_ucan,
        &peer_ucan,
        peer_ucan_pub_key,
        repo_ctx,
        crypto_utils,
    )
    .await?;

    Ok((our_ucan, peer_ucan, cid, encrypted_data, encrypted_key))
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

/// Accept resource from peer
pub async fn accept_resource_from_peer(
    _resource_id: &str,
    _encrypted_data: Vec<u8>,
    _encrypted_key: Vec<u8>,
    _ucan_token: String,
    _folder_id: String,
    _metadata: serde_json::Value,
    _repo_ctx: Arc<RepositoryContext>,
) -> Result<(), ResourceServiceError> {
    // TODO: Implement accept logic
    info!("Accepting resource from peer");
    Ok(())
}
