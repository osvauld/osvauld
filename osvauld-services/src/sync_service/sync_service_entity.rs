use super::sync_service_core::SyncService;
use osvauld_core::models::p2p::SyncPayload;
use osvauld_core::models::sync_record::{SyncRecord, SyncRecordSet};
use osvauld_core::models::sync_types::ResourceType;
use osvauld_core::models::vector_clock::ResourceVectorClock;
use osvauld_core::repositories::RepositoryError;
use tracing::{Span, debug, error, info, instrument};

impl SyncService {

    #[instrument(
    skip(self, sync_payload, current_span), 
    fields(
        user_id = %user_id,
        payload_type = ?std::mem::discriminant(&sync_payload)
    ),
    level = "info"
    )]
    pub async fn add_new_device_sync(
        &self,
        sync_payload: SyncPayload,
        user_id: String,
        current_span: Span,
    ) -> Result<(), RepositoryError> {
         let _guard = current_span.enter();
        info!("Adding new device sync");

        // Extract data from sync payload
        debug!("Extracting data from sync payload");
        let (device, sync_record, device_records, device_record_statuses) = match sync_payload {
            SyncPayload::DeviceSync {
                sync_record,
                device_records,
                device_record_statuses,
                device,
            } => {
                debug!(
                    device_id = %device.id,
                    sync_record_id = %sync_record.id,
                    device_records = device_records.len(),
                    status_records = device_record_statuses.len(),
                    "Extracted device sync data"
                );
                (device, sync_record, device_records, device_record_statuses)
            }
            _ => {
                error!("Invalid sync payload type: expected DeviceSync");
                return Err(RepositoryError::DatabaseError(
                    "Invalid sync payload: expected DeviceSync payload".to_string(),
                ));
            }
        };

        // Create the sync record set with the extracted data
        debug!("Creating sync record set");
        let sync_record_set = SyncRecordSet {
            sync_record,
            device_records,
            device_record_statuses,
        };

        // Get current device ID
        debug!("Getting current device key");
        let current_device_id = match self.store_repository.get_device_key().await {
            Ok(id) => {
                debug!(
                    current_device_id = %id,
                    "Retrieved current device key"
                );
                id
            }
            Err(e) => {
                error!(
                    error = %e,
                    "Failed to get current device key"
                );
                return Err(e);
            }
        };

        // Get all sync records
        debug!("Getting all sync records");
        let all_sync_records = match self.sync_repository.get_all_sync_records().await {
            Ok(records) => {
                debug!(record_count = records.len(), "Retrieved all sync records");
                records
            }
            Err(e) => {
                error!(
                    error = %e,
                    "Failed to get all sync records"
                );
                return Err(e);
            }
        };

        // Get all devices for this user
        debug!("Getting all devices for user");
        let all_devices = match self
            .device_repository
            .get_devices_by_user_id(&user_id)
            .await
        {
            Ok(devices) => {
                debug!(device_count = devices.len(), "Retrieved devices for user");
                devices
            }
            Err(e) => {
                error!(
                    error = %e,
                    user_id = %user_id,
                    "Failed to get devices for user"
                );
                return Err(e);
            }
        };

        // Create initial device sync records
        debug!("Creating initial device sync records");
        let sync_set = SyncRecord::create_initial_device_sync_records(
            device.clone(),
            current_device_id,
            &all_sync_records,
            &all_devices,
            sync_record_set,
        );

        debug!(
            sync_record_id = %sync_set.sync_record.id,
            device_records = sync_set.device_records.len(),
            status_records = sync_set.device_record_statuses.len(),
            "Initial device sync records created"
        );

        // Filter and collect resource IDs
        debug!("Collecting resource IDs from sync records");
        let resource_ids: Vec<String> = all_sync_records
            .iter()
            .filter(|record| record.resource_type == ResourceType::Resource)
            .map(|record| record.resource_id.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        debug!(
            resource_id_count = resource_ids.len(),
            "Resource IDs collected"
        );

        // Create vector clocks for the new device for all resources
        debug!("Creating vector clocks for new device");
        let vector_clocks =
            ResourceVectorClock::create_entires_for_new_device(&resource_ids, &device.id);

        debug!(
            vector_clock_count = vector_clocks.len(),
            "Vector clocks created"
        );

        // Use db transaction method for saving device sync
        debug!("Saving device sync to database");
        match self
            .db
            .save_device_sync(&device, &sync_set, &vector_clocks)
            .await
        {
            Ok(_) => {
                debug!("Device sync saved successfully");
            }
            Err(e) => {
                error!(
                    error = %e,
                    device_id = %device.id,
                    "Failed to save device sync"
                );
                return Err(e);
            }
        };

        info!(
            device_id = %device.id,
            "New device sync added successfully"
        );
        Ok(())
    }

}
