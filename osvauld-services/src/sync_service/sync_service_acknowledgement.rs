use osvauld_core::models::device::Device;
use osvauld_core::models::p2p::SyncAckType;
use osvauld_core::models::sync_types::SyncOperations;
use osvauld_core::repositories::RepositoryError;

use tracing::{Span, debug, error, info, instrument, trace};

use super::sync_service_core::SyncService;

impl SyncService {
    #[instrument(
        skip(self, ack, device, current_span), 
        fields(
            ack_type = ?std::mem::discriminant(&ack),
            device_id = %device.id,
            current_device_id = %current_device_id
        ),
        level = "debug"
    )]
    pub async fn process_acknowledgement(
        &self,
        ack: SyncAckType,
        device: &Device,
        current_device_id: &str,
        current_span: Span,
    ) -> Result<Option<Vec<String>>, RepositoryError> {
        // Enter the parent span
        let _guard = current_span.enter();

        info!("Processing acknowledgement");

        //TODO: we need to send the pending device record id as well for which it was completed
        match ack {
            SyncAckType::FullSync(operations) => {
                debug!(
                    records_to_add = operations.records_to_add.len(),
                    status_records_to_add = operations.status_records_to_add.len(),
                    record_ids_to_update = operations.record_ids_to_update.len(),
                    status_ids_to_update = operations.status_ids_to_update.len(),
                    "Processing full sync acknowledgment"
                );

                let result = self
                    .process_fullsync_acknowledgment(operations, current_device_id)
                    .await;

                match &result {
                    Ok(Some(ids)) => {
                        info!(
                            record_ids = ids.len(),
                            "Full sync acknowledgment processed with record IDs"
                        );
                    }
                    Ok(None) => {
                        info!("Full sync acknowledgment processed with no record IDs");
                    }
                    Err(e) => {
                        error!(error = %e, "Failed to process full sync acknowledgment");
                    }
                }

                result
            }

            SyncAckType::DeviceSyncRecords(records) => {
                debug!(
                    record_count = records.len(),
                    "Processing device sync records acknowledgment"
                );

                // Use db transaction method for updating device sync records
                match self.db.update_device_sync_records(&records).await {
                    Ok(_) => {
                        info!("Device sync records updated successfully");
                        Ok(None)
                    }
                    Err(e) => {
                        error!(error = %e, "Failed to update device sync records");
                        Err(e)
                    }
                }
            }

            SyncAckType::UpdateReceived => {
                info!("Update received at remote");
                Ok(None)
            }
        }
    }

    #[instrument(
        skip(self, operations), 
        fields(
            records_to_add = operations.records_to_add.len(),
            status_records_to_add = operations.status_records_to_add.len(),
            record_ids_to_update = operations.record_ids_to_update.len(),
            status_ids_to_update = operations.status_ids_to_update.len(),
            current_device_id = %current_device_id
        ),
        level = "debug"
    )]
    pub async fn process_fullsync_acknowledgment(
        &self,
        operations: SyncOperations,
        current_device_id: &str,
    ) -> Result<Option<Vec<String>>, RepositoryError> {
        debug!("Processing full sync acknowledgment");

        // First, update any status records for the current device to have synced=true
        let mut modified_operations = operations.clone();

        // Find all status records for the current device and mark them as synced
        let mut synced_device_record_ids = Vec::new();

        debug!("Marking current device status records as synced");
        for (i, status) in modified_operations
            .status_records_to_add
            .iter_mut()
            .enumerate()
        {
            if status.aware_device_id == current_device_id {
                trace!(
                    index = i,
                    status_id = %status.id,
                    aware_device_id = %status.aware_device_id,
                    "Marking status record as synced"
                );
                status.synced = true;
                synced_device_record_ids.push(status.id.clone());
            }
        }

        debug!(
            synced_records = synced_device_record_ids.len(),
            "Applying sync operations to database"
        );

        // Apply all operations to the database
        match self.db.apply_sync_operations(&modified_operations).await {
            Ok(_) => debug!("Sync operations applied successfully"),
            Err(e) => {
                error!(error = %e, "Failed to apply sync operations");
                return Err(e);
            }
        };

        // Return all status record IDs for the current device
        if synced_device_record_ids.is_empty() {
            info!("No device record IDs synced");
            Ok(None)
        } else {
            info!(
                record_ids = synced_device_record_ids.len(),
                "Device record IDs synced"
            );
            Ok(Some(synced_device_record_ids))
        }
    }

    #[instrument(
        skip(self), 
        fields(
            device_record_ids = device_record_ids.len(),
            status_record_ids = status_record_ids.len()
        ),
        level = "debug"
    )]
    pub async fn update_sync_status(
        &self,
        device_record_ids: Vec<String>,
        status_record_ids: Vec<String>,
    ) -> Result<(), RepositoryError> {
        info!("Updating sync status");

        // Use db transaction method for updating sync status
        match self
            .db
            .update_sync_status(device_record_ids, status_record_ids)
            .await
        {
            Ok(_) => {
                info!("Sync status updated successfully");
                Ok(())
            }
            Err(e) => {
                error!(error = %e, "Failed to update sync status");
                Err(e)
            }
        }
    }

    #[instrument(
        skip(self), 
        fields(
            device_sync_record_ids = device_sync_record_ids.len()
        ),
        level = "debug"
    )]
    pub async fn handle_ack_complete(
        &self,
        device_sync_record_ids: Vec<String>,
        current_span: Span,
    ) -> Result<(), RepositoryError> {
        // Enter the parent span
        let _guard = current_span.enter();

        info!(
            record_ids = ?device_sync_record_ids,
            "Handling acknowledgment complete"
        );

        // Use db transaction method for completing acknowledgment
        match self.db.complete_ack(device_sync_record_ids).await {
            Ok(_) => {
                info!("Acknowledgment complete handled successfully");
                Ok(())
            }
            Err(e) => {
                error!(error = %e, "Failed to complete acknowledgment");
                Err(e)
            }
        }
    }
}
