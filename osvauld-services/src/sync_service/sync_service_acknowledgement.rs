use osvauld_core::models::device::Device;
use osvauld_core::models::p2p::SyncAckType;
use osvauld_core::models::sync_types::SyncOperations;
use osvauld_core::repositories::RepositoryError;

use log::info;

use super::sync_service_core::SyncService;

impl SyncService {
    pub async fn process_acknowledgement(
        &self,
        ack: SyncAckType,
        device: &Device,
        current_device_id: &str,
    ) -> Result<Option<String>, RepositoryError> {
        //TODO: we need to send the pending device record id as well for which it was completed
        match ack {
            SyncAckType::FullSync(operations) => {
                self.process_fullsync_acknowledgment(operations, current_device_id)
                    .await
            }
            SyncAckType::DeviceSyncRecord(record) => {
                info!("processing device sync record status {}", record);
                todo!();
            }

            SyncAckType::DeviceRecords(records) => {
                // Use db transaction method for updating device sync records
                self.db
                    .update_device_sync_records(records, device.id.clone())
                    .await?;
                Ok(None)
            }
            SyncAckType::UpdateRecieved(resource_id) => {
                info!("update recieved at remote");
                Ok(None)
            }
        }
    }

    async fn process_fullsync_acknowledgment(
        &self,
        operations: SyncOperations,
        current_device_id: &str,
    ) -> Result<Option<String>, RepositoryError> {
        // First, update any status records for the current device to have synced=true
        let mut modified_operations = operations.clone();

        // Find any status records for the current device and mark them as synced
        let mut synced_device_record_id = None;

        for status in &mut modified_operations.status_records_to_add {
            if status.aware_device_id == current_device_id {
                status.synced = true;
                // Keep track of the first status ID for the current device (if any)
                if synced_device_record_id.is_none() {
                    synced_device_record_id = Some(status.id.clone());
                }
            }
        }

        // Apply all operations to the database
        self.db.apply_sync_operations(&modified_operations).await?;

        // Return the ID of the first status record for the current device, if any was found
        Ok(synced_device_record_id)
    }

    pub async fn update_sync_status(
        &self,
        device_record_ids: Vec<String>,
        status_record_ids: Vec<String>,
    ) -> Result<(), RepositoryError> {
        // Use db transaction method for updating sync status
        self.db
            .update_sync_status(device_record_ids, status_record_ids)
            .await
    }

    pub async fn handle_ack_complete(&self, device_sync_record_id: String) -> Result<(), String> {
        info!(
            "updating device sync record status {:?}",
            device_sync_record_id
        );
        // Use db transaction method for completing acknowledgment
        self.db
            .complete_ack(device_sync_record_id)
            .await
            .map_err(|e| e.to_string())
    }
}
