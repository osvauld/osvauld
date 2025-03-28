use osvauld_core::models::device::Device;
use osvauld_core::models::folder::Folder;
use osvauld_core::models::resource::{Resource, ResourceKeyPair};
use osvauld_core::models::sync_record::{
    DeviceRecord, DeviceRecordSet, DeviceRecordStatus, StatusChangeSet, SyncRecordSet,
};
use osvauld_core::models::sync_types::SyncOperations;
use osvauld_core::models::user::User;
use osvauld_core::models::vector_clock::ResourceVectorClock;
use osvauld_core::repositories::RepositoryError;

use log::info;

use crate::transaction_service::TransactionService;

// Implementation of database transaction methods for SyncService
impl TransactionService {
    // Database operations for device sync
    pub async fn save_device_sync(
        &self,
        device: &Device,
        sync_record_set: &SyncRecordSet,
        vector_clocks: &[ResourceVectorClock],
    ) -> Result<(), RepositoryError> {
        // Save the device first
        self.device_repository.save(device).await?;

        // Add the sync record set
        self.sync_repository
            .add_sync_record_set(sync_record_set)
            .await?;

        // Save vector clocks if any
        if !vector_clocks.is_empty() {
            self.vector_clock_repository
                .save_vector_clocks(vector_clocks)
                .await?;
        }

        Ok(())
    }

    // Database operations for resource sync
    pub async fn save_resource_sync(
        &self,
        resource: &ResourceKeyPair,
        vector_clocks: &[ResourceVectorClock],
        sync_record_set: &SyncRecordSet,
    ) -> Result<(), RepositoryError> {
        // Save resource with its key
        self.resource_repository
            .save_resource_with_key(&resource.resource, &resource.key)
            .await?;

        // Save vector clocks
        self.vector_clock_repository
            .save_vector_clocks(vector_clocks)
            .await?;

        // Save sync record set
        self.sync_repository
            .add_sync_record_set(sync_record_set)
            .await?;

        Ok(())
    }

    // Database operations for folder sync
    pub async fn save_folder_sync(
        &self,
        folder: &Folder,
        sync_record_set: &SyncRecordSet,
    ) -> Result<(), RepositoryError> {
        // Save folder
        self.folder_repository.save(folder).await?;

        // Save sync record set
        self.sync_repository
            .add_sync_record_set(sync_record_set)
            .await?;

        Ok(())
    }

    // Database operations for device record status updates
    pub async fn update_device_record_set(
        &self,
        device_record_set: DeviceRecordSet,
    ) -> Result<(), RepositoryError> {
        self.sync_repository
            .update_device_record_set(device_record_set)
            .await
    }

    // Database operations for acknowledgement processing
    pub async fn process_full_sync_ack(
        &self,
        device_id: String,
        sync_record_id: String,
        status_change_set: StatusChangeSet,
    ) -> Result<(), RepositoryError> {
        // Add status change set
        self.sync_repository
            .add_status_change_set(status_change_set)
            .await?;

        // Update device record
        self.sync_repository
            .update_device_record(device_id.clone(), sync_record_id.clone())
            .await?;

        // Update device record statuses
        self.sync_repository
            .update_device_record_statuses_for_sync(sync_record_id, device_id)
            .await?;

        Ok(())
    }

    // Database operations for device sync record updates
    pub async fn update_device_sync_records(
        &self,
        records: Vec<String>,
        device_id: String,
    ) -> Result<(), RepositoryError> {
        self.sync_repository
            .update_device_sync_record_by_device_id(records, device_id)
            .await
    }

    // Database operations to merge updated documents
    pub async fn merge_document(
        &self,
        doc: &str,
        add_vector: &[ResourceVectorClock],
        update_vector: &[ResourceVectorClock],
        resource_id: &str,
    ) -> Result<(), RepositoryError> {
        // Update the resource with the new document
        self.resource_repository
            .update_resource(doc, resource_id)
            .await?;

        // Update vector clocks
        self.vector_clock_repository
            .update_vector_clocks(update_vector, add_vector)
            .await?;

        Ok(())
    }

    // Database operations for updating sync status
    pub async fn update_sync_status(
        &self,
        device_record_ids: Vec<String>,
        status_record_ids: Vec<String>,
    ) -> Result<(), RepositoryError> {
        self.sync_repository
            .update_sync_status(device_record_ids, status_record_ids)
            .await
    }

    // Database operations for completing acknowledgment
    pub async fn complete_ack(&self, device_sync_record_id: String) -> Result<(), RepositoryError> {
        info!(
            "Completing acknowledgment for record: {}",
            device_sync_record_id
        );
        self.sync_repository
            .update_device_sync_record_status(device_sync_record_id)
            .await
    }

    // Database operations to create and save a new resource for syncing
    pub async fn prepare_new_resource_sync(
        &self,
        resource: &Resource,
        sync_record_set: &SyncRecordSet,
        vector_clocks: &[ResourceVectorClock],
    ) -> Result<(), RepositoryError> {
        // Save the resource
        self.resource_repository.save(resource).await?;

        // Save sync record set
        self.sync_repository
            .add_sync_record_set(sync_record_set)
            .await?;

        // Save vector clocks
        self.vector_clock_repository
            .save_vector_clocks(vector_clocks)
            .await?;

        Ok(())
    }

    pub async fn sync_add_new_user(
        &self,
        user: &User,
        devices: &[Device],
        user_addition_record: &SyncRecordSet,
    ) -> Result<(), RepositoryError> {
        self.user_repository.add_known_user(user).await?;
        self.device_repository.save_many(devices).await?;
        self.sync_repository
            .add_sync_record_set(user_addition_record)
            .await?;
        Ok(())
    }

    pub async fn complete_new_user_add(
        &self,
        user_id: &str,
        devices: &[Device],
        user_addition_record: &SyncRecordSet,
        processed_user_addition_record: &SyncRecordSet,
    ) -> Result<(), RepositoryError> {
        self.user_repository.complete_user_addtion(user_id).await?;
        self.device_repository.save_many(devices).await?;

        self.sync_repository
            .add_sync_record_set(processed_user_addition_record)
            .await?;
        self.sync_repository
            .add_sync_record_set(user_addition_record)
            .await?;
        Ok(())
    }
    pub async fn handle_user_add_ack(
        &self,
        remote_user_id: &str,
        user_addition_record: &SyncRecordSet,
    ) -> Result<(), RepositoryError> {
        // Mark the user addition as complete
        self.user_repository
            .complete_user_addtion(remote_user_id)
            .await?;

        // Save the user addition records
        self.sync_repository
            .add_sync_record_set(user_addition_record)
            .await?;

        Ok(())
    }

    pub async fn process_user_add_ack(
        &self,
        device_record_status: &[DeviceRecordStatus],
        device_record: &DeviceRecord,
        updated_record_ids: Vec<String>,
        updated_status_ids: Vec<String>,
    ) -> Result<(), RepositoryError> {
        self.sync_repository
            .add_status_change_set(StatusChangeSet {
                device_record: device_record.clone(),
                device_record_statuses: device_record_status.to_vec(),
            })
            .await?;

        self.sync_repository
            .update_device_sync_status_by_ids(updated_record_ids, updated_status_ids)
            .await
    }

    pub async fn apply_sync_operations(
        &self,
        operations: &SyncOperations,
    ) -> Result<(), RepositoryError> {
        // Skip processing if there are no operations to perform
        if operations.is_empty() {
            return Ok(());
        }

        // Add new device records
        if !operations.records_to_add.is_empty() {
            self.sync_repository
                .add_device_records_bulk(&operations.records_to_add)
                .await?;
        }

        // Add new device record statuses
        if !operations.status_records_to_add.is_empty() {
            self.sync_repository
                .add_device_record_statuses_bulk(&operations.status_records_to_add)
                .await?;
        }

        // Update existing device records
        if !operations.record_ids_to_update.is_empty() {
            self.sync_repository
                .update_device_records_synced_bulk(&operations.record_ids_to_update)
                .await?;
        }

        // Update existing device record statuses
        if !operations.status_ids_to_update.is_empty() {
            self.sync_repository
                .update_device_record_statuses_synced_bulk(&operations.status_ids_to_update)
                .await?;
        }

        Ok(())
    }
}
