// src/application/services/folder_service.rs
use crate::domains::models::folder::Folder;
use crate::domains::repositories::{FolderRepository, RepositoryError};
use std::sync::Arc;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum FolderServiceError {
    #[error("Repository error: {0}")]
    RepositoryError(#[from] RepositoryError),
    #[error("Invalid input: {0}")]
    ValidationError(String),
}

pub struct FolderService {
    folder_repository: Arc<dyn FolderRepository>,
}

impl FolderService {
    pub fn new(folder_repository: Arc<dyn FolderRepository>) -> Self {
        Self { folder_repository }
    }

    pub async fn create_folder(
        &self,
        name: String,
        description: Option<String>,
    ) -> Result<Folder, FolderServiceError> {
        // Validate input
        if name.trim().is_empty() {
            return Err(FolderServiceError::ValidationError(
                "Name cannot be empty".to_string(),
            ));
        }

        // Create folder
        let folder = Folder::new(name, description);

        // Save folder and sync record in a transaction
        self.folder_repository.save(&folder).await?;

        Ok(folder)
    }

    pub async fn get_all_folders(&self) -> Result<Vec<Folder>, FolderServiceError> {
        self.folder_repository
            .find_all()
            .await
            .map_err(FolderServiceError::RepositoryError)
    }

    pub async fn soft_delete_folder(&self, folder_id: &str) -> Result<(), RepositoryError> {
        self.folder_repository.soft_delete(folder_id).await
    }
}
