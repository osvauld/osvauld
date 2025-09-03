use crate::{
    errors::{FolderServiceError, ServiceResult},
    prepare_share_resource,
};
use crypto_utils::{CryptoUtils, errors::UcanError};
use osvauld_core::models::{Folder, FolderShareRecord, PermissionLevel, User};
use persistance::database::RepositoryContext;
use std::sync::Arc;
use tokio::sync::Mutex;

pub async fn create_folder(
    name: String,
    description: Option<String>,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<Mutex<CryptoUtils>>,
    domain: &str,
    user: &User,
) -> ServiceResult<Folder> {
    // Validate input
    if name.trim().is_empty() {
        return Err(FolderServiceError::EmptyFolderName.into());
    }
    let folder = Folder::new(name, description, false);
    let encrypted_ucan_key = repo_ctx.store_repo.get_ucan_key().await?;
    let (folder_root_ucan_key, ucan_cid) = {
        let crypto = crypto_utils.lock().await;
        crypto
            .generate_folder_owner_ucan(&encrypted_ucan_key, &folder.id, domain)
            .await?
    };
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
    folder_permissions: Vec<(String, String)>,
    current_user: &User,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<Mutex<CryptoUtils>>,
    domain: &str,
) -> ServiceResult<()> {
    // 1. Validate folder exists and user has access
    let _folder = repo_ctx
        .folder_repo
        .find_by_id(folder_id)
        .await
        .map_err(|_| FolderServiceError::FolderNotFound {
            folder_id: folder_id.to_string(),
        })?;

    // 2. Check if folder is already shared with this user
    let existing_folder_shares = repo_ctx
        .folder_share_repo
        .get_records_by_folder_id(folder_id)
        .await?;

    let folder_already_shared = existing_folder_shares
        .iter()
        .any(|share| share.recipient_user_id == recipient_user_id);
    if folder_already_shared {
        return Ok(());
    }

    // 3. Prepare folder UCAN delegation if not already shared

    // Get current user's folder UCAN token (their own share record)
    let user_folder_ucan_token = existing_folder_shares
        .iter()
        .find(|share| share.recipient_user_id == current_user.id)
        .ok_or(FolderServiceError::InsufficientPermissions)?
        .ucan_token
        .clone();

    // Get encrypted UCAN private key
    let encrypted_ucan_key = repo_ctx.store_repo.get_ucan_key().await?;

    // Create proof resolver for folder UCAN validation
    let repo_ctx_clone = repo_ctx.clone();
    let proof_resolver = move |cid: &str| resolve_proof(repo_ctx_clone.clone(), cid.to_string());

    // Generate delegated folder UCAN
    let (folder_ucan_token, folder_ucan_cid) = {
        let crypto = crypto_utils.lock().await;
        crypto
            .issue_delegated_folder_ucan(
                &encrypted_ucan_key,
                &user_folder_ucan_token,
                &current_user.ucan_pub_key,
                folder_id,
                &repo_ctx
                    .user_repo
                    .get_user_by_id(recipient_user_id)
                    .await?
                    .ucan_pub_key,
                folder_permissions,
                domain,
                &proof_resolver,
            )
            .await?
    };

    // Create folder share record
    let folder_share_record = FolderShareRecord::prepare_folder_share_record(
        folder_id.to_string(),
        current_user.id.clone(),
        recipient_user_id.to_string(),
        PermissionLevel::Admin, // Full permissions for now
        folder_ucan_token,
        folder_ucan_cid,
    );

    // 4. Get all resources in folder and prepare sharing data
    let folder_resources = repo_ctx
        .resource_repo
        .find_all_by_folder(folder_id, &current_user.id)
        .await?;

    let mut all_resource_sharing_data = Vec::new();

    for resource_with_key in folder_resources {
        let resource_id = &resource_with_key.resource.id;

        // Create full resource permissions for each resource
        let resource_permissions = vec![
            (
                format!("{}:resource:{}", domain, resource_id),
                "crud/read".to_string(),
            ),
            (
                format!("{}:resource:{}", domain, resource_id),
                "crud/update".to_string(),
            ),
            (
                format!("{}:resource:{}", domain, resource_id),
                "ucan/share".to_string(),
            ),
        ];

        // Prepare sharing data - this will return None if already shared
        if let Some(sharing_data) = prepare_share_resource(
            recipient_user_id,
            resource_id,
            resource_permissions,
            current_user,
            repo_ctx.clone(),
            crypto_utils,
        )
        .await?
        {
            all_resource_sharing_data.push(sharing_data);
        }
    }

    // 6. Extract data for transaction
    let resource_keys: Vec<_> = all_resource_sharing_data
        .iter()
        .map(|d| d.resource_key.clone())
        .collect();
    let resource_share_records: Vec<_> = all_resource_sharing_data
        .iter()
        .map(|d| d.share_record.clone())
        .collect();
    let all_vector_clocks: Vec<_> = all_resource_sharing_data
        .iter()
        .flat_map(|d| d.vector_clocks.clone())
        .collect();

    // 7. Save everything in a single transaction
    repo_ctx
        .folder_share_repo
        .share_folder_transaction(
            &folder_share_record,
            &resource_keys,
            &resource_share_records,
            &all_vector_clocks,
        )
        .await?;

    Ok(())
}
/// Resolve proof function for folder sharing context
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
