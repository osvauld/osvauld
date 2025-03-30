use crate::p2p::peer_connection::PeerConnection;
use osvauld_core::models::p2p::{Message, UserConnectionPayload};

impl PeerConnection {
    pub async fn process_user_connection_payload(
        &self,
        payload: &UserConnectionPayload,
    ) -> Result<(), String> {
        if let Some(current_device) = self.get_local_device().await {
            let return_payload = self
                .context
                .sync_service
                .process_user_connection_payload(
                    payload,
                    &current_device.user_id,
                    &current_device.id,
                )
                .await
                .map_err(|e| e.to_string())?;
            if let Some(payload) = return_payload {
                let message = Message::UserConnection(payload);
                self.send_message(message).await?;
            }
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
            let message = Message::UserConnection(UserConnectionPayload::Request { user, devices });
            self.send_message(message).await?;
            return Ok(());
        }
        Err("Local user not found".into())
    }
}
