use crate::p2p::peer_connection::PeerConnection;
use osvauld_core::models::p2p::{Message, UserConnectionPayload};

impl PeerConnection {
    pub async fn process_user_connection_payload(
        &self,
        payload: &UserConnectionPayload,
    ) -> Result<(), String> {
        let current_device = match self.get_local_device().await {
            Some(device) => device,
            None => return Err("Local device not found".into()),
        };
        let current_span = tracing::Span::current();
        let return_payload = self
            .context
            .sync_service
            .process_user_connection_payload(
                payload,
                &current_device.user_id,
                &current_device.id,
                current_span,
            )
            .await
            .map_err(|e| e.to_string())?;
        if let Some(payload) = return_payload {
            let message = Message::UserConnection(payload);
            self.send_message(message).await?;
        }
        Ok(())
    }

    pub async fn initiate_user_first_connection(&self) -> Result<(), String> {
        let user = match self.get_local_user().await {
            Some(user) => user,
            None => return Err("Local user not found".into()),
        };
        let (user, devices) = self
            .context
            .sync_service
            .get_payload_for_first_user_sync(&user.id)
            .await
            .map_err(|e| e.to_string())?;
        let message = Message::UserConnection(UserConnectionPayload::Request { user, devices });
        self.send_message(message).await?;
        Ok(())
    }
}
