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
    SyncRepository, UserRepository, VectorClockRepository,
};

use log::info;

use std::sync::Arc;

pub enum SyncEvent {
    UpdateEvent {
        remote_resource: Resource,
        vector_clock: Vec<ResourceVectorClock>,
        device_id: String,
        user_id: String,
    },
}

pub struct SyncService {
    pub sync_repository: Arc<dyn SyncRepository>,
    pub folder_repository: Arc<dyn FolderRepository>,
    pub resource_repository: Arc<dyn ResourceRepository>,
    pub device_repository: Arc<dyn DeviceRepository>,
    pub store_repository: Arc<dyn StoreRepository>,
    pub vector_clock_repository: Arc<dyn VectorClockRepository>,
    pub user_repository: Arc<dyn UserRepository>,
}

impl SyncService {
    pub fn new(
        sync_repository: Arc<dyn SyncRepository>,
        folder_repository: Arc<dyn FolderRepository>,
        resource_repository: Arc<dyn ResourceRepository>,
        device_repository: Arc<dyn DeviceRepository>,
        store_repository: Arc<dyn StoreRepository>,
        vector_clock_repository: Arc<dyn VectorClockRepository>,
        user_repository: Arc<dyn UserRepository>,
    ) -> Self {
        Self {
            sync_repository,
            folder_repository,
            resource_repository,
            device_repository,
            store_repository,
            vector_clock_repository,
            user_repository,
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

        // Use db transaction method for saving device sync
        self.db_save_device_sync(&device, &sync_set, &vector_clocks)
            .await?;

        Ok(())
    }

    pub async fn add_device_entry(&self, device: Device) -> Result<(), RepositoryError> {
        //TODO: handle check for device alreay here.
        self.device_repository.save(&device).await
    }

    pub async fn process_acknowledgement(
        &self,
        ack: SyncAckType,
        device: &Device,
        current_device_id: &str,
    ) -> Result<Option<String>, RepositoryError> {
        //TODO: we need to send the pending device record id as well for which it was completed
        match ack {
            SyncAckType::FullSync {
                sync_record_id,
                device_record,
                mut device_sync_records,
            } => {
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

                // Use db transaction method for processing full sync ack
                self.db_process_full_sync_ack(
                    device.id.clone(),
                    sync_record_id.clone(),
                    StatusChangeSet {
                        device_record_statuses: device_sync_records,
                        device_record,
                    },
                )
                .await?;

                Ok(Some(synced_device_record_id))
            }
            SyncAckType::DeviceSyncRecord(record) => {
                info!("processing device sync record status {}", record);
                todo!();
            }

            SyncAckType::DeviceRecords(records) => {
                // Use db transaction method for updating device sync records
                self.db_update_device_sync_records(records, device.id.clone())
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
        let (processed_records, processed_statuses, completion_records) = self
            .prepare_common_sync_data(
                sync_record,
                device_records,
                device_record_statuses,
                current_device_id,
                devices,
            )
            .await?;

        // Create sync record set
        let record_set = SyncRecordSet {
            sync_record: sync_record.clone(),
            device_records: processed_records,
            device_record_statuses: processed_statuses,
        };

        // Use db transaction method for saving device sync
        self.db_save_device_sync(device, &record_set, &[]).await?;

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

        let devices = self
            .get_user_other_devices(current_device_id, user_id)
            .await?;
        let (processed_records, processed_statuses, completion_records) = self
            .prepare_common_sync_data(
                sync_record,
                device_records,
                device_record_statuses,
                current_device_id,
                devices,
            )
            .await?;

        // Create sync record set
        let record_set = SyncRecordSet {
            sync_record: sync_record.clone(),
            device_records: processed_records,
            device_record_statuses: processed_statuses,
        };

        // Use db transaction method for saving resource sync
        self.db_save_resource_sync(resource, vector_clocks, &record_set)
            .await?;

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
        let devices = self
            .get_user_other_devices(current_device_id, user_id)
            .await?;
        let (processed_records, processed_statuses, completion_records) = self
            .prepare_common_sync_data(
                sync_record,
                device_records,
                device_record_statuses,
                current_device_id,
                devices,
            )
            .await?;

        // Create sync record set
        let record_set = SyncRecordSet {
            sync_record: sync_record.clone(),
            device_records: processed_records,
            device_record_statuses: processed_statuses,
        };

        // Use db transaction method for saving folder sync
        self.db_save_folder_sync(folder, &record_set).await?;

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

        // Create device record set
        let device_record_set = DeviceRecordSet {
            device_record_statuses: processed_statuses,
            device_records: processed_records,
        };

        // Use db transaction method for updating device record set
        self.db_update_device_record_set(device_record_set).await?;

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
        let (processed_records, processed_statuses, completion_records) = self
            .prepare_common_sync_data(
                sync_record,
                device_records,
                device_record_statuses,
                current_device_id,
                all_devices,
            )
            .await?;

        // Create sync record set
        let record_set = SyncRecordSet {
            sync_record: sync_record.clone(),
            device_records: processed_records,
            device_record_statuses: processed_statuses,
        };

        // Add appropriate DB transaction method
        // Something like this:
        self.db_add_new_user(user, devices, &record_set).await?;

        Ok(SyncAckType::FullSync {
            sync_record_id: sync_record.id.clone(),
            device_record: completion_records.device_record,
            device_sync_records: completion_records.device_record_statuses,
        })
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
        devices: Vec<Device>,
    ) -> Result<(Vec<DeviceRecord>, Vec<DeviceRecordStatus>, StatusChangeSet), RepositoryError>
    {
        // Process device records
        let (mut processed_records, mut processed_statuses) = SyncRecord::process_device_records(
            device_records,
            device_record_statuses,
            current_device_id,
        );

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
            .get_user_other_devices(&current_device_id, user_id)
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
            .get_user_other_devices(&current_device_id, user_id)
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
        // Use db transaction method for updating sync status
        self.db_update_sync_status(device_record_ids, status_record_ids)
            .await
    }

    pub async fn handle_ack_complete(&self, device_sync_record_id: String) -> Result<(), String> {
        info!(
            "updating device sync record status {:?}",
            device_sync_record_id
        );
        // Use db transaction method for completing acknowledgment
        self.db_complete_ack(device_sync_record_id)
            .await
            .map_err(|e| e.to_string())
    }

    pub async fn merge_updated_doc(
        &self,
        doc: &str,
        add_vector: &[ResourceVectorClock],
        update_vector: &[ResourceVectorClock],
        resource_id: &str,
    ) -> Result<(), RepositoryError> {
        // Use db transaction method for merging updated document
        self.db_merge_document(doc, add_vector, update_vector, resource_id)
            .await
    }

    pub async fn process_first_user_connection(
        &self,
        user: &User,
        devices: &Vec<Device>,
        current_user_id: &str,
        current_device_id: &str,
    ) -> Result<(User, Vec<Device>, SyncRecordSet), RepositoryError> {
        let user_devices = self
            .device_repository
            .get_devices_by_user_except(current_user_id, &[current_device_id.to_string()])
            .await?;
        let mut new_user = user.clone();
        new_user.first_sync = false;
        let current_user = self.user_repository.get_user_by_id(current_user_id).await?;

        let user_addition_record = SyncRecord::create_user_sync_record(
            user.id.clone(),
            current_device_id.to_string(),
            &user_devices,
        );
        self.db_add_new_user(&new_user, devices, &user_addition_record)
            .await?;
        Ok((current_user, user_devices, user_addition_record))
    }

    pub async fn process_first_user_connection_response(
        &self,
        user: &User,
        devices: &Vec<Device>,
        user_addition_record: &SyncRecordSet,
        current_user_id: &str,
        current_device_id: &str,
    ) -> Result<SyncRecordSet, RepositoryError> {
        let user_devices = self
            .device_repository
            .get_devices_by_user_except(current_user_id, &[current_device_id.to_string()])
            .await?;
        let remote_user_addition_record = SyncRecord::create_user_sync_record(
            user.id.clone(),
            current_device_id.to_string(),
            &user_devices,
        );

        self.db_complete_new_user_add(
            &user.id,
            devices,
            &remote_user_addition_record,
            &user_addition_record,
        )
        .await?;
        Ok(remote_user_addition_record)
    }

    pub async fn handle_user_add_ack(
        &self,
        remote_user_id: &str,
        user_addition_records: &SyncRecordSet,
    ) -> Result<(), RepositoryError> {
        // Use the abstracted DB function
        self.db_handle_user_add_ack(remote_user_id, &user_addition_records)
            .await?;
        Ok(())
    }

    async fn get_user_other_devices(
        &self,
        current_device_id: &str,
        user_id: &str,
    ) -> Result<Vec<Device>, RepositoryError> {
        let excluded_devices = vec![current_device_id.to_string()];
        self.device_repository
            .get_devices_by_user_except(user_id, &excluded_devices)
            .await
    }
}
