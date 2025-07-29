use super::P2PEvent;
use crate::p2p::peer_connection::PeerConnection;
use osvauld_core::models::{
    ConnectionAction, ConnectionType, Device, FirstConnectRequest, FirstConnectResponse,
    HandshakeMessage, Message, UcanAndUserExchange, User, UserWithDevices,
};
use services::get_my_user_devices;
use tracing::{debug, error, info, info_span, instrument, trace, warn, Instrument};

impl PeerConnection {
    pub async fn initiate_handshake(
        &self,
        connection_type: ConnectionType,
        action: ConnectionAction,
        current_user: User,
        current_device: Device,
    ) -> Result<(), String> {
        let peer_id = self
            .connection
            .remote_node_id()
            .map_err(|e| e.to_string())?;
        let user = self
            .repo_ctx
            .user_repo
            .get_user_by_device_id(&peer_id.to_string())
            .await
            .map_err(|e| e.to_string())?;
        let token_validation =
            crypto_utils::validate_connect_token(&user.ucan_token, &self.domain).await?;
        if !token_validation.is_valid {
            return Err("token invalid".to_string());
        }
        if user.first_sync {
            let user_devices = get_my_user_devices(&user.id, &self.repo_ctx)
                .await
                .map_err(|e| e.to_string())?;
            let (new_ucan_token, signed_ucan_pub) = {
                let encrypted_pvt_key = self
                    .repo_ctx
                    .store_repo
                    .get_ucan_key()
                    .await
                    .map_err(|e| e.to_string())?;
                let crypto = self.crypto_utils.lock().await;
                let ucan_pub_key = crypto
                    .get_public_ucan_key(&encrypted_pvt_key)
                    .await
                    .map_err(|e| e.to_string())?;
                let signed_pub_key = crypto
                    .sign_clear_text_message(&ucan_pub_key)
                    .map_err(|e| e.to_string())?;
                let issued_token = crypto
                    .issue_connect_and_share_user_token(
                        &encrypted_pvt_key,
                        &self.domain,
                        &user.ucan_pub_key,
                    )
                    .await
                    .map_err(|e| e.to_string())?;
                (issued_token, signed_pub_key)
            };
            self.send_message(Message::Handshake(
                HandshakeMessage::HandshakeFirstConnectRequest(FirstConnectRequest {
                    devices: user_devices,
                    issued_ucan: new_ucan_token,
                    signed_ucan_pub,
                    one_time_ucan: user.ucan_token.clone(),
                    peer_device: current_device,
                    peer_user: current_user,
                    connection_type,
                }),
            ))
            .await?;
        } else {
            let signed_ucan_pub = {
                let encrypted_pvt_key = self
                    .repo_ctx
                    .store_repo
                    .get_ucan_key()
                    .await
                    .map_err(|e| e.to_string())?;

                let crypto = self.crypto_utils.lock().await;
                let ucan_pub_key = crypto
                    .get_public_ucan_key(&encrypted_pvt_key)
                    .await
                    .map_err(|e| e.to_string())?;
                crypto
                    .sign_clear_text_message(&ucan_pub_key)
                    .map_err(|e| e.to_string())?
            };
            let exchange_message = UcanAndUserExchange {
                ucan_token: user.ucan_token,
                peer_user: current_user,
                peer_device: current_device,
                connection_type,
                signed_ucan_pub,
            };
            self.send_message(Message::Handshake(HandshakeMessage::HandshakeExchange(
                exchange_message,
            )))
            .await?;
        }
        Ok(())
    }
    pub async fn handle_handshake_message(
        &mut self,
        payload: &mut HandshakeMessage,
    ) -> Result<(), String> {
        match payload {
            HandshakeMessage::HandshakeFirstConnectRequest(payload) => {
                self.process_first_user_connection_request(payload).await
            }
            HandshakeMessage::HandshakeFirstConnectResponse(payload) => {
                self.process_first_user_connection_handshake_response(payload)
                    .await
            }
            HandshakeMessage::HandshakeExchange(payload) => {
                self.process_exchange_message(payload).await
            }
        }
    }

    pub async fn process_exchange_message(
        &mut self,
        payload: &UcanAndUserExchange,
    ) -> Result<(), String> {
        let ucan_pub_key = crypto_utils::verify_clear_text_message(
            &payload.peer_user.public_key,
            &payload.signed_ucan_pub,
        )
        .await
        .map_err(|e| e.to_string())?;
        let token_validation =
            crypto_utils::validate_connect_token(&payload.ucan_token, &self.domain)
                .await
                .map_err(|e| e.to_string())?;
        if !token_validation.is_valid {
            return Err("invalid token".to_string());
        }
        self.user = payload.peer_user.clone();
        self.device = payload.peer_device.clone();
        if self.is_initiator {
            let mut handshake_complete = self.handshake_complete.lock().await;
            *handshake_complete = true;
        } else {
            let signed_ucan_pub = {
                let encrypted_pvt_key = self
                    .repo_ctx
                    .store_repo
                    .get_ucan_key()
                    .await
                    .map_err(|e| e.to_string())?;

                let crypto = self.crypto_utils.lock().await;
                crypto
                    .get_public_ucan_key(&encrypted_pvt_key)
                    .await
                    .map_err(|e| e.to_string())?
            };
            let current_user = self.get_local_user().await?;
            let current_device = self
                .get_local_device()
                .await
                .ok_or("No current device available")?;

            let peer_id = self
                .connection
                .remote_node_id()
                .map_err(|e| e.to_string())?;
            let peer_user = self
                .repo_ctx
                .user_repo
                .get_user_by_device_id(&peer_id.to_string())
                .await
                .map_err(|e| e.to_string())?;
            let exchange_message = UcanAndUserExchange {
                signed_ucan_pub,
                peer_user: current_user,
                peer_device: current_device,
                ucan_token: peer_user.ucan_token,
                connection_type: payload.connection_type.clone(),
            };
            let mut handshake_complete = self.handshake_complete.lock().await;
            *handshake_complete = true;
            self.send_message(Message::Handshake(HandshakeMessage::HandshakeExchange(
                exchange_message,
            )))
            .await?;
        }
        Ok(())
    }

    pub async fn process_first_user_connection_request(
        &mut self,
        payload: &FirstConnectRequest,
    ) -> Result<(), String> {
        // Get our current user and device
        let current_user = self.get_local_user().await?;
        let current_device = self
            .get_local_device()
            .await
            .ok_or("No current device available")?;

        let token_validation =
            crypto_utils::validate_connect_token(&payload.one_time_ucan, &self.domain).await?;
        if !token_validation.is_valid {
            return Err("validation failed".to_string());
        }
        let peer_ucan_pub = crypto_utils::verify_clear_text_message(
            &payload.peer_user.public_key,
            &payload.signed_ucan_pub,
        )
        .await
        .map_err(|e| e.to_string())?;

        let (peer_issued_ucan_token, signed_ucan_pub) = {
            let encrypted_pvt_key = self
                .repo_ctx
                .store_repo
                .get_ucan_key()
                .await
                .map_err(|e| e.to_string())?;

            let crypto = self.crypto_utils.lock().await;
            let ucan_pub_key = crypto
                .get_public_ucan_key(&encrypted_pvt_key)
                .await
                .map_err(|e| e.to_string())?;
            let signed_ucan_pub = crypto
                .sign_clear_text_message(&ucan_pub_key)
                .map_err(|e| e.to_string())?;
            let issued_token = crypto
                .issue_connect_and_share_user_token(
                    &encrypted_pvt_key,
                    &self.domain,
                    &peer_ucan_pub,
                )
                .await
                .map_err(|e| e.to_string())?;
            (issued_token, signed_ucan_pub)
        };
        let mut user = payload.peer_user.clone();
        user.first_sync = true;
        user.owner = false;
        user.ucan_token = peer_issued_ucan_token.clone();
        user.ucan_pub_key = peer_ucan_pub;
        self.connection_type = Some(payload.connection_type.clone());
        self.device = payload.peer_device.clone();
        self.user = user.clone();
        let user_with_devices = UserWithDevices {
            user,
            devices: payload.devices.clone(),
        };
        self.repo_ctx
            .user_repo
            .add_users_with_devices_bulk(&vec![user_with_devices])
            .await
            .map_err(|e| e.to_string())?;
        let my_devices = get_my_user_devices(&current_user.id, &self.repo_ctx).await?;
        let mut handshake_complete = self.handshake_complete.lock().await;
        *handshake_complete = true;
        let handshake_response = FirstConnectResponse {
            peer_user: current_user.clone(),
            peer_device: current_device.clone(),
            devices: my_devices,
            ucan_token: payload.issued_ucan.clone(),
            issued_ucan: peer_issued_ucan_token,
            signed_ucan_pub,
        };
        self.send_message(Message::Handshake(
            HandshakeMessage::HandshakeFirstConnectResponse(handshake_response),
        ))
        .await?;

        info!("first handshake completed for user sent message to peer");
        Ok(())
    }

    pub async fn process_first_user_connection_handshake_response(
        &mut self,
        payload: &FirstConnectResponse,
    ) -> Result<(), String> {
        self.user.ucan_token = payload.ucan_token.clone();
        let peer_ucan_pub = crypto_utils::verify_clear_text_message(
            &payload.peer_user.public_key,
            &payload.signed_ucan_pub,
        )
        .await
        .map_err(|e| e.to_string())?;
        let token_validation_result =
            crypto_utils::validate_connect_token(&payload.issued_ucan, &self.domain)
                .await
                .map_err(|e| e.to_string())?;
        if !token_validation_result.is_valid {
            return Err("token invalid".to_string());
        }
        let mut user = payload.peer_user.clone();
        user.ucan_token = payload.issued_ucan.clone();
        user.first_sync = true;
        user.owner = false;
        user.ucan_pub_key = peer_ucan_pub;
        self.user = user.clone();
        self.device = payload.peer_device.clone();
        let user_with_devices = UserWithDevices {
            user,
            devices: payload.devices.clone(),
        };
        let mut handshake_complete = self.handshake_complete.lock().await;
        *handshake_complete = true;
        self.repo_ctx
            .user_repo
            .add_users_with_devices_bulk(&vec![user_with_devices])
            .await
            .map_err(|e| e.to_string())?;
        info!("first handshake completed for user");
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
