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

    // Database operations for device sync record updates
    pub async fn update_device_sync_records(
        &self,
        records: &Vec<String>,
    ) -> Result<(), RepositoryError> {
        self.sync_repository
            .update_device_record_statuses_synced_bulk(records)
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
    pub async fn complete_ack(
        &self,
        device_sync_record_ids: Vec<String>,
    ) -> Result<(), RepositoryError> {
        self.sync_repository
            .update_device_record_statuses_synced_bulk(&device_sync_record_ids)
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

    pub async fn add_only_record_set(
        &self,
        record_set: &SyncRecordSet,
    ) -> Result<(), RepositoryError> {
        self.sync_repository.add_sync_record_set(record_set).await
    }

    pub async fn complete_new_user_add(
        &self,
        user_id: &str,
        devices: &[Device],
        processed_user_addition_record: &SyncRecordSet,
        user_addition_record: &SyncRecordSet,
        completion_record: &StatusChangeSet,
    ) -> Result<(), RepositoryError> {
        self.user_repository.complete_user_addtion(user_id).await?;
        self.device_repository.save_many(devices).await?;

        self.sync_repository
            .add_sync_record_set(processed_user_addition_record)
            .await?;
        self.sync_repository
            .add_status_change_set(completion_record)
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
        updated_completion_record: &StatusChangeSet,
        local_completion_record: &StatusChangeSet,
        updated_record_ids: &[String],
        updated_status_ids: &[String],
    ) -> Result<(), RepositoryError> {
        // Mark the user addition as complete
        self.user_repository
            .complete_user_addtion(remote_user_id)
            .await?;

        // Save the user addition records
        self.sync_repository
            .add_sync_record_set(user_addition_record)
            .await?;
        self.sync_repository
            .add_status_change_set(updated_completion_record)
            .await?;
        self.sync_repository
            .add_status_change_set(local_completion_record)
            .await?;
        self.sync_repository
            .update_device_record_statuses_synced_bulk(updated_status_ids)
            .await?;
        self.sync_repository
            .update_device_records_synced_bulk(updated_record_ids)
            .await?;

        Ok(())
    }

    pub async fn handle_user_connection_complete(
        &self,
        updated_completion_record: &StatusChangeSet,
        device_sync_record_id: Option<String>,
        updated_record_ids: &[String],
        updated_status_ids: &[String],
    ) -> Result<(), RepositoryError> {
        self.sync_repository
            .add_status_change_set(updated_completion_record)
            .await?;
        if let Some(device_sync_record_id) = device_sync_record_id {
            self.sync_repository
                .update_device_sync_record_status(device_sync_record_id)
                .await?;
        }
        self.sync_repository
            .update_device_record_statuses_synced_bulk(updated_status_ids)
            .await?;
        self.sync_repository
            .update_device_records_synced_bulk(updated_record_ids)
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
            .add_status_change_set(&StatusChangeSet {
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
    pub async fn add_users_and_devices_batch(
        &self,
        users: &[User],
        devices: &[Device],
        record_sets: &[SyncRecordSet],
    ) -> Result<(), RepositoryError> {
        // save all users
        for user in users {
            self.user_repository.add_known_user(user).await?;
        }
        // save all devices (they must exist before users that reference them)
        if !devices.is_empty() {
            self.device_repository.save_many(devices).await?;
        }

        // Finally save all record sets (which reference both users and devices)
        for record_set in record_sets {
            self.sync_repository.add_sync_record_set(record_set).await?;
        }

        // Note: Currently apply_sync_operations is called separately.
        // TODO: In the future, integrate apply_sync_operations into this function
        // to handle everything in a single transaction

        Ok(())
    }

    pub async fn apply_device_sync_updates(
        &self,
        to_add_records: &Vec<(DeviceRecord, Vec<DeviceRecordStatus>)>,
        updated_device_sync_records: &Vec<String>,
        updated_device_record_ids: &Vec<String>,
    ) -> Result<(), RepositoryError> {
        // Extract device records and statuses from to_add_records
        let mut device_records_to_add = Vec::new();
        let mut status_records_to_add = Vec::new();

        for (device_record, statuses) in to_add_records {
            device_records_to_add.push(device_record.clone());
            status_records_to_add.extend(statuses.clone());
        }

        // 1. Add new device records if any exist
        if !device_records_to_add.is_empty() {
            self.sync_repository
                .add_device_records_bulk(&device_records_to_add)
                .await?;
        }

        // 2. Add new device record statuses if any exist
        if !status_records_to_add.is_empty() {
            self.sync_repository
                .add_device_record_statuses_bulk(&status_records_to_add)
                .await?;
        }

        // 3. Update existing device records if any exist
        if !updated_device_record_ids.is_empty() {
            self.sync_repository
                .update_device_records_synced_bulk(updated_device_record_ids)
                .await?;
        }

        // 4. Update device record statuses if any exist
        if !updated_device_sync_records.is_empty() {
            self.sync_repository
                .update_device_record_statuses_synced_bulk(updated_device_sync_records)
                .await?;
        }

        Ok(())
    }
}
