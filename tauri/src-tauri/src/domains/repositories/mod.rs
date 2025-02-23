use crate::domains::models::{
    auth::Certificate,
    credential::Credential,
    device::Device,
    folder::Folder,
    sync_record::{
        DeviceRecord, DeviceRecordSet, DeviceRecordStatus, InitialDeviceSyncSet, StatusChangeSet,
        SyncRecord, SyncRecordSet,
    },
};
use async_trait::async_trait;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum RepositoryError {
    #[error("Database error: {0}")]
    DatabaseError(String),
    #[error("Not found")]
    NotFound,
    #[error("Custom error: {0}")]
    CustomError(String),
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
    async fn add_sync_record_set(&self, record_set: SyncRecordSet) -> Result<(), RepositoryError>;
    async fn get_all_sync_records(&self) -> Result<Vec<SyncRecord>, RepositoryError>;
    async fn add_status_change_set(
        &self,
        status_set: StatusChangeSet,
    ) -> Result<(), RepositoryError>;
    async fn update_device_record(
        &self,
        device_id: String,
        sync_id: String,
    ) -> Result<(), RepositoryError>;

    async fn get_pending_sync_by_type(
        &self,
        device_id: &str,
        resource_type: &str,
    ) -> Result<Option<(SyncRecord, Vec<DeviceRecord>, Vec<DeviceRecordStatus>)>, RepositoryError>;
    // Status Updates
    async fn get_unsynced_device_sync_records(
        &self,
        device_id: &str,
    ) -> Result<Vec<(DeviceRecord, Vec<DeviceRecordStatus>)>, RepositoryError>;
    async fn update_sync_status(
        &self,
        device_record_ids: Vec<String>,
        status_record_ids: Vec<String>,
    ) -> Result<(), RepositoryError>;
    async fn update_device_sync_record_status(
        &self,
        device_sync_record_status: String,
    ) -> Result<(), RepositoryError>;
    async fn update_device_record_set(
        &self,
        record_set: DeviceRecordSet,
    ) -> Result<(), RepositoryError>;
    async fn update_device_sync_record_by_device_id(
        &self,
        device_record_ids: Vec<String>,
        synced_device_id: String,
    ) -> Result<(), RepositoryError>;
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
    async fn find_all_by_folder(&self, folder_id: &str)
        -> Result<Vec<Credential>, RepositoryError>;
    async fn find_by_id(&self, id: &str) -> Result<Credential, RepositoryError>;
    async fn delete_credential(&self, id: &str) -> Result<(), RepositoryError>;
    async fn soft_delete_credential(&self, id: &str) -> Result<(), RepositoryError>;
    async fn toggle_fav(&self, id: &str) -> Result<(), RepositoryError>;
    async fn update_last_accessed(&self, id: &str) -> Result<(), RepositoryError>;
    async fn get_all_credentails(&self) -> Result<Vec<Credential>, RepositoryError>;
    async fn get_favourites(&self) -> Result<Vec<Credential>, RepositoryError>;
    async fn update_credential(&self, credential: &Credential) -> Result<(), RepositoryError>;
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

    async fn get_devices_except(
        &self,
        exclude_ids: &[String],
    ) -> Result<Vec<Device>, RepositoryError>;
}

#[async_trait]
pub trait DeviceRecordRepository: Send + Sync {
    async fn add_records(&self, record: DeviceRecord) -> Result<(), RepositoryError>;
}

#[async_trait]
pub trait DeviceRecordStatusRepository: Send + Sync {
    async fn add_device_records(&self, records: DeviceRecordStatus) -> Result<(), RepositoryError>;
}
