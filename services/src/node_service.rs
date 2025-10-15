use crate::errors::ServiceResult;
use crypto_utils::{CryptoUtils, errors::UcanError};
use osvauld_core::models::{
    FolderShareRecord, FolderWithShareRecords, PermissionLevel, ResourceKey, ResourceSyncData,
    ShareOperation, ShareRecord, User,
};
use persistance::database::RepositoryContext;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

/// Generate a folder token for public access via sovereign node
/// This creates a connection string that viewers can use to access the folder
pub async fn generate_folder_token(
    folder_id: &str,
    domain: &str,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
    repo_ctx: Arc<RepositoryContext>,
) -> ServiceResult<(String, String)> {
    info!(
        "Generating folder token for folder {} in domain {}",
        folder_id, domain
    );

    // Get the encrypted UCAN private key from the store
    let encrypted_ucan_pvt_key = repo_ctx.store_repo.get_ucan_key().await?;

    let crypto = crypto_utils.read().await;

    // Generate the token with view capability
    let capability_prefix = format!("{}:folder:{}", domain, folder_id);

    let ucan_token = crypto
        .generate_public_folder_view_token(&encrypted_ucan_pvt_key, folder_id, &capability_prefix)
        .await?;

    // Get the public UCAN key
    let ucan_public_key = crypto.get_public_ucan_key(&encrypted_ucan_pvt_key).await?;

    info!(
        "Successfully generated folder token for folder {}",
        folder_id
    );

    Ok((ucan_token, ucan_public_key))
}

/// Prepare folder data for sending to a viewer
/// Creates a new folder share record for the viewer user with a read-only folder token
pub async fn prepare_folder_for_viewer(
    folder_id: &str,
    viewer_user: &User,
    local_user: &User,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
) -> ServiceResult<FolderWithShareRecords> {
    info!(
        "Preparing folder {} for viewer {}",
        folder_id, viewer_user.id
    );

    // Get the folder
    let folder = repo_ctx.folder_repo.find_by_id(folder_id).await?;

    // Get all folder share records
    let folder_share_records = repo_ctx
        .folder_share_repo
        .get_records_by_folder_id(folder_id)
        .await?;

    // Get the local user's folder share record (to delegate from)
    let local_folder_share_record = folder_share_records
        .iter()
        .find(|share| share.recipient_user_id == local_user.id)
        .ok_or(crate::errors::FolderServiceError::InsufficientPermissions)?
        .clone();

    // Find the folder owner (root authority) from self-share record
    let folder_owner_id = folder_share_records
        .iter()
        .find(|share| share.shared_by_user_id == share.recipient_user_id)
        .ok_or(crate::errors::FolderServiceError::InsufficientPermissions)?
        .recipient_user_id
        .clone();

    let folder_owner = repo_ctx.user_repo.get_user_by_id(&folder_owner_id).await?;

    info!(
        "Delegating from local user's folder token. Token CID: {}, Token: {}, Folder owner: {}",
        local_folder_share_record.ucan_cid,
        &local_folder_share_record.ucan_token[..50.min(local_folder_share_record.ucan_token.len())],
        folder_owner.id
    );

    // Get encrypted UCAN private key
    let encrypted_ucan_pvt_key = repo_ctx.store_repo.get_ucan_key().await?;

    // Prepare folder proof resolver
    let repo_ctx_clone = repo_ctx.clone();
    let proof_resolver = move |cid: &str| {
        info!("Proof resolver called for CID: {}", cid);
        let result = resolve_folder_proof(repo_ctx_clone.clone(), cid.to_string());
        result
    };

    // Generate a read-only folder token for the viewer
    info!("Generating read-only folder token for viewer");
    let folder_capability = format!("sthalam:folder:{}:read", folder_id);

    let (folder_ucan_token, folder_ucan_cid) = {
        let crypto = crypto_utils.read().await;
        crypto
            .issue_delegated_folder_ucan(
                &encrypted_ucan_pvt_key,
                &local_folder_share_record.ucan_token,
                &folder_owner.ucan_pub_key,  // Use folder owner's key, not local user's key
                folder_id,
                &viewer_user.ucan_pub_key,
                vec![(folder_capability, "crud/read".to_string())],
                &proof_resolver,
            )
            .await?
    };

    // Create a new folder share record for the viewer with the generated token
    let folder_share_record = FolderShareRecord::prepare_folder_share_record(
        folder_id.to_string(),
        local_user.id.clone(),
        viewer_user.id.clone(),
        PermissionLevel::Read,
        folder_ucan_token,
        folder_ucan_cid,
    );

    info!(
        "Created folder share record with read-only token for viewer {} on folder {}",
        viewer_user.id, folder_id
    );

    Ok(FolderWithShareRecords {
        folder,
        share_records: vec![folder_share_record],
    })
}

/// Prepare a resource for sending to a viewer
/// Creates new resource_key and share_record for the viewer with resource-specific permissions
pub async fn prepare_resource_for_viewer(
    resource_id: &str,
    resource_type: &str,
    viewer_user: &User,
    local_user: &User,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
) -> ServiceResult<ResourceSyncData> {
    info!(
        "Preparing resource {} (type: {}) for viewer {}",
        resource_id, resource_type, viewer_user.id
    );

    // Get the resource
    let resource_with_key = repo_ctx
        .resource_repo
        .find_by_id(resource_id, &local_user.id)
        .await?;

    // Get the local user's resource_key
    let local_resource_key = repo_ctx
        .resource_key_repo
        .find_by_resource_and_user(resource_id, &local_user.id)
        .await?;

    // Re-encrypt the key for the viewer's public key
    let viewer_encrypted_key = {
        let crypto = crypto_utils.read().await;
        crypto.encrypt_key_with_new_pub_key(
            &local_resource_key.encrypted_key,
            &viewer_user.public_key,
        )?
    };

    // Create new ResourceKey for viewer
    let viewer_resource_key = ResourceKey::new(
        resource_id.to_string(),
        viewer_user.id.clone(),
        viewer_encrypted_key,
        false, // Not the original key
    );

    // Get the local user's share record (to delegate from)
    let local_share_record = repo_ctx
        .share_repo
        .find_by_resource_and_operation_and_user(
            resource_id,
            &ShareOperation::Share.to_string(),
            &local_user.id,
        )
        .await?;

    // Get resource owner for validation
    let resource_owner = repo_ctx
        .resource_repo
        .find_owner_by_resource_id(resource_id)
        .await?;

    // Prepare resource proof resolver
    let repo_ctx_clone = repo_ctx.clone();
    let proof_resolver = move |cid: &str| resolve_resource_proof(repo_ctx_clone.clone(), cid.to_string());

    // Get encrypted UCAN private key
    let encrypted_ucan_pvt_key = repo_ctx.store_repo.get_ucan_key().await?;

    // Generate permissions for 3-doc architecture
    // All resources use blocksuite_doc for main content
    // thread_comments_doc for collaborative comments
    // form_submissions_doc for append-only submissions
    info!("Generating permissions for {} resource (3-doc architecture)", resource_type);
    let permissions = vec![
        // Main website content - read-only for viewer
        (
            format!("sthalam:resource:{}:blocksuite_doc", resource_id),
            "crud/read".to_string(),
        ),
        // Thread comments - bidirectional sync (collaborative)
        (
            format!("sthalam:resource:{}:thread_comments_doc", resource_id),
            "crud/write".to_string(),
        ),
        // Form submissions - append-only (viewer → sovereign, no updates back)
        (
            format!("sthalam:resource:{}:form_submissions_doc", resource_id),
            "crud/append".to_string(),
        ),
    ];

    let (ucan_token, ucan_cid) = {
        let crypto = crypto_utils.read().await;
        crypto
            .issue_delegated_resource_ucan(
                &encrypted_ucan_pvt_key,
                &local_share_record.ucan_token,
                &resource_owner.ucan_pub_key,
                resource_id,
                &viewer_user.ucan_pub_key,
                permissions,
                &proof_resolver,
            )
            .await?
    };

    // Create share record for viewer
    let viewer_share_record = ShareRecord::prepare_share_record(
        resource_id.to_string(),
        local_user.id.clone(),
        viewer_user.id.clone(),
        PermissionLevel::Write,
        ucan_token,
        ucan_cid,
    );

    // Get viewer's devices
    let viewer_devices = repo_ctx
        .device_repo
        .get_devices_by_user_id(&viewer_user.id)
        .await?;

    let viewer_device_ids: Vec<String> = viewer_devices.iter().map(|d| d.id.clone()).collect();

    // Create new vector clock entries for viewer's devices
    let vector_clocks =
        osvauld_core::models::ResourceVectorClock::create_entries_for_sharing(
            resource_id,
            &viewer_device_ids,
        );

    info!(
        "Prepared resource {} with new keys and {} vector clocks for viewer {}",
        resource_id,
        vector_clocks.len(),
        viewer_user.id
    );

    Ok(ResourceSyncData {
        resource: resource_with_key.resource,
        resource_keys: vec![viewer_resource_key],
        share_records: vec![viewer_share_record],
        vector_clocks,
    })
}

/// Resolve folder UCAN proof from folder_share_records
async fn resolve_folder_proof(repo_ctx: Arc<RepositoryContext>, cid: String) -> Result<String, UcanError> {
    repo_ctx
        .folder_share_repo
        .get_ucan_by_cid(&cid)
        .await
        .map_err(|e| UcanError::ProofChainInvalid(format!(
            "Folder UCAN with CID '{}' not found: {}",
            cid, e
        )))
}

/// Resolve resource UCAN proof from share_records
async fn resolve_resource_proof(repo_ctx: Arc<RepositoryContext>, cid: String) -> Result<String, UcanError> {
    repo_ctx
        .share_repo
        .get_ucan_by_cid(&cid)
        .await
        .map_err(|e| UcanError::ProofChainInvalid(format!(
            "Resource UCAN with CID '{}' not found: {}",
            cid, e
        )))
}
