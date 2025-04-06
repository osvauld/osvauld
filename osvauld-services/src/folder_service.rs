// src/application/services/folder_service.rs
use osvauld_core::models::folder::Folder;
use osvauld_core::models::sync_record::{SyncRecord, SyncRecordSet};
use osvauld_core::repositories::{DeviceRepository, FolderRepository, RepositoryError};
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
    device_repository: Arc<dyn DeviceRepository>,
}

impl FolderService {
    pub fn new(
        folder_repository: Arc<dyn FolderRepository>,
        device_repository: Arc<dyn DeviceRepository>,
    ) -> Self {
        Self {
            folder_repository,
            device_repository,
        }
    }

    pub async fn create_folder(
        &self,
        name: String,
        description: Option<String>,
        current_device_id: &str,
        current_user_id: &str,
    ) -> Result<(Folder, SyncRecordSet), FolderServiceError> {
        //TODO: move to transaction
        // Validate input
        if name.trim().is_empty() {
            return Err(FolderServiceError::ValidationError(
                "Name cannot be empty".to_string(),
            ));
        }

        // Create folder
        let folder = Folder::new(name, description, false);
        let user_devices = self
            .device_repository
            .get_devices_by_user_id(current_user_id)
            .await?;

        let sync_record_set = SyncRecord::create_folder_sync_record(
            folder.id.clone(),
            current_device_id.to_string(),
            &user_devices,
        );

        // Save folder and sync record in a transaction

        Ok((folder, sync_record_set))
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

    pub async fn create_default_folder(
        &self,
        current_device_id: &str,
        current_user_id: &str,
    ) -> Result<(Folder, SyncRecordSet), FolderServiceError> {
        self.create_folder(
            "default".to_string(),
            None,
            current_device_id,
            current_user_id,
        )
        .await
    }
}
