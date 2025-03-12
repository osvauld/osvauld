use crate::p2p::service::P2PService;
use log::info;
use osvauld_core::models::p2p::{ConnectionType, Message, SharePayload};
use osvauld_core::models::user::User;

impl P2PService {
    pub async fn initiate_first_user_connection(
        &self,
        user: &User,
        ticket: &str,
    ) -> Result<(), String> {
        let message = Message::FirstUserConnection(user.clone());
        self.connect_with_ticket(&ticket, ConnectionType::User)
            .await?;
        let serialized = serde_json::to_string(&message)
            .map_err(|e| format!("Failed to serialize AddDevice message: {}", e))?;
        self.send_message(serialized).await?;
        Ok(())
    }

    pub async fn handle_first_user_connection(&self, user: &User) -> Result<(), String> {
        self.user_service
            .add_known_user(user.username.clone(), user.public_key.clone(), false)
            .await
            .map_err(|e| e.to_string())?;

        let message = Message::UserAddAck(user.id.clone());
        let serialized = serde_json::to_string(&message)
            .map_err(|e| format!("Failed to serialize AddDevice message: {}", e))?;
        self.send_message(serialized).await?;
        Ok(())
    }

    pub async fn handle_user_add_ack(&self, user_id: &str) -> Result<(), String> {
        //TODO: make it so that user addtion is complete only after reciving ack
        info!("received acknowledgment {}", user_id);
        Ok(())
    }

    pub async fn start_user_sync(&self, _user: &User) -> Result<(), String> {
        let connected_user_id = {
            let user_guard = self.user.lock().await;
            match &*user_guard {
                Some(connected_user) => connected_user.id.clone(),
                None => return Err("No user connected".to_string()),
            }
        };

        let pending_shares = self
            .share_service
            .get_pending_shares(&connected_user_id)
            .await
            .map_err(|e| e.to_string())?;
        if let Some(share_payload) = pending_shares {
            // Create a ShareResponse message
            let message = Message::SharePayload(share_payload);
            let serialized = serde_json::to_string(&message)
                .map_err(|e| format!("Failed to serialize ShareResponse: {}", e))?;

            // Send the share payload
            self.send_message(serialized).await?;
            info!("Sent share payload to peer");
        } else {
            // No pending shares, send completion
            let message = Message::ShareComplete;
            let serialized = serde_json::to_string(&message)
                .map_err(|e| format!("Failed to serialize ShareComplete: {}", e))?;

            self.send_message(serialized).await?;
            info!("No pending shares, sent completion message");
        }

        Ok(())
    }

    pub async fn handle_share_payload(&self, _payload: &SharePayload) -> Result<(), String> {
        // self.share_service
        //     .process_incoming_payload(payload.clone())
        //     .await
        //     .map_err(|e| e.to_string())
        todo!()
    }
}
