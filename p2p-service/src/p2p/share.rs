use crate::p2p::peer_connection::PeerConnection;
use crate::p2p::P2PEvent;
use log::info;
use osvauld_core::models::p2p::{ConnectionType, Message, SharePayload};
use osvauld_core::models::user::User;

impl PeerConnection {
    pub async fn initiate_first_user_connection(
        &self,
        user: &User,
        ticket: &str,
    ) -> Result<(), String> {
        // let message = Message::FirstUserializedserConnection(user.clone()); self.context
        //     .connect_with_ticket(&ticket, ConnectionType::User)
        //     .await?;
        // let serialized = serde_json::to_string(&message)
        //     .map_err(|e| format!("Failed to serialize AddDevice message: {}", e))?;
        // self.send_message(serialized).await?;
        Ok(())
    }

    pub async fn handle_first_user_connection(&self, user: &User) -> Result<(), String> {
        self.context
            .user_service
            .add_known_user(user.username.clone(), user.public_key.clone(), false)
            .await
            .map_err(|e| e.to_string())?;

        let message = Message::UserAddAck(user.id.clone());
        self.send_message(message).await?;
        Ok(())
    }

    pub async fn handle_user_add_ack(&self, user_id: &str) -> Result<(), String> {
        //TODO: make it so that user addtion is complete only after reciving ack
        info!("received acknowledgment {}", user_id);
        Ok(())
    }

    pub async fn start_user_sync(&self) -> Result<(), String> {
        let pending_shares = self
            .context
            .share_service
            .get_pending_shares(&self.user.id)
            .await
            .map_err(|e| e.to_string())?;
        if let Some(share_payload) = pending_shares {
            // Create a ShareResponse message
            let message = Message::SharePayload(share_payload);

            self.send_message(message).await?;
            info!("Sent share payload to peer");
        } else {
            // No pending shares, send completion
            let message = Message::ShareComplete;

            self.send_message(message).await?;
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
    pub async fn handle_sync_event(&self, event_name: &str, payload: String) -> Result<(), String> {
        log::info!(
            "Handling sync event '{}' with payload size: {}",
            event_name,
            payload.len()
        );

        match event_name {
            "sync-update" => {
                log::info!("Received sync update event");
                self.event_emitter.emit(P2PEvent::EditingEvent { payload });
                Ok(())
            }
            _ => {
                let err = format!("Unknown sync event type: {}", event_name);
                log::error!("{}", err);
                Err(err)
            }
        }
    }
}
