use crate::domains::models::{
    auth::Certificate, credential::Credential, device::Device, folder::Folder,
    sync_record::SyncRecord,
};
use async_trait::async_trait;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum RepositoryError {
    #[error("Database error: {0}")]
    DatabaseError(String),
    #[error("Not found")]
    NotFound,
}

#[async_trait]
pub trait FolderRepository: Send + Sync {
    async fn save(&self, folder: &Folder) -> Result<(), RepositoryError>;
    async fn find_all(&self) -> Result<Vec<Folder>, RepositoryError>;
    async fn find_by_id(&self, id: &str) -> Result<Folder, RepositoryError>;
    async fn soft_delete(&self, id: &str) -> Result<(), RepositoryError>;
}

#[async_trait]
pub trait SyncRepository: Send + Sync {
    async fn save_sync_records(&self, records: &[SyncRecord]) -> Result<(), RepositoryError>;
    async fn get_pending_records(
        &self,
        target_device_id: &str,
    ) -> Result<Vec<SyncRecord>, RepositoryError>;
    async fn update_status(&self, sync_id: &str, status: &str) -> Result<(), RepositoryError>;
    async fn get_sync_records_by_device_id(
        &self,
        target_device_id: &str,
    ) -> Result<Vec<SyncRecord>, RepositoryError>;
    async fn find_by_id(&self, sync_id: &str) -> Result<SyncRecord, RepositoryError>;
}

#[async_trait]
pub trait StoreRepository: Send + Sync {
    async fn store_certificate(
        &self,
        certificate: &Certificate,
        certificate_key: String,
        salt_key: String,
    ) -> Result<(), RepositoryError>;
    async fn get_certificate(
        &self,
        certificate_key: String,
        salt_key: String,
    ) -> Result<Certificate, RepositoryError>;
    async fn is_signed_up(&self) -> Result<bool, RepositoryError>;
    async fn store_device_key(&self, device_key: &str) -> Result<(), RepositoryError>;
    async fn get_device_key(&self) -> Result<String, RepositoryError>;
}

#[async_trait]
pub trait CredentialRepository: Send + Sync {
    async fn save(&self, credential: &Credential) -> Result<(), RepositoryError>;
    async fn find_by_folder(&self, folder_id: &str) -> Result<Vec<Credential>, RepositoryError>;
    async fn find_by_id(&self, id: &str) -> Result<Credential, RepositoryError>;
    async fn delete_credential(&self, id: &str) -> Result<(), RepositoryError>;
    async fn soft_delete_credential(&self, id: &str) -> Result<(), RepositoryError>;
    async fn toggle_fav(&self, id: &str) -> Result<(), RepositoryError>;
    async fn update_last_accessed(&self, id: &str) -> Result<(), RepositoryError>;
    async fn get_all_credentails(&self) -> Result<Vec<Credential>, RepositoryError>;
    async fn get_favourites(&self) -> Result<Vec<Credential>, RepositoryError>;
}

#[async_trait]

pub trait DeviceRepository: Send + Sync {
    async fn save(&self, device: Device) -> Result<(), RepositoryError>;
    async fn find_by_id(&self, device_id: &str) -> Result<Device, RepositoryError>;
    async fn get_all_devices(&self) -> Result<Vec<Device>, RepositoryError>;
    async fn udpate_last_synced_at(
        &self,
        device_id: &str,
        timestamp: i64,
    ) -> Result<(), RepositoryError>;
}
