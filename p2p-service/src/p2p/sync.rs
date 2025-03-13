use crate::p2p::peer_connection::PeerConnection;

use log::{error, info};
use osvauld_core::models::p2p::{ConnectionType, Message, SyncAckType, SyncPayload};

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
        // info!("Starting sync process");
        // let connection = self.get_active_connection().await?;
        // // Send sync request
        // let (mut send, _recv) = connection
        //     .open_bi()
        //     .await
        //     .map_err(|e| format!("Failed to open bi-directional stream: {}", e))?;
        //
        // let sync_request = Message::SyncRequest;
        // let serialized = serde_json::to_string(&sync_request)
        //     .map_err(|e| format!("Serialization error: {}", e))?;
        //
        // send.write_all(serialized.as_bytes())
        //     .await
        //     .map_err(|e| format!("Failed to send sync request: {}", e))?;
        // send.flush()
        //     .await
        //     .map_err(|e| format!("Failed to flush sync request: {}", e))?;
        // info!("Sync process started");
        Ok(())
    }

    pub async fn handle_sync_request(&self) -> Result<(), String> {
        info!("Handling incoming sync request");
        match self
            .context
            .sync_service
            .get_next_pending_sync(&self.device)
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
            .add_new_device_sync(records)
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
        let device_sync_record_id = self
            .context
            .sync_service
            .process_acknowledgement(ack_type, self.device.clone())
            .await
            .map_err(|e| e.to_string())?;
        if let Some(record_id) = device_sync_record_id {
            let ack_complete_msg = Message::AckComplete(record_id);
            self.send_message(ack_complete_msg).await?;
        }

        match self
            .context
            .sync_service
            .get_next_pending_sync(&self.device)
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

        Ok(())
    }

    pub async fn handle_sync_response(&self, payload: SyncPayload) -> Result<(), String> {
        let ack_message = match self
            .context
            .sync_service
            .process_sync_payload(&payload)
            .await
        {
            Ok(sync_ack) => Message::SyncAck(sync_ack),
            Err(_e) => Message::Error,
        };

        self.send_message(ack_message).await?;
        Ok(())
    }

    pub async fn handle_sync_complete(&self) -> Result<(), String> {
        info!("Sync process completed");

        // If not initiator, start sync
        if self.is_initiator {
            match self
                .context
                .sync_service
                .get_next_pending_sync(&self.device)
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
        }

        Ok(())
    }

    pub async fn ack_complete(&self, device_record_status_id: String) -> Result<(), String> {
        self.context
            .sync_service
            .handle_ack_complete(device_record_status_id)
            .await
    }

    pub async fn send_update(&self, payload: String) -> Result<(), String> {
        log::info!("got update...");
        Ok(())
    }
}
