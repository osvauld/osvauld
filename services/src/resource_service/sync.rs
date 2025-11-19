//! Synchronization operations for resources
//!
//! CRDT sync operations, state vectors, peer updates, and viewer operations
//!
//! This module consolidates:
//! - 13 sync functions from resource_service.rs
//! - 9 functions from merge_service.rs (refactored with SyncContext)
//! - 3 functions from website_service.rs (refactored with typed tokens)

use super::core::{self, load_and_decrypt_resource, encrypt_and_save_resource};
use crate::errors::{ResourceServiceError, ServiceError, ServiceResult};
use crypto_utils::CryptoUtils;
use gurkha::decision::SyncContext;
use log::{error, info};
use osvauld_core::models::{
    ShareRecord, User,
    p2p::{ResourceSyncRequestMsg, ResourceUpdateMsg},
    resource::{EncryptedResource, Resource},
};
use persistance::database::RepositoryContext;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

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
    ucan_service: &Arc<gurkha::UcanService>,
) -> Result<String, ResourceServiceError> {
    info!("Getting state vectors from UCAN token");

    // Load resource by UCAN
    let resource_id = ucan_service.extract_resource_id(ucan_token)?;
    let resource = core::load_and_decrypt_resource(&resource_id, repo_ctx, crypto_utils).await?;

    // TODO: Replace with gurkha::MergeService::generate_state_vector for each document
    // Get documents from resource and generate state vectors using UCAN-aware logic
    use std::collections::HashMap;
    let mut state_vectors = HashMap::new();

    for doc_name in resource.doc_names() {
        if let Some(doc) = resource.get_doc(&doc_name) {
            let state_vector = gurkha::MergeService::generate_state_vector(doc, ucan_token)
                .map_err(|e| {
                    ResourceServiceError::InvalidState(format!(
                        "Failed to generate state vector: {}",
                        e
                    ))
                })?;
            state_vectors.insert(doc_name.clone(), state_vector);
        }
    }

    // Serialize to JSON string
    serde_json::to_string(&state_vectors).map_err(|e| {
        ResourceServiceError::InvalidState(format!("Failed to serialize state vectors: {}", e))
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
    peer_state_vectors_json: String,
    _domain: &str,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
    ucan_service: &Arc<gurkha::UcanService>,
) -> Result<String, ResourceServiceError> {
    info!("Generating updates for peer");

    // Load resource
    let resource_id = ucan_service.extract_resource_id(ucan_token)?;
    let resource = core::load_and_decrypt_resource(&resource_id, repo_ctx, crypto_utils).await?;

    // TODO: Refactor to use gurkha::MergeService::filter_documents_for_peer
    // This function needs BOTH our UCAN and peer's UCAN for proper UCAN-aware filtering
    // Current signature only has our UCAN - need to update calling code to pass peer UCAN too

    // Temporary: Return error until proper refactor
    Err(ResourceServiceError::InvalidState(
        "generate_updates_for_peer needs refactoring to use gurkha::MergeService with both UCANs"
            .to_string(),
    ))
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
    peer_updates_json: String,
    _domain: &str,
    user: &User,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
    ucan_service: &Arc<gurkha::UcanService>,
) -> Result<(), ResourceServiceError> {
    info!("Applying peer updates");

    // Load resource
    let resource_id = ucan_service.extract_resource_id(ucan_token)?;
    let mut resource =
        core::load_and_decrypt_resource(&resource_id, repo_ctx.clone(), crypto_utils).await?;

    // TODO: Refactor to use gurkha::MergeService::apply_peer_updates
    // This function needs BOTH our UCAN and peer's UCAN for proper UCAN-aware filtering
    // Current signature only has our UCAN - need to update calling code to pass peer UCAN too

    // Temporary: Return error until proper refactor
    Err(ResourceServiceError::InvalidState(
        "apply_peer_updates needs refactoring to use gurkha::MergeService with both UCANs"
            .to_string(),
    ))
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
    ucan_service: &Arc<RwLock<gurkha::UcanService>>,
) -> Result<(String, String, String), ResourceServiceError> {
    info!("Getting resource UCANs for sync: {}", resource_id);
    info!("  Peer role: {}", peer_role);
    info!("  Current user: {}", current_user.id);

    // Get resource from database to get our UCAN and folder_id
    let encrypted_resource = repo_ctx.resource_repo.find_by_id(resource_id).await?;
    let our_ucan = encrypted_resource.ucan_token.clone();
    let folder_id = &encrypted_resource.folder_id;

    // Validate peer has folder access and get folder_id from their folder UCAN
    info!("  Validating peer folder UCAN");
    let ucan_service_guard = ucan_service.read().await;
    let peer_folder_id = ucan_service_guard
        .extract_folder_id(peer_folder_ucan)
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

    // Delegate resource UCAN based on peer's role
    info!("  Delegating resource UCAN by role: {}", peer_role);

    // Map role to template_key
    let template_key = match peer_role {
        "node" => "node",
        "owner" | "admin" => "user", // Admin roles use user template
        "viewer" => "viewer",
        _ => "user", // Default to user template for unknown roles
    };

    // Use unified delegation API
    let (peer_ucan, cid) = ucan_service_guard
        .delegate_resource(&our_ucan, template_key, peer_user.ucan_pub_key.as_str())
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

    // 🔍 LOG FULL PUBLIC KEY RECEIVED FOR RE-ENCRYPTION
    info!("🔑 [prepare_resource_for_peer] FULL peer PGP public key parameter:");
    info!("{}", peer_public_key);
    info!("🔑 [prepare_resource_for_peer] End of peer PGP public key parameter");

    // Load resource
    info!("🔓 [prepare_resource_for_peer] Loading and decrypting resource from database");
    let resource =
        core::load_and_decrypt_resource(resource_id, repo_ctx.clone(), crypto_utils).await?;
    info!("✅ [prepare_resource_for_peer] Successfully decrypted resource");

    // Create SyncContext
    let sync_context =
        SyncContext::new(our_ucan, peer_ucan).map_err(|e| ResourceServiceError::UcanError(e))?;

    // Filter and encrypt for peer (returns base64 strings)
    info!("🔐 [prepare_resource_for_peer] Re-encrypting resource with peer's public key");
    let result = core::filter_and_encrypt_for_peer(&resource, &sync_context, peer_public_key)
        .await
        .map_err(|e| match e {
            ServiceError::Resource(err) => err,
            _ => ResourceServiceError::InvalidState(format!("Unexpected error: {}", e)),
        })?;

    info!("✅ [prepare_resource_for_peer] Successfully re-encrypted resource for peer");
    info!("   Encrypted data length: {} bytes", result.0.len());
    info!("   Encrypted key length: {} bytes", result.1.len());

    Ok(result)
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
    ucan_service: &Arc<RwLock<gurkha::UcanService>>,
) -> Result<String, ResourceServiceError> {
    info!("Preparing resource sync request");

    // Load resource
    let ucan_service_guard = ucan_service.read().await;
    let resource_id = ucan_service_guard.extract_resource_id(ucan_token)?;
    drop(ucan_service_guard);
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
// Note: merge_service removed - all merge logic now in gurkha::MergeService
// Services call gurkha::MergeService directly for UCAN-aware CRDT operations
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
    ucan_service: &Arc<RwLock<gurkha::UcanService>>,
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
        ucan_service,
    )
    .await?;

    // Prepare resource data (filter and encrypt using peer's PGP public key)
    // Returns base64-encoded strings ready for EncryptedResource
    let peer_pgp_key = &peer_user.public_key;
    info!("🔐 Encrypting resource for peer");
    info!("  Peer user ID: {}", peer_user.id);
    info!(
        "  Peer PGP key (first 20 chars): {}...",
        &peer_pgp_key.chars().take(20).collect::<String>()
    );
    info!(
        "  Peer UCAN key (first 20 chars): {}...",
        &peer_user.ucan_pub_key.chars().take(20).collect::<String>()
    );
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
    let peer_encrypted_resource = crate::resource_service::core::create_encrypted_resource_struct(
        original_resource.id,
        original_resource.folder_id,
        encrypted_data, // Already base64-encoded
        encrypted_key,  // Already base64-encoded
        peer_ucan,      // Peer's delegated UCAN
        original_resource.metadata,
        Some((original_resource.created_at, original_resource.updated_at)), // Preserve original timestamps
    );

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
    repo_ctx: Arc<RepositoryContext>,
    ucan_service: &Arc<RwLock<gurkha::UcanService>>,
) -> ServiceResult<()> {
    info!("📥 Accepting resource {} from peer", resource.id);
    info!("  Folder: {}", resource.folder_id);
    info!("  Share records: {}", share_records.len());

    // 1. Validate folder_ucan has add_resources capability
    info!("  Step 1: Validating owner folder UCAN has add_resources capability");
    let ucan_service_guard = ucan_service.read().await;
    let folder_id = ucan_service_guard
        .extract_folder_id(folder_ucan)
        .map_err(|e| {
            error!("❌ Owner folder UCAN validation failed: {}", e);
            ResourceServiceError::UcanError(format!(
                "Owner folder UCAN doesn't have add_resources: {}",
                e
            ))
        })?;
    drop(ucan_service_guard);

    // 2. Verify the folder_id matches the resource's folder
    if folder_id != resource.folder_id {
        error!(
            "❌ Folder ID mismatch: UCAN folder {} vs resource folder {}",
            folder_id, resource.folder_id
        );
        return Err(ServiceError::Resource(ResourceServiceError::UcanError(
            format!(
                "Owner folder UCAN is for folder {} but resource is in folder {}",
                folder_id, resource.folder_id
            ),
        )));
    }

    info!(
        "  ✓ Owner has add_resources capability for folder {}",
        folder_id
    );

    // 3. Validate resource UCAN structure
    info!("  Step 2: Validating resource UCAN structure");
    gurkha::parser::Permit::from_token(&resource.ucan_token).map_err(|e| {
        error!("❌ Invalid resource UCAN: {}", e);
        ResourceServiceError::UcanError(format!("Invalid resource UCAN: {}", e))
    })?;

    info!("  ✓ Resource UCAN structure valid");

    // 4. Validate all share_record UCANs
    info!(
        "  Step 3: Validating {} share_record UCANs",
        share_records.len()
    );
    for (i, share_record) in share_records.iter().enumerate() {
        gurkha::parser::Permit::from_token(&share_record.ucan_token).map_err(|e| {
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

    info!(
        "✅ Successfully accepted resource {} with {} share_records",
        resource.id,
        share_records.len()
    );

    Ok(())
}

// ============================================================================
// Single Resource Sync Functions (Permit-Driven CRDT Merge)
// ============================================================================

/// Prepare sync request for a single resource (viewer initiates)
///
/// Viewer calls this to prepare ResourceSyncRequestMsg to send to node.
/// Uses dual-permit validation via Gurkha to determine what to send.
///
/// # Arguments
/// * `resource_id` - Resource to sync
/// * `peer_user_id` - Node's user ID (to get their permit from share_records)
/// * `current_user` - Current user (viewer)
/// * `repo_ctx` - Database context
/// * `crypto_utils` - Crypto utilities for decryption
///
/// # Returns
/// ResourceSyncRequestMsg ready to send to node
pub async fn prepare_single_resource_sync_request(
    resource_id: &str,
    peer_user_id: &str,
    current_user: &User,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
) -> ServiceResult<ResourceSyncRequestMsg> {
    info!("📤 Preparing sync request for resource {}", resource_id);

    // 1. Load and decrypt resource (contains our permit in resource.ucan_token)
    let resource = load_and_decrypt_resource(resource_id, repo_ctx.clone(), crypto_utils).await?;
    let our_permit = &resource.ucan_token;

    // 2. Get peer's permit directly from share repository
    let peer_permit = repo_ctx
        .share_repo
        .get_ucan_token_by_resource(resource_id, peer_user_id)
        .await
        .map_err(|e| {
            ResourceServiceError::UcanError(format!(
                "Failed to get peer permit for user {}: {}",
                peer_user_id, e
            ))
        })?;

    // 3. Use resource.docs (already loaded as LoroDoc instances)
    let documents = &resource.docs;

    // 4. Call Gurkha to prepare sync request
    let sync_data = gurkha::sync::prepare_sync_request(documents, our_permit, &peer_permit)
        .map_err(|e| ResourceServiceError::UcanError(e.to_string()))?;

    // 5. Serialize to JSON
    let state_vectors_json = serde_json::to_string(&sync_data.state_vectors)
        .map_err(|e| ResourceServiceError::DatabaseError(e.to_string()))?;
    let full_docs_json = serde_json::to_string(&sync_data.full_docs)
        .map_err(|e| ResourceServiceError::DatabaseError(e.to_string()))?;

    // 6. Collect asset IDs (TODO: implement asset tracking)
    let asset_ids = vec![];

    info!(
        "✅ Prepared sync request with {} state vectors, {} full docs",
        sync_data.state_vectors.len(),
        sync_data.full_docs.len()
    );

    Ok(ResourceSyncRequestMsg {
        resource_id: resource_id.to_string(),
        sender_permit: our_permit.clone(),
        state_vectors: state_vectors_json,
        full_docs: full_docs_json,
        asset_ids,
    })
}

/// Process incoming sync request and generate response (node receives from viewer)
///
/// Node calls this when receiving ResourceSyncRequestMsg from viewer.
/// Applies peer's full_docs, generates diff updates based on peer's state vectors.
///
/// # Arguments
/// * `resource_id` - Resource being synced
/// * `peer_permit` - Viewer's permit (from message)
/// * `peer_state_vectors_json` - Viewer's state vectors (JSON)
/// * `peer_full_docs_json` - Viewer's full documents (JSON)
/// * `current_user` - Current user (node)
/// * `repo_ctx` - Database context
/// * `crypto_utils` - Crypto utilities
///
/// # Returns
/// UpdatesResponse message ready to send back to viewer
pub async fn process_single_resource_sync_request(
    resource_id: &str,
    peer_permit: &str,
    peer_state_vectors_json: &str,
    peer_full_docs_json: &str,
    current_user: &User,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
) -> ServiceResult<ResourceUpdateMsg> {
    info!("📥 Processing sync request for resource {}", resource_id);

    // 1. Load and decrypt resource
    let mut resource =
        load_and_decrypt_resource(resource_id, repo_ctx.clone(), crypto_utils).await?;
    let our_permit = &resource.ucan_token;

    // 2. Use resource.docs (already loaded)
    let documents = &mut resource.docs;

    // 3. Parse peer's state vectors and full_docs
    let peer_state_vectors: HashMap<String, Vec<u8>> =
        serde_json::from_str(peer_state_vectors_json).map_err(|e| {
            ResourceServiceError::UcanError(format!("Invalid state vectors: {}", e))
        })?;
    let peer_full_docs: HashMap<String, Vec<u8>> = serde_json::from_str(peer_full_docs_json)
        .map_err(|e| ResourceServiceError::UcanError(format!("Invalid full docs: {}", e)))?;

    // 4. Apply peer's full documents (viewer submissions)
    if !peer_full_docs.is_empty() {
        let full_docs_count = peer_full_docs.len();
        gurkha::sync::apply_peer_docs(documents, our_permit, peer_permit, peer_full_docs)
            .map_err(|e| ResourceServiceError::UcanError(e.to_string()))?;
        info!(
            "✓ Applied {} full documents from peer",
            full_docs_count
        );
    }

    // 5. Generate sync response (diff updates based on peer's state vectors)
    let sync_response = gurkha::sync::generate_sync_response(
        documents,
        our_permit,
        peer_permit,
        &peer_state_vectors,
    )
    .map_err(|e| ResourceServiceError::UcanError(e.to_string()))?;

    // 6. Save updated resource using helper
    encrypt_and_save_resource(&resource, &current_user.public_key, repo_ctx.clone()).await?;

    // 7. Serialize response
    let updates_json = serde_json::to_string(&sync_response.updates)
        .map_err(|e| ResourceServiceError::DatabaseError(e.to_string()))?;
    let state_vectors_json = serde_json::to_string(&sync_response.state_vectors)
        .map_err(|e| ResourceServiceError::DatabaseError(e.to_string()))?;

    info!(
        "✅ Generated sync response with {} updates",
        sync_response.updates.len()
    );

    Ok(ResourceUpdateMsg::UpdatesResponse {
        resource_id: resource_id.to_string(),
        sender_permit: our_permit.clone(),
        updates: updates_json,
        state_vectors: state_vectors_json,
        assets: "{}".to_string(), // TODO: implement assets
    })
}

/// Apply updates and generate collaborative response (viewer receives updates from node)
///
/// Viewer calls this after receiving UpdatesResponse from node.
/// Applies node's updates, then immediately generates collaborative updates to send back.
///
/// # Arguments
/// * `resource_id` - Resource being synced
/// * `peer_permit` - Node's permit (from message)
/// * `peer_updates_json` - Node's updates (JSON)
/// * `peer_state_vectors_json` - Node's current state vectors (JSON)
/// * `current_user` - Current user (viewer)
/// * `repo_ctx` - Database context
/// * `crypto_utils` - Crypto utilities
///
/// # Returns
/// Some(UpdatesResponse) if we have collaborative updates, None otherwise
pub async fn apply_updates_and_generate_collaborative_response(
    resource_id: &str,
    peer_permit: &str,
    peer_updates_json: &str,
    peer_state_vectors_json: &str,
    current_user: &User,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
) -> ServiceResult<Option<ResourceUpdateMsg>> {
    info!(
        "🔄 Applying updates and generating collaborative response for resource {}",
        resource_id
    );

    // 1. Load and decrypt resource
    info!("📂 [apply_updates_and_generate_collaborative_response] Loading resource {}", resource_id);
    let mut resource =
        load_and_decrypt_resource(resource_id, repo_ctx.clone(), crypto_utils).await?;
    let our_permit = &resource.ucan_token;
    info!("📂 [apply_updates_and_generate_collaborative_response] Resource loaded with {} documents", resource.docs.len());

    // 2. Use resource.docs (already loaded)
    let documents = &mut resource.docs;

    // Log initial document states
    for (doc_name, doc) in documents.iter() {
        let state = gurkha::MergeService::state_frontiers(doc);
        info!("📊 [apply_updates_and_generate_collaborative_response] Document '{}' state BEFORE merge: {} bytes", doc_name, state.len());

        // NEW: Log content BEFORE merge
        gurkha::MergeService::log_doc_content(doc, doc_name, "apply_updates_and_generate_collaborative_response BEFORE merge");
    }

    // 3. Parse peer's updates and state vectors
    let peer_updates: HashMap<String, Vec<u8>> = serde_json::from_str(peer_updates_json)
        .map_err(|e| ResourceServiceError::UcanError(format!("Invalid updates: {}", e)))?;
    let peer_state_vectors: HashMap<String, Vec<u8>> =
        serde_json::from_str(peer_state_vectors_json).map_err(|e| {
            ResourceServiceError::UcanError(format!("Invalid state vectors: {}", e))
        })?;

    info!("📦 [apply_updates_and_generate_collaborative_response] Parsed {} peer updates and {} state vectors", peer_updates.len(), peer_state_vectors.len());

    // 4. Apply peer's updates
    gurkha::sync::apply_peer_updates(documents, our_permit, peer_permit, peer_updates)
        .map_err(|e| ResourceServiceError::UcanError(e.to_string()))?;
    info!("✓ Applied peer updates");

    // Log document states after merge
    for (doc_name, doc) in documents.iter() {
        let state = gurkha::MergeService::state_frontiers(doc);
        info!("📊 [apply_updates_and_generate_collaborative_response] Document '{}' state AFTER merge: {} bytes", doc_name, state.len());

        // NEW: Log content AFTER merge
        gurkha::MergeService::log_doc_content(doc, doc_name, "apply_updates_and_generate_collaborative_response AFTER merge");
    }

    // 5. Generate collaborative updates (if we have any)
    let collaborative_response = gurkha::sync::generate_collaborative_updates(
        documents,
        our_permit,
        peer_permit,
        &peer_state_vectors,
    )
    .map_err(|e| ResourceServiceError::UcanError(e.to_string()))?;

    // NEW: Log content just BEFORE save (to verify mutation)
    info!("🔍 [apply_updates_and_generate_collaborative_response] Verifying document content BEFORE save...");
    for (doc_name, doc) in documents.iter() {
        gurkha::MergeService::log_doc_content(doc, doc_name, "apply_updates_and_generate_collaborative_response BEFORE save");
    }

    // 6. Save updated resource using helper
    info!("💾 [apply_updates_and_generate_collaborative_response] Saving resource {} to database", resource_id);
    encrypt_and_save_resource(&resource, &current_user.public_key, repo_ctx.clone()).await?;
    info!("✅ [apply_updates_and_generate_collaborative_response] Resource {} saved successfully to database", resource_id);

    // NEW: Post-save verification - reload and check
    info!("🔍 [apply_updates_and_generate_collaborative_response] POST-SAVE VERIFICATION: Reloading resource to confirm persistence...");
    let reloaded_resource = load_and_decrypt_resource(resource_id, repo_ctx.clone(), crypto_utils).await?;
    for (doc_name, doc) in reloaded_resource.docs.iter() {
        gurkha::MergeService::log_doc_content(doc, doc_name, "apply_updates_and_generate_collaborative_response POST-SAVE (reloaded)");
    }

    // 7. If we have collaborative updates, serialize and return
    if let Some(sync_data) = collaborative_response {
        let updates_json = serde_json::to_string(&sync_data.updates)
            .map_err(|e| ResourceServiceError::DatabaseError(e.to_string()))?;
        let state_vectors_json = serde_json::to_string(&sync_data.state_vectors)
            .map_err(|e| ResourceServiceError::DatabaseError(e.to_string()))?;

        info!(
            "✅ Generated {} collaborative updates",
            sync_data.updates.len()
        );

        Ok(Some(
            ResourceUpdateMsg::UpdatesResponse {
                resource_id: resource_id.to_string(),
                sender_permit: our_permit.clone(),
                updates: updates_json,
                state_vectors: state_vectors_json,
                assets: "{}".to_string(), // TODO: implement assets
            },
        ))
    } else {
        info!("✅ No collaborative updates to send");
        Ok(None)
    }
}

/// Apply final collaborative updates (node receives final updates from viewer)
///
/// Node calls this to apply viewer's collaborative updates (final merge).
///
/// # Arguments
/// * `resource_id` - Resource being synced
/// * `peer_permit` - Viewer's permit (from message)
/// * `peer_updates_json` - Viewer's collaborative updates (JSON)
/// * `current_user` - Current user (node)
/// * `repo_ctx` - Database context
/// * `crypto_utils` - Crypto utilities
pub async fn apply_final_collaborative_updates(
    resource_id: &str,
    peer_permit: &str,
    peer_updates_json: &str,
    current_user: &User,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
) -> ServiceResult<()> {
    info!(
        "🏁 Applying final collaborative updates for resource {}",
        resource_id
    );

    // 1. Load and decrypt resource
    info!("📂 [apply_final_collaborative_updates] Loading resource {}", resource_id);
    let mut resource =
        load_and_decrypt_resource(resource_id, repo_ctx.clone(), crypto_utils).await?;
    let our_permit = &resource.ucan_token;
    info!("📂 [apply_final_collaborative_updates] Resource loaded with {} documents", resource.docs.len());

    // 2. Use resource.docs (already loaded)
    let documents = &mut resource.docs;

    // Log initial document states
    for (doc_name, doc) in documents.iter() {
        let state = gurkha::MergeService::state_frontiers(doc);
        info!("📊 [apply_final_collaborative_updates] Document '{}' state BEFORE merge: {} bytes", doc_name, state.len());

        // NEW: Log content BEFORE merge
        gurkha::MergeService::log_doc_content(doc, doc_name, "apply_final_collaborative_updates BEFORE merge");
    }

    // 3. Parse peer's updates
    let peer_updates: HashMap<String, Vec<u8>> = serde_json::from_str(peer_updates_json)
        .map_err(|e| ResourceServiceError::UcanError(format!("Invalid updates: {}", e)))?;

    info!("📦 [apply_final_collaborative_updates] Parsed {} peer collaborative updates", peer_updates.len());

    // 4. Apply peer's collaborative updates
    gurkha::sync::apply_peer_updates(documents, our_permit, peer_permit, peer_updates)
        .map_err(|e| ResourceServiceError::UcanError(e.to_string()))?;

    // Log document states after merge
    for (doc_name, doc) in documents.iter() {
        let state = gurkha::MergeService::state_frontiers(doc);
        info!("📊 [apply_final_collaborative_updates] Document '{}' state AFTER merge: {} bytes", doc_name, state.len());

        // NEW: Log content AFTER merge
        gurkha::MergeService::log_doc_content(doc, doc_name, "apply_final_collaborative_updates AFTER merge");
    }

    // NEW: Log content just BEFORE save (to verify mutation)
    info!("🔍 [apply_final_collaborative_updates] Verifying document content BEFORE save...");
    for (doc_name, doc) in documents.iter() {
        gurkha::MergeService::log_doc_content(doc, doc_name, "apply_final_collaborative_updates BEFORE save");
    }

    // 5. Save updated resource using helper
    info!("💾 [apply_final_collaborative_updates] Saving resource {} to database", resource_id);
    encrypt_and_save_resource(&resource, &current_user.public_key, repo_ctx.clone()).await?;
    info!("✅ [apply_final_collaborative_updates] Resource {} saved successfully to database", resource_id);

    // NEW: Post-save verification - reload and check
    info!("🔍 [apply_final_collaborative_updates] POST-SAVE VERIFICATION: Reloading resource to confirm persistence...");
    let reloaded_resource = load_and_decrypt_resource(resource_id, repo_ctx.clone(), crypto_utils).await?;
    for (doc_name, doc) in reloaded_resource.docs.iter() {
        gurkha::MergeService::log_doc_content(doc, doc_name, "apply_final_collaborative_updates POST-SAVE (reloaded)");
    }

    info!("✅ Final collaborative updates applied successfully");

    Ok(())
}
