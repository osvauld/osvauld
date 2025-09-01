use crate::error::{FolderServiceError, ServiceResult};
use osvauld_core::models::Folder;
use persistance::database::RepositoryContext;
use std::sync::Arc;

pub async fn create_folder(
    name: String,
    description: Option<String>,
    repo_ctx: Arc<RepositoryContext>,
) -> ServiceResult<Folder> {
    // Validate input
    if name.trim().is_empty() {
        return Err(FolderServiceError::EmptyFolderName.into());
    }

    // Create folder
    let folder = Folder::new(name, description, false);
    repo_ctx.folder_repo.save(&folder).await?;

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

pub async fn create_default_folder(repo_ctx: Arc<RepositoryContext>) -> ServiceResult<Folder> {
    let folder = create_folder("default".to_string(), None, repo_ctx).await?;
    Ok(folder)
}
