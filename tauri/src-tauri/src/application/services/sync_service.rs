use crate::database::schema::device_record_status;
use crate::domains::models::device::Device;
use crate::domains::models::p2p::{SyncData, SyncPayload};
use crate::domains::models::{
    credential::Credential,
    folder::Folder,
    sync_record::{DeviceRecord, DeviceRecordStatus, StatusChangeSet, SyncRecord, SyncRecordSet},
    sync_types::{OperationType, ResourceType, SyncStatus},
};
use crate::domains::models::{sync_record, sync_types};
use crate::domains::repositories::{
    CredentialRepository, DeviceRepository, FolderRepository, RepositoryError, StoreRepository,
    SyncRepository,
};
use log::{error, info};

use std::sync::Arc;

pub struct SyncService {
    sync_repository: Arc<dyn SyncRepository>,
    folder_repository: Arc<dyn FolderRepository>,
    credential_repository: Arc<dyn CredentialRepository>,
    device_repository: Arc<dyn DeviceRepository>,
    store_repository: Arc<dyn StoreRepository>,
}

impl SyncService {
    pub fn new(
        sync_repository: Arc<dyn SyncRepository>,
        folder_repository: Arc<dyn FolderRepository>,
        credential_repository: Arc<dyn CredentialRepository>,
        device_repository: Arc<dyn DeviceRepository>,
        store_repository: Arc<dyn StoreRepository>,
    ) -> Self {
        Self {
            sync_repository,
            folder_repository,
            credential_repository,
            device_repository,
            store_repository,
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

        // Then credentials
        if let Some((sync_record, device_records, statuses)) = self
            .sync_repository
            .get_pending_sync_by_type(&device.id, "credential")
            .await?
        {
            let credential = self
                .credential_repository
                .find_by_id(&sync_record.resource_id)
                .await?;

            return Ok(Some(SyncPayload {
                sync_record: Some(sync_record),
                device_records,
                device_record_statuses: statuses,
                data: Some(SyncData::Credential(credential)),
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

    fn process_device_records(
        &self,
        device_records: &[DeviceRecord],
        device_statuses: &[DeviceRecordStatus],
        current_device_id: &str,
    ) -> (
        Vec<DeviceRecord>,
        Vec<DeviceRecordStatus>,
        Vec<String>, // (device_record_id, sync_record_id) for synced records
        Vec<String>, // device_record_status_ids that were marked as synced
    ) {
        let mut synced_record_ids = Vec::new();
        let mut synced_status_ids = Vec::new();

        let updated_records = device_records
            .iter()
            .map(|record| {
                let mut r = record.clone();
                if r.device_id == current_device_id {
                    r.status = SyncStatus::Completed;
                    r.synced = true;
                    synced_record_ids.push(r.id.clone());
                }
                r
            })
            .collect();

        let updated_statuses = device_statuses
            .iter()
            .map(|status| {
                let mut s = status.clone();
                if s.aware_device_id == current_device_id {
                    s.synced = true;
                    synced_status_ids.push(s.id.clone());
                }
                s
            })
            .collect();

        (
            updated_records,
            updated_statuses,
            synced_record_ids,
            synced_status_ids,
        )
    }

    pub async fn mark_sync_complete(
        &self,
        sync_id: &str,
        device: Device,
    ) -> Result<(), RepositoryError> {
        let current_device_id = self.store_repository.get_device_key().await?;
        let devices = self
            .device_repository
            .get_devices_except(vec![current_device_id.clone(), device.id.clone()].as_slice())
            .await?;
        let status_change_records = SyncRecord::create_completion_records(
            sync_id.to_string(),
            device.id.clone(),
            current_device_id,
            &devices,
        );
        self.sync_repository
            .update_device_record(device.id.clone(), sync_id.to_string())
            .await?;
        self.sync_repository
            .add_status_change_set(status_change_records)
            .await?;
        Ok(())
    }

    pub async fn process_sync_payload(
        &self,
        payload: &SyncPayload,
    ) -> Result<(Vec<String>, Vec<String>), RepositoryError> {
        let current_device_id = self.store_repository.get_device_key().await?;
        let (device_records, device_statuses, synced_record_ids, synced_status_ids) = self
            .process_device_records(
                &payload.device_records,
                &payload.device_record_statuses,
                &current_device_id,
            );

        if let Some(sync_record) = &payload.sync_record {
            if let Some(data) = &payload.data {
                match data {
                    SyncData::Folder(folder) => self.folder_repository.save(folder).await?,
                    SyncData::Credential(credential) => {
                        self.credential_repository.save(credential).await?
                    }
                    SyncData::Device(device) => {
                        if device.id != current_device_id {
                            self.device_repository.save(device.clone()).await?
                        };
                    }
                    _ => {}
                }
            }

            let record_set = SyncRecordSet {
                sync_record: sync_record.clone(),
                device_records,
                device_record_statuses: device_statuses,
            };
            self.sync_repository.add_sync_record_set(record_set).await?;
        } else {
            let status_set = StatusChangeSet {
                device_records,
                device_record_statuses: device_statuses,
            };
            self.sync_repository
                .add_status_change_set(status_set)
                .await?;
        }

        Ok((synced_record_ids, synced_status_ids))
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

    pub async fn add_credential_to_sync(
        &self,
        credential: Credential,
    ) -> Result<(), RepositoryError> {
        let current_device_id = self.store_repository.get_device_key().await?;
        let devices = self
            .device_repository
            .get_devices_except(vec![current_device_id.clone()].as_slice())
            .await?;
        let sync_record_set =
            SyncRecord::create_credential_sync_record(credential.id, current_device_id, &devices);
        self.sync_repository
            .add_sync_record_set(sync_record_set)
            .await?;
        Ok(())
    }

    pub async fn add_soft_deletion_sync_record(
        &self,
        resource_id: String,
        resource_type: sync_types::ResourceType,
    ) -> Result<(), RepositoryError> {
        // Get current device ID
        let current_device_id = self.store_repository.get_device_key().await?;

        // Get all devices to sync with
        let devices = self.device_repository.get_all_devices().await?;

        match resource_type {
            sync_types::ResourceType::Folder => {
                let credentials = self
                    .credential_repository
                    .find_all_by_folder(&resource_id)
                    .await?;
                let sync_sets = SyncRecord::create_soft_delete_folder_records(
                    resource_id,
                    credentials,
                    current_device_id,
                    &devices,
                );
                //TODO: make this into a single transaction
                for sync_set in sync_sets {
                    self.sync_repository.add_sync_record_set(sync_set).await?;
                }
            }
            sync_types::ResourceType::Credential => {
                let sync_set = SyncRecord::create_soft_delete_credential_records(
                    resource_id,
                    current_device_id,
                    &devices,
                );
                self.sync_repository.add_sync_record_set(sync_set).await?;
            }
            _ => {
                return Err(RepositoryError::DatabaseError(
                    "Unsupported resource type for deletion".to_string(),
                ))
            }
        }

        Ok(())
    }

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
}
