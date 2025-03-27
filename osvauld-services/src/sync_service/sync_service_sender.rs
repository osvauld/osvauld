use osvauld_core::models::device::Device;
use osvauld_core::models::p2p::SyncPayload;
use osvauld_core::models::user::User;
use osvauld_core::repositories::RepositoryError;

use log::info;

use std::sync::Arc;
use tokio::sync::Mutex;

// Import the SyncService struct to implement methods on it
use crate::sync_service::SyncService;

// Implement methods related to sending/retrieving sync data on SyncService
impl SyncService {
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
        // 2. User syncs
        if let Some(payload) = self.get_user_sync_for_device(device).await? {
            return Ok(Some(payload));
        }
        // 3. Folder syncs
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

    async fn get_user_sync_for_device(
        &self,
        device: &Device,
    ) -> Result<Option<SyncPayload>, RepositoryError> {
        if let Some((sync_record, device_records, statuses)) = self
            .sync_repository
            .get_pending_sync_by_type(&device.id, "user")
            .await?
        {
            // Get the user data
            let user = self
                .user_repository
                .get_user_by_id(&sync_record.resource_id)
                .await?;

            // Get devices associated with this user
            let user_devices = self
                .device_repository
                .get_devices_by_user_id(&user.id)
                .await?;

            return Ok(Some(SyncPayload::UserSync {
                sync_record,
                device_records,
                device_record_statuses: statuses,
                user,
                devices: user_devices,
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

    pub async fn get_payload_for_first_user_sync(
        &self,
        user_id: &str,
    ) -> Result<(User, Vec<Device>), RepositoryError> {
        let devices = self
            .device_repository
            .get_devices_by_user_id(user_id)
            .await?;
        let user = self.user_repository.get_user_by_id(user_id).await?;
        Ok((user, devices))
    }
}
