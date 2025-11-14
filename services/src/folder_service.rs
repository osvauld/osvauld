use crate::{
    errors::{FolderServiceError, ServiceResult},
    resource_service,
};
use crypto_utils::{CryptoUtils, errors::UcanError};
use osvauld_core::models::{
    Folder, FolderShareRecord,
    PermissionLevel, User, ViewerFolderInfo,
};
use persistance::database::RepositoryContext;
use std::sync::Arc;
use tokio::sync::RwLock;

pub async fn create_folder(
    name: String,
    description: Option<String>,
    folder_template_json: String,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
    domain: &str,
    user: &User,
) -> ServiceResult<Folder> {
    // Validate input
    if name.trim().is_empty() {
        return Err(FolderServiceError::EmptyFolderName.into());
    }

    // Create folder with temporary empty UCAN (will be updated)
    let mut folder = Folder::new(name, description, false, String::new());

    // Generate owner UCAN for this folder using the generated folder ID
    let (folder_root_ucan_key, ucan_cid) = crate::ucan_service::issue_folder_owner_token(
        &folder.id,
        domain,
        &folder_template_json,
        crypto_utils,
        &repo_ctx,
    )
    .await?;

    // Update folder with the generated UCAN
    folder.ucan = folder_root_ucan_key.clone();
    let folder_share_record = FolderShareRecord::prepare_folder_share_record(
        folder.id.clone(),
        user.id.clone(),
        user.id.clone(),
        PermissionLevel::Admin,
        folder_root_ucan_key,
        ucan_cid,
    );
    repo_ctx
        .folder_repo
        .save_folder_with_share_record(&folder, &folder_share_record)
        .await?;

    Ok(folder)
}

pub async fn get_all_folders(repo_ctx: Arc<RepositoryContext>) -> ServiceResult<Vec<Folder>> {
    let folders = repo_ctx.folder_repo.find_all().await?;
    Ok(folders)
}

pub async fn soft_delete_folder(
    folder_id: &str,
    repo_ctx: Arc<RepositoryContext>,
) -> ServiceResult<()> {
    // Check if folder exists first
    let _folder = repo_ctx
        .folder_repo
        .find_by_id(folder_id)
        .await
        .map_err(|_| FolderServiceError::FolderNotFound {
            folder_id: folder_id.to_string(),
        })?;

    // TODO: Check if folder contains resources
    // This would require a method to check if folder has resources
    // For now, we'll proceed with deletion

    repo_ctx.folder_repo.soft_delete(folder_id).await?;
    Ok(())
}
/// Share a folder with a user
pub async fn share_folder(
    folder_id: &str,
    recipient_user_id: &str,
    recipient_role: &str,
    current_user: &User,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
    domain: &str,
) -> ServiceResult<()> {
    // 1. Validate folder exists
    let folder = repo_ctx
        .folder_repo
        .find_by_id(folder_id)
        .await
        .map_err(|_| FolderServiceError::Validation("Folder not found".into()))?;

    // 2. Validate recipient exists
    let recipient_user = repo_ctx
        .user_repo
        .get_user_by_id(recipient_user_id)
        .await
        .map_err(|_| FolderServiceError::Validation("Recipient not found".into()))?;

    // 3. Check if already shared (efficient query)
    if repo_ctx
        .folder_share_repo
        .find_by_folder_and_user(folder_id, recipient_user_id)
        .await?
        .is_some()
    {
        return Err(FolderServiceError::Validation(
            "Folder already shared with this user".into(),
        )
        .into());
    }

    // 4. Get current user's folder share record to get the folder owner token
    let current_user_folder_share = repo_ctx
        .folder_share_repo
        .find_by_folder_and_user(folder_id, &current_user.id)
        .await?
        .ok_or_else(|| FolderServiceError::Validation("Current user doesn't have access to folder".into()))?;

    // 6 & 7. Generate delegated folder UCAN for recipient using ucan_service
    let (folder_ucan_token, folder_ucan_cid) = crate::ucan_service::issue_delegated_folder_token(
        &current_user_folder_share.ucan_token,  // folder_owner_token
        &recipient_user.ucan_pub_key,            // peer_pub_key
        folder_id,                                // folder_id
        recipient_role,                           // role
        domain,                                   // domain
        repo_ctx.clone(),                         // repo_ctx
        crypto_utils.clone(),                     // crypto_utils
    )
    .await?;

    // 9. Create and save folder_share_record
    let folder_share_record = FolderShareRecord::prepare_folder_share_record(
        folder_id.to_string(),
        current_user.id.clone(),
        recipient_user_id.to_string(),
        PermissionLevel::Admin,
        folder_ucan_token.clone(),
        folder_ucan_cid.clone(),
    );

    repo_ctx
        .folder_share_repo
        .save(&folder_share_record)
        .await?;

    // 10. Get all resources in folder
    let resources = repo_ctx
        .resource_repo
        .find_all_by_folder(folder_id, &current_user.id)
        .await?;

    // 11. For each resource, share using resource_service with same role
    for resource in resources {
        resource_service::share_resource(
            &resource.id,
            recipient_user_id,
            recipient_role,
            current_user,
            domain,
            repo_ctx.clone(),
            crypto_utils,
        )
        .await?;
    }

    Ok(())
}

/// Resolve proof function for folder sharing context (used by other functions)
async fn resolve_proof(repo_ctx: Arc<RepositoryContext>, cid: String) -> Result<String, UcanError> {
    repo_ctx
        .folder_share_repo
        .get_ucan_by_cid(&cid)
        .await
        .map_err(|e| UcanError::ProofChainInvalid(e.to_string()))
}

pub async fn get_folder_shared_users(
    folder_id: &str,
    repo_ctx: Arc<RepositoryContext>,
) -> ServiceResult<Vec<User>> {
    Ok(repo_ctx
        .folder_share_repo
        .get_shared_users(folder_id)
        .await?)
}

pub async fn get_folder_by_id(
    folder_id: &str,
    repo_ctx: Arc<RepositoryContext>,
) -> ServiceResult<Folder> {
    Ok(repo_ctx.folder_repo.find_by_id(folder_id).await?)
}

pub async fn get_viewer_folder_manifest(
    peer_user_id: &str,
    repo_ctx: Arc<RepositoryContext>,
) -> ServiceResult<Vec<ViewerFolderInfo>> {
    // 1. Get folder share records for peer_user_id
    let folder_share_records = repo_ctx
        .folder_share_repo
        .get_records_by_recipient_user_id(peer_user_id)
        .await?;

    // 2. For each folder, get resource_ids
    let mut result = Vec::new();
    for folder_share_record in folder_share_records {
        let resource_ids = repo_ctx
            .resource_repo
            .get_resource_ids_by_folder_id(&folder_share_record.folder_id)
            .await?;

        result.push(ViewerFolderInfo {
            folder_id: folder_share_record.folder_id,
            resource_ids,
            folder_ucan: folder_share_record.ucan_token,
        });
    }

    Ok(result)
}

/// Get responder's folder UCAN for a folder
///
/// This function is used by the resource sync protocol to look up the responder's
/// folder UCAN when they receive a sync request from an initiator.
///
/// # Arguments
/// * `initiator_folder_ucan` - The initiator's folder UCAN token
/// * `local_user_id` - The responder's (local) user ID
/// * `domain` - Domain for UCAN validation
/// * `repo_ctx` - Database repository context
///
/// # Returns
/// * `String` - The responder's folder UCAN token
///
/// # Errors
/// * `FolderServiceError` - If folder_id extraction fails, folder share not found, or database error
pub async fn get_responder_folder_ucan_for_folder(
    initiator_folder_ucan: &str,
    local_user_id: &str,
    domain: &str,
    repo_ctx: Arc<RepositoryContext>,
) -> ServiceResult<String> {
    tracing::info!("Looking up responder's folder_ucan");

    // Extract folder_id from initiator's folder_ucan
    let folder_id = crate::ucan_service::extract_folder_id_with_add_resources(
        initiator_folder_ucan,
        domain
    )
    .await
    .map_err(|e| {
        tracing::error!("Failed to extract folder_id from initiator's folder_ucan: {}", e);
        FolderServiceError::UcanError(format!("No folder_id found in UCAN: {}", e))
    })?;

    tracing::info!("Looking up responder's folder_ucan for folder {}", folder_id);

    // Get responder's folder share record to get their folder_ucan
    let folder_share_option = repo_ctx
        .folder_share_repo
        .find_by_folder_and_user(&folder_id, local_user_id)
        .await?;

    let folder_share = folder_share_option.ok_or_else(|| {
        tracing::error!(
            "No folder share found for folder {} and user {}",
            folder_id, local_user_id
        );
        FolderServiceError::Validation(
            "Responder doesn't have access to folder".to_string()
        )
    })?;

    tracing::info!("✓ Found responder's folder_ucan");
    Ok(folder_share.ucan_token)
}

/// Accept and save a folder from a peer after validating add_folder capability
pub async fn accept_folder_from_peer(
    folder: &Folder,
    folder_share_record: &FolderShareRecord,
    peer_connection_token: &str,
    domain: &str,
    repo_ctx: Arc<RepositoryContext>,
) -> ServiceResult<()> {
    tracing::info!("📂 Accepting folder from peer:");
    tracing::info!("  - Folder ID: {}", folder.id);
    tracing::info!("  - Folder name: {}", folder.name);
    tracing::info!("  - Share record recipient: {}", folder_share_record.recipient_user_id);
    tracing::info!("  - Share record shared_by: {}", folder_share_record.shared_by_user_id);

    // Validate that peer has add_folder capability (UCAN-first: check capability, not role)
    tracing::info!("  Step 1: Validating peer connection token...");
    let peer_token = osvauld_core::models::ConnectionToken::from_token(peer_connection_token)
        .map_err(|e| FolderServiceError::Validation(format!("Invalid peer connection token: {}", e)))?;
    crate::ucan_service::validation::validate_peer_can_add_folder(&peer_token, domain).await?;

    // Validate folder share UCAN token structure
    tracing::info!("  Step 2: Validating folder share UCAN structure...");
    crate::ucan_service::validate_ucan_structure(&folder_share_record.ucan_token)
        .await
        .map_err(|e| {
            tracing::error!("❌ Invalid folder share UCAN: {}", e);
            FolderServiceError::Validation(format!("Invalid folder UCAN: {}", e))
        })?;
    tracing::info!("  ✓ Folder share UCAN structure valid");

    // Save folder and share record in transaction
    tracing::info!("  Step 3: Saving folder and share record to database...");
    repo_ctx
        .folder_repo
        .save_folder_with_share_record(folder, folder_share_record)
        .await?;

    tracing::info!("✅ Successfully accepted and saved folder {}", folder.id);
    Ok(())
}

/// Prepare folder data for viewer first connection
///
/// Loads the folder, issues a FolderViewer token, and creates a FolderShareRecord
/// so the viewer can reconnect. This is used during viewer's first handshake to
/// provide immediate access and enable future reconnections.
///
/// # Arguments
/// * `folder_id` - ID of folder to prepare
/// * `viewer_user_id` - Viewer's user ID (for share record)
/// * `node_user_id` - Node's user ID (owner of the folder, for share record)
/// * `viewer_ucan_pub_key` - Viewer's UCAN public key (for token audience)
/// * `crypto_utils` - Crypto utilities for token generation
/// * `repo_ctx` - Repository context for database access
/// * `domain` - Domain for UCAN validation
///
/// # Returns
/// * `Ok(())` - FolderShareRecord created successfully
pub async fn prepare_viewer_folder_data(
    folder_id: &str,
    viewer_user_id: &str,
    node_user_id: &str,
    viewer_ucan_pub_key: &str,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
    repo_ctx: &Arc<RepositoryContext>,
    domain: &str,
) -> ServiceResult<()> {
    // 1. Load folder from database
    let folder = repo_ctx
        .folder_repo
        .find_by_id(folder_id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to load folder {}: {}", folder_id, e);
            FolderServiceError::FolderNotFound {
                folder_id: folder_id.to_string(),
            }
        })?;

    tracing::info!("Loaded folder: {} ({})", folder.name, folder.id);

    // 2. Get node's folder share token (we need this to delegate from)
    let node_folder_share = repo_ctx
        .folder_share_repo
        .find_by_folder_and_user(folder_id, node_user_id)
        .await?
        .ok_or_else(|| {
            FolderServiceError::Validation(format!(
                "Node {} doesn't have access to folder {}",
                node_user_id, folder_id
            ))
        })?;

    // 3. Parse node's folder token for delegation
    use osvauld_core::models::FolderShareToken;
    let node_folder_token = FolderShareToken::from_token(&node_folder_share.ucan_token)
        .map_err(|e| FolderServiceError::UcanError(format!("Invalid folder token: {}", e)))?;

    // 4. Delegate FolderViewer token from node's token
    let viewer_folder_token = crate::ucan_service::delegate_folder_to_viewer(
        &node_folder_token,
        viewer_ucan_pub_key,
        folder_id,
        repo_ctx.clone(),
        crypto_utils.clone(),
    )
    .await
    .map_err(|e| {
        tracing::error!("Failed to delegate folder token to viewer: {}", e);
        FolderServiceError::UcanError(format!("Failed to issue FolderViewer token: {}", e))
    })?;

    let folder_ucan = viewer_folder_token.ucan().raw_token().to_string();
    let folder_ucan_cid = crate::ucan_service::get_cid(&folder_ucan)
        .map_err(|e| FolderServiceError::UcanError(format!("Failed to get CID: {}", e)))?;

    tracing::info!("✓ Issued FolderViewer token for folder {}", folder_id);

    // 3. Create and save FolderShareRecord so viewer can reconnect
    let folder_share_record = FolderShareRecord::prepare_folder_share_record(
        folder_id.to_string(),
        node_user_id.to_string(),      // shared_by (the node/owner)
        viewer_user_id.to_string(),     // recipient (the viewer)
        PermissionLevel::Admin,         // viewer gets admin access
        folder_ucan.clone(),
        folder_ucan_cid,
    );

    repo_ctx
        .folder_share_repo
        .save(&folder_share_record)
        .await
        .map_err(|e| {
            tracing::error!("Failed to save folder share record: {}", e);
            FolderServiceError::Validation(format!("Failed to save share record: {}", e))
        })?;

    tracing::info!("✓ Saved FolderShareRecord for viewer {}", viewer_user_id);

    Ok(())
}
