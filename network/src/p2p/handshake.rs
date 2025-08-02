use super::P2PEvent;
use crate::p2p::peer_connection::PeerConnection;
use base64::{engine::general_purpose, Engine as _};
use crypto_utils::errors::UcanError;
use osvauld_core::models::{
    ConnectionAction, ConnectionType, Device, FirstConnectRequest, FirstConnectResponse,
    HandshakeMessage, Message, UcanAndUserExchange, User, UserWithDevices,
};
use services::{get_my_user_devices, issue_connect_ucan_token, sign_ucan_pub_key};
use tracing::{debug, error, info, instrument};

impl PeerConnection {
    #[instrument(skip(self, current_user, current_device), fields(
        connection_id = %self.get_id(),
        peer_id = ?self.connection.remote_node_id(),
        connection_type = ?connection_type,
        action = ?action
    ), level = "info")]
    pub async fn initiate_handshake(
        &self,
        connection_type: ConnectionType,
        action: ConnectionAction,
        current_user: User,
        current_device: Device,
    ) -> Result<(), String> {
        info!("Initiating handshake");
        let peer_id = self.connection.remote_node_id().map_err(|e| {
            error!("Failed to get remote node id: {}", e);
            e.to_string()
        })?;
        debug!("Successfully retrieved peer id: {}", peer_id);
        let device_id_b64 = general_purpose::STANDARD.encode(peer_id);
        let peer_user = self
            .repo_ctx
            .user_repo
            .get_user_by_device_id(&device_id_b64)
            .await
            .map_err(|e| {
                error!("Failed to get user by device id {}: {}", peer_id, e);
                e.to_string()
            })?;
        debug!("Successfully retrieved user for peer");

        let signed_ucan_pub = sign_ucan_pub_key(&self.crypto_utils, self.repo_ctx.clone()).await?;
        debug!("Successfully signed the UCAN public key, {:?}", peer_user);
        if !peer_user.first_sync {
            info!("Peer is a first-time connection, preparing FirstConnectRequest");
            let user_devices = get_my_user_devices(&current_user.id, self.repo_ctx.clone())
                .await
                .map_err(|e| {
                    error!("Failed to get devices for user {}: {}", current_user.id, e);
                    e.to_string()
                })?;
            debug!("Retrieved {} devices for the user", user_devices.len());

            let new_ucan_token = issue_connect_ucan_token(
                self.repo_ctx.clone(),
                &self.crypto_utils,
                &self.domain,
                &peer_user.ucan_pub_key,
            )
            .await?;
            debug!("Successfully issued new UCAN token for peer");

            self.send_message(Message::Handshake(
                HandshakeMessage::HandshakeFirstConnectRequest(FirstConnectRequest {
                    devices: user_devices,
                    issued_ucan: new_ucan_token,
                    signed_ucan_pub,
                    one_time_ucan: peer_user.ucan_token.clone(),
                    peer_device: current_device,
                    peer_user: current_user,
                    connection_type,
                }),
            ))
            .await?;
            info!("Sent HandshakeFirstConnectRequest to peer");
        } else {
            info!("Peer is an existing user, preparing HandshakeExchange");
            let exchange_message = UcanAndUserExchange {
                ucan_token: peer_user.ucan_token,
                peer_user: current_user,
                peer_device: current_device,
                connection_type,
                signed_ucan_pub,
            };
            self.send_message(Message::Handshake(HandshakeMessage::HandshakeExchange(
                exchange_message,
            )))
            .await?;
            self.set_connection_type(ConnectionType::User).await;

            info!("Sent HandshakeExchange to peer");
        }
        info!("Handshake initiation process completed successfully");
        Ok(())
    }
    pub async fn handle_handshake_message(
        &self,
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

    #[instrument(skip(self, payload), fields(connection_id = %self.get_id(), is_initiator = self.is_initiator), level = "info")]
    pub async fn process_exchange_message(
        &self,
        payload: &UcanAndUserExchange,
    ) -> Result<(), String> {
        info!("Processing handshake exchange message");
        let peer_ucan_pub = crypto_utils::verify_clear_text_message(
            &payload.peer_user.public_key,
            &payload.signed_ucan_pub,
        )
        .await
        .map_err(|e| {
            error!("Failed to verify peer's signed UCAN pub key: {}", e);
            e.to_string()
        })?;
        debug!("Successfully verified peer's signed UCAN public key");

        let current_user = self.get_local_user().await?;
        let current_device = self
            .get_local_device()
            .await
            .ok_or("No current device available")?;
        debug!("(Responder) Retrieved local user and device");
        let repo_ctx_clone = self.repo_ctx.clone();
        let proof_resolver = move |cid: &str| {
            let cid_owned = cid.to_string();
            let repo_ctx_for_async = repo_ctx_clone.clone();
            async move {
                repo_ctx_for_async
                    .user_repo
                    .get_ucan_by_cid(&cid_owned) // Use the owned String as a reference.
                    .await
                    .map_err(|e| UcanError::ProofChainInvalid(e.to_string()))
            }
        };
        let token_validation = crypto_utils::validate_connect_token(
            &payload.ucan_token,
            &peer_ucan_pub,
            &current_user.ucan_pub_key,
            &current_user.id,
            &self.domain,
            &proof_resolver,
        )
        .await
        .map_err(|e| {
            error!("Peer's connect token validation failed: {}", e);
            e.to_string()
        })?;

        if !token_validation {
            error!("Peer's connect token is invalid");
            return Err("invalid token".to_string());
        }
        debug!("Peer's connect token is valid");
        self.set_peer_user_and_device(payload.peer_user.clone(), payload.peer_device.clone())
            .await;

        if self.is_initiator {
            info!("This peer is the initiator. Completing handshake.");
            let mut handshake_complete = self.handshake_complete.lock().await;
            *handshake_complete = true;
            self.start_user_network_sync().await?;
            info!("Handshake marked as complete for initiator.");
        } else {
            info!("This peer is the responder. Preparing and sending exchange response.");
            let signed_ucan_pub =
                sign_ucan_pub_key(&self.crypto_utils, self.repo_ctx.clone()).await?;
            debug!("(Responder) Successfully signed the UCAN public key");

            let peer_id = self.connection.remote_node_id().map_err(|e| {
                error!("(Responder) Failed to get remote node id: {}", e);
                e.to_string()
            })?;
            let device_id_b64 = general_purpose::STANDARD.encode(peer_id);

            let peer_user = self
                .repo_ctx
                .user_repo
                .get_user_by_device_id(&device_id_b64)
                .await
                .map_err(|e| {
                    error!(
                        "(Responder) Failed to get user by device id {}: {}",
                        peer_id, e
                    );
                    e.to_string()
                })?;
            debug!("(Responder) Retrieved peer user from repository");

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
            self.set_connection_type(ConnectionType::User);

            info!("(Responder) Sent handshake exchange response and marked handshake as complete.");
        }
        Ok(())
    }

    #[instrument(skip(self, payload), fields(connection_id = %self.get_id()), level = "info")]
    pub async fn process_first_user_connection_request(
        &self,
        payload: &FirstConnectRequest,
    ) -> Result<(), String> {
        info!("Processing first user connection request");
        let current_user = self.get_local_user().await?;
        let current_device = self
            .get_local_device()
            .await
            .ok_or("No current device available")?;
        debug!("Retrieved local user and device information");

        let peer_ucan_pub = crypto_utils::verify_clear_text_message(
            &payload.peer_user.public_key,
            &payload.signed_ucan_pub,
        )
        .await
        .map_err(|e| {
            error!("Failed to verify peer's signed UCAN pub key: {}", e);
            e.to_string()
        })?;
        debug!("Successfully verified peer's signed UCAN public key");

        let repo_ctx_clone = self.repo_ctx.clone();
        let proof_resolver = move |cid: &str| {
            let cid_owned = cid.to_string();
            let repo_ctx_for_async = repo_ctx_clone.clone();
            async move {
                repo_ctx_for_async
                    .user_repo
                    .get_ucan_by_cid(&cid_owned) // Use the owned String as a reference.
                    .await
                    .map_err(|e| UcanError::ProofChainInvalid(e.to_string()))
            }
        };
        let one_time_token_validation = crypto_utils::validate_connect_token(
            &payload.one_time_ucan,
            &peer_ucan_pub,
            &self.domain,
            &current_user.ucan_pub_key,
            &current_user.id,
            &proof_resolver,
        )
        .await
        .map_err(|e| e.to_string())?;
        debug!("Completed validation of one-time UCAN");
        if !one_time_token_validation {
            error!("Peer's one-time UCAN is invalid.");
            return Err("validation failed".to_string());
        }
        info!("Peer's one-time UCAN is valid. Proceeding to issue persistent UCAN.");

        let (peer_issued_ucan_token, signed_ucan_pub) = {
            let signed_ucan_pub =
                sign_ucan_pub_key(&self.crypto_utils, self.repo_ctx.clone()).await?;
            let issued_token = issue_connect_ucan_token(
                self.repo_ctx.clone(),
                &self.crypto_utils,
                &self.domain,
                &peer_ucan_pub,
            )
            .await?;
            debug!("Successfully issued new UCAN token for peer and signed local public key");
            (issued_token, signed_ucan_pub)
        };

        let mut user = payload.peer_user.clone();
        user.first_sync = true;
        user.owner = false;
        user.ucan_token = payload.issued_ucan.clone();
        user.ucan_pub_key = peer_ucan_pub;
        self.set_connection_type(ConnectionType::User);
        self.set_peer_user_and_device(payload.peer_user.clone(), payload.peer_device.clone())
            .await;

        let user_with_devices = UserWithDevices {
            user,
            devices: payload.devices.clone(),
        };
        self.repo_ctx
            .user_repo
            .add_users_with_devices_bulk(&vec![user_with_devices])
            .await
            .map_err(|e| {
                error!("Failed to add new user and devices to repository: {}", e);
                e.to_string()
            })?;
        info!("Successfully added new user and their devices to the repository");

        let my_devices = get_my_user_devices(&current_user.id, self.repo_ctx.clone()).await?;
        debug!("Retrieved local devices to send in response");

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

        info!("First connect handshake completed. Sent response message to peer.");
        Ok(())
    }
    #[instrument(skip(self, payload), fields(connection_id = %self.get_id()), level = "info")]
    pub async fn process_first_user_connection_handshake_response(
        &self,
        payload: &FirstConnectResponse,
    ) -> Result<(), String> {
        info!("Processing first user connection handshake response");
        debug!("Updated local user's UCAN token with the one from the payload");

        let peer_ucan_pub = crypto_utils::verify_clear_text_message(
            &payload.peer_user.public_key,
            &payload.signed_ucan_pub,
        )
        .await
        .map_err(|e| {
            error!("Failed to verify peer's signed UCAN pub key: {}", e);
            e.to_string()
        })?;
        debug!("Successfully verified peer's signed UCAN public key from response");

        let current_user = self.get_local_user().await?;
        debug!("Retrieved local user and device information");
        let repo_ctx_clone = self.repo_ctx.clone();
        let proof_resolver = move |cid: &str| {
            let cid_owned = cid.to_string();
            let repo_ctx_for_async = repo_ctx_clone.clone();
            async move {
                repo_ctx_for_async
                    .user_repo
                    .get_ucan_by_cid(&cid_owned) // Use the owned String as a reference.
                    .await
                    .map_err(|e| UcanError::ProofChainInvalid(e.to_string()))
            }
        };
        let token_validation_result = crypto_utils::validate_connect_token(
            &payload.ucan_token,
            &peer_ucan_pub,
            &current_user.ucan_pub_key,
            &current_user.id,
            &self.domain,
            &proof_resolver,
        )
        .await
        .map_err(|e| {
            error!("Validation of UCAN issued by peer failed: {}", e);
            e.to_string()
        })?;

        if !token_validation_result {
            error!("The UCAN issued by the peer is invalid");
            return Err("token invalid".to_string());
        }
        debug!("The UCAN issued by the peer is valid");

        let mut user = payload.peer_user.clone();
        user.ucan_token = payload.issued_ucan.clone();
        user.first_sync = true;
        user.owner = false;
        user.ucan_pub_key = peer_ucan_pub;

        self.set_peer_user_and_device(payload.peer_user.clone(), payload.peer_device.clone())
            .await;

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
            .map_err(|e| {
                error!(
                    "Failed to add peer user and their devices to repository: {}",
                    e
                );
                e.to_string()
            })?;
        info!("Successfully added peer user and devices to repository. Handshake complete.");
        self.start_user_network_sync().await?;
        Ok(())
    }

    #[instrument(skip(self), level = "info")]
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
