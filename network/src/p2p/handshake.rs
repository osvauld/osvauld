use crate::p2p::{
    errors::{HandshakeError, P2PError, P2PResult},
    peer_connection::PeerConnection,
    P2PEvent,
};

use base64::{engine::general_purpose, Engine as _};
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
    ) -> P2PResult<()> {
        info!("Initiating handshake");

        let peer_id = self.connection.remote_node_id().map_err(|e| P2PError::Custom(e.to_string()))?;
        debug!("Successfully retrieved peer id: {}", peer_id);
        let device_id_b64 = general_purpose::STANDARD.encode(peer_id);
        // Repository error automatically propagates
        let peer_user = self.repo_ctx
            .user_repo
            .get_user_by_device_id(&device_id_b64)
            .await?;
        debug!("Successfully retrieved user for peer");

        // Extract role from peer's UCAN token to determine connection type
        let role = crypto_utils::get_role_from_ucan_token(&peer_user.ucan_token)
            .await
            .unwrap_or_else(|e| {
                debug!("Failed to extract role from token: {}, defaulting to viewer", e);
                "viewer".to_string()
            });

        debug!("Extracted role from peer UCAN token: {}", role);

        // Handle viewer role specially - initiate website handshake
        if role == "viewer" {
            info!("Peer has viewer role, initiating website handshake with UCAN token");

            // Check if this is a first connection or reconnection
            if !peer_user.first_sync {
                // First connection: send WebsiteHandshakeRequest
                info!("First connection to node, sending WebsiteHandshakeRequest");
                let request = osvauld_core::models::WebsiteHandshakeRequest {
                    ucan_token: peer_user.ucan_token.clone(),
                    viewer_user: current_user,
                    viewer_device: current_device,
                };

                self.send_message(Message::Handshake(
                    HandshakeMessage::HandshakeWebsiteRequest(request),
                )).await?;

                info!("Sent website handshake request for first connection");
            } else {
                // Reconnection: send WebsiteReconnectRequest
                info!("Reconnecting to node, sending WebsiteReconnectRequest");
                let request = osvauld_core::models::WebsiteReconnectRequest {
                    ucan_token: peer_user.ucan_token.clone(),
                    viewer_user: current_user,
                    viewer_device: current_device,
                };

                self.send_message(Message::Handshake(
                    HandshakeMessage::HandshakeWebsiteReconnectRequest(request),
                )).await?;

                info!("Sent website reconnect request for reconnection");
            }

            return Ok(());
        }
        // Service error automatically propagates
        let signed_ucan_pub = sign_ucan_pub_key(&self.crypto_utils, self.repo_ctx.clone()).await?;
        if !peer_user.first_sync {
            info!("Peer is a first-time connection, preparing FirstConnectRequest");
            // Service error automatically propagates
            let user_devices = get_my_user_devices(&current_user.id, self.repo_ctx.clone()).await?;
            debug!("Retrieved {} devices for the user", user_devices.len());
            // Service error automatically propagates
            let new_ucan_token = issue_connect_ucan_token(
                self.repo_ctx.clone(),
                &self.crypto_utils,
                &self.domain,
                &peer_user.ucan_pub_key,
            ).await?;
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
            )).await?;
            
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
            ))).await?;
            self.set_connection_type(ConnectionType::User).await;
            info!("Sent HandshakeExchange to peer");
        }
        info!("Handshake initiation process completed successfully");
        Ok(())
    }

    pub async fn handle_handshake_message(
        &self,
        payload: &mut HandshakeMessage,
    ) -> P2PResult<()> {
        match payload {
            HandshakeMessage::HandshakeFirstConnectRequest(payload) => {
                self.process_first_user_connection_request(payload).await
            }
            HandshakeMessage::HandshakeFirstConnectResponse(payload) => {
                self.process_first_user_connection_handshake_response(payload).await
            }
            HandshakeMessage::HandshakeExchange(payload) => {
                self.process_exchange_message(payload).await
            }
            HandshakeMessage::HandshakeWebsiteRequest(payload) => {
                self.process_website_handshake_request(payload).await
            }
            HandshakeMessage::HandshakeWebsiteResponse(payload) => {
                self.process_website_handshake_response(payload).await
            }
            HandshakeMessage::HandshakeWebsiteReconnectRequest(payload) => {
                self.process_website_reconnect_request(payload).await
            }
            HandshakeMessage::HandshakeWebsiteReconnectResponse(payload) => {
                self.process_website_reconnect_response(payload).await
            }
        }
    }

    #[instrument(skip(self, payload), fields(
        connection_id = %self.get_id(), 
        is_initiator = self.is_initiator
    ), level = "info")]
    pub async fn process_exchange_message(
        &self,
        payload: &UcanAndUserExchange,
    ) -> P2PResult<()> {
        info!("Processing handshake exchange message");
        
        // Crypto error automatically propagates
        let peer_ucan_pub = crypto_utils::verify_clear_text_message(
            &payload.peer_user.public_key,
            &payload.signed_ucan_pub,
        ).await?;
        debug!("Successfully verified peer's signed UCAN public key");

        let current_user = self.get_local_user().await?;
        let current_device = self.get_local_device().await
            .ok_or_else(|| HandshakeError::MissingPeerInfo)?;
        debug!("Retrieved local user and device");
        
        // Crypto error automatically propagates
        let token_validation = crypto_utils::validate_connect_token(
            &payload.ucan_token,
            &peer_ucan_pub,
            &current_user.ucan_pub_key,
            &current_user.id,
            &self.domain,
        ).await?;

        if !token_validation {
            error!("Peer's connect token is invalid");
            return Err(HandshakeError::InvalidCredentials {
                user_id: payload.peer_user.id.clone(),
            }.into());
        }
        
        debug!("Peer's connect token is valid");
        self.set_peer_user_and_device(payload.peer_user.clone(), payload.peer_device.clone()).await;

        if self.is_initiator {
            info!("This peer is the initiator. Completing handshake.");
            let mut handshake_complete = self.handshake_complete.lock().await;
            *handshake_complete = true;
            self.start_user_network_sync().await?;
            info!("Handshake marked as complete for initiator.");
        } else {
            info!("This peer is the responder. Preparing and sending exchange response.");
            
            // Service error automatically propagates
            let signed_ucan_pub = sign_ucan_pub_key(&self.crypto_utils, self.repo_ctx.clone()).await?;
            debug!("(Responder) Successfully signed the UCAN public key");

            let peer_id = self.connection.remote_node_id().map_err(|e| P2PError::Custom(e.to_string()))?;
            let device_id_b64 = general_purpose::STANDARD.encode(peer_id);

            // Repository error automatically propagates
            let peer_user = self.repo_ctx
                .user_repo
                .get_user_by_device_id(&device_id_b64)
                .await?;
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
            ))).await?;
            
            self.set_connection_type(ConnectionType::User).await;
            info!("(Responder) Sent handshake exchange response and marked handshake as complete.");
        }
        
        Ok(())
    }

    #[instrument(skip(self, payload), fields(connection_id = %self.get_id()), level = "info")]
    pub async fn process_first_user_connection_request(
        &self,
        payload: &FirstConnectRequest,
    ) -> P2PResult<()> {
        info!("Processing first user connection request");
        
        let current_user = self.get_local_user().await?;
        let current_device = self.get_local_device().await
            .ok_or_else(|| HandshakeError::MissingPeerInfo)?;
        debug!("Retrieved local user and device information");

        // Crypto error automatically propagates
        let peer_ucan_pub = crypto_utils::verify_clear_text_message(
            &payload.peer_user.public_key,
            &payload.signed_ucan_pub,
        ).await?;
        debug!("Successfully verified peer's signed UCAN public key");

        // Crypto error automatically propagates
        let one_time_token_validation = crypto_utils::validate_connect_token(
            &payload.one_time_ucan,
            &peer_ucan_pub,
            &current_user.ucan_pub_key,
            &current_user.id,
            &self.domain,
        ).await?;
        
        debug!("Completed validation of one-time UCAN");
        
        if !one_time_token_validation {
            error!("Peer's one-time UCAN is invalid");
            return Err(HandshakeError::InvalidCredentials {
                user_id: payload.peer_user.id.clone(),
            }.into());
        }
        
        info!("Peer's one-time UCAN is valid. Proceeding to issue persistent UCAN.");

        // Service errors automatically propagate
        let signed_ucan_pub = sign_ucan_pub_key(&self.crypto_utils, self.repo_ctx.clone()).await?;
        let peer_issued_ucan_token = issue_connect_ucan_token(
            self.repo_ctx.clone(),
            &self.crypto_utils,
            &self.domain,
            &peer_ucan_pub,
        ).await?;
        debug!("Successfully issued new UCAN token for peer and signed local public key");

        let mut user = payload.peer_user.clone();
        user.first_sync = true;
        user.owner = false;
        user.ucan_token = payload.issued_ucan.clone();
        user.ucan_pub_key = peer_ucan_pub;
        
        self.set_connection_type(ConnectionType::User).await;
        self.set_peer_user_and_device(payload.peer_user.clone(), payload.peer_device.clone()).await;

        let user_with_devices = UserWithDevices {
            user,
            devices: payload.devices.clone(),
        };
        
        // Repository error automatically propagates
        self.repo_ctx
            .user_repo
            .add_users_with_devices_bulk(&vec![user_with_devices])
            .await?;
        info!("Successfully added new user and their devices to the repository");

        // Service error automatically propagates
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
        )).await?;

        info!("First connect handshake completed. Sent response message to peer.");
        Ok(())
    }

    #[instrument(skip(self, payload), fields(connection_id = %self.get_id()), level = "info")]
    pub async fn process_first_user_connection_handshake_response(
        &self,
        payload: &FirstConnectResponse,
    ) -> P2PResult<()> {
        info!("Processing first user connection handshake response");
        debug!("Updated local user's UCAN token with the one from the payload");

        // Crypto error automatically propagates
        let peer_ucan_pub = crypto_utils::verify_clear_text_message(
            &payload.peer_user.public_key,
            &payload.signed_ucan_pub,
        ).await?;
        debug!("Successfully verified peer's signed UCAN public key from response");

        let current_user = self.get_local_user().await?;
        debug!("Retrieved local user information");
        
        // Crypto error automatically propagates
        let token_validation_result = crypto_utils::validate_connect_token(
            &payload.ucan_token,
            &peer_ucan_pub,
            &current_user.ucan_pub_key,
            &current_user.id,
            &self.domain,
        ).await?;

        if !token_validation_result {
            error!("The UCAN issued by the peer is invalid");
            return Err(HandshakeError::InvalidCredentials {
                user_id: payload.peer_user.id.clone(),
            }.into());
        }
        
        debug!("The UCAN issued by the peer is valid");

        let mut user = payload.peer_user.clone();
        user.ucan_token = payload.issued_ucan.clone();
        user.first_sync = true;
        user.owner = false;
        user.ucan_pub_key = peer_ucan_pub;

        self.set_peer_user_and_device(payload.peer_user.clone(), payload.peer_device.clone()).await;

        let user_with_devices = UserWithDevices {
            user,
            devices: payload.devices.clone(),
        };

        let mut handshake_complete = self.handshake_complete.lock().await;
        *handshake_complete = true;

        // Repository error automatically propagates
        self.repo_ctx
            .user_repo
            .add_users_with_devices_bulk(&vec![user_with_devices])
            .await?;
        
        info!("Successfully added peer user and devices to repository. Handshake complete.");
        
        self.start_user_network_sync().await?;
        Ok(())
    }

    #[instrument(skip(self), level = "info")]
    pub async fn execute_connection_action(&self) -> P2PResult<()> {
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
                    info!("Live edit triggered");
                    let connection_id = self.get_id();
                    self.event_emitter.emit(P2PEvent::LiveEditConnected { connection_id });
                    Ok(())
                }
                ConnectionAction::UserSync => self.start_user_network_sync().await,
            }
        } else {
            debug!("No connection action to execute");
            Ok(())
        }
    }

    #[instrument(skip(self, payload), fields(connection_id = %self.get_id()), level = "info")]
    pub async fn process_website_handshake_request(
        &self,
        payload: &osvauld_core::models::WebsiteHandshakeRequest,
    ) -> P2PResult<()> {
        info!("Processing website handshake request from viewer");

        let current_user = self.get_local_user().await?;
        let current_device = self.get_local_device().await
            .ok_or_else(|| HandshakeError::MissingPeerInfo)?;
        debug!("Retrieved local user and device information");

        // Validate folder UCAN token
        info!("Validating folder UCAN token from viewer");
        let ucan = crypto_utils::ucan_utils::validate_structure(&payload.ucan_token)
            .await
            .map_err(|e| {
                error!("Failed to parse viewer's UCAN token: {}", e);
                HandshakeError::InvalidCredentials {
                    user_id: payload.viewer_user.id.clone(),
                }
            })?;

        // Extract folder_id from token capabilities
        let folder_id = crypto_utils::ucan_utils::extract_folder_id_from_ucan(&ucan)
            .map_err(|e| {
                error!("Failed to extract folder_id from UCAN: {}", e);
                HandshakeError::InvalidCredentials {
                    user_id: payload.viewer_user.id.clone(),
                }
            })?;

        info!("Validated folder token for folder_id: {}", folder_id);

        // Return the same wildcard token back to viewer
        // Viewer will use this for the first resource request
        // After receiving folder + resources, viewer will use the folder-specific tokens
        let viewer_token = payload.ucan_token.clone();
        info!("Returning wildcard folder token back to viewer for resource requests");

        // Set peer user and device from the viewer (in-memory only, not saved to DB)
        self.set_peer_user_and_device(payload.viewer_user.clone(), payload.viewer_device.clone()).await;
        self.set_connection_type(ConnectionType::Website).await;
        info!("Set peer user and connection type to Website (viewer not saved to database)");

        // Mark handshake as complete
        let mut handshake_complete = self.handshake_complete.lock().await;
        *handshake_complete = true;

        // Send response back to viewer with the wildcard token
        let response = osvauld_core::models::WebsiteHandshakeResponse {
            node_user: current_user,
            node_device: current_device,
            viewer_specific_token: viewer_token,
        };

        self.send_message(Message::Handshake(
            HandshakeMessage::HandshakeWebsiteResponse(response),
        )).await?;

        info!("Website handshake request processed successfully");
        Ok(())
    }

    #[instrument(skip(self, payload), fields(connection_id = %self.get_id()), level = "info")]
    pub async fn process_website_handshake_response(
        &self,
        payload: &osvauld_core::models::WebsiteHandshakeResponse,
    ) -> P2PResult<()> {
        info!("Processing website handshake response from sovereign node");

        // Receive the wildcard folder token back from sovereign node
        // This is the same token we sent in the request (from connection string)
        // We'll use this for the first resource request to get folder + resources
        // After that, we'll use the folder-specific tokens from the folder_share_record
        info!("Received wildcard folder token from sovereign node");

        // Update node user in viewer's local database with the wildcard token
        let mut node_user = payload.node_user.clone();
        node_user.ucan_token = payload.viewer_specific_token.clone(); // Save wildcard token for first resource request
        node_user.first_sync = true; // Mark as synced
        node_user.owner = false;

        // Get node's devices (for now, just the current device)
        let node_devices = vec![payload.node_device.clone()];

        let user_with_devices = UserWithDevices {
            user: node_user.clone(),
            devices: node_devices,
        };

        // Save or update node user in viewer's database
        self.repo_ctx
            .user_repo
            .add_users_with_devices_bulk(&vec![user_with_devices])
            .await?;

        info!("Updated sovereign node user in local database with new token and first_sync=true");

        // Set peer user and device from the sovereign node
        self.set_peer_user_and_device(node_user, payload.node_device.clone()).await;
        debug!("Set peer user and device from node response");

        // Mark handshake as complete
        let mut handshake_complete = self.handshake_complete.lock().await;
        *handshake_complete = true;
        drop(handshake_complete);

        info!("Website handshake response processed successfully");

        // Start website sync directly (only if initiator)
        if self.is_initiator {
            info!("Initiator: Starting website sync");
            self.start_website_sync(false).await?;
        }

        Ok(())
    }

    #[instrument(skip(self, payload), fields(connection_id = %self.get_id()), level = "info")]
    pub async fn process_website_reconnect_request(
        &self,
        payload: &osvauld_core::models::WebsiteReconnectRequest,
    ) -> P2PResult<()> {
        info!("Processing website reconnect request from viewer");

        let current_user = self.get_local_user().await?;
        let current_device = self.get_local_device().await
            .ok_or_else(|| HandshakeError::MissingPeerInfo)?;
        debug!("Retrieved local user and device information");

        // Validate folder UCAN token
        info!("Validating folder UCAN token from viewer");
        let ucan = crypto_utils::ucan_utils::validate_structure(&payload.ucan_token)
            .await
            .map_err(|e| {
                error!("Failed to parse viewer's UCAN token: {}", e);
                HandshakeError::InvalidCredentials {
                    user_id: payload.viewer_user.id.clone(),
                }
            })?;

        // Extract folder_id from token capabilities for validation
        let folder_id = crypto_utils::ucan_utils::extract_folder_id_from_ucan(&ucan)
            .map_err(|e| {
                error!("Failed to extract folder_id from UCAN: {}", e);
                HandshakeError::InvalidCredentials {
                    user_id: payload.viewer_user.id.clone(),
                }
            })?;

        info!("Validated folder token for folder_id: {} (reconnection)", folder_id);

        // Set peer user and device from the viewer (in-memory only, not saved to DB)
        self.set_peer_user_and_device(payload.viewer_user.clone(), payload.viewer_device.clone()).await;
        self.set_connection_type(ConnectionType::Website).await;
        info!("Set peer user and connection type to Website (reconnection)");

        // Mark handshake as complete
        let mut handshake_complete = self.handshake_complete.lock().await;
        *handshake_complete = true;

        // Send response back to viewer
        let response = osvauld_core::models::WebsiteReconnectResponse {
            node_user: current_user,
            node_device: current_device,
        };

        self.send_message(Message::Handshake(
            HandshakeMessage::HandshakeWebsiteReconnectResponse(response),
        )).await?;

        info!("Website reconnect request processed successfully");
        Ok(())
    }

    #[instrument(skip(self, payload), fields(connection_id = %self.get_id()), level = "info")]
    pub async fn process_website_reconnect_response(
        &self,
        payload: &osvauld_core::models::WebsiteReconnectResponse,
    ) -> P2PResult<()> {
        info!("Processing website reconnect response from sovereign node");

        // Set peer user and device from the sovereign node
        self.set_peer_user_and_device(payload.node_user.clone(), payload.node_device.clone()).await;
        debug!("Set peer user and device from node response");

        // Mark handshake as complete
        let mut handshake_complete = self.handshake_complete.lock().await;
        *handshake_complete = true;
        drop(handshake_complete);

        info!("Website reconnect response processed successfully");

        // Start website sync with first_sync = true (reconnection = incremental sync)
        if self.is_initiator {
            info!("Initiator: Starting website sync for reconnection");
            self.start_website_sync(true).await?;
        }

        Ok(())
    }
}
