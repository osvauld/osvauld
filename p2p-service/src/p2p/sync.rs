use crate::p2p::peer_connection::PeerConnection;
use osvauld_services::SyncEvent;

use osvauld_core::models::device::Device;
use osvauld_core::models::p2p::{Message, SyncAckType, SyncPayload, UpdateResource};

use super::P2PEvent;
use tracing::{debug, error, info, instrument, Span};

// Helper method signatures to reduce repeated patterns
impl PeerConnection {
    /// Helper method to get current device with early return pattern
    #[instrument(skip(self), fields(connection_id = %self.get_id()), level = "debug")]
    async fn ensure_current_device(&self) -> Result<Device, String> {
        debug!("Retrieving current device");

        match self.get_local_device().await {
            Some(device) => {
                debug!(device_id = %device.id, "Current device retrieved successfully");
                Ok(device)
            }
            None => {
                error!(connection_id = %self.get_id(), "Current device not found");
                Err("Local device not found".into())
            }
        }
    }

    pub async fn start_device_sync(&self) -> Result<(), String> {
        info!("Starting device sync phase");

        // Get current device with early return pattern
        let current_device = self.ensure_current_device().await?;

        // Get current span for context propagation
        let current_span = Span::current();

        // Get the current phase
        let current_phase = self.phase.get_current_phase().await;

        // Get next pending sync from sync service, specifying the current phase
        match self
            .context
            .sync_service
            .get_next_pending_sync(
                &self.device,
                &self.user,
                Some(self.pending_resource_ids.clone()),
                current_phase,
                current_span,
            )
            .await
        {
            Ok(Some(payload)) => {
                info!("Sending sync payload for device sync phase");
                let message = Message::SyncResponse(payload);
                self.send_message(message).await?;
                Ok(())
            }
            Ok(None) => {
                // No pending syncs for this phase
                info!("No pending device syncs, completing phase");
                self.complete_current_phase().await
            }
            Err(e) => {
                error!(error = %e, "Failed to get pending device syncs");
                Err(format!("Failed to get pending device syncs: {}", e))
            }
        }
    }

    #[instrument(skip(self, records), fields(
        connection_id = %self.get_id(),
        record_count = ?{if let SyncPayload::DeviceSync { device_records, .. } = &records { device_records.len() } else { 0 }}
    ), level = "info")]
    pub async fn handle_add_device_request(&self, records: SyncPayload) -> Result<(), String> {
        info!("Processing device addition request");

        let current_span = Span::current();

        match self
            .context
            .sync_service
            .add_new_device_sync(records, self.user.id.clone(), current_span)
            .await
        {
            Ok(_) => {
                let ack_message = Message::AddDeviceAck;
                match self.send_message(ack_message).await {
                    Ok(_) => {
                        info!("Device addition processed successfully");

                        // Mark the AddDevice phase as complete
                        self.complete_current_phase().await?;

                        // The phase transition logic will handle moving to DeviceSync

                        Ok(())
                    }
                    Err(e) => {
                        error!("Failed to send device add acknowledgment: {}", e);
                        Err(format!("Failed to send acknowledgment: {}", e))
                    }
                }
            }
            Err(e) => {
                error!(error = %e, "Failed to add device sync records");
                Err(format!("Failed to add device: {}", e))
            }
        }
    }

    pub async fn get_and_send_next_sync(&self) -> Result<(), String> {
        debug!("Retrieving next pending sync");
        let current_span = Span::current();
        let current_phase = self.phase.get_current_phase().await;
        match self
            .context
            .sync_service
            .get_next_pending_sync(
                &self.device,
                &self.user,
                Some(self.pending_resource_ids.clone()),
                current_phase,
                current_span,
            )
            .await
        {
            Ok(Some(payload)) => {
                info!(
                    payload_type = ?std::mem::discriminant(&payload),
                    "Sending sync payload response"
                );
                let message = Message::SyncResponse(payload);
                self.send_message(message).await?;
                Ok(())
            }
            Ok(None) => {
                info!("No more pending syncs, for current phase");
                self.complete_current_phase().await?;
                Ok(())
            }
            Err(e) => {
                error!(error = %e, "Failed to get next pending sync");
                Err(format!("Failed to get next pending sync: {}", e))
            }
        }
    }

    #[instrument(skip(self, ack_type), fields(
        connection_id = %self.get_id(),
        ack_type = ?std::mem::discriminant(&ack_type)
    ), level = "info")]
    pub async fn handle_sync_ack(&self, ack_type: SyncAckType) -> Result<(), String> {
        info!("Processing sync acknowledgment");

        // Get current device with early return pattern
        let current_device = self.ensure_current_device().await?;

        // Get current span for context propagation
        let current_span = Span::current();

        // Process acknowledgment
        match self
            .context
            .sync_service
            .process_acknowledgement(ack_type, &self.device, &current_device.id, current_span)
            .await
        {
            Ok(Some(record_ids)) => {
                debug!(
                    record_count = record_ids.len(),
                    "Records processed successfully, sending ACK complete"
                );

                // Send acknowledgment complete message
                let ack_complete_msg = Message::AckComplete(record_ids);
                self.send_message(ack_complete_msg).await?;

                // Get next pending sync
                self.get_and_send_next_sync().await
            }
            Ok(None) => {
                debug!("No records to acknowledge, continuing with next sync");

                // Get next pending sync
                self.get_and_send_next_sync().await
            }
            Err(e) => {
                error!(error = %e, "Failed to process acknowledgment");
                Err(format!("Failed to process acknowledgment: {}", e))
            }
        }
    }

    #[instrument(skip(self, payload), fields(
        connection_id = %self.get_id(),
        payload_type = ?std::mem::discriminant(&payload)
    ), level = "info")]
    pub async fn handle_sync_response(&self, payload: SyncPayload) -> Result<(), String> {
        info!("Processing sync response");

        // Get current device with early return
        let current_device = self.ensure_current_device().await?;

        // Get current span for propagation
        let current_span = Span::current();

        // Create event adapter
        let event_adapter = self.sync_event_adapter();

        // Process sync payload
        let ack_message = match self
            .context
            .sync_service
            .process_sync_payload(
                &payload,
                &self.user.id,
                &self.device.id,
                Some(event_adapter),
                &current_device.id,
                &current_device.user_id,
                current_span,
            )
            .await
        {
            Ok(sync_ack) => {
                debug!(
                    ack_type = ?std::mem::discriminant(&sync_ack),
                    "Sync payload processed successfully"
                );
                Message::SyncAck(sync_ack)
            }
            Err(e) => {
                error!(error = %e, "Failed to process sync payload");
                return Err(format!("Failed to process sync payload: {}", e));
            }
        };

        // Send acknowledgment
        match self.send_message(ack_message).await {
            Ok(_) => {
                info!("Sync response processed successfully");
                Ok(())
            }
            Err(e) => {
                error!(error = %e, "Failed to send sync acknowledgment");
                Err(format!("Failed to send acknowledgment: {}", e))
            }
        }
    }

    #[instrument(skip(self, device_record_status_ids), fields(
        connection_id = %self.get_id(),
        record_count = device_record_status_ids.len()
    ), level = "info")]
    pub async fn ack_complete(&self, device_record_status_ids: Vec<String>) -> Result<(), String> {
        info!("Processing acknowledgment complete");

        let current_span = Span::current();
        match self
            .context
            .sync_service
            .handle_ack_complete(device_record_status_ids, current_span)
            .await
        {
            Ok(_) => {
                info!("Acknowledgment complete processed successfully");
                Ok(())
            }
            Err(e) => {
                error!(error = %e, "Failed to handle acknowledgment complete");
                Err(format!("Failed to process acknowledgment complete: {}", e))
            }
        }
    }

    #[instrument(skip(self, payload), fields(connection_id = %self.get_id()), level = "debug")]
    pub async fn send_update(&self, payload: String) -> Result<(), String> {
        debug!("Sending update payload");
        info!("Update notification sent");
        Ok(())
    }

    // Helper to adapt sync events to P2P events
    fn sync_event_adapter(&self) -> impl Fn(SyncEvent) + Send + Sync {
        let event_emitter = self.event_emitter.clone();

        move |sync_event| {
            // Map SyncEvent to P2PEvent
            let p2p_event = match sync_event {
                SyncEvent::UpdateEvent {
                    remote_resource,
                    vector_clock,
                    device_id,
                    user_id,
                } => P2PEvent::UpdateEvent {
                    remote_resource,
                    vector_clock,
                    device_id,
                    user_id,
                },
            };
            event_emitter.emit(p2p_event);
        }
    }

    #[instrument(skip(self, payload), fields(
        connection_id = %self.get_id(),
        resource_id = %payload.resource_id
    ), level = "info")]
    pub async fn handle_merge_update(&self, payload: &UpdateResource) -> Result<(), String> {
        info!("Processing merge update");

        let current_span = Span::current();
        match self
            .context
            .sync_service
            .merge_updated_doc(
                &payload.encrypted_data,
                &payload.add_vector_clock,
                &payload.update_vector_clock,
                &payload.resource_id,
                current_span,
            )
            .await
        {
            Ok(_) => {
                info!("Merge update processed successfully");
                Ok(())
            }
            Err(e) => {
                error!(error = %e, "Failed to merge updated document");
                Err(format!("Failed to merge document: {}", e))
            }
        }
    }
    #[instrument(skip(self), fields(connection_id = %self.get_id()), level = "info")]
    pub async fn start_add_device_sync(&self) -> Result<(), String> {
        info!("Starting add device sync process");

        // For non-initiators, there's nothing to do - we wait for the AddDevice message
        if !self.is_initiator {
            debug!("This connection is not the initiator, waiting for AddDevice message");
            return Ok(());
        }

        let current_device = match self.get_local_device().await {
            Some(device) => device,
            None => return Err("Local device not found".into()),
        };

        // Get the add device payload from the sync service
        match self
            .context
            .sync_service
            .get_add_device_record_set(&current_device.id)
            .await
        {
            Ok(record_set) => {
                info!("Add device sync payload retrieved successfully");
                let add_device_payload = SyncPayload::DeviceSync {
                    sync_record: record_set.sync_record,
                    device_records: record_set.device_records,
                    device_record_statuses: record_set.device_record_statuses,
                    device: current_device.clone(),
                };
                let message = Message::AddDevice(add_device_payload);

                match self.send_message(message).await {
                    Ok(_) => {
                        info!("Add device sync message sent successfully");
                        Ok(())
                    }
                    Err(e) => {
                        error!(error = %e, "Failed to send add device message");
                        Err(format!("Failed to send add device message: {}", e))
                    }
                }
            }
            Err(e) => {
                error!(error = %e, "Failed to get add device payload");
                Err(format!("Failed to get add device payload: {}", e))
            }
        }
    }
}
