use crate::p2p::peer_connection::PeerConnection;
use osvauld_services::SyncEvent;

use log::{error, info};
use osvauld_core::models::p2p::{Message, SyncAckType, SyncPayload, UpdateResource};

use super::P2PEvent;

impl PeerConnection {
    pub async fn add_device(&self, records: SyncPayload, ticket: String) -> Result<(), String> {
        // First establish connection with the target device using the ticket
        // self.connect_with_ticket(&ticket, ConnectionType::Device)
        //     .await?;
        //
        // // Once connected, send the AddDevice message
        // info!("Connection established, sending AddDevice message");
        // let add_device_message = Message::AddDevice(records);
        // let serialized = serde_json::to_string(&add_device_message)
        //     .map_err(|e| format!("Failed to serialize AddDevice message: {}", e))?;
        //
        // // Send the message and wait for acknowledgment
        // self.send_message(serialized).await?;

        Ok(())
    }

    pub async fn start_device_sync(&self) -> Result<(), String> {
        let sync_request = Message::SyncRequest;
        self.send_message(sync_request).await?;
        info!("Sync process started");
        Ok(())
    }

    pub async fn handle_sync_request(&self) -> Result<(), String> {
        info!("Handling incoming sync request");
        match self
            .context
            .sync_service
            .get_next_pending_sync(
                &self.device,
                &self.user,
                Some(self.pending_resource_ids.clone()),
            )
            .await
        {
            Ok(Some(payload)) => {
                let message = Message::SyncResponse(payload);
                self.send_message(message).await?;
            }
            Ok(None) => {
                info!("No pending syncs, sending sync complete");
                let message = Message::SyncComplete;
                self.send_message(message).await?;
            }
            Err(e) => {
                error!("Failed to get pending sync: {}", e);
                return Err(e.to_string());
            }
        }
        Ok(())
    }

    pub async fn handle_add_device_request(&self, records: SyncPayload) -> Result<(), String> {
        self.context
            .sync_service
            .add_new_device_sync(records, self.user.id.clone())
            .await
            .map_err(|e| e.to_string())?;
        // Create and send acknowledgment message
        let ack_message = Message::AddDeviceAck;

        // Send the acknowledgment
        self.send_message(ack_message).await?;

        info!("Successfully processed device addition request");
        Ok(())
    }

    pub async fn handle_sync_ack(&self, ack_type: SyncAckType) -> Result<(), String> {
        if let Some(current_device) = self.get_local_device().await {
            let device_sync_record_id = self
                .context
                .sync_service
                .process_acknowledgement(ack_type, &self.device, &current_device.id)
                .await
                .map_err(|e| e.to_string())?;
            if let Some(record_id) = device_sync_record_id {
                let ack_complete_msg = Message::AckComplete(record_id);
                self.send_message(ack_complete_msg).await?;
            }

            match self
                .context
                .sync_service
                .get_next_pending_sync(
                    &self.device,
                    &self.user,
                    Some(self.pending_resource_ids.clone()),
                )
                .await
            {
                Ok(Some(payload)) => {
                    info!("Sending next sync payload");
                    let message = Message::SyncResponse(payload);
                    self.send_message(message).await?;
                }
                Ok(None) => {
                    info!("No more pending syncs, sending complete");
                    let message = Message::SyncComplete;
                    self.send_message(message).await?;
                }
                Err(e) => {
                    error!("Failed to get next pending sync: {}", e);
                    return Err(e.to_string());
                }
            }

            return Ok(());
        }
        Err("failed to get current device".into())
    }

    pub async fn handle_sync_response(&self, payload: SyncPayload) -> Result<(), String> {
        if let Some(current_device) = self.get_local_device().await {
            let event_adapter = self.sync_event_adapter();
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
                )
                .await
            {
                Ok(sync_ack) => Message::SyncAck(sync_ack),
                Err(e) => {
                    error!("something happend {:?}", e);
                    Message::Error
                }
            };

            self.send_message(ack_message).await?;
            return Ok(());
        }
        Err("Local user not found".into())
    }

    pub async fn handle_sync_complete(&self) -> Result<(), String> {
        info!("Sync process completed");

        // If not initiator, start sync
        if self.is_initiator {
            match self
                .context
                .sync_service
                .get_next_pending_sync(
                    &self.device,
                    &self.user,
                    Some(self.pending_resource_ids.clone()),
                )
                .await
            {
                Ok(Some(payload)) => {
                    let message = Message::SyncResponse(payload);
                    self.send_message(message).await?;
                }
                Ok(None) => {
                    info!("No pending syncs, sending sync complete");
                    self.context
                        .user_service
                        .update_device_last_synced(&self.device.id)
                        .await
                        .map_err(|e| e.to_string())?;
                    let message = Message::SyncComplete;
                    self.send_message(message).await?;
                }
                Err(e) => {
                    error!("Failed to get pending sync: {}", e);
                    return Err(e.to_string());
                }
            }
        } else {
            self.context
                .user_service
                .update_device_last_synced(&self.device.id)
                .await
                .map_err(|e| e.to_string())?;
        }

        Ok(())
    }

    pub async fn ack_complete(&self, device_record_status_ids: Vec<String>) -> Result<(), String> {
        self.context
            .sync_service
            .handle_ack_complete(device_record_status_ids)
            .await
            .map_err(|e| e.to_string())
    }

    pub async fn send_update(&self, payload: String) -> Result<(), String> {
        log::info!("got update...");
        Ok(())
    }

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

    pub async fn handle_merge_update(&self, payload: &UpdateResource) -> Result<(), String> {
        self.context
            .sync_service
            .merge_updated_doc(
                &payload.encrypted_data,
                &payload.add_vector_clock,
                &payload.update_vector_clock,
                &payload.resource_id,
            )
            .await
            .map_err(|e| e.to_string())
    }
}
