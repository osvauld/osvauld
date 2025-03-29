use osvauld_core::models::device::Device;
use osvauld_core::models::folder::Folder;
use osvauld_core::models::p2p::{SyncAckType, SyncPayload};
use osvauld_core::models::resource::{Resource, ResourceKeyPair};
use osvauld_core::models::sync_record::{
    DeviceRecord, DeviceRecordSet, DeviceRecordStatus, SyncRecord, SyncRecordSet,
};
use osvauld_core::models::sync_types::SyncMergeResult;
use osvauld_core::models::sync_types::SyncOperations;
use osvauld_core::models::user::User;
use osvauld_core::models::vector_clock::ResourceVectorClock;
use osvauld_core::repositories::RepositoryError;

use log::info;

use super::sync_service_core::{SyncEvent, SyncService};

impl SyncService {
    pub async fn process_sync_payload<F>(
        &self,
        payload: &SyncPayload,
        user_id: &str,
        device_id: &str,
        emit_event: Option<F>,
        current_device_id: &str,
        current_user_id: &str,
    ) -> Result<SyncAckType, RepositoryError>
    where
        F: Fn(SyncEvent) + Send + Sync,
    {
        match payload {
            SyncPayload::DeviceSync {
                sync_record,
                device_records,
                device_record_statuses,
                device,
            } => {
                self.process_device_sync(
                    sync_record,
                    device_records,
                    device_record_statuses,
                    device,
                    current_device_id,
                    user_id,
                )
                .await
            }

            SyncPayload::UserSync {
                sync_data,
                user_data,
            } => {
                self.process_user_sync(sync_data, user_data, current_device_id, current_user_id)
                    .await
            }

            SyncPayload::ResourceSync {
                sync_record,
                device_records,
                device_record_statuses,
                resource,
                vector_clocks,
            } => {
                self.process_resource_sync(
                    sync_record,
                    device_records,
                    device_record_statuses,
                    resource,
                    vector_clocks,
                    current_device_id,
                    user_id,
                )
                .await
            }

            SyncPayload::FolderSync {
                sync_record,
                device_records,
                device_record_statuses,
                folder,
            } => {
                self.process_folder_sync(
                    sync_record,
                    device_records,
                    device_record_statuses,
                    folder,
                    current_device_id,
                    user_id,
                )
                .await
            }

            SyncPayload::ResourceUpdate {
                resource,
                vector_clocks,
            } => {
                self.process_resource_update_sync(
                    resource,
                    device_id,
                    user_id,
                    vector_clocks,
                    emit_event,
                )
                .await
            }

            SyncPayload::StatusUpdate(payload) => {
                self.process_status_update(payload, current_device_id).await
            }
        }
    }

    // Process device sync payload
    async fn process_device_sync(
        &self,
        sync_record: &SyncRecord,
        device_records: &[DeviceRecord],
        device_record_statuses: &[DeviceRecordStatus],
        device: &Device,
        current_device_id: &str,
        user_id: &str,
    ) -> Result<SyncAckType, RepositoryError> {
        // Process common sync logic

        let mut devices = self
            .get_user_other_devices(current_device_id, user_id)
            .await?;
        devices.push(device.clone());
        let (merge_result, record_exists) = self
            .prepare_common_sync_data(
                sync_record,
                device_records,
                device_record_statuses,
                current_device_id,
                devices,
            )
            .await?;

        if !record_exists {
            let record_set = SyncRecordSet {
                sync_record: sync_record.clone(),
                device_records: merge_result.local_operations.records_to_add.clone(),
                device_record_statuses: merge_result.local_operations.status_records_to_add.clone(),
            };

            // Use db transaction method for saving device sync
            self.db.save_device_sync(device, &record_set, &[]).await?;
        } else {
            self.db
                .apply_sync_operations(&merge_result.local_operations)
                .await?;
        }
        Ok(SyncAckType::FullSync(merge_result.remote_operations))
    }

    // Process resource sync payload
    async fn process_resource_sync(
        &self,
        sync_record: &SyncRecord,
        device_records: &[DeviceRecord],
        device_record_statuses: &[DeviceRecordStatus],
        resource: &ResourceKeyPair,
        vector_clocks: &[ResourceVectorClock],
        current_device_id: &str,
        user_id: &str,
    ) -> Result<SyncAckType, RepositoryError> {
        // Process common sync logic

        let devices = self
            .get_user_other_devices(current_device_id, user_id)
            .await?;
        let (merge_result, record_exist) = self
            .prepare_common_sync_data(
                sync_record,
                device_records,
                device_record_statuses,
                current_device_id,
                devices,
            )
            .await?;

        if !record_exist {
            // Create sync record set
            let record_set = SyncRecordSet {
                sync_record: sync_record.clone(),
                device_records: merge_result.local_operations.records_to_add.clone(),
                device_record_statuses: merge_result.local_operations.status_records_to_add.clone(),
            };

            // Use db transaction method for saving resource sync
            self.db
                .save_resource_sync(resource, vector_clocks, &record_set)
                .await?;
        } else {
            self.db
                .apply_sync_operations(&merge_result.local_operations)
                .await?;
        }

        Ok(SyncAckType::FullSync(merge_result.remote_operations))
    }

    // Process folder sync payload
    async fn process_folder_sync(
        &self,
        sync_record: &SyncRecord,
        device_records: &[DeviceRecord],
        device_record_statuses: &[DeviceRecordStatus],
        folder: &Folder,
        current_device_id: &str,
        user_id: &str,
    ) -> Result<SyncAckType, RepositoryError> {
        // Process common sync logic
        let devices = self
            .get_user_other_devices(current_device_id, user_id)
            .await?;

        let (merge_result, record_exist) = self
            .prepare_common_sync_data(
                sync_record,
                device_records,
                device_record_statuses,
                current_device_id,
                devices,
            )
            .await?;

        if !record_exist {
            // Create sync record set
            let record_set = SyncRecordSet {
                sync_record: sync_record.clone(),
                device_records: merge_result.local_operations.records_to_add.clone(),
                device_record_statuses: merge_result.local_operations.status_records_to_add.clone(),
            };

            // Use db transaction method for saving resource sync
            self.db.save_folder_sync(folder, &record_set).await?;
        } else {
            self.db
                .apply_sync_operations(&merge_result.local_operations)
                .await?;
        }

        Ok(SyncAckType::FullSync(merge_result.remote_operations))
    }

    // Process status update payload
    async fn process_status_update(
        &self,
        payload: &Vec<(DeviceRecord, Vec<DeviceRecordStatus>)>,
        current_device_id: &str,
    ) -> Result<SyncAckType, RepositoryError> {
        // Process device records
        let mut updated_device_sync_records = Vec::new();
        let mut updated_device_record_ids = Vec::new();
        let mut to_add_records = Vec::new();

        for (device_record, device_record_statuses) in payload {
            let local_device_record = self
                .sync_repository
                .get_device_record_by_id(&device_record.id)
                .await?;

            // Check if we need to add this record's status for the current device
            if let Some(status) = device_record_statuses
                .iter()
                .find(|status| status.aware_device_id == current_device_id)
            {
                updated_device_sync_records.push(status.id.clone());
            }

            if local_device_record.is_some() {
                // Record exists locally
                if device_record.synced {
                    // If synced is true, add to updated IDs
                    updated_device_record_ids.push(device_record.id.clone());
                }
            } else {
                // Record doesn't exist locally, add to to_add_records
                to_add_records.push((device_record.clone(), device_record_statuses.clone()));
            }
        }
        self.db
            .apply_device_sync_updates(
                &to_add_records,
                &updated_device_sync_records,
                &updated_device_record_ids,
            )
            .await?;

        Ok(SyncAckType::DeviceSyncRecords(updated_device_sync_records))
    }

    async fn process_user_sync(
        &self,
        sync_data: &Vec<(SyncRecord, Vec<DeviceRecord>, Vec<DeviceRecordStatus>)>,
        user_data: &Vec<(User, Vec<Device>)>,
        current_device_id: &str,
        current_user_id: &str,
    ) -> Result<SyncAckType, RepositoryError> {
        // Get current user's other devices
        let other_devices = self
            .get_user_other_devices(current_device_id, current_user_id)
            .await?;

        info!("other devices {:?}, {:?}", other_devices, current_device_id);

        // Collect all devices from other users (not current user)
        let mut all_devices = Vec::new();
        let mut users_to_add = Vec::new();
        let mut devices_to_add = Vec::new();

        for (user, devices) in user_data {
            let user_exists = match self.user_repository.get_user_by_id(&user.id).await {
                Ok(_) => true,
                Err(RepositoryError::NotFound) => false,
                Err(e) => return Err(e),
            };

            // Only add the user if they don't already exist
            if !user_exists {
                users_to_add.push(user.clone());
                devices_to_add.extend(devices.clone());
            }
            // Only add other users' data to be saved
            if user.id != current_user_id {
                all_devices.extend(devices.clone());
            }
        }

        // Add current user's other devices to the full device list for sync record processing
        all_devices.extend(other_devices.clone());

        // Create a combined SyncOperations for the final result
        let mut combined_remote_operations = SyncOperations::new();

        // List to collect all record sets that need to be added
        let mut record_sets_to_add = Vec::new();
        let mut operations_to_apply = Vec::new();

        // Process all sync records
        for (sync_record, device_records, device_record_statuses) in sync_data {
            let (merge_result, record_exists) = self
                .prepare_common_sync_data(
                    sync_record,
                    device_records,
                    device_record_statuses,
                    current_device_id,
                    all_devices.clone(),
                )
                .await?;

            if !record_exists {
                // Create sync record set for new records
                let record_set = SyncRecordSet {
                    sync_record: sync_record.clone(),
                    device_records: merge_result.local_operations.records_to_add.clone(),
                    device_record_statuses: merge_result
                        .local_operations
                        .status_records_to_add
                        .clone(),
                };

                record_sets_to_add.push(record_set);
            } else {
                // Store operations to apply for existing records
                operations_to_apply.push(merge_result.local_operations.clone());
            }

            // Combine remote operations
            combined_remote_operations
                .records_to_add
                .extend(merge_result.remote_operations.records_to_add);
            combined_remote_operations
                .status_records_to_add
                .extend(merge_result.remote_operations.status_records_to_add);
            combined_remote_operations
                .record_ids_to_update
                .extend(merge_result.remote_operations.record_ids_to_update);
            combined_remote_operations
                .status_ids_to_update
                .extend(merge_result.remote_operations.status_ids_to_update);
        }

        // Now use the transaction service to add everything in one go
        self.db
            .add_users_and_devices_batch(&users_to_add, &devices_to_add, &record_sets_to_add)
            .await?;

        // Apply operations for existing records
        for ops in operations_to_apply {
            self.db.apply_sync_operations(&ops).await?;
        }

        // Return the combined remote operations as the acknowledgment
        Ok(SyncAckType::FullSync(combined_remote_operations))
    }

    async fn process_resource_update_sync<F>(
        &self,
        resource: &Resource,
        device_id: &str,
        user_id: &str,
        vector_clock: &[ResourceVectorClock],
        emit_event: Option<F>,
    ) -> Result<SyncAckType, RepositoryError>
    where
        F: Fn(SyncEvent) + Send + Sync,
    {
        if let Some(emit) = &emit_event {
            emit(SyncEvent::UpdateEvent {
                remote_resource: resource.clone(),
                user_id: user_id.to_string(),
                device_id: device_id.to_string(),
                vector_clock: vector_clock.to_vec(),
            })
        }
        Ok(SyncAckType::UpdateRecieved(resource.id.clone()))
    }

    pub async fn prepare_common_sync_data(
        &self,
        sync_record: &SyncRecord,
        device_records: &[DeviceRecord],
        device_record_statuses: &[DeviceRecordStatus],
        current_device_id: &str,
        devices: Vec<Device>,
    ) -> Result<(SyncMergeResult, bool), RepositoryError> {
        let mut add_sync_record = false;
        // Check if this sync record already exists by ID directly
        let existing_record = self
            .sync_repository
            .get_sync_record_by_id(&sync_record.id)
            .await?;

        // If the sync record exists, fetch existing device records and statuses
        if existing_record.is_some() {
            add_sync_record = true;
            // Get existing device records and statuses in one call
            let (local_device_records, local_device_statuses) = self
                .sync_repository
                .get_device_records_and_statuses_by_sync_record(&sync_record.id)
                .await?;

            // Perform the merge operation
            let merge_result = SyncRecord::merge_sync_records(
                &local_device_records,
                &local_device_statuses,
                device_records,
                device_record_statuses,
                current_device_id,
            );

            // Return the merge result directly
            return Ok((merge_result, add_sync_record));
        }

        // If no existing record was found, this is a new sync record
        // Process device records using the existing logic - this updates sync flags for records associated with current_device_id
        let (processed_records, processed_statuses, updated_record_ids, updated_status_ids) =
            SyncRecord::process_device_records(
                device_records,
                device_record_statuses,
                current_device_id,
            );

        // Create completion records for the current device
        let completion_records = SyncRecord::create_completion_records(
            sync_record.id.clone(),
            current_device_id.to_string(),
            &devices,
        );

        // Construct a new merge result for the new record
        let mut merge_result = SyncMergeResult::new();

        // For a new record, all processed records and statuses go into local_operations
        merge_result.local_operations.records_to_add = processed_records;
        merge_result
            .local_operations
            .records_to_add
            .push(completion_records.device_record.clone());
        merge_result.local_operations.status_records_to_add = processed_statuses;
        merge_result
            .local_operations
            .status_records_to_add
            .extend(completion_records.device_record_statuses.clone());

        // For the remote operations, we need to:
        // 1. Send back our completion record so the sender knows we processed their sync
        merge_result
            .remote_operations
            .records_to_add
            .push(completion_records.device_record);
        merge_result.remote_operations.status_records_to_add =
            completion_records.device_record_statuses;

        // 2. Tell the remote device which records and statuses we've updated locally
        // These are the IDs returned by process_device_records - they indicate which records
        // were marked as synced=true on our side and need to be updated on the remote side
        merge_result.remote_operations.record_ids_to_update = updated_record_ids;
        merge_result.remote_operations.status_ids_to_update = updated_status_ids;

        Ok((merge_result, add_sync_record))
    }

    pub async fn merge_updated_doc(
        &self,
        doc: &str,
        add_vector: &[ResourceVectorClock],
        update_vector: &[ResourceVectorClock],
        resource_id: &str,
    ) -> Result<(), RepositoryError> {
        // Use db transaction method for merging updated document
        self.db
            .merge_document(doc, add_vector, update_vector, resource_id)
            .await
    }
}
