use osvauld_core::models::device::Device;
use osvauld_core::models::p2p::{SyncAckType, SyncPayload};
use osvauld_core::models::sync_types::ResourceType;
use osvauld_core::models::user::User;
use osvauld_core::models::vector_clock::ResourceVectorClock;
use osvauld_core::models::{
    folder::Folder,
    resource::{Resource, ResourceKeyPair},
    sync_record::{
        DeviceRecord, DeviceRecordSet, DeviceRecordStatus, StatusChangeSet, SyncRecord,
        SyncRecordSet,
    },
};

use osvauld_core::repositories::{
    DeviceRepository, FolderRepository, RepositoryError, ResourceRepository, StoreRepository,
    SyncRepository, VectorClockRepository,
};

use log::info;

use std::sync::Arc;
use tokio::sync::Mutex;

pub enum SyncEvent {
    UpdateEvent {
        remote_resource: Resource,
        vector_clock: Vec<ResourceVectorClock>,
        device_id: String,
        user_id: String,
    },
}

pub struct SyncService {
    sync_repository: Arc<dyn SyncRepository>,
    folder_repository: Arc<dyn FolderRepository>,
    resource_repository: Arc<dyn ResourceRepository>,
    device_repository: Arc<dyn DeviceRepository>,
    store_repository: Arc<dyn StoreRepository>,
    vector_clock_repository: Arc<dyn VectorClockRepository>,
}
impl SyncService {
    pub fn new(
        sync_repository: Arc<dyn SyncRepository>,
        folder_repository: Arc<dyn FolderRepository>,
        resource_repository: Arc<dyn ResourceRepository>,
        device_repository: Arc<dyn DeviceRepository>,
        store_repository: Arc<dyn StoreRepository>,
        vector_clock_repository: Arc<dyn VectorClockRepository>,
    ) -> Self {
        Self {
            sync_repository,
            folder_repository,
            resource_repository,
            device_repository,
            store_repository,
            vector_clock_repository,
        }
    }

    pub async fn add_new_device_sync(
        &self,
        sync_payload: SyncPayload,
        user_id: String,
    ) -> Result<(), RepositoryError> {
        let (device, sync_record, device_records, device_record_statuses) = match sync_payload {
            SyncPayload::DeviceSync {
                sync_record,
                device_records,
                device_record_statuses,
                device,
            } => (device, sync_record, device_records, device_record_statuses),
            _ => {
                return Err(RepositoryError::DatabaseError(
                    "Invalid sync payload: expected DeviceSync payload".to_string(),
                ));
            }
        };

        // Create the sync record set with the extracted data
        let sync_record_set = SyncRecordSet {
            sync_record,
            device_records,
            device_record_statuses,
        };

        let current_device_id = self.store_repository.get_device_key().await?;
        let all_sync_records = self.sync_repository.get_all_sync_records().await?;
        let all_devices = self
            .device_repository
            .get_devices_by_user_id(&user_id)
            .await?;

        let sync_set = SyncRecord::create_initial_device_sync_records(
            device.clone(),
            current_device_id,
            &all_sync_records,
            &all_devices,
            sync_record_set,
        );
        let resource_ids: Vec<String> = all_sync_records
            .iter()
            .filter(|record| record.resource_type == ResourceType::Resource)
            .map(|record| record.resource_id.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        // Create vector clocks for the new device for all resources
        let vector_clocks =
            ResourceVectorClock::create_entires_for_new_device(&resource_ids, &device.id);
        // Save the device first
        let _ = self.device_repository.save(device.clone()).await?;
        // Add the sync record set
        self.sync_repository.add_sync_record_set(sync_set).await?;
        self.vector_clock_repository
            .save_vector_clocks(&vector_clocks)
            .await?;
        Ok(())
    }

    pub async fn get_next_pending_sync(
        &self,
        device: &Device,
        user: &User,
        pending_resource_ids: Option<Arc<Mutex<Vec<String>>>>,
    ) -> Result<Option<SyncPayload>, RepositoryError> {
        info!("Getting pending syncs for device: {}", device.id);

        info!("pending resource_ids {:?}", pending_resource_ids);
        // Try each type in priority order

        // 1. Device syncs (highest priority)
        if let Some(payload) = self.get_device_sync_for_device(device).await? {
            return Ok(Some(payload));
        }

        // 2. Folder syncs
        if let Some(payload) = self.get_folder_sync_for_device(device).await? {
            return Ok(Some(payload));
        }

        // 3. Resource syncs
        if let Some(payload) = self.get_resource_sync_for_device(device, user).await? {
            return Ok(Some(payload));
        }

        // 4. Check for resource updates in the pending_resource_ids
        if let Some(pending_resources) = pending_resource_ids {
            // Lock the mutex to access the vector
            let mut resources = pending_resources.lock().await;

            // If we have any pending resources, pop one
            if !resources.is_empty() {
                let resource_id = resources.remove(0); // Pop the first item
                info!(
                    "Processing pending resource update for resource: {}",
                    resource_id
                );

                // Get the resource data for update
                let payload = self.get_resource_for_update(&resource_id).await?;
                return Ok(Some(payload));
            }
        }

        // 5. Device record status updates
        if let Some(payload) = self.get_unsynced_device_records(device).await? {
            return Ok(Some(payload));
        }

        Ok(None)
    }

    async fn get_device_sync_for_device(
        &self,
        device: &Device,
    ) -> Result<Option<SyncPayload>, RepositoryError> {
        if let Some((sync_record, device_records, statuses)) = self
            .sync_repository
            .get_pending_sync_by_type(&device.id, "device")
            .await?
        {
            // Get the device data
            let device_data = self
                .device_repository
                .find_by_id(&sync_record.resource_id)
                .await?;

            return Ok(Some(SyncPayload::DeviceSync {
                sync_record,
                device_records,
                device_record_statuses: statuses,
                device: device_data,
            }));
        }

        Ok(None)
    }

    async fn get_folder_sync_for_device(
        &self,
        device: &Device,
    ) -> Result<Option<SyncPayload>, RepositoryError> {
        if let Some((sync_record, device_records, statuses)) = self
            .sync_repository
            .get_pending_sync_by_type(&device.id, "folder")
            .await?
        {
            // Get the folder data
            let folder = self
                .folder_repository
                .find_by_id(&sync_record.resource_id)
                .await?;

            return Ok(Some(SyncPayload::FolderSync {
                sync_record,
                device_records,
                device_record_statuses: statuses,
                folder,
            }));
        }

        Ok(None)
    }

    async fn get_resource_sync_for_device(
        &self,
        device: &Device,
        user: &User,
    ) -> Result<Option<SyncPayload>, RepositoryError> {
        if let Some((sync_record, device_records, statuses)) = self
            .sync_repository
            .get_pending_sync_by_type(&device.id, "resource")
            .await?
        {
            // Get the resource with key
            let resource = self
                .resource_repository
                .find_resource_with_key(&sync_record.resource_id, &user.id)
                .await?;
            let vector_clocks = self
                .vector_clock_repository
                .get_vector_clocks_for_resource(&sync_record.resource_id)
                .await?;

            return Ok(Some(SyncPayload::ResourceSync {
                sync_record,
                device_records,
                device_record_statuses: statuses,
                resource,
                vector_clocks,
            }));
        }

        Ok(None)
    }

    async fn get_resource_for_update(
        &self,
        resource_id: &str,
    ) -> Result<SyncPayload, RepositoryError> {
        let resource = self.resource_repository.find_by_id_raw(resource_id).await?;
        let vector_clocks = self
            .vector_clock_repository
            .get_vector_clocks_for_resource(resource_id)
            .await?;
        Ok(SyncPayload::ResourceUpdate {
            resource,
            vector_clocks,
        })
    }

    async fn get_unsynced_device_records(
        &self,
        device: &Device,
    ) -> Result<Option<SyncPayload>, RepositoryError> {
        let unsynced_records = self
            .sync_repository
            .get_unsynced_device_sync_records(&device.id)
            .await?;

        if !unsynced_records.is_empty() {
            let mut all_device_records = Vec::new();
            let mut all_statuses = Vec::new();

            for (device_record, statuses) in unsynced_records {
                all_device_records.push(device_record);
                all_statuses.extend(statuses);
            }

            return Ok(Some(SyncPayload::StatusUpdate {
                device_records: all_device_records,
                device_record_statuses: all_statuses,
            }));
        }

        Ok(None)
    }

    pub async fn add_device_entry(&self, device: Device) -> Result<(), RepositoryError> {
        //TODO: handle check for device alreay here.
        self.device_repository.save(device).await
    }

    pub async fn process_acknowledgement(
        &self,
        ack: SyncAckType,
        device: Device,
    ) -> Result<Option<String>, RepositoryError> {
        let current_device_id = self.store_repository.get_device_key().await?;
        match ack {
            SyncAckType::FullSync {
                sync_record_id,
                device_record,
                mut device_sync_records,
            } => {
                info!(
                    "Processing full sync acknowledgement for sync record: {}",
                    sync_record_id
                );
                let synced_device_record_id = device_sync_records
                    .iter_mut()
                    .find_map(|dsr| {
                        if dsr.aware_device_id == current_device_id {
                            dsr.synced = true;
                            Some(dsr.id.clone())
                        } else {
                            None
                        }
                    })
                    .ok_or_else(|| {
                        RepositoryError::CustomError(
                            "No matching device sync record found for current device".to_string(),
                        )
                    })?;

                self.sync_repository
                    .add_status_change_set(StatusChangeSet {
                        device_record_statuses: device_sync_records,
                        device_record,
                    })
                    .await?;
                self.sync_repository
                    .update_device_record(device.id.clone(), sync_record_id.clone())
                    .await?;
                self.sync_repository
                    .update_device_record_statuses_for_sync(
                        sync_record_id.clone(),
                        device.id.clone(),
                    )
                    .await?;
                Ok(Some(synced_device_record_id))
            }
            SyncAckType::DeviceSyncRecord(record) => {
                info!("processing device sync record status {}", record);
                todo!();
            }

            SyncAckType::DeviceRecords(records) => {
                self.sync_repository
                    .update_device_sync_record_by_device_id(records, device.id)
                    .await?;
                Ok(None)
            }
            SyncAckType::UpdateRecieved(resource_id) => {
                info!("update recieved at remote");
                Ok(None)
            }
        }
    }

    pub async fn process_sync_payload<F>(
        &self,
        payload: &SyncPayload,
        user_id: &str,
        device_id: &str,
        emit_event: Option<F>,
    ) -> Result<SyncAckType, RepositoryError>
    where
        F: Fn(SyncEvent) + Send + Sync,
    {
        let current_device_id = self.store_repository.get_device_key().await?;

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
                    &current_device_id,
                    user_id,
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
                    &current_device_id,
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
                    &current_device_id,
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
                    &current_device_id,
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
        self.device_repository.save(device.clone()).await?;
        let (processed_records, processed_statuses, completion_records) = self
            .prepare_common_sync_data(
                sync_record,
                device_records,
                device_record_statuses,
                current_device_id,
                user_id,
            )
            .await?;

        // Save sync record set
        let record_set = SyncRecordSet {
            sync_record: sync_record.clone(),
            device_records: processed_records,
            device_record_statuses: processed_statuses,
        };
        self.sync_repository.add_sync_record_set(record_set).await?;

        Ok(SyncAckType::FullSync {
            sync_record_id: sync_record.id.clone(),
            device_record: completion_records.device_record,
            device_sync_records: completion_records.device_record_statuses,
        })
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
        let (processed_records, processed_statuses, completion_records) = self
            .prepare_common_sync_data(
                sync_record,
                device_records,
                device_record_statuses,
                current_device_id,
                user_id,
            )
            .await?;

        // DB WRITE OPERATIONS

        // Save resource and vector clocks
        self.resource_repository
            .save_resource_with_key(&resource.resource, &resource.key)
            .await?;

        self.vector_clock_repository
            .save_vector_clocks(vector_clocks)
            .await?;

        // Save sync record set
        let record_set = SyncRecordSet {
            sync_record: sync_record.clone(),
            device_records: processed_records,
            device_record_statuses: processed_statuses,
        };
        self.sync_repository.add_sync_record_set(record_set).await?;

        Ok(SyncAckType::FullSync {
            sync_record_id: sync_record.id.clone(),
            device_record: completion_records.device_record,
            device_sync_records: completion_records.device_record_statuses,
        })
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
        let (processed_records, processed_statuses, completion_records) = self
            .prepare_common_sync_data(
                sync_record,
                device_records,
                device_record_statuses,
                current_device_id,
                user_id,
            )
            .await?;

        // DB WRITE OPERATIONS

        // Save folder
        self.folder_repository.save(folder).await?;

        // Save sync record set
        let record_set = SyncRecordSet {
            sync_record: sync_record.clone(),
            device_records: processed_records,
            device_record_statuses: processed_statuses,
        };
        self.sync_repository.add_sync_record_set(record_set).await?;

        Ok(SyncAckType::FullSync {
            sync_record_id: sync_record.id.clone(),
            device_record: completion_records.device_record,
            device_sync_records: completion_records.device_record_statuses,
        })
    }

    // Process status update payload
    async fn process_status_update(
        &self,
        device_records: &[DeviceRecord],
        device_record_statuses: &[DeviceRecordStatus],
        current_device_id: &str,
    ) -> Result<SyncAckType, RepositoryError> {
        // Process device records
        let (processed_records, processed_statuses) = SyncRecord::process_device_records(
            device_records,
            device_record_statuses,
            current_device_id,
        );

        // Extract record IDs for acknowledgment
        let device_record_ids: Vec<_> = processed_records
            .iter()
            .map(|device_record| device_record.id.clone())
            .collect();

        // DB WRITE OPERATION

        // Update device record set
        let device_record_set = DeviceRecordSet {
            device_record_statuses: processed_statuses,
            device_records: processed_records,
        };
        self.sync_repository
            .update_device_record_set(device_record_set)
            .await?;

        Ok(SyncAckType::DeviceRecords(device_record_ids))
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

    // Helper function to prepare common data (no DB writes)
    async fn prepare_common_sync_data(
        &self,
        sync_record: &SyncRecord,
        device_records: &[DeviceRecord],
        device_record_statuses: &[DeviceRecordStatus],
        current_device_id: &str,
        user_id: &str,
    ) -> Result<(Vec<DeviceRecord>, Vec<DeviceRecordStatus>, StatusChangeSet), RepositoryError>
    {
        // Process device records
        let (mut processed_records, mut processed_statuses) = SyncRecord::process_device_records(
            device_records,
            device_record_statuses,
            current_device_id,
        );

        // Get other devices
        let excluded_devices = vec![current_device_id.to_string()];
        let devices = self
            .device_repository
            .get_devices_by_user_except(user_id, &excluded_devices)
            .await?;

        // Create completion records
        let completion_records = SyncRecord::create_completion_records(
            sync_record.id.clone(),
            current_device_id.to_string(),
            &devices,
        );

        // Add completion records to processed records
        processed_records.push(completion_records.device_record.clone());
        processed_statuses.extend(completion_records.device_record_statuses.clone());

        Ok((processed_records, processed_statuses, completion_records))
    }

    pub async fn add_folder_to_sync(
        &self,
        folder: Folder,
        user_id: &str,
    ) -> Result<SyncRecordSet, RepositoryError> {
        //TODO: move to transaction
        let current_device_id = self.store_repository.get_device_key().await?;
        let devices = self
            .device_repository
            .get_devices_by_user_except(user_id, vec![current_device_id.clone()].as_slice())
            .await?;
        let sync_record_set =
            SyncRecord::create_folder_sync_record(folder.id, current_device_id, &devices);
        Ok(sync_record_set)
    }

    pub async fn prepare_resource_to_sync(
        &self,
        resource: &Resource,
        user_id: &str,
        device: &Device,
    ) -> Result<(SyncRecordSet, Vec<ResourceVectorClock>), RepositoryError> {
        let current_device_id = self.store_repository.get_device_key().await?;
        let devices = self
            .device_repository
            .get_devices_by_user_except(user_id, vec![current_device_id.clone()].as_slice())
            .await?;
        let sync_record_set = SyncRecord::create_resource_sync_record(
            resource.id.clone(),
            current_device_id,
            &devices,
        );
        let device_ids: Vec<String> = devices
            .iter()
            .map(|d| d.id.clone())
            .chain(std::iter::once(device.id.clone()))
            .collect();
        let vector_clocks =
            ResourceVectorClock::create_initial_entries(&resource.id, &device_ids, &device.id);
        Ok((sync_record_set, vector_clocks))
    }

    // pub async fn add_soft_deletion_sync_record(
    //     &self,
    //     resource_id: String,
    //     resource_type: sync_types::ResourceType,
    // ) -> Result<(), RepositoryError> {
    //     // Get current device ID
    //     let user_id = self.get_current_user_id().await?;
    //     let current_device_id = self.store_repository.get_device_key().await?;
    //
    //     // Get all devices to sync with
    //     let devices = self.device_repository.get_all_devices().await?;
    //
    //     match resource_type {
    //         sync_types::ResourceType::Folder => {
    //             let resources = self
    //                 .resource_repository
    //                 .find_all_by_folder(&resource_id, &user_id)
    //                 .await?;
    //             let sync_sets = SyncRecord::create_soft_delete_folder_records(
    //                 resource_id,
    //                 resources,
    //                 current_device_id,
    //                 &devices,
    //             );
    //             //TODO: make this into a single transaction
    //             for sync_set in sync_sets {
    //                 self.sync_repository.add_sync_record_set(sync_set).await?;
    //             }
    //         }
    //         sync_types::ResourceType::Resource => {
    //             let sync_set = SyncRecord::create_soft_delete_resource_records(
    //                 resource_id,
    //                 current_device_id,
    //                 &devices,
    //             );
    //             self.sync_repository.add_sync_record_set(sync_set).await?;
    //         }
    //         _ => {
    //             return Err(RepositoryError::DatabaseError(
    //                 "Unsupported resource type for deletion".to_string(),
    //             ))
    //         }
    //     }
    //
    //     Ok(())
    // }

    //TODO: change these types of function to syncpayload domain
    pub fn generate_add_device_payload(
        &self,
        device: Device,
        records: SyncRecordSet,
    ) -> SyncPayload {
        SyncPayload::DeviceSync {
            device,
            sync_record: records.sync_record,
            device_records: records.device_records,
            device_record_statuses: records.device_record_statuses,
        }
    }

    pub async fn update_sync_status(
        &self,
        device_record_ids: Vec<String>,
        status_record_ids: Vec<String>,
    ) -> Result<(), RepositoryError> {
        self.sync_repository
            .update_sync_status(device_record_ids, status_record_ids)
            .await
    }
    pub async fn handle_ack_complete(&self, device_sync_record_id: String) -> Result<(), String> {
        info!(
            "updating device sync record status {:?}",
            device_sync_record_id
        );
        self.sync_repository
            .update_device_sync_record_status(device_sync_record_id)
            .await
            .map_err(|e| e.to_string())
    }

    pub async fn get_resources_needing_sync(
        &self,
        device_id: &str,
    ) -> Result<Vec<String>, RepositoryError> {
        // Check if device exists
        let device = match self.device_repository.find_by_id(device_id).await {
            Ok(device) => device,
            Err(RepositoryError::NotFound) => {
                // Device not found, return empty
                return Ok(Vec::new());
            }
            Err(e) => return Err(e),
        };

        // If device has never synced, return empty list
        if device.last_synced_at.is_none() {
            return Ok(Vec::new());
        }

        let last_synced_at = device.last_synced_at.unwrap();

        // Get all other devices
        let all_other_devices = self
            .device_repository
            .get_all_devices_except(&[device_id.to_string()])
            .await?;

        if all_other_devices.is_empty() {
            // No other devices to sync with
            return Ok(Vec::new());
        }

        let device_resource_ids = self
            .sync_repository
            .get_resource_ids_for_device(&device.id)
            .await?;
        // Get resource IDs that need syncing based on vector clocks
        let needs_sync = self
            .vector_clock_repository
            .get_resource_ids_needing_updates(&device_resource_ids, last_synced_at, device_id)
            .await?;

        Ok(needs_sync)
    }

    pub async fn merge_updated_doc(
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

        // Update vector clocks - handles both adding new ones and updating existing ones
        self.vector_clock_repository
            .update_vector_clocks(update_vector, add_vector)
            .await?;

        Ok(())
    }
}
