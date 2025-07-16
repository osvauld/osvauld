use super::P2PEvent;
use crate::p2p::peer_connection::PeerConnection;
use crypto_utils::verify_signature;
use osvauld_core::models::{
    ConnectionAction, ConnectionType, Device, HandshakeConfirm, HandshakeInit, HandshakeMessage,
    HandshakeResponse, Message, User,
};
use services::generate_challenge;
use tracing::{debug, error, info, info_span, instrument, trace, warn, Instrument};

impl PeerConnection {
    pub async fn initiate_handshake(
        &self,
        connection_type: ConnectionType,
        action: ConnectionAction,
        current_user: User,
        current_device: Device,
    ) -> Result<(), String> {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let handshake_init = HandshakeInit {
            connection_type,
            user: current_user,
            device: current_device,
            challenge: self.challenge.clone(),
            timestamp,
            action,
        };
        self.send_message(Message::Handshake(HandshakeMessage::HandshakeInit(
            handshake_init,
        )))
        .await?;
        Ok(())
    }
    pub async fn handle_handshake_message(
        &mut self,
        payload: &mut HandshakeMessage,
    ) -> Result<(), String> {
        match payload {
            HandshakeMessage::HandshakeInit(payload) => self.process_handshake_init(payload).await,
            HandshakeMessage::HandshakeResponse(payload) => {
                self.process_handshake_response(payload).await
            }
            HandshakeMessage::HandshakeConfirm(payload) => {
                self.process_handshake_confirm(payload).await
            }
            HandshakeMessage::HandshakeAck => self.process_handshake_ack().await,
        }
    }
    pub async fn process_handshake_init(&mut self, payload: &HandshakeInit) -> Result<(), String> {
        // Get our current user and device
        let current_user = self.get_local_user().await?;
        let current_device = self
            .get_local_device()
            .await
            .ok_or("No current device available")?;

        // Generate our challenge

        // Sign their challenge
        let their_challenge_signature = {
            let crypto_utils = self.crypto_utils.lock().await;
            crypto_utils.sign_message(&payload.challenge).map_err(|e| {
                error!("Failed to sign initiator's challenge: {}", e);
                format!("Failed to sign challenge: {}", e)
            })?
        };
        self.connection_type = Some(payload.connection_type.clone());
        self.device = payload.device.clone();
        self.user = payload.user.clone();
        self.action = Some(payload.action.clone());

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        // Create HandshakeResponse
        let handshake_response = HandshakeResponse {
            user: current_user.clone(),
            device: current_device.clone(),
            challenge: self.challenge.clone(),
            timestamp,
            challenge_signature: their_challenge_signature,
        };

        // Send the response
        self.send_message(Message::Handshake(HandshakeMessage::HandshakeResponse(
            handshake_response,
        )))
        .await?;

        Ok(())
    }

    pub async fn process_handshake_response(
        &mut self,
        payload: &HandshakeResponse,
    ) -> Result<(), String> {
        // Validate timestamp (allow 5 minutes skew)
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        if payload.timestamp.abs_diff(current_time) > 300 {
            error!("HandshakeResponse timestamp too old or too far in future");
            return Err("Timestamp validation failed".to_string());
        }

        // Get our original challenge that we sent in HandshakeInit
        let our_challenge = self.challenge.clone();
        // Verify that the receiver correctly signed our challenge
        let verified = verify_signature(
            &payload.user.public_key,
            &our_challenge,
            &payload.challenge_signature,
        )
        .map_err(|e| {
            error!("Failed to verify challenge signature: {}", e);
            format!("Signature verification failed: {}", e)
        })?;

        if !verified {
            error!("Invalid challenge signature from receiver");
            return Err("Challenge signature verification failed".to_string());
        }
        self.device = payload.device.clone();
        self.user = payload.user.clone();

        info!("Successfully verified receiver's signature of our challenge");

        // Sign their challenge and send confirmation
        let their_challenge = payload.challenge.clone();

        let our_signature = {
            let crypto_utils = self.crypto_utils.lock().await;
            crypto_utils.sign_message(&their_challenge).map_err(|e| {
                error!("Failed to sign receiver's challenge: {}", e);
                format!("Failed to sign challenge: {}", e)
            })?
        };
        // Create and send HandshakeConfirm
        let handshake_confirm = HandshakeConfirm {
            challenge_signature: our_signature,
        };

        self.send_message(Message::Handshake(HandshakeMessage::HandshakeConfirm(
            handshake_confirm,
        )))
        .await?;

        // Update our connection with peer data and mark handshake complete
        info!("Initiator: Handshake completed successfully");
        Ok(())
    }
    pub async fn process_handshake_confirm(
        &mut self,
        payload: &HandshakeConfirm,
    ) -> Result<(), String> {
        // Get our challenge that we sent in HandshakeResponse
        let our_challenge = self.challenge.clone();

        // Verify that the initiator correctly signed our challenge
        let verified = verify_signature(
            &self.user.public_key,
            &our_challenge,
            &payload.challenge_signature,
        )
        .map_err(|e| {
            error!("Failed to verify confirmation signature: {}", e);
            format!("Confirmation signature verification failed: {}", e)
        })?;

        if !verified {
            error!("Invalid confirmation signature from initiator");
            return Err("Confirmation signature verification failed".to_string());
        }

        info!("Successfully verified initiator's signature of our challenge");
        let mut handshake_guard = self.handshake_complete.lock().await;
        *handshake_guard = true;
        // Get the stored initiator data from HandshakeInit

        self.send_message(Message::Handshake(HandshakeMessage::HandshakeAck))
            .await?;
        info!("Receiver: Handshake completed successfully");
        Ok(())
    }
    pub async fn process_handshake_ack(&self) -> Result<(), String> {
        let mut handshake_guard = self.handshake_complete.lock().await;
        *handshake_guard = true;
        self.execute_connection_action().await?;
        if self.is_live_edit {
            self.event_emitter.emit(P2PEvent::LiveEditConnected {
                connection_id: self.node_id.clone(),
            });
        }
        Ok(())
    }

    #[instrument(skip(self), fields(action = ?self.action, is_initiator = self.is_initiator), level = "info")]
    pub async fn execute_connection_action(&self) -> Result<(), String> {
        // Only execute if we have an action and we're the initiator
        if let Some(action) = &self.action {
            if !self.is_initiator {
                info!(
                    "Not executing action {:?} as this peer is not the initiator",
                    action
                );
                return Ok(());
            }

            info!("Executing connection action: {:?}", action);

            // Execute the appropriate action
            match action {
                ConnectionAction::DeviceSync => self.start_add_device_process().await,
                ConnectionAction::UserFirstConnection => {
                    self.send_first_user_connection_payload(true).await
                }
                ConnectionAction::AddDevice => {
                    info!("Initiator: Starting add device phase");
                    self.start_add_device_process().await
                }
                ConnectionAction::LiveEdit => {
                    info!("live edit triggered");
                    let connection_id = self.get_id();
                    self.event_emitter
                        .emit(P2PEvent::LiveEditConnected { connection_id });
                    Ok(())
                }
                ConnectionAction::UserSync => self.start_user_network_sync().await,
            }
        } else {
            debug!("No connection action to execute");
            Ok(())
        }
    }
}
