use crate::domains::models::device::Device;
use crate::domains::models::sync_record::{self, SyncStatus};
use crate::domains::models::{credential::Credential, folder::Folder, sync_record::SyncRecord};
use crate::domains::repositories::{
    CredentialRepository, DeviceRepository, FolderRepository, RepositoryError, StoreRepository,
    SyncRepository,
};
use log::{error, info};
use serde::{Deserialize, Serialize};
use std::ffi::c_uint;
use std::sync::Arc;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SyncPayload {
    pub sync_record: SyncRecord,
    pub data: SyncData,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type")]
pub enum SyncData {
    Folder(Folder),
    Credential(Credential),
    Device(Device),
    SyncRecord(SyncRecord),
}

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
        let mut all_sync_records: Vec<SyncRecord> = Vec::new();
        let mut current_device_pub_key: String = "".to_string();
        //create sync records for existing devices
        let devices = self.device_repository.get_all_devices().await?;
        for target_device in devices {
            if target_device.id != current_device_id {
                let mut sync_record = SyncRecord::new_device_sync(
                    device.id.clone(),
                    sync_record::OperationType::Create,
                    current_device_id.clone(),
                    target_device.id.clone(),
                );
                //if current device status should be completed
                if target_device.id == current_device_id {
                    sync_record.status = SyncStatus::Completed;
                    current_device_pub_key = target_device.device_key;
                }
                all_sync_records.push(sync_record);
            }
        }

        let existing_sync_records = self
            .sync_repository
            .get_sync_records_by_device_id(&current_device_id)
            .await?;
        let content_records = existing_sync_records
            .into_iter()
            .map(|record| record.create_new_sync_for_device(device.id.clone()))
            .collect::<Vec<_>>();
        all_sync_records.extend(content_records);
        if !all_sync_records.is_empty() {
            self.sync_repository
                .save_sync_records(&all_sync_records)
                .await?;
        }

        let current_device = Device::new(current_device_id, current_device_pub_key);
        Ok(current_device)
    }

    pub async fn get_next_pending_sync(
        &self,
        device: &Device,
    ) -> Result<Option<SyncPayload>, RepositoryError> {
        // Get the next pending sync record for target device
        let pending_records = self.sync_repository.get_pending_records(&device.id).await?;
        info!("Found {} pending records", pending_records.len());
        // Get the oldest pending record
        let sync_record = match pending_records.first() {
            Some(record) => record.clone(),
            None => return Ok(None),
        };

        // Fetch associated data based on resource type
        let data = match sync_record.resource_type {
            sync_record::ResourceType::Folder => {
                let folder = self
                    .folder_repository
                    .find_by_id(&sync_record.resource_id)
                    .await?;
                SyncData::Folder(folder)
            }
            sync_record::ResourceType::Credential => {
                let credential = self
                    .credential_repository
                    .find_by_id(&sync_record.resource_id)
                    .await?;
                log::info!("found credential {:?}", credential);
                SyncData::Credential(credential)
            }
            sync_record::ResourceType::Device => {
                let device = self
                    .device_repository
                    .find_by_id(&sync_record.resource_id)
                    .await?;
                SyncData::Device(device)
            }
            sync_record::ResourceType::Record => SyncData::SyncRecord(sync_record.clone()),
            _ => {
                return Err(RepositoryError::DatabaseError(format!(
                    "Unknown resource type: {:?}",
                    sync_record.resource_type
                )))
            }
        };

        Ok(Some(SyncPayload { sync_record, data }))
    }

    pub async fn add_device_entry(&self, device: Device) -> Result<(), RepositoryError> {
        //TODO: handle check for device alreay here.
        self.device_repository.save(device).await
    }

    pub async fn update_sync_status(
        &self,
        sync_id: &str,
        status: &str,
    ) -> Result<(), RepositoryError> {
        self.sync_repository.update_status(sync_id, status).await
    }

    pub async fn process_sync_payload(&self, payload: &SyncPayload) -> Result<(), RepositoryError> {
        log::info!(
            "Processing sync payload for resource: {:?}",
            payload.sync_record.resource_type
        );

        match &payload.data {
            SyncData::Folder(folder) => {
                log::info!("Processing folder sync: {}", folder.id);
                match &payload.sync_record.operation_type {
                    sync_record::OperationType::Create => {
                        log::info!("Creating folder: {}", folder.name);
                        self.folder_repository.save(folder).await?;
                    }
                    sync_record::OperationType::Update => {
                        log::info!("Updating folder: {}", folder.name);
                        self.folder_repository.save(folder).await?;
                    }
                    sync_record::OperationType::Delete => {
                        log::info!("Delete operation for folder: {}", folder.id);
                        // TODO: Implement delete in folder repository
                        // self.folder_repository.delete(&folder.id).await?;
                    }
                    op => {
                        log::error!("Unknown folder operation: {:?}", op);
                        return Err(RepositoryError::DatabaseError(format!(
                            "Unknown operation type: {:?}",
                            op
                        )));
                    }
                }
            }
            SyncData::Credential(credential) => {
                log::info!("Processing credential sync: {}", credential.id);
                match &payload.sync_record.operation_type {
                    sync_record::OperationType::Create => {
                        log::info!("Creating credential in folder: {}", credential.folder_id);
                        self.credential_repository.save(credential).await?;
                    }
                    sync_record::OperationType::Update => {
                        log::info!("Updating credential: {}", credential.id);
                        self.credential_repository.save(credential).await?;
                    }
                    sync_record::OperationType::Delete => {
                        log::info!("Delete operation for credential: {}", credential.id);
                        // TODO: Implement delete in credential repository
                        // self.credential_repository.delete(&credential.id).await?;
                    }
                    sync_record::OperationType::SoftDelete => {
                        info!("soft delete");
                        self.credential_repository
                            .soft_delete_credential(&credential.id)
                            .await?;
                    }
                    op => {
                        log::error!("Unknown credential operation: {:?}", op);
                        return Err(RepositoryError::DatabaseError(format!(
                            "Unknown operation type: {:?}",
                            op
                        )));
                    }
                }
            }
            SyncData::Device(device) => {
                let current_device_id = self.store_repository.get_device_key().await?;
                info!("processing device sync record");
                match &payload.sync_record.operation_type {
                    sync_record::OperationType::Create => {
                        if current_device_id != device.id {
                            self.device_repository.save(device.clone()).await?;
                        };
                    }
                    op => {
                        todo!("other operation types not implemented {:?}", op)
                    }
                };
            }

            SyncData::SyncRecord(record) => {
                self.sync_repository
                    .save_sync_records(&[record.clone()])
                    .await?;
            }
        }

        // Update the sync record status if needed
        // self.sync_repository.update_status(
        //     &payload.sync_record.id,
        //     "processed"
        // ).await?;

        log::info!("Successfully processed sync payload");
        Ok(())
    }

    pub async fn add_folder_to_sync(&self, folder: Folder) -> Result<(), RepositoryError> {
        let current_device_id = self.store_repository.get_device_key().await?;
        let devices = self.device_repository.get_all_devices().await?;
        for device in devices {
            let mut sync_record = SyncRecord::new_folder_sync(
                folder.id.clone(),
                sync_record::OperationType::Create,
                current_device_id.clone(),
                device.id.clone(),
            );
            if current_device_id == device.id.clone() {
                sync_record.status = sync_record::SyncStatus::Completed;
            }
            self.sync_repository
                .save_sync_records(&[sync_record])
                .await?;
        }
        Ok(())
    }

    pub async fn add_credential_to_sync(
        &self,
        credential: Credential,
    ) -> Result<(), RepositoryError> {
        let current_device_id = self.store_repository.get_device_key().await?;
        let devices = self.device_repository.get_all_devices().await?;
        for device in devices {
            let mut sync_record = SyncRecord::new_credential_sync(
                credential.id.clone(),
                sync_record::OperationType::Create,
                current_device_id.clone(),
                device.id.clone(),
            );
            if current_device_id == device.id.clone() {
                sync_record.status = sync_record::SyncStatus::Completed
            }
            self.sync_repository
                .save_sync_records(&[sync_record])
                .await?;
        }
        Ok(())
    }

    pub async fn add_soft_deletion_sync_record(
        &self,
        resource_id: String,
        resource_type: sync_record::ResourceType,
    ) -> Result<(), RepositoryError> {
        // Get current device ID
        let current_device_id = self.store_repository.get_device_key().await?;

        // Get all devices to sync with
        let devices = self.device_repository.get_all_devices().await?;

        let sync_records: Vec<SyncRecord> = devices
            .into_iter()
            .map(|device| {
                let mut sync_record = match resource_type {
                    sync_record::ResourceType::Credential => SyncRecord::new_credential_sync(
                        resource_id.clone(),
                        sync_record::OperationType::SoftDelete,
                        current_device_id.clone(),
                        device.id.clone(),
                    ),
                    sync_record::ResourceType::Folder => SyncRecord::new_folder_sync(
                        resource_id.clone(),
                        sync_record::OperationType::SoftDelete,
                        current_device_id.clone(),
                        device.id.clone(),
                    ),
                    _ => panic!("Unsupported resource type for deletion sync"),
                };

                // Mark as completed for current device
                if current_device_id == device.id {
                    sync_record.status = sync_record::SyncStatus::Completed;
                }

                sync_record
            })
            .collect();

        // Save all sync records
        self.sync_repository
            .save_sync_records(&sync_records)
            .await?;

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
