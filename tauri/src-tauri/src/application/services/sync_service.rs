use crate::domains::models::device::Device;
use crate::domains::models::p2p::{SyncAckType, SyncData, SyncPayload};
use crate::domains::models::{
    folder::Folder,
    resource::Resource,
    sync_record::{DeviceRecordSet, StatusChangeSet, SyncRecord, SyncRecordSet},
    sync_types::{OperationType, ResourceType},
};

use crate::domains::repositories::{
    DeviceRepository, FolderRepository, RepositoryError, ResourceRepository, StoreRepository,
    SyncRepository,
};
use crypto_utils::{get_key_id, CryptoUtils};

use log::info;
use tokio::sync::Mutex;

use std::sync::Arc;

pub struct SyncService {
    sync_repository: Arc<dyn SyncRepository>,
    folder_repository: Arc<dyn FolderRepository>,
    resource_repository: Arc<dyn ResourceRepository>,
    device_repository: Arc<dyn DeviceRepository>,
    store_repository: Arc<dyn StoreRepository>,
    crypto_utils: Arc<Mutex<CryptoUtils>>,
}

impl SyncService {
    pub fn new(
        sync_repository: Arc<dyn SyncRepository>,
        folder_repository: Arc<dyn FolderRepository>,
        resource_repository: Arc<dyn ResourceRepository>,
        device_repository: Arc<dyn DeviceRepository>,
        store_repository: Arc<dyn StoreRepository>,
        crypto_utils: Arc<Mutex<CryptoUtils>>,
    ) -> Self {
        Self {
            sync_repository,
            folder_repository,
            resource_repository,
            device_repository,
            store_repository,
            crypto_utils,
        }
    }

    pub async fn add_new_device_sync(
        &self,
        sync_payload: SyncPayload,
    ) -> Result<(), RepositoryError> {
        let device = match &sync_payload.data {
            Some(SyncData::Device(device)) => device.clone(),
            _ => {
                return Err(RepositoryError::DatabaseError(
                    "Invalid sync payload: expected device data".to_string(),
                ))
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
                ))
            }
        };

        let current_device_id = self.store_repository.get_device_key().await?;
        let all_sync_records = self.sync_repository.get_all_sync_records().await?;
        let all_devices = self.device_repository.get_all_devices().await?;

        let sync_set = SyncRecord::create_initial_device_sync_records(
            device.id.clone(),
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
    ) -> Result<Option<SyncPayload>, RepositoryError> {
        // Try device syncs first
        let user_id = self
            .get_current_user_id()
            .await
            .map_err(|e| RepositoryError::CustomError(e.to_string()))?;
        info!("getting records for********* {}", device.id);
        if let Some((sync_record, device_records, statuses)) = self
            .sync_repository
            .get_pending_sync_by_type(&device.id, "device")
            .await?
        {
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

        // Then folders
        if let Some((sync_record, device_records, statuses)) = self
            .sync_repository
            .get_pending_sync_by_type(&device.id, "folder")
            .await?
        {
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

        // Then resources
        if let Some((sync_record, device_records, statuses)) = self
            .sync_repository
            .get_pending_sync_by_type(&device.id, "resource")
            .await?
        {
            let resource = self
                .resource_repository
                .find_resource_with_key(&sync_record.resource_id, &user_id)
                .await?;
            return Ok(Some(SyncPayload {
                sync_record: Some(sync_record),
                device_records,
                device_record_statuses: statuses,
                data: Some(SyncData::Resource(resource)),
            }));
        }

        // Check for unsynced device records

        let unsynced_device_records = self
            .sync_repository
            .get_unsynced_device_sync_records(&device.id)
            .await?;
        if !unsynced_device_records.is_empty() {
            // Collect all device records and their statuses
            let mut all_device_records = Vec::new();
            let mut all_statuses = Vec::new();

            for (device_record, statuses) in unsynced_device_records {
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
                    .update_device_record(device.id, sync_record_id)
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

            let devices = self
                .device_repository
                .get_devices_except(vec![current_device_id.clone()].as_slice())
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

    pub async fn add_folder_to_sync(&self, folder: Folder) -> Result<(), RepositoryError> {
        let current_device_id = self.store_repository.get_device_key().await?;
        let devices = self
            .device_repository
            .get_devices_except(vec![current_device_id.clone()].as_slice())
            .await?;
        let sync_record_set =
            SyncRecord::create_folder_sync_record(folder.id, current_device_id, &devices);
        self.sync_repository
            .add_sync_record_set(sync_record_set)
            .await?;

        Ok(())
    }

    pub async fn prepare_resource_to_sync(
        &self,
        resource: Resource,
    ) -> Result<SyncRecordSet, RepositoryError> {
        let current_device_id = self.store_repository.get_device_key().await?;
        let devices = self
            .device_repository
            .get_devices_except(vec![current_device_id.clone()].as_slice())
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
        self.sync_repository
            .update_device_sync_record_status(device_sync_record_id)
            .await
            .map_err(|e| e.to_string())
    }

    async fn get_current_user_id(&self) -> Result<String, String> {
        let public_key = {
            let crypto = self.crypto_utils.lock().await;
            crypto.get_public_key().map_err(|e| e.to_string())?
        };

        get_key_id(&public_key).map_err(|e| e.to_string())
    }

    pub async fn add_resource_update_to_sync(
        &self,
        resource_id: String,
    ) -> Result<Option<SyncRecordSet>, RepositoryError> {
        // Get current device ID
        let current_device_id = self.store_repository.get_device_key().await?;

        // Get all devices
        let all_devices = self.device_repository.get_all_devices().await?;

        // Get devices that already have pending sync records for this resource
        let devices_with_pending = self
            .sync_repository
            .get_devices_with_pending_sync(&resource_id, "update")
            .await?;

        // Create a set of device IDs with pending sync for efficient lookup
        let pending_device_ids: std::collections::HashSet<String> =
            devices_with_pending.into_iter().collect();

        // Find devices that need new sync records
        let mut devices_needing_update = Vec::new();

        for device in &all_devices {
            // Skip current device - we don't need a sync record for it
            if device.id == current_device_id {
                continue;
            }

            // Check if this device already has a pending sync record
            if !pending_device_ids.contains(&device.id) {
                // This device needs a new update record
                devices_needing_update.push(device.clone());
            }
        }

        // If no devices need updates, return None
        if devices_needing_update.is_empty() {
            return Ok(None);
        }

        // Create sync records for devices that need updates
        let sync_record_set = SyncRecord::create_update_record(
            resource_id,
            ResourceType::Resource,
            OperationType::Update,
            current_device_id,
            &devices_needing_update,
        );

        Ok(Some(sync_record_set))
    }
}
