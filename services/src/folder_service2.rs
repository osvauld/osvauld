use osvauld_core::models::Folder;
use osvauld_core::repositories::RepositoryError;
use persistance::database::RepositoryContext;
use std::sync::Arc;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum FolderServiceError {
    #[error("Repository error: {0}")]
    RepositoryError(#[from] RepositoryError),
    #[error("Invalid input: {0}")]
    ValidationError(String),
}
pub async fn create_folder(
    name: String,
    description: Option<String>,
    repo_ctx: Arc<RepositoryContext>,
) -> Result<Folder, FolderServiceError> {
    // Validate input
    if name.trim().is_empty() {
        return Err(FolderServiceError::ValidationError(
            "Name cannot be empty".to_string(),
        ));
    }

    // Create folder
    let folder = Folder::new(name, description, false);
    repo_ctx.folder_repo.save(&folder).await?;

    Ok(folder)
}

pub async fn get_all_folders(
    repo_ctx: Arc<RepositoryContext>,
) -> Result<Vec<Folder>, FolderServiceError> {
    repo_ctx
        .folder_repo
        .find_all()
        .await
        .map_err(FolderServiceError::RepositoryError)
}

pub async fn soft_delete_folder(
    folder_id: &str,
    repo_ctx: Arc<RepositoryContext>,
) -> Result<(), RepositoryError> {
    repo_ctx.folder_repo.soft_delete(folder_id).await
}

pub async fn create_default_folder(
    repo_ctx: Arc<RepositoryContext>,
) -> Result<Folder, FolderServiceError> {
    create_folder("default".to_string(), None, repo_ctx).await
}
