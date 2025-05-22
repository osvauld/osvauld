use crate::p2p::peer_connection::PeerConnection;
use osvauld_core::models::p2p::{Message, UserConnectionPayload};
use osvauld_core::models::p2p::{Phase, PhaseAction, PhaseType};

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
        if let UserConnectionPayload::FinalSyncAck = payload {
            self.phase.reset_for_new_phase(PhaseType::DeviceSync).await;
            self.send_message(Message::Phase(Phase {
                action: PhaseAction::Init,
                phase_type: PhaseType::DeviceSync,
            }))
            .await?;
            return Ok(());
        }

        // Process the payload with sync service
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

        if let Some(response_payload) = return_payload {
            // Send the response message
            let message = Message::UserConnection(response_payload);
            self.send_message(message).await?;
        }
        Ok(())
    }

    pub async fn start_first_user_connection(&self) -> Result<(), String> {
        // Only the initiator sends the UserConnection message
        if self.is_initiator {
            // Get local user for the UserConnection message
            let user = match self.get_local_user().await {
                Some(user) => user,
                None => return Err("Local user not found".into()),
            };

            // Get devices for the UserConnection message
            let (user, devices) = self
                .context
                .sync_service
                .get_payload_for_first_user_sync(&user.id)
                .await
                .map_err(|e| e.to_string())?;

            // Send the UserConnection message
            let message = Message::UserConnection(UserConnectionPayload::Request { user, devices });
            self.send_message(message).await?;
        }

        Ok(())
    }
}
