use crate::p2p::peer_connection::PeerConnection;
use osvauld_core::models::device::Device;
use osvauld_core::models::p2p::Message;
use osvauld_core::models::user::User;

impl PeerConnection {
    pub async fn handle_first_user_connection(
        &self,
        user: &User,
        devices: &Vec<Device>,
    ) -> Result<(), String> {
        if let Some(current_device) = self.get_local_device().await {
            let (current_user, current_user_devices) = self
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
    ) -> Result<(), String> {
        if let Some(current_device) = self.get_local_device().await {
            self.context
                .sync_service
                .process_first_user_connection_response(
                    user,
                    devices,
                    &current_device.user_id,
                    &current_device.id,
                )
                .await
                .map_err(|e| e.to_string())?;
            let message = Message::FristUserConnectionAck(current_device.user_id);
            self.send_message(message).await?;
            return Ok(());
        }

        Err("Local user not found".into())
    }

    pub async fn handle_user_add_ack(&self, user_id: &str) -> Result<(), String> {
        self.context
            .sync_service
            .handle_user_add_ack(user_id)
            .await
            .map_err(|e| e.to_string())
    }
}
