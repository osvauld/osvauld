use crate::p2p::{
    errors::{HandshakeError, P2PError, P2PResult},
    peer_connection::{ConnectionType, PeerConnection},
    P2PEvent,
};

use base64::{engine::general_purpose, Engine as _};
use osvauld_core::models::{
    Device, FirstConnectRequest, FirstConnectResponse,
    HandshakeMessage, Message, PeerRole, UcanAndUserExchange, User, UserWithDevices,
    ViewerHandshakeRequest, ViewerHandshakeResponse,
};
use services::{get_my_user_devices, issue_connect_ucan_token, sign_ucan_pub_key};
use tracing::{debug, error, info, instrument};

impl PeerConnection {
    #[instrument(skip(self, current_user, current_device), fields(
        connection_id = %self.get_id(),
        peer_id = ?self.connection.remote_node_id()
    ), level = "info")]
    pub async fn initiate_handshake(
        &self,
        current_user: User,
        current_device: Device,
    ) -> P2PResult<()> {
        info!("Initiating handshake");

        let peer_id = self.connection.remote_node_id().map_err(|e| P2PError::Custom(e.to_string()))?;
        debug!("Successfully retrieved peer id: {}", peer_id);
        let device_id_b64 = general_purpose::STANDARD.encode(peer_id);

        // CRITICAL: Fetch peer_user BEFORE role check (so viewer can issue token and set peer_user)
        let peer_user = self.repo_ctx
            .user_repo
            .get_user_by_device_id(&device_id_b64)
            .await?;
        debug!("Successfully retrieved user for peer");

        // Extract role from peer's UCAN token and convert to PeerRole enum
        let peer_role_from_token = services::ucan_service::get_role(&peer_user.ucan_token)
            .await
            .unwrap_or_else(|e| {
                debug!("Failed to extract role from token: {}, defaulting to user", e);
                "user".to_string()
            });
        let peer_role = PeerRole::from_string(&peer_role_from_token);

        debug!("Extracted peer role from UCAN token: {:?}", peer_role);

        // Check if viewer and handle viewer handshake
        if peer_role == PeerRole::Viewer {
            info!("Viewer connection detected, initiating viewer handshake");
            self.set_connection_type(ConnectionType::Viewer).await;

            // 1. Issue local viewer_node token for the node peer
            let viewer_node_token = services::ucan_service::issue_viewer_to_node_token(
                &peer_user.ucan_pub_key,
                &self.domain,
                self.crypto_utils.clone(),
                &self.repo_ctx,
            )
            .await?;
            debug!("Issued viewer_node token locally");

            // 2. Set peer_user with viewer_node token (stored locally)
            let mut peer_user_with_token = peer_user.clone();
            peer_user_with_token.ucan_token = viewer_node_token;

            // Get peer_device from database
            let peer_device = self.repo_ctx
                .device_repo
                .find_by_id(&device_id_b64)
                .await?;

            self.set_peer_user_and_device(peer_user_with_token, peer_device).await;
            debug!("Set peer_user with viewer_node token");

            // 3. Send ViewerHandshakeRequest with auth token from connection string
            let request = ViewerHandshakeRequest {
                viewer_user: current_user,
                viewer_device: current_device,
                viewer_auth_token: peer_user.ucan_token.clone(), // Auth token from connection string
            };

            self.send_message(Message::Handshake(
                HandshakeMessage::ViewerHandshakeRequest(request),
            ))
            .await?;

            info!("Sent ViewerHandshakeRequest to node");
            return Ok(());
        }

        // Use the same role from the peer's token
        // This ensures role consistency: owner gets 'owner', node gets 'node'
        let issued_token_role = peer_role.as_str();

        debug!("Will issue token with role: {}", issued_token_role);

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
                issued_token_role,  // Use explicit role for token we're issuing
            ).await?;
            debug!("Successfully issued new UCAN token for peer with role '{}'", issued_token_role);

            // Replace current_user's ucan_token with the peer's token (the one we're connecting to)
            let mut user_to_send = current_user.clone();
            
            user_to_send.ucan_token = peer_user.ucan_token.clone();

            self.send_message(Message::Handshake(
                HandshakeMessage::HandshakeFirstConnectRequest(FirstConnectRequest {
                    devices: user_devices,
                    issued_ucan: new_ucan_token,
                    signed_ucan_pub,
                    one_time_ucan: peer_user.ucan_token.clone(),
                    peer_device: current_device,
                    peer_user: user_to_send,
                }),
            )).await?;

            info!("Sent HandshakeFirstConnectRequest to peer");
        } else {
            let mut user_to_send = current_user.clone();
            user_to_send.ucan_token = peer_user.ucan_token.clone();
            info!("Peer is an existing user, preparing HandshakeExchange");
            let exchange_message = UcanAndUserExchange {
                ucan_token: peer_user.ucan_token,
                peer_user: user_to_send,
                peer_device: current_device,
                signed_ucan_pub,
            };
            self.send_message(Message::Handshake(HandshakeMessage::HandshakeExchange(
                exchange_message,
            ))).await?;
            // Set connection type from peer role
            let connection_type = ConnectionType::from_peer_role(&peer_role);
            self.set_connection_type(connection_type).await;
            info!("Sent HandshakeExchange to peer with connection type: {:?}", peer_role);
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
            HandshakeMessage::ViewerHandshakeRequest(payload) => {
                self.process_viewer_handshake_request(payload).await
            }
            HandshakeMessage::ViewerHandshakeResponse(payload) => {
                self.process_viewer_handshake_response(payload).await
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
        services::ucan_service::validate_connect_token(
            &payload.ucan_token,
            &peer_ucan_pub,
            &current_user.ucan_pub_key,
            &current_user.id,
            &self.domain,
        ).await?;

        debug!("Peer's connect token is valid");

        // Update peer user's ucan_token with the token they sent us (proves their capabilities)
        let mut updated_peer_user = payload.peer_user.clone();
        updated_peer_user.ucan_token = payload.ucan_token.clone();

        self.set_peer_user_and_device(updated_peer_user, payload.peer_device.clone()).await;

        // Extract role from UCAN and convert to PeerRole enum
        let peer_role_from_token = services::ucan_service::get_role(&payload.ucan_token)
            .await
            .unwrap_or_else(|e| {
                debug!("Failed to extract role from token: {}, defaulting to user", e);
                "user".to_string()
            });
        let peer_role = PeerRole::from_string(&peer_role_from_token);

        if self.is_initiator {
            info!("This peer is the initiator. Completing handshake.");
            let mut handshake_complete = self.handshake_complete.lock().await;
            *handshake_complete = true;

            // Set connection type from peer role
            let connection_type = ConnectionType::from_peer_role(&peer_role);
            self.set_connection_type(connection_type).await;

            // Emit Connected event
            self.event_emitter.emit(P2PEvent::Connected {
                peer_id: payload.peer_user.id.clone(),
            });

            info!("Handshake marked as complete for initiator. Connection ready for sync requests.");
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
            };

            let mut handshake_complete = self.handshake_complete.lock().await;
            *handshake_complete = true;

            self.send_message(Message::Handshake(HandshakeMessage::HandshakeExchange(
                exchange_message,
            ))).await?;

            // Set connection type from peer role
            let connection_type = ConnectionType::from_peer_role(&peer_role);
            self.set_connection_type(connection_type).await;

            // Emit Connected event
            self.event_emitter.emit(P2PEvent::Connected {
                peer_id: payload.peer_user.id.clone(),
            });

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
        services::ucan_service::validate_connect_token(
            &payload.one_time_ucan,
            &peer_ucan_pub,
            &current_user.ucan_pub_key,
            &current_user.id,
            &self.domain,
        ).await?;

        info!("Peer's one-time UCAN is valid. Proceeding to issue persistent UCAN.");

        // Extract role from one-time UCAN token and convert to PeerRole enum
        let peer_role_from_token = services::ucan_service::get_role(&payload.one_time_ucan).await?;
        let peer_role = PeerRole::from_string(&peer_role_from_token);
        info!("Extracted peer role from one-time UCAN token: {:?}", peer_role);

        // Determine what role to issue in the token we give them
        // Reciprocal relationship: owner gets 'node' token, node gets 'owner' token
        let issued_token_role = match peer_role {
            PeerRole::Owner => "node",  // If peer is owner, issue them 'node' token
            PeerRole::Node => "owner",  // If peer is node, issue them 'owner' token
            _ => "user",  // Default for other peer types
        };
        info!("Will issue token with role: {}", issued_token_role);

        // Service errors automatically propagate
        let signed_ucan_pub = sign_ucan_pub_key(&self.crypto_utils, self.repo_ctx.clone()).await?;
        let peer_issued_ucan_token = issue_connect_ucan_token(
            self.repo_ctx.clone(),
            &self.crypto_utils,
            &self.domain,
            &peer_ucan_pub,
            issued_token_role,  // Use explicit role for token we're issuing
        ).await?;
        debug!("Successfully issued new UCAN token for peer with role '{}' and signed local public key", issued_token_role);

        let mut user = payload.peer_user.clone();
        user.first_sync = true;
        user.owner = false;
        user.ucan_token = payload.issued_ucan.clone();
        user.ucan_pub_key = peer_ucan_pub;

        // Set connection type from peer role
        let connection_type = ConnectionType::from_peer_role(&peer_role);
        self.set_connection_type(connection_type).await;
        // Use updated user with correct ucan_token (the one we received from peer)
        self.set_peer_user_and_device(user.clone(), payload.peer_device.clone()).await;

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

        // Emit role-specific connection event
        let event = match peer_role {
            PeerRole::Node => P2PEvent::NodeConnected {
                peer_id: payload.peer_user.id.clone(),
            },
            _ => P2PEvent::UserConnected {
                peer_id: payload.peer_user.id.clone(),
            },
        };
        self.event_emitter.emit(event);

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
        services::ucan_service::validate_connect_token(
            &payload.ucan_token,
            &peer_ucan_pub,
            &current_user.ucan_pub_key,
            &current_user.id,
            &self.domain,
        ).await?;

        debug!("The UCAN issued by the peer is valid");

        // Extract role from UCAN and convert to PeerRole enum
        let peer_role_from_token = services::ucan_service::get_role(&payload.issued_ucan)
            .await
            .unwrap_or_else(|e| {
                debug!("Failed to extract role from token: {}, defaulting to user", e);
                "user".to_string()
            });
        let peer_role = PeerRole::from_string(&peer_role_from_token);

        let mut user = payload.peer_user.clone();
        user.ucan_token = payload.issued_ucan.clone();
        user.first_sync = true;
        user.owner = false;
        user.ucan_pub_key = peer_ucan_pub;

        // Use updated user with correct ucan_token (the one peer issued to us)
        self.set_peer_user_and_device(user.clone(), payload.peer_device.clone()).await;

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

        // Set connection type from peer role
        let connection_type = ConnectionType::from_peer_role(&peer_role);
        self.set_connection_type(connection_type).await;

        // Emit role-specific connection event
        let event = match peer_role {
            PeerRole::Node => P2PEvent::NodeConnected {
                peer_id: payload.peer_user.id.clone(),
            },
            _ => P2PEvent::UserConnected {
                peer_id: payload.peer_user.id.clone(),
            },
        };
        self.event_emitter.emit(event);

        info!("Successfully added peer user and devices to repository. Handshake complete. Connection ready for sync requests.");

        Ok(())
    }

    /// Process incoming ViewerHandshakeRequest from viewer (node side)
    ///
    /// Node validates the viewer's auth token and sends back ViewerHandshakeResponse.
    #[instrument(skip(self, payload), fields(connection_id = %self.get_id()), level = "info")]
    pub async fn process_viewer_handshake_request(
        &self,
        payload: &ViewerHandshakeRequest,
    ) -> P2PResult<()> {
        info!("Processing ViewerHandshakeRequest from viewer: {}", payload.viewer_user.username);

        // 1. Get local node user and device
        let node_user = self.get_local_user().await?;
        let node_device = self.get_local_device().await
            .ok_or_else(|| HandshakeError::MissingPeerInfo)?;

        debug!("Node user: {}, device: {}", node_user.username, node_device.id);

        // 2. Validate auth token (issuer must be node's DID + structure validation)
        services::ucan_service::validate_ucan_issuer(
            &payload.viewer_auth_token,
            &node_user.ucan_pub_key,
        )
        .await?;

        info!("✓ Viewer auth token validated (issuer: node, structure: valid)");

        // 3. Set peer_user (viewer) and peer_device
        self.set_peer_user_and_device(payload.viewer_user.clone(), payload.viewer_device.clone()).await;
        debug!("Set peer_user (viewer) and peer_device");

        // 4. Mark handshake as complete
        let mut handshake_complete = self.handshake_complete.lock().await;
        *handshake_complete = true;
        info!("Handshake marked as complete on node side");

        // 5. Send ViewerHandshakeResponse back to viewer
        let response = ViewerHandshakeResponse {
            node_user,
            node_device,
        };

        self.send_message(Message::Handshake(
            HandshakeMessage::ViewerHandshakeResponse(response),
        ))
        .await?;

        info!("✅ Sent ViewerHandshakeResponse to viewer. Handshake complete.");
        Ok(())
    }

    /// Process incoming ViewerHandshakeResponse from node (viewer side)
    ///
    /// Viewer marks handshake as complete after receiving response from node.
    #[instrument(skip(self, payload), fields(connection_id = %self.get_id()), level = "info")]
    pub async fn process_viewer_handshake_response(
        &self,
        payload: &ViewerHandshakeResponse,
    ) -> P2PResult<()> {
        info!("Processing ViewerHandshakeResponse from node: {}", payload.node_user.username);

        // Mark handshake as complete
        let mut handshake_complete = self.handshake_complete.lock().await;
        *handshake_complete = true;

        info!("✅ Handshake marked as complete on viewer side. Connection ready.");
        Ok(())
    }

}
