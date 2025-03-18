use osvauld_core::models::device::Device;
use osvauld_core::models::p2p::{SyncAckType, SyncData, SyncPayload};
use osvauld_core::models::user::User;
use osvauld_core::models::{
    folder::Folder,
    resource::Resource,
    sync_record::{DeviceRecordSet, StatusChangeSet, SyncRecord, SyncRecordSet, SyncUpdateData},
};

use osvauld_core::repositories::{
    DeviceRepository, FolderRepository, RepositoryError, ResourceRepository, StoreRepository,
    SyncRepository,
};

use log::info;

use std::sync::Arc;

pub struct SyncService {
    sync_repository: Arc<dyn SyncRepository>,
    folder_repository: Arc<dyn FolderRepository>,
    resource_repository: Arc<dyn ResourceRepository>,
    device_repository: Arc<dyn DeviceRepository>,
    store_repository: Arc<dyn StoreRepository>,
}
impl SyncService {
    pub fn new(
        sync_repository: Arc<dyn SyncRepository>,
        folder_repository: Arc<dyn FolderRepository>,
        resource_repository: Arc<dyn ResourceRepository>,
        device_repository: Arc<dyn DeviceRepository>,
        store_repository: Arc<dyn StoreRepository>,
    ) -> Self {
        Self {
            sync_repository,
            folder_repository,
            resource_repository,
            device_repository,
            store_repository,
        }
    }

    pub async fn add_new_device_sync(
        &self,
        sync_payload: SyncPayload,
        user_id: String,
    ) -> Result<(), RepositoryError> {
        let device = match &sync_payload.data {
            Some(SyncData::Device(device)) => device.clone(),
            _ => {
                return Err(RepositoryError::DatabaseError(
                    "Invalid sync payload: expected device data".to_string(),
                ));
            }
        };
        let sync_record_set = match &sync_payload.sync_record {
            Some(sync_record) => SyncRecordSet {
                sync_record: sync_record.clone(),
                device_records: sync_payload.device_records.clone(),
                device_record_statuses: sync_payload.device_record_statuses.clone(),
            },
            None => {
                return Err(RepositoryError::DatabaseError(
                    "Invalid sync payload: missing sync record for device addition".to_string(),
                ));
            }
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
        // Save the device first
        let _ = self.device_repository.save(device.clone()).await?;
        // Add the sync record set
        self.sync_repository.add_sync_record_set(sync_set).await?;
        Ok(())
    }

    pub async fn get_next_pending_sync(
        &self,
        device: &Device,
        user: &User,
    ) -> Result<Option<SyncPayload>, RepositoryError> {
        info!("Getting pending syncs for device: {}", device.id);

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

        // 4. Device record status updates
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

            return Ok(Some(SyncPayload {
                sync_record: Some(sync_record),
                device_records,
                device_record_statuses: statuses,
                data: Some(SyncData::Device(device_data)),
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

            return Ok(Some(SyncPayload {
                sync_record: Some(sync_record),
                device_records,
                device_record_statuses: statuses,
                data: Some(SyncData::Folder(folder)),
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

            return Ok(Some(SyncPayload {
                sync_record: Some(sync_record),
                device_records,
                device_record_statuses: statuses,
                data: Some(SyncData::Resource(resource)),
            }));
        }

        Ok(None)
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

            return Ok(Some(SyncPayload {
                sync_record: None,
                device_records: all_device_records,
                device_record_statuses: all_statuses,
                data: None,
            }));
        }

        Ok(None)
    }

    pub async fn add_device_entry(&self, device: Device) -> Result<(), RepositoryError> {
        //TODO: handle check for device alreay here.
        self.device_repository.save(device).await
    }

    // pub async fn mark_sync_complete(
    //     &self,
    //     sync_id: &str,
    //     device: Device,
    // ) -> Result<(), RepositoryError> {
    //     let current_device_id = self.store_repository.get_device_key().await?;
    //     let devices = self
    //         .device_repository
    //         .get_devices_except(vec![current_device_id.clone(), device.id.clone()].as_slice())
    //         .await?;
    //     let status_change_records = SyncRecord::create_completion_records(
    //         sync_id.to_string(),
    //         device.id.clone(),
    //         current_device_id,
    //         &devices,
    //     );
    //     self.sync_repository
    //         .update_device_record(device.id.clone(), sync_id.to_string())
    //         .await?;
    //     self.sync_repository
    //         .add_status_change_set(status_change_records)
    //         .await?;
    //     Ok(())
    // }

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
        }
    }
    pub async fn process_sync_payload(
        &self,
        payload: &SyncPayload,
        user_id: String,
    ) -> Result<SyncAckType, RepositoryError> {
        let current_device_id = self.store_repository.get_device_key().await?;
        let (mut device_records, mut device_record_statuses) = SyncRecord::process_device_records(
            &payload.device_records,
            &payload.device_record_statuses,
            &current_device_id,
        );

        if let Some(sync_record) = &payload.sync_record {
            if let Some(data) = &payload.data {
                match data {
                    SyncData::Folder(folder) => self.folder_repository.save(folder).await?,
                    SyncData::Resource(resource_key_pair) => {
                        self.resource_repository
                            .save_resource_with_key(
                                &resource_key_pair.resource,
                                &resource_key_pair.key,
                            )
                            .await?
                    }
                    SyncData::Device(device) => {
                        if device.id != current_device_id {
                            self.device_repository.save(device.clone()).await?
                        };
                    }
                    _ => {}
                }
            }

            let excluded_devices = vec![current_device_id.clone()];
            let devices = self
                .device_repository
                .get_devices_by_user_except(&user_id, &excluded_devices)
                .await?;
            let status_change_records = SyncRecord::create_completion_records(
                sync_record.id.clone(),
                current_device_id,
                &devices,
            );
            device_records.push(status_change_records.device_record.clone());
            device_record_statuses.extend(status_change_records.device_record_statuses.clone());

            let record_set = SyncRecordSet {
                sync_record: sync_record.clone(),
                device_records,
                device_record_statuses,
            };
            self.sync_repository.add_sync_record_set(record_set).await?;
            Ok(SyncAckType::FullSync {
                sync_record_id: sync_record.id.clone(),
                device_record: status_change_records.device_record.clone(),
                device_sync_records: status_change_records.device_record_statuses.clone(),
            })
        } else {
            let device_record_ids: Vec<_> = device_records
                .iter()
                .map(|device_record| device_record.id.clone())
                .collect();
            let device_record_set = DeviceRecordSet {
                device_record_statuses,
                device_records,
            };
            self.sync_repository
                .update_device_record_set(device_record_set)
                .await?;
            Ok(SyncAckType::DeviceRecords(device_record_ids))
        }
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
        resource: Resource,
        user_id: &str,
    ) -> Result<SyncRecordSet, RepositoryError> {
        let current_device_id = self.store_repository.get_device_key().await?;
        let devices = self
            .device_repository
            .get_devices_by_user_except(user_id, vec![current_device_id.clone()].as_slice())
            .await?;
        let sync_record_set =
            SyncRecord::create_resource_sync_record(resource.id, current_device_id, &devices);
        Ok(sync_record_set)
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
        SyncPayload {
            data: Some(SyncData::Device(device)),
            sync_record: Some(records.sync_record),
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
}
