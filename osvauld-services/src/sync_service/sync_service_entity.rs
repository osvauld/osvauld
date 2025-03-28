use osvauld_core::models::device::Device;
use osvauld_core::models::folder::Folder;
use osvauld_core::models::p2p::SyncPayload;
use osvauld_core::models::resource::Resource;
use osvauld_core::models::sync_record::{SyncRecord, SyncRecordSet};
use osvauld_core::models::sync_types::ResourceType;
use osvauld_core::models::vector_clock::ResourceVectorClock;
use osvauld_core::repositories::RepositoryError;

use log::info;

use super::sync_service_core::SyncService;

impl SyncService {
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
        self.db
            .save_device_sync(&device, &sync_set, &vector_clocks)
            .await?;

        Ok(())
    }

    pub async fn add_device_entry(&self, device: Device) -> Result<(), RepositoryError> {
        //TODO: handle check for device alreay here.
        self.device_repository.save(&device).await
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
}
