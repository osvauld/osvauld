use crate::domains::models::device::Device;
use crate::domains::models::p2p::{SyncData, SyncPayload};
use crate::domains::models::sync_types;
use crate::domains::models::{
    credential::Credential,
    folder::Folder,
    sync_record::{DeviceRecord, DeviceRecordStatus, StatusChangeSet, SyncRecord, SyncRecordSet},
    sync_types::{OperationType, ResourceType, SyncStatus},
};
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

    pub async fn add_new_device_sync(&self, device: Device) -> Result<Device, RepositoryError> {
        let _ = self.device_repository.save(device.clone()).await;
        let current_device_id = self.store_repository.get_device_key().await?;
        let current_device = self
            .device_repository
            .find_by_id(&current_device_id)
            .await?;
        let records = self.sync_repository.get_all_sync_records().await?;
        let devices = self.device_repository.get_all_devices().await?;
        let device_record_set = SyncRecord::create_initial_device_sync_records(
            device.id,
            current_device_id,
            &records,
            &devices,
        );
        self.sync_repository
            .add_initial_device_sync_set(device_record_set)
            .await?;
        Ok(current_device)
    }

    pub async fn get_next_pending_sync(
        &self,
        device: &Device,
    ) -> Result<Option<SyncPayload>, RepositoryError> {
        // Try device syncs first
        if let Some((sync_record, device_records, statuses)) = self
            .sync_repository
            .get_pending_sync_by_type(&device.id, "device")
            .await?
        {
            let device = self
                .device_repository
                .find_by_id(&sync_record.resource_id)
                .await?;
            return Ok(Some(SyncPayload {
                sync_record: Some(sync_record),
                device_records,
                device_record_statuses: statuses,
                data: Some(SyncData::Device(device)),
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

        // Finally status updates
        let status_updates = self
            .sync_repository
            .get_unsynced_status_updates(&device.id)
            .await?;
        if !status_updates.is_empty() {
            let (device_record, statuses) = status_updates.into_iter().next().unwrap();
            return Ok(Some(SyncPayload {
                sync_record: None,
                device_records: vec![device_record],
                device_record_statuses: statuses,
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
    ) -> (Vec<DeviceRecord>, Vec<DeviceRecordStatus>) {
        let updated_records = device_records
            .iter()
            .map(|record| {
                let mut r = record.clone();
                if r.device_id == current_device_id {
                    r.status = SyncStatus::Completed;
                    r.synced = true;
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
                }
                s
            })
            .collect();

        (updated_records, updated_statuses)
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
            device.id,
            current_device_id,
            &devices,
        );
        self.sync_repository
            .add_status_change_set(status_change_records)
            .await?;
        Ok(())
    }

    pub async fn process_sync_payload(&self, payload: &SyncPayload) -> Result<(), RepositoryError> {
        let current_device_id = self.store_repository.get_device_key().await?;
        let (device_records, device_statuses) = self.process_device_records(
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
                    SyncData::Device(device) => self.device_repository.save(device.clone()).await?,
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

        Ok(())
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

    async fn process_sync_record_sync(
        &self,
        sync_record: &SyncRecord,
    ) -> Result<(), RepositoryError> {
        // Check if we already have this sync record
        let _ = self
            .sync_repository
            .find_by_id(&sync_record.id)
            .await
            .is_ok();

        // Save the sync record
        self.sync_repository
            .save_sync_records(&[sync_record.clone()])
            .await?;

        Ok(())
    }
}
