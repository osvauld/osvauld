use osvauld_core::models::device::Device;
use osvauld_core::models::folder::Folder;
use osvauld_core::models::p2p::{SyncAckType, SyncPayload};
use osvauld_core::models::resource::{Resource, ResourceKeyPair};
use osvauld_core::models::sync_record::{
    DeviceRecord, DeviceRecordSet, DeviceRecordStatus, SyncRecord, SyncRecordSet,
};
use osvauld_core::models::sync_types::{ResourceType, SyncMergeResult};
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
                sync_record,
                device_records,
                device_record_statuses,
                user,
                devices,
            } => {
                self.process_user_sync(
                    sync_record,
                    device_records,
                    device_record_statuses,
                    user,
                    devices,
                    current_device_id,
                    current_user_id,
                )
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

            SyncPayload::StatusUpdate {
                device_records,
                device_record_statuses,
            } => {
                self.process_status_update(
                    device_records,
                    device_record_statuses,
                    current_device_id,
                )
                .await
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
        device_records: &[DeviceRecord],
        device_record_statuses: &[DeviceRecordStatus],
        current_device_id: &str,
    ) -> Result<SyncAckType, RepositoryError> {
        // Process device records
        let (processed_records, processed_statuses, _updated_record_ids, _update_status_ids) =
            SyncRecord::process_device_records(
                device_records,
                device_record_statuses,
                current_device_id,
            );

        // Extract record IDs for acknowledgment
        let device_record_ids: Vec<_> = processed_records
            .iter()
            .map(|device_record| device_record.id.clone())
            .collect();

        // Create device record set
        let device_record_set = DeviceRecordSet {
            device_record_statuses: processed_statuses,
            device_records: processed_records,
        };

        // Use db transaction method for updating device record set
        self.db.update_device_record_set(device_record_set).await?;

        Ok(SyncAckType::DeviceRecords(device_record_ids))
    }

    async fn process_user_sync(
        &self,
        sync_record: &SyncRecord,
        device_records: &[DeviceRecord],
        device_record_statuses: &[DeviceRecordStatus],
        user: &User,
        devices: &[Device],
        current_device_id: &str,
        current_user_id: &str,
    ) -> Result<SyncAckType, RepositoryError> {
        // Process common sync logic
        let other_devices = self
            .get_user_other_devices(current_device_id, current_user_id)
            .await?;
        info!("other devices {:?}, {:?}", other_devices, current_device_id);
        let mut all_devices = devices.to_vec();
        all_devices.extend(other_devices.clone());
        let (merge_result, record_exist) = self
            .prepare_common_sync_data(
                sync_record,
                device_records,
                device_record_statuses,
                current_device_id,
                all_devices,
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
                .sync_add_new_user(user, devices, &record_set)
                .await?;
        } else {
            self.db
                .apply_sync_operations(&merge_result.local_operations)
                .await?;
        }

        Ok(SyncAckType::FullSync(merge_result.remote_operations))
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
