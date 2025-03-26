use crate::p2p::peer_connection::PeerConnection;
use osvauld_core::models::device::Device;
use osvauld_core::models::p2p::Message;
use osvauld_core::models::sync_record::{StatusChangeSet, SyncRecordSet};
use osvauld_core::models::user::User;

impl PeerConnection {
    pub async fn handle_first_user_connection(
        &self,
        user: &User,
        devices: &Vec<Device>,
    ) -> Result<(), String> {
        if let Some(current_device) = self.get_local_device().await {
            let (current_user, current_user_devices, user_addition_record) = self
                .context
                .sync_service
                .process_first_user_connection(
                    user,
                    devices,
                    &current_device.user_id,
                    &current_device.id,
                )
                .await
                .map_err(|e| e.to_string())?;
            let message = Message::FirstUserConnectionResponse {
                user: current_user,
                devices: current_user_devices,
                user_addition_record,
            };
            self.send_message(message).await?;
            return Ok(());
        }

        Err("Local user not found".into())
    }

    pub async fn initiate_user_first_connection(&self) -> Result<(), String> {
        if let Some(user) = self.get_local_user().await {
            let (user, devices) = self
                .context
                .sync_service
                .get_payload_for_first_user_sync(&user.id)
                .await
                .map_err(|e| e.to_string())?;
            let message = Message::FirstUserConnectionRequest { user, devices };
            self.send_message(message).await?;
            return Ok(());
        }
        Err("Local user not found".into())
    }

    pub async fn handle_first_user_connection_response(
        &self,
        user: &User,
        devices: &Vec<Device>,
        user_addition_record: &SyncRecordSet,
    ) -> Result<(), String> {
        if let Some(current_device) = self.get_local_device().await {
            let (user_addition_records, completion_records) = self
                .context
                .sync_service
                .process_first_user_connection_response(
                    user,
                    devices,
                    user_addition_record,
                    &current_device.user_id,
                    &current_device.id,
                )
                .await
                .map_err(|e| e.to_string())?;
            let message = Message::FristUserConnectionAck {
                user_id: current_device.user_id,
                user_addition_records,
                completion_records,
            };
            self.send_message(message).await?;
            return Ok(());
        }

        Err("Local user not found".into())
    }

    pub async fn handle_user_add_ack(
        &self,
        user_id: &str,
        completion_records: &StatusChangeSet,
        user_addition_records: &SyncRecordSet,
    ) -> Result<(), String> {
        if let Some(current_device) = self.get_local_device().await {
            let addition_completion_record = self
                .context
                .sync_service
                .handle_user_add_ack(
                    user_id,
                    completion_records,
                    user_addition_records,
                    &current_device.id,
                    &current_device.user_id,
                )
                .await
                .map_err(|e| e.to_string())?;
            let message = Message::FirstUserConnectionAckResponse {
                addition_completion_record,
            };
            self.send_message(message).await?;
            return Ok(());
        };
        Err("Local device not found".into())
    }

    pub async fn handle_user_add_ack_response(
        &self,
        user_addition_record: &StatusChangeSet,
    ) -> Result<(), String> {
        if let Some(current_device) = self.get_local_device().await {
            let device_record_id = self
                .context
                .sync_service
                .handle_user_add_ack_response(user_addition_record, &current_device.id)
                .await
                .map_err(|e| e.to_string())?;
            let message = Message::FirstUserConnectionFinalAck { device_record_id };
            self.send_message(message).await?;
            return Ok(());
        }
        Err("Local device not found".into())
    }

    pub async fn handle_final_user_add_ack(&self, device_record_id: &str) -> Result<(), String> {
        if let Some(current_device) = self.get_local_device().await {
            self.context
                .sync_service
                .handle_user_add_final_ack(device_record_id, &current_device.id)
                .await
                .map_err(|e| e.to_string())?;

            return Ok(());
        }
        Err("Local device not found".into())
    }
}
