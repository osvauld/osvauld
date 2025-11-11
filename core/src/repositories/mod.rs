use crate::models::{
    Certificate, Device, EncryptedResource, Folder, FolderManifestData, FolderShareRecord,
    ShareRecord, User, UserWithDeviceIds, UserWithDevices,
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
    async fn save_folder_with_share_record(
        &self,
        folder: &Folder,
        folder_share_record: &FolderShareRecord,
    ) -> Result<(), RepositoryError>;

    async fn find_all(&self) -> Result<Vec<Folder>, RepositoryError>;
    async fn find_by_id(&self, id: &str) -> Result<Folder, RepositoryError>;
    async fn soft_delete(&self, id: &str) -> Result<(), RepositoryError>;
    async fn get_default_folder(&self) -> Result<Folder, RepositoryError>;
    async fn get_folders_by_ids(
        &self,
        folder_ids: &[String],
    ) -> Result<Vec<Folder>, RepositoryError>;
    async fn add_folders_bulk(&self, folders: &[Folder]) -> Result<(), RepositoryError>;
    async fn get_folder_manifest_for_user(
        &self,
        peer_user_id: &str,
    ) -> Result<Vec<FolderManifestData>, RepositoryError>;

    async fn save_folder_with_share_records(
        &self,
        folder: &Folder,
        share_records: &[FolderShareRecord],
    ) -> Result<(), RepositoryError>;
    async fn get_share_records_for_folder_and_recipients(
        &self,
        folder_id: &str,
        recipient_ids: &[String],
    ) -> Result<Vec<FolderShareRecord>, RepositoryError>;

    async fn add_folder_share_records_bulk(
        &self,
        share_records: &[FolderShareRecord],
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
    async fn get_node_key(&self) -> Result<String, RepositoryError>;
    async fn get_ucan_key(&self) -> Result<String, RepositoryError>;
    async fn add_index_key(&self, index_key: &str) -> Result<(), RepositoryError>;
    async fn get_index_key(&self) -> Result<String, RepositoryError>;
}

#[async_trait]
pub trait ResourceRepository: Send + Sync {
    /// Save an encrypted resource to the database
    async fn save_encrypted(&self, resource: &EncryptedResource) -> Result<(), RepositoryError>;

    /// Find resources by folder ID
    async fn find_by_folder(
        &self,
        folder_id: &str,
        user_id: &str,
    ) -> Result<Vec<EncryptedResource>, RepositoryError>;

    /// Find all resources by folder (alias for find_by_folder)
    async fn find_all_by_folder(
        &self,
        folder_id: &str,
        user_id: &str,
    ) -> Result<Vec<EncryptedResource>, RepositoryError>;

    /// Find a single resource by ID
    async fn find_by_id(&self, id: &str) -> Result<EncryptedResource, RepositoryError>;

    /// Delete a resource (hard delete)
    async fn delete_resource(&self, id: &str) -> Result<(), RepositoryError>;

    /// Get all resources for a user
    async fn get_all_resources(
        &self,
        user_id: &str,
    ) -> Result<Vec<EncryptedResource>, RepositoryError>;

    /// Update encrypted resource data and key
    async fn update_resource(&self, data: &str, encrypted_key: &str, resource_id: &str) -> Result<(), RepositoryError>;

    /// Get all resource IDs
    async fn get_all_resource_ids(&self) -> Result<Vec<String>, RepositoryError>;

    /// Get mapping of folder IDs to resource IDs
    async fn get_folder_ids_for_resources(
        &self,
        resource_ids: &[String],
    ) -> Result<HashMap<String, Vec<String>>, RepositoryError>;

    /// Get resource IDs for a specific folder
    async fn get_resource_ids_by_folder_id(
        &self,
        folder_id: &str,
    ) -> Result<Vec<String>, RepositoryError>;

    /// Save resource with multiple share records in a transaction
    async fn save_resource_with_share_records(
        &self,
        resource: &EncryptedResource,
        share_records: &[ShareRecord],
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
        ucan_certificate: &Certificate,
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
    async fn get_user_by_device_id(&self, device_id: &str) -> Result<User, RepositoryError>;
    async fn get_ucan_by_cid(&self, cid: &str) -> Result<String, RepositoryError>;
    async fn update_first_sync(&self, user_id: &str, first_sync: bool)
        -> Result<(), RepositoryError>;
    async fn update_ucan(
        &self,
        user_id: &str,
        new_ucan_token: String,
        new_ucan_cid: String,
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
    async fn find_by_resource_and_operation_and_user(
        &self,
        resource_id: &str,
        operation_type: &str,
        user_id: &str,
    ) -> Result<ShareRecord, RepositoryError>;
    async fn get_ucan_by_cid(&self, cid: &str) -> Result<String, RepositoryError>;

    async fn get_ucan_token_by_resource(
        &self,
        resource_id: &str,
        user_id: &str,
    ) -> Result<String, RepositoryError>;

    /// Find share records for multiple resources and a user with specific operation type
    /// Returns records in same order as input resource_ids
    async fn find_by_resources_and_user(
        &self,
        resource_ids: &[String],
        user_id: &str,
        operation_type: &str,
    ) -> Result<Vec<ShareRecord>, RepositoryError>;
}
#[async_trait]
pub trait FolderShareRecordRepository: Send + Sync {
    /// Save a single folder share record
    async fn save(&self, folder_share_record: &FolderShareRecord) -> Result<(), RepositoryError>;

    /// Get all share records for a specific folder (who has access to this folder)
    async fn get_records_by_folder_id(
        &self,
        folder_id: &str,
    ) -> Result<Vec<FolderShareRecord>, RepositoryError>;

    /// Get all folders shared with a specific user
    async fn get_records_by_recipient_user_id(
        &self,
        user_id: &str,
    ) -> Result<Vec<FolderShareRecord>, RepositoryError>;

    /// Bulk save folder share records with conflict ignore (for sync operations)
    async fn save_bulk_with_conflict_ignore(
        &self,
        folder_share_records: &[FolderShareRecord],
    ) -> Result<(), RepositoryError>;

    async fn get_ucan_by_cid(&self, cid: &str) -> Result<String, RepositoryError>;
    async fn get_shared_users(&self, folder_id: &str) -> Result<Vec<User>, RepositoryError>;

    /// Check if a folder is shared with a specific user (efficient single query)
    async fn find_by_folder_and_user(
        &self,
        folder_id: &str,
        user_id: &str,
    ) -> Result<Option<FolderShareRecord>, RepositoryError>;
}
