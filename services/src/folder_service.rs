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
    let encrypted_ucan_key = repo_ctx.store_repo.get_ucan_key().await?;
    let (folder_root_ucan_key, ucan_cid) = {
        let crypto = crypto_utils.read().await;
        crypto
            .generate_folder_owner_ucan(&encrypted_ucan_key, &folder.id, domain)
            .await?
    };

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
    _folder_permissions: Vec<(String, String)>,
    current_user: &User,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
    domain: &str,
) -> ServiceResult<()> {
    // 1. Validate folder exists
    let _folder = repo_ctx
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

    // 4. Get owner's encrypted UCAN key
    let encrypted_ucan_key = repo_ctx.store_repo.get_ucan_key().await?;

    // 5. Generate folder UCAN for recipient using provided capabilities
    let folder_capabilities: Vec<&str> = _folder_permissions
        .iter()
        .map(|(_, ability)| ability.as_str())
        .collect();

    // Decrypt UCAN keys and generate folder token
    let (signing_key, verifying_key) = {
        let crypto = crypto_utils.read().await;
        crypto.decrypt_ucan_key(&encrypted_ucan_key)?
    };

    let folder_ucan_token = crypto_utils::ucan_utils::generate_flexible_folder_token(
        &signing_key,
        &verifying_key,
        folder_id,
        domain,
        None, // 30 year expiry
        folder_capabilities,
        &recipient_user.ucan_pub_key,
        "node", // role
    )
    .await
    .map_err(|e| FolderServiceError::Validation(e.to_string()))?;

    // 6. Generate CID from folder UCAN token
    let folder_ucan_cid = crypto_utils::get_cid_from_ucan_token(&folder_ucan_token)?;

    // 7. Create and save folder_share_record
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

    // 8. Get all resources in folder
    let resources = repo_ctx
        .resource_repo
        .find_all_by_folder(folder_id, &current_user.id)
        .await?;

    // 9. For each resource, share using resource_service (cleaner abstraction)
    for resource in resources {
        resource_service::share_resource(
            &resource.id,
            recipient_user_id,
            "owner", // Node gets owner_template (full permissions)
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

/// Accept and save a folder from a peer after validating add_folder capability
pub async fn accept_folder_from_peer(
    folder: &Folder,
    folder_share_record: &FolderShareRecord,
    peer_connection_token: &str,
    domain: &str,
    repo_ctx: Arc<RepositoryContext>,
) -> ServiceResult<()> {
    // Validate that peer has add_folder capability
    crate::validate_peer_can_add_folder(peer_connection_token, domain).await?;

    // Validate folder share UCAN token structure
    crypto_utils::ucan_utils::validate_structure(&folder_share_record.ucan_token)
        .await
        .map_err(|e| FolderServiceError::Validation(format!("Invalid folder UCAN: {}", e)))?;

    // Save folder and share record in transaction
    repo_ctx
        .folder_repo
        .save_folder_with_share_record(folder, folder_share_record)
        .await?;

    Ok(())
}
