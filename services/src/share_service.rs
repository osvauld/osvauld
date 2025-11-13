//! Share service - handles folder and resource share records

use crate::errors::{FolderServiceError, ResourceServiceError, ServiceResult};
use osvauld_core::models::{FolderShareRecord, ShareOperation, ShareRecord};
use persistance::database::RepositoryContext;
use std::sync::Arc;

/// Get folder share record for a user
pub async fn get_folder_share_record(
    folder_id: &str,
    user_id: &str,
    repo_ctx: Arc<RepositoryContext>,
) -> ServiceResult<FolderShareRecord> {
    repo_ctx
        .folder_share_repo
        .find_by_folder_and_user(folder_id, user_id)
        .await?
        .ok_or_else(|| {
            FolderServiceError::Validation(format!(
                "Folder share record not found for folder {} and user {}",
                folder_id, user_id
            ))
            .into()
        })
}

/// Get all folder share records for a folder (all users with access)
pub async fn get_all_folder_share_records(
    _folder_id: &str,
    _repo_ctx: Arc<RepositoryContext>,
) -> ServiceResult<Vec<FolderShareRecord>> {
    // TODO: Implement when we need folder sync
    // let share_records = repo_ctx
    //     .folder_share_repo
    //     .find_by_folder(folder_id)
    //     .await?;
    //
    // Ok(share_records)
    todo!()
}

/// Get resource share record for a user
pub async fn get_resource_share_record(
    resource_id: &str,
    user_id: &str,
    repo_ctx: Arc<RepositoryContext>,
) -> ServiceResult<ShareRecord> {
    repo_ctx
        .share_repo
        .find_by_resource_and_operation_and_user(
            resource_id,
            &ShareOperation::Share.to_string(),
            user_id,
        )
        .await
        .map_err(|_e| ResourceServiceError::ShareRecordNotFound.into())
}

/// Get all resource share records for a folder (bulk operation)
pub async fn get_resource_share_records_for_folder(
    folder_id: &str,
    user_id: &str,
    repo_ctx: Arc<RepositoryContext>,
) -> ServiceResult<Vec<ShareRecord>> {
    // Get all resource IDs in the folder
    let resource_ids = repo_ctx
        .resource_repo
        .get_resource_ids_by_folder_id(folder_id)
        .await?;

    if resource_ids.is_empty() {
        return Ok(Vec::new());
    }

    // Get all share records for those resources (bulk query)
    let share_records = repo_ctx
        .share_repo
        .find_by_resources_and_user(&resource_ids, user_id, &ShareOperation::Share.to_string())
        .await?;

    Ok(share_records)
}

/// Get all share records for a specific resource
pub async fn get_all_share_records_for_resource(
    resource_id: &str,
    repo_ctx: Arc<RepositoryContext>,
) -> ServiceResult<Vec<ShareRecord>> {
    let share_records = repo_ctx
        .share_repo
        .find_by_resource_and_operation(resource_id, &ShareOperation::Share.to_string())
        .await?;

    Ok(share_records)
}
