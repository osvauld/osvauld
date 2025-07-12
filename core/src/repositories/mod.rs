use crate::models::{
    Certificate, Device, Folder, Resource, ResourceKey, ResourceKeyPair, ResourceManifestData,
    ResourceSyncData, ResourceVectorClock, ResourceWithKey, ShareRecord, User, UserWithDeviceIds,
    UserWithDevices,
};
use async_trait::async_trait;
use std::collections::HashMap;
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
    async fn get_default_folder(&self) -> Result<Folder, RepositoryError>;
    async fn get_folders_by_ids(
        &self,
        folder_ids: &[String],
    ) -> Result<Vec<Folder>, RepositoryError>;
    async fn add_folders_bulk(&self, folders: &[Folder]) -> Result<(), RepositoryError>;
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
    async fn get_node_key(&self) -> Result<String, RepositoryError>;
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
    async fn save_resource_with_dependencies(
        &self,
        resource: &Resource,
        resource_key: &ResourceKey,
        share_record: &ShareRecord,
        vector_clocks: &[ResourceVectorClock],
    ) -> Result<(), RepositoryError>;
    async fn share_resource_transaction(
        &self,
        resource_key: &ResourceKey,
        share_record: &ShareRecord,
        recipient_vector_clocks: &[ResourceVectorClock],
    ) -> Result<(), RepositoryError>;
    async fn add_device_with_vector_clocks(
        &self,
        device: &Device,
        vector_clocks: &[ResourceVectorClock],
    ) -> Result<(), RepositoryError>;
    async fn get_resource_manifest_data(
        &self,
        resource_ids: Option<&[String]>,
    ) -> Result<Vec<ResourceManifestData>, RepositoryError>;
    async fn get_resource_sync_data(
        &self,
        resource_id: &str,
    ) -> Result<ResourceSyncData, RepositoryError>;

    async fn save_resource_sync_data(
        &self,
        sync_data: &ResourceSyncData,
    ) -> Result<(), RepositoryError>;
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
    async fn get_devices_by_user_ids(
        &self,
        user_ids: &[String],
    ) -> Result<Vec<Device>, RepositoryError>;
    async fn save_many(&self, devices: &[Device]) -> Result<(), RepositoryError>;
    async fn get_device_ids_by_user_id(
        &self,
        user_id: &str,
    ) -> Result<Vec<String>, RepositoryError>;

    async fn get_devices_by_ids(
        &self,
        device_ids: &[String],
    ) -> Result<Vec<Device>, RepositoryError>;
}

#[async_trait]
pub trait UserRepository: Send + Sync {
    async fn add_known_user(&self, user: &User) -> Result<(), RepositoryError>;
    async fn get_known_users(&self) -> Result<Vec<User>, RepositoryError>;
    async fn get_user_by_id(&self, user_id: &str) -> Result<User, RepositoryError>;
    async fn complete_user_addition(&self, user_id: &str) -> Result<(), RepositoryError>;
    async fn add_known_users_bulk(&self, users: &[User]) -> Result<(), RepositoryError>;
    async fn commit_signup_transaction(
        &self,
        user: &User,
        primary_certificate: &Certificate,
        device: &Device,
        device_certificate: &Certificate,
        peer_device: Option<&Device>,
    ) -> Result<(), RepositoryError>;
    async fn get_other_users_with_device_ids(
        &self,
    ) -> Result<Vec<UserWithDeviceIds>, RepositoryError>;
    async fn get_users_with_devices_by_user_ids(
        &self,
        user_ids: &[String],
    ) -> Result<Vec<UserWithDevices>, RepositoryError>;

    async fn add_users_with_devices_bulk(
        &self,
        users_with_devices: &[UserWithDevices],
    ) -> Result<(), RepositoryError>;
    async fn get_user_device_mapping(
        &self,
    ) -> Result<HashMap<String, Vec<String>>, RepositoryError>;
    async fn get_users_with_device_ids_by_user_ids(
        &self,
        user_ids: &[String],
    ) -> Result<Vec<UserWithDeviceIds>, RepositoryError>;

    async fn get_users_by_ids(&self, user_ids: &[String]) -> Result<Vec<User>, RepositoryError>;
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
    async fn save_vector_clock(
        &self,
        vector_clock: &ResourceVectorClock,
    ) -> Result<(), RepositoryError>;
}

#[async_trait]
pub trait ShareRepository: Send + Sync {
    /// Save a share record to the database
    async fn save(&self, share_record: &ShareRecord) -> Result<(), RepositoryError>;

    async fn find_by_resource(
        &self,
        resource_id: &str,
    ) -> Result<Vec<ShareRecord>, RepositoryError>;
    /// Find share records by resource ID and operation type
    async fn find_by_resource_and_operation(
        &self,
        resource_id: &str,
        operation_type: &str,
    ) -> Result<Vec<ShareRecord>, RepositoryError>;
    async fn find_by_id(&self, id: &str) -> Result<ShareRecord, RepositoryError>;
    async fn save_many(&self, share_records: &[ShareRecord]) -> Result<(), RepositoryError>;
    async fn get_user_share_records(
        &self,
        user_id: &str,
    ) -> Result<Vec<ShareRecord>, RepositoryError>;
}
