use crate::models::{
    auth::Certificate,
    device::Device,
    folder::Folder,
    resource::{Resource, ResourceKeyPair, ResourceWithKey},
    resource_key::ResourceKey,
    share_record::{
        ShareRecord, ShareRecordSet, ShareStatusChangeSet, UserRecord, UserRecordSet,
        UserRecordStatus,
    },
    sync_record::{
        DeviceRecord, DeviceRecordSet, DeviceRecordStatus, StatusChangeSet, SyncRecord,
        SyncRecordSet,
    },
    user::User,
    vector_clock::ResourceVectorClock,
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
    async fn add_sync_record_set(&self, record_set: &SyncRecordSet) -> Result<(), RepositoryError>;
    async fn get_all_sync_records(&self) -> Result<Vec<SyncRecord>, RepositoryError>;
    async fn add_status_change_set(
        &self,
        status_set: &StatusChangeSet,
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
    async fn get_sync_records_by_resource_and_operation(
        &self,
        resource_id: &str,
        operation_type: &str,
    ) -> Result<Vec<SyncRecord>, RepositoryError>;
    async fn get_device_records_by_sync_id(
        &self,
        sync_id: &str,
    ) -> Result<Vec<DeviceRecord>, RepositoryError>;
    async fn get_users_with_unsynced_devices(&self) -> Result<Vec<Device>, RepositoryError>;
    async fn update_device_record_statuses_for_sync(
        &self,
        sync_record_id: String,
        device_id: String,
    ) -> Result<(), RepositoryError>;
    async fn get_resource_ids_for_device(
        &self,
        device_id: &str,
    ) -> Result<Vec<String>, RepositoryError>;

    async fn update_device_status_record_for_device(
        &self,
        device_id: &str,
        device_record_id: &str,
    ) -> Result<(), RepositoryError>;

    async fn update_device_sync_status_by_ids(
        &self,
        device_record_ids: Vec<String>,
        device_record_status_ids: Vec<String>,
    ) -> Result<(), RepositoryError>;

    async fn get_device_records_and_statuses_by_sync_record(
        &self,
        sync_record_id: &str,
    ) -> Result<(Vec<DeviceRecord>, Vec<DeviceRecordStatus>), RepositoryError>;

    async fn get_sync_record_by_id(
        &self,
        sync_id: &str,
    ) -> Result<Option<SyncRecord>, RepositoryError>;

    async fn add_device_records_bulk(
        &self,
        records: &[DeviceRecord],
    ) -> Result<(), RepositoryError>;

    /// Adds multiple device record statuses in a single operation
    async fn add_device_record_statuses_bulk(
        &self,
        statuses: &[DeviceRecordStatus],
    ) -> Result<(), RepositoryError>;

    /// Updates the 'synced' flag to true for multiple device records by their IDs
    async fn update_device_records_synced_bulk(
        &self,
        record_ids: &[String],
    ) -> Result<(), RepositoryError>;

    /// Updates the 'synced' flag to true for multiple device record statuses by their IDs
    async fn update_device_record_statuses_synced_bulk(
        &self,
        status_ids: &[String],
    ) -> Result<(), RepositoryError>;
    async fn get_all_pending_syncs_by_type(
        &self,
        device_id: &str,
        resource_type: &str,
    ) -> Result<
        Option<Vec<(SyncRecord, Vec<DeviceRecord>, Vec<DeviceRecordStatus>)>>,
        RepositoryError,
    >;

    async fn get_device_record_by_id(
        &self,
        device_record_id: &str,
    ) -> Result<Option<DeviceRecord>, RepositoryError>;
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
pub trait ResourceRepository: Send + Sync {
    //TODO: change fav and last accessed
    async fn save(&self, resource: &Resource) -> Result<(), RepositoryError>;
    async fn find_by_folder(
        &self,
        folder_id: &str,
        user_id: &str,
    ) -> Result<Vec<ResourceWithKey>, RepositoryError>;
    async fn find_all_by_folder(
        &self,
        folder_id: &str,
        user_id: &str,
    ) -> Result<Vec<ResourceWithKey>, RepositoryError>;
    async fn find_by_id(&self, id: &str, user_id: &str)
    -> Result<ResourceWithKey, RepositoryError>;
    async fn delete_resource(&self, id: &str) -> Result<(), RepositoryError>;
    async fn soft_delete_resource(&self, id: &str) -> Result<(), RepositoryError>;
    async fn toggle_fav(&self, id: &str) -> Result<(), RepositoryError>;
    async fn update_last_accessed(&self, id: &str) -> Result<(), RepositoryError>;
    async fn get_all_resources(
        &self,
        user_id: &str,
    ) -> Result<Vec<ResourceWithKey>, RepositoryError>;
    async fn get_favourites(&self, user_id: &str) -> Result<Vec<ResourceWithKey>, RepositoryError>;
    async fn update_resource(&self, data: &str, resource_id: &str) -> Result<(), RepositoryError>;
    async fn find_resource_with_key(
        &self,
        resource_id: &str,
        user_id: &str,
    ) -> Result<ResourceKeyPair, RepositoryError>;
    async fn save_resource_with_key(
        &self,
        resource: &Resource,
        key: &ResourceKey,
    ) -> Result<(), RepositoryError>;
    async fn find_by_id_raw(&self, id: &str) -> Result<Resource, RepositoryError>;
}

#[async_trait]

pub trait DeviceRepository: Send + Sync {
    async fn save(&self, device: &Device) -> Result<(), RepositoryError>;
    async fn find_by_id(&self, device_id: &str) -> Result<Device, RepositoryError>;
    async fn update_last_synced_at(&self, device_id: &str) -> Result<(), RepositoryError>;
    async fn get_devices_by_user_id(&self, user_id: &str) -> Result<Vec<Device>, RepositoryError>;
    async fn get_devices_by_user_except(
        &self,
        user_id: &str,
        exclude_ids: &[String],
    ) -> Result<Vec<Device>, RepositoryError>;

    async fn get_all_devices_except(
        &self,
        current_device_id: &[String],
    ) -> Result<Vec<Device>, RepositoryError>;
    async fn save_many(&self, devices: &[Device]) -> Result<(), RepositoryError>;
}

#[async_trait]
pub trait DeviceRecordRepository: Send + Sync {
    async fn add_records(&self, record: DeviceRecord) -> Result<(), RepositoryError>;
}

#[async_trait]
pub trait DeviceRecordStatusRepository: Send + Sync {
    async fn add_device_records(&self, records: DeviceRecordStatus) -> Result<(), RepositoryError>;
}

#[async_trait]
pub trait UserRepository: Send + Sync {
    async fn add_known_user(&self, user: &User) -> Result<(), RepositoryError>;
    async fn get_known_users(&self) -> Result<Vec<User>, RepositoryError>;
    async fn get_user_by_id(&self, user_id: &str) -> Result<User, RepositoryError>;
    async fn complete_user_addtion(&self, user_id: &str) -> Result<(), RepositoryError>;
}

#[async_trait]
pub trait ResourceKeyRepository: Send + Sync {
    async fn save(&self, key: &ResourceKey) -> Result<(), RepositoryError>;
    async fn find_by_resource_id(
        &self,
        resource_id: &str,
    ) -> Result<Vec<ResourceKey>, RepositoryError>;
    async fn find_by_resource_and_user(
        &self,
        resource_id: &str,
        user_id: &str,
    ) -> Result<ResourceKey, RepositoryError>;
    async fn delete_by_resource_id(&self, resource_id: &str) -> Result<(), RepositoryError>;
    async fn delete_by_resource_and_user(
        &self,
        resource_id: &str,
        user_id: &str,
    ) -> Result<(), RepositoryError>;
}

#[async_trait]
pub trait ShareRepository: Send + Sync {
    async fn add_share_record_set(&self, record_set: ShareRecordSet)
    -> Result<(), RepositoryError>;

    async fn add_status_change_set(
        &self,
        status_set: ShareStatusChangeSet,
    ) -> Result<(), RepositoryError>;

    async fn update_user_record(
        &self,
        user_id: String,
        share_id: String,
    ) -> Result<(), RepositoryError>;

    async fn update_user_record_set(
        &self,
        record_set: UserRecordSet,
    ) -> Result<(), RepositoryError>;

    async fn get_pending_share_by_user(
        &self,
        user_id: &str,
    ) -> Result<Option<(ShareRecord, Vec<UserRecord>, Vec<UserRecordStatus>)>, RepositoryError>;

    async fn get_unsynced_user_share_records(
        &self,
        user_id: &str,
    ) -> Result<Vec<(UserRecord, Vec<UserRecordStatus>)>, RepositoryError>;

    async fn update_user_sync_record_status(
        &self,
        user_sync_record_id: String,
    ) -> Result<(), RepositoryError>;

    async fn update_share_status(
        &self,
        user_record_ids: Vec<String>,
        status_record_ids: Vec<String>,
    ) -> Result<(), RepositoryError>;

    async fn update_user_sync_record_by_user_id(
        &self,
        user_record_ids: Vec<String>,
        synced_user_id: String,
    ) -> Result<(), RepositoryError>;

    async fn get_all_share_records(&self) -> Result<Vec<ShareRecord>, RepositoryError>;

    async fn get_effective_share_records(
        &self,
        resource_id: &str,
    ) -> Result<Vec<ShareRecord>, RepositoryError>;

    async fn get_user_records_by_share_id(
        &self,
        share_id: &str,
    ) -> Result<Vec<UserRecord>, RepositoryError>;

    async fn get_share_record_by_id(&self, share_id: &str) -> Result<ShareRecord, RepositoryError>;
}

#[async_trait]
pub trait VectorClockRepository: Send + Sync {
    /// Save multiple vector clock entries
    async fn save_vector_clocks(
        &self,
        vector_clocks: &[ResourceVectorClock],
    ) -> Result<(), RepositoryError>;

    /// Increment a vector clock for a specific resource and device
    async fn increment_vector_clock(
        &self,
        resource_id: &str,
        device_id: &str,
    ) -> Result<ResourceVectorClock, RepositoryError>;

    /// Get all vector clock entries for a resource
    async fn get_vector_clocks_for_resource(
        &self,
        resource_id: &str,
    ) -> Result<Vec<ResourceVectorClock>, RepositoryError>;

    /// Get a specific vector clock entry
    async fn get_vector_clock(
        &self,
        resource_id: &str,
        device_id: &str,
    ) -> Result<ResourceVectorClock, RepositoryError>;

    async fn check_if_device_needs_update(
        &self,
        resource_ids: &[String],
        last_synced_at: i64,
        device_id: &str,
    ) -> Result<bool, RepositoryError>;

    async fn get_resource_ids_needing_updates(
        &self,
        resource_ids: &[String],
        last_synced_at: i64,
        device_id: &str,
    ) -> Result<Vec<String>, RepositoryError>;

    async fn update_vector_clocks(
        &self,
        update_vector_clocks: &[ResourceVectorClock],
        add_vector_clocks: &[ResourceVectorClock],
    ) -> Result<(), RepositoryError>;
}
