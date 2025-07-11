use crate::p2p::constants::*;
use crate::p2p::errors::{HandshakeError, P2PError};
use crate::p2p::p2p_service::P2PService;
use crate::p2p::peer_connection::PeerConnection;
use crate::p2p::P2PEvent;

use crypto_utils::{verify_signature, CryptoUtils};
use iroh::endpoint::{Connection, RecvStream, SendStream};
use iroh::{NodeAddr, NodeId};
use osvauld_core::models::p2p::{ConnectionAction, ConnectionType, HandshakeMessage};
use osvauld_core::models::{
    Device, HandshakeConfirm, HandshakeInit, HandshakeResponse, Message, User,
};
use osvauld_services::{generate_challenge, sign_random_challenge};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::time::timeout;
use tracing::{debug, error, info, info_span, instrument, trace, warn, Instrument};
#[derive(Clone, Debug)]
pub struct HandshakeResult {
    pub connection_type: ConnectionType,
    pub device: Device,
    pub user: User,
}
impl P2PService {
    /// Performs the handshake process and creates a peer connection
    ///
    /// This function handles both initiator and receiver sides of the handshake.
    /// It establishes a bi-directional stream, exchanges handshake messages,
    /// and creates a peer connection upon successful handshake.
    pub async fn perform_handshake_and_create_peer(
        &self,
        conn: &Connection,
        is_initiator: bool,
        connection_type: Option<ConnectionType>,
        action: Option<ConnectionAction>,
    ) -> Result<Arc<PeerConnection>, P2PError> {
        debug!("Opening bi-directional stream for handshake");

        let (mut send, mut recv) = match timeout(CONNECTION_TIMEOUT, async {
            if is_initiator {
                trace!("Initiator: Opening bi-directional stream");
                conn.open_bi().await
            } else {
                trace!("Receiver: Accepting bi-directional stream");
                conn.accept_bi().await
            }
        })
        .await
        {
            Ok(Ok(stream)) => {
                debug!("Bi-directional stream established");
                stream
            }
            Ok(Err(e)) => {
                error!("Failed to establish bi-directional stream: {}", e);
                return Err(P2PError::Connection(format!(
                    "Stream establishment failed: {}",
                    e
                )));
            }
            Err(e) => {
                error!("Timeout while establishing bi-directional stream: {}", e);
                return Err(P2PError::Timeout(format!("Stream timeout: {}", e)));
            }
        };

        let handshake_message = if is_initiator {
            let conn_type = connection_type.unwrap_or(ConnectionType::Device);
            debug!("Initiating handshake as {:?}", conn_type);

            match self
                .initiate_handshake(&mut send, &mut recv, conn_type)
                .await
            {
                Ok(msg) => {
                    debug!("Handshake initiated successfully");
                    trace!("Received handshake response from user: {}", msg.user.id);
                    msg
                }
                Err(e) => {
                    error!("Handshake initiation failed: {}", e);
                    return Err(P2PError::Handshake(e));
                }
            }
        } else {
            debug!("Accepting incoming handshake");

            match self.accept_handshake(&mut send, &mut recv).await {
                Ok(msg) => {
                    debug!("Handshake accepted successfully");
                    trace!("Accepted handshake from user: {}", msg.user.id);
                    msg
                }
                Err(e) => {
                    error!("Handshake acceptance failed: {}", e);
                    return Err(P2PError::Handshake(e));
                }
            }
        };

        // Get the state once to access what we need
        let state_guard = self.state.lock().await;
        let state = state_guard.as_ref().ok_or(P2PError::NotInitialized)?;

        // Create the PeerConnection object with the new design
        let connection_arc = Arc::new(conn.clone());
        debug!("Creating peer connection object");
        let resources_needing_update = Vec::new();
        let self_clone = self.clone();
        let cleanup_callback = Box::new(move |connection_id: String| {
            let service = self_clone.clone();

            // Spawn a task to handle the cleanup
            tokio::spawn(async move {
                info!(
                    "Connection cleanup callback triggered for: {}",
                    connection_id
                );

                // Get lock on the state
                let state_guard = service.state.lock().await;
                if let Some(state) = state_guard.as_ref() {
                    // Remove the connection
                    if let Err(e) = state.connections.remove_connection(&connection_id).await {
                        error!("Failed to remove connection {}: {}", connection_id, e);
                    } else {
                        info!(
                            "Successfully removed connection from manager: {}",
                            connection_id
                        );
                    }
                }
            });
        });
        let peer_device = handshake_message.device.clone();

        let node_id_bytes = crypto_utils::derive_node_id_from_public_key(&peer_device.device_key)?;
        let node_id = NodeId::try_from(&node_id_bytes).map_err(|e| e.to_string())?;
        let node_id = node_id.to_string();

        let peer_connection = PeerConnection::new(
            connection_arc,
            handshake_message.connection_type.clone(),
            handshake_message.device.clone(),
            handshake_message.user.clone(),
            node_id,
            is_initiator,
            state.service_context.clone(),
            self.event_emitter.clone(),
            resources_needing_update,
            action,
            Some(cleanup_callback),
            self.crypto_utils.clone(),
            self.repo_ctx.clone(),
        );
        peer_connection.execute_connection_action().await?;

        let peer_connection_arc = Arc::new(peer_connection);
        debug!("Created peer connection: {}", peer_connection_arc.get_id());

        // Insert the connection into the connection manager
        debug!("Inserting connection into connection manager");
        if let Err(e) = state
            .connections
            .insert_connection(peer_connection_arc.clone())
            .await
        {
            error!("Failed to insert connection: {}", e);
            return Err(P2PError::PeerConnection(e));
        }
        let connection_id = peer_connection_arc.get_id();
        let has_pending_live_edit = state
            .connections
            .get_and_clear_pending_live_edit_requests(&connection_id)
            .await;
        if has_pending_live_edit {
            info!(
                "Found pending live edit request for connection: {}",
                connection_id
            );
            self.event_emitter.emit(P2PEvent::LiveEditConnected {
                connection_id: connection_id.clone(),
            });
        }
        // Emit connected event
        debug!("Emitting connection events");
        self.event_emitter.emit(P2PEvent::Connected);

        info!(
            "Handshake and peer creation successful: {}",
            peer_connection_arc.get_id()
        );
        Ok(peer_connection_arc)
    }

    /// Reads a complete message from the stream, handling fragmentation
    ///
    /// This function handles the case where messages may come in multiple
    /// chunks, and reassembles them into a complete message.
    #[instrument(skip(self, recv), level = "debug")]
    async fn read_complete_message(&self, recv: &mut RecvStream) -> Result<String, HandshakeError> {
        debug!("Reading complete message (potentially fragmented)");
        let mut buffer = Vec::with_capacity(MAX_HANDSHAKE_SIZE);
        let mut temp_buf = [0u8; 8192];
        let mut total_read = 0;
        let mut read_attempts = 0;

        // Continue reading until we get a complete JSON object or error out
        loop {
            read_attempts += 1;
            trace!("Read attempt #{}", read_attempts);

            match timeout(HANDSHAKE_TIMEOUT, recv.read(&mut temp_buf)).await {
                Ok(Ok(Some(n))) => {
                    if n == 0 {
                        // End of stream
                        debug!("End of stream reached after reading {} bytes", total_read);
                        break;
                    }

                    total_read += n;
                    debug!(
                        "Read chunk {}: {} bytes (total: {} bytes)",
                        read_attempts, n, total_read
                    );

                    // Add the new chunk to our buffer
                    buffer.extend_from_slice(&temp_buf[..n]);

                    // Check if we've exceeded max size
                    if buffer.len() > MAX_HANDSHAKE_SIZE {
                        let err = format!(
                            "Message too large: {} bytes (max: {})",
                            buffer.len(),
                            MAX_HANDSHAKE_SIZE
                        );
                        error!("{}", err);
                        return Err(HandshakeError::Connection(err));
                    }

                    // Check if we now have a valid UTF-8 string that can be parsed as valid JSON
                    if let Ok(message_str) = String::from_utf8(buffer.clone()) {
                        match serde_json::from_str::<HandshakeMessage>(&message_str) {
                            Ok(_) => {
                                // Success! We have a complete JSON message
                                debug!(
                                    "Successfully parsed complete JSON message ({} bytes)",
                                    message_str.len()
                                );
                                return Ok(message_str);
                            }
                            Err(e) if e.is_eof() || e.is_data() => {
                                // JSON is incomplete or invalid, continue reading
                                trace!("JSON parsing incomplete, continuing to read: {}", e);
                            }
                            Err(e) => {
                                // Other JSON error, log but continue trying
                                debug!("JSON parsing error (continuing): {}", e);
                            }
                        }
                    }
                }
                Ok(Ok(None)) => {
                    // End of stream reached
                    debug!("End of stream reached after reading {} bytes", total_read);
                    break;
                }
                Ok(Err(e)) => {
                    error!("Error reading from stream: {}", e);
                    return Err(HandshakeError::Connection(e.to_string()));
                }
                Err(e) => {
                    error!(
                        "Timeout while reading from stream after {} attempts: {}",
                        read_attempts, e
                    );
                    return Err(HandshakeError::Connection(format!("Timeout: {}", e)));
                }
            }
        }

        // If we got here, the stream ended before we got a complete message
        if buffer.is_empty() {
            warn!("Empty message received");
            return Err(HandshakeError::Connection("Empty message received".into()));
        }

        // Try to convert to a string for better error reporting
        match String::from_utf8(buffer) {
            Ok(s) => {
                let preview = if s.len() > 100 { &s[..100] } else { &s };
                error!(
                    "Incomplete JSON after reading {} bytes. Preview: {}...",
                    total_read, preview
                );
                Err(HandshakeError::Connection(format!(
                    "Incomplete message: {} bytes read but no valid JSON",
                    total_read
                )))
            }
            Err(_) => {
                error!("Invalid UTF-8 in message ({} bytes read)", total_read);
                Err(HandshakeError::Connection(format!(
                    "Invalid UTF-8 in message ({} bytes read)",
                    total_read
                )))
            }
        }
    }

    async fn send_handshake_message<T: Serialize>(
        send: &mut SendStream,
        message: &T,
        message_name: &str,
    ) -> Result<(), HandshakeError> {
        let serialized = match serde_json::to_string(message) {
            Ok(json) => {
                debug!("Serialized {} message: {} bytes", message_name, json.len());
                json
            }
            Err(e) => {
                error!("Failed to serialize {}: {}", message_name, e);
                return Err(HandshakeError::Serialization(e.to_string()));
            }
        };

        // Send the message
        if let Err(e) = send.write_all(serialized.as_bytes()).await {
            error!("Failed to write {}: {}", message_name, e);
            return Err(HandshakeError::Connection(e.to_string()));
        }

        if let Err(e) = send.finish() {
            error!("Failed to finish sending {}: {}", message_name, e);
            return Err(HandshakeError::Connection(e.to_string()));
        }

        debug!("Successfully sent {}", message_name);
        Ok(())
    }
    /// Initiates the handshake process by sending our handshake message and waiting for a response
    ///
    /// This is called by the party that initiated the connection.
    #[instrument(skip_all, level = "debug")]
    async fn initiate_handshake(
        &self,
        send: &mut SendStream,
        recv: &mut RecvStream,
        connection_type: ConnectionType,
    ) -> Result<HandshakeResult, HandshakeError> {
        info!("Initiator: Starting secure 3-message handshake");

        // Get our device and user information
        let current_user = self
            .get_current_user()
            .await
            .map_err(|_| HandshakeError::AuthService("Failed to get current user".to_string()))?;

        let current_device = self
            .get_current_device()
            .await
            .map_err(|_| HandshakeError::AuthService("Failed to get current device".to_string()))?;

        // Generate our challenge and timestamp
        let our_challenge = generate_challenge();
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Step 1: Send HandshakeInit
        let handshake_init = HandshakeInit {
            connection_type: connection_type.clone(),
            user: current_user.clone(),
            device: current_device.clone(),
            challenge: our_challenge.clone(),
            timestamp,
        };

        Self::send_handshake_message(send, &handshake_init, "HandshakeInit").await?;
        info!("Initiator: Sent HandshakeInit, waiting for HandshakeResponse");

        // Step 2: Receive HandshakeResponse
        let response_str = self.read_complete_message(recv).await?;
        let handshake_response: HandshakeResponse =
            serde_json::from_str(&response_str).map_err(|e| HandshakeError::Deserialization(e))?;

        // Validate timestamp (allow 5 minutes skew)
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        if handshake_response.timestamp.abs_diff(current_time) > 300 {
            error!("HandshakeResponse timestamp too old or too far in future");
            return Err(HandshakeError::InvalidChallenge(
                "Timestamp validation failed".to_string(),
            ));
        }

        // Verify that the receiver correctly signed our challenge
        let verified = verify_signature(
            &handshake_response.user.public_key,
            &our_challenge,
            &handshake_response.challenge_signature,
        )
        .map_err(|e| {
            error!("Failed to verify challenge signature: {}", e);
            HandshakeError::InvalidChallenge(format!("Signature verification failed: {}", e))
        })?;

        if !verified {
            error!("Invalid challenge signature from receiver");
            return Err(HandshakeError::InvalidChallenge(
                "Challenge signature verification failed".to_string(),
            ));
        }

        info!("Initiator: Successfully verified receiver's signature of our challenge");

        // Step 3: Sign their challenge and send confirmation
        let their_challenge = handshake_response.challenge.clone();

        let our_signature = {
            let crypto_utils = self.crypto_utils.lock().await;
            crypto_utils.sign_message(&their_challenge).map_err(|e| {
                error!("Failed to sign receiver's challenge: {}", e);
                HandshakeError::InvalidChallenge(format!("Failed to sign challenge: {}", e))
            })?
        };

        let handshake_confirm = HandshakeConfirm {
            challenge_signature: our_signature,
        };

        Self::send_handshake_message(send, &handshake_confirm, "HandshakeConfirm").await?;
        info!("Initiator: Handshake completed successfully");

        // Return the peer's information
        Ok(HandshakeResult {
            connection_type,
            device: handshake_response.device,
            user: handshake_response.user,
        })
    }

    /// Accepts an incoming handshake by receiving a handshake message and responding
    ///
    /// This is called by the party that accepted the connection.
    #[instrument(skip(self, send, recv), level = "debug")]
    async fn accept_handshake(
        &self,
        send: &mut SendStream,
        recv: &mut RecvStream,
    ) -> Result<HandshakeResult, HandshakeError> {
        info!("Receiver: Starting secure 3-message handshake");

        // Step 1: Receive HandshakeInit
        let message_str = self.read_complete_message(recv).await?;
        let handshake_init: HandshakeInit = serde_json::from_str(&message_str)
            .map_err(|e| HandshakeError::Serialization(e.to_string()))?;

        // Validate timestamp (allow 5 minutes skew)
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        if handshake_init.timestamp.abs_diff(current_time) > 300 {
            error!("HandshakeInit timestamp too old or too far in future");
            return Err(HandshakeError::InvalidChallenge(
                "Timestamp validation failed".to_string(),
            ));
        }

        // Get our device and user information
        let current_device = self
            .get_current_device()
            .await
            .map_err(|_| HandshakeError::AuthService("Failed to get current device".to_string()))?;

        let current_user = self
            .get_current_user()
            .await
            .map_err(|_| HandshakeError::AuthService("Failed to get current user".to_string()))?;

        // Generate our challenge and sign their challenge
        let our_challenge = generate_challenge();

        let their_challenge_signature = {
            let crypto_utils = self.crypto_utils.lock().await;
            crypto_utils
                .sign_message(&handshake_init.challenge)
                .map_err(|e| {
                    error!("Failed to sign initiator's challenge: {}", e);
                    HandshakeError::InvalidChallenge(format!("Failed to sign challenge: {}", e))
                })?
        };

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Step 2: Send HandshakeResponse
        let handshake_response = HandshakeResponse {
            user: current_user.clone(),
            device: current_device.clone(),
            challenge: our_challenge.clone(),
            timestamp,
            challenge_signature: their_challenge_signature,
        };

        Self::send_handshake_message(send, &handshake_response, "HandshakeResponse").await?;
        info!("Receiver: Sent HandshakeResponse, waiting for HandshakeConfirm");

        // Step 3: Receive HandshakeConfirm
        let confirm_str = self.read_complete_message(recv).await?;
        let handshake_confirm: HandshakeConfirm =
            serde_json::from_str(&confirm_str).map_err(|e| HandshakeError::Deserialization(e))?;

        // Verify that the initiator correctly signed our challenge
        let verified = verify_signature(
            &handshake_init.user.public_key,
            &our_challenge,
            &handshake_confirm.challenge_signature,
        )
        .map_err(|e| {
            error!("Failed to verify confirmation signature: {}", e);
            HandshakeError::InvalidChallenge(format!(
                "Confirmation signature verification failed: {}",
                e
            ))
        })?;

        if !verified {
            error!("Invalid confirmation signature from initiator");
            return Err(HandshakeError::InvalidChallenge(
                "Confirmation signature verification failed".to_string(),
            ));
        }

        info!("Receiver: Successfully verified initiator's signature of our challenge");
        info!("Receiver: Handshake completed successfully");

        // Return the peer's information
        Ok(HandshakeResult {
            connection_type: handshake_init.connection_type,
            device: handshake_init.device,
            user: handshake_init.user,
        })
    }

    #[instrument(skip(self,  conn_type), fields( conn_type = ?conn_type ), level = "info")]
    pub async fn connect_with_ticket(
        &self,
        device_id: &str,
        conn_type: ConnectionType,
        action: Option<ConnectionAction>,
    ) -> Result<Option<Arc<PeerConnection>>, P2PError> {
        info!("Starting connection process with ticket");

        let node_id_bytes = crypto_utils::derive_node_id_from_public_key(&device_id)?;
        let node_id = NodeId::try_from(&node_id_bytes).map_err(|e| e.to_string())?;
        // Ensure P2P service is initialized
        self.ensure_initialized().await?;

        let connection_id = node_id.to_string();
        // Get the state for access to connections
        let (endpoint, connections) = {
            let state_guard = self.state.lock().await;
            let state = state_guard.as_ref().ok_or(P2PError::NotInitialized)?;
            (state.endpoint.clone(), state.connections.clone())
        };

        // If a connection ID was provided, check if the connection is already active
        // Check if connection is already established or in connecting state
        if connections.is_connection_active(&connection_id).await {
            // Try to get an established connection
            if let Ok(existing_connection) = connections.get_peer_connection(&connection_id).await {
                info!("Using existing established connection: {}", &connection_id);
                if let Some(action) = action {
                    if action == ConnectionAction::LiveEdit {
                        self.event_emitter
                            .emit(P2PEvent::LiveEditConnected { connection_id });
                    } else {
                        if existing_connection.is_initiator {
                            let _ = existing_connection.execute_connection_action().await;
                        } else {
                            let _ = existing_connection
                                .send_message(Message::RetryRequest)
                                .await;
                        }
                    }
                }
                return Ok(Some(existing_connection));
            } else {
                // If we're here, the connection is in connecting state but not yet established
                info!(
                    "Connection is currently being established (returning None): {}",
                    &connection_id
                );

                connections
                    .add_pending_live_edit_request(&connection_id)
                    .await;
                return Ok(None);
            }
        }

        // Mark the connection as connecting
        if let Err(e) = connections.mark_as_connecting(&connection_id).await {
            // This shouldn't happen given the previous check, but handle it just in case
            warn!("Failed to mark connection as connecting: {}", e);
            return Err(P2PError::Connection(format!(
                "Failed to mark connection as connecting: {}",
                e
            )));
        }

        // Helper function to clean up connecting state on error
        let cleanup_connecting = |connection_id: Option<&str>, error: P2PError| -> P2PError {
            if let Some(id) = connection_id {
                // We need to spawn a task because we can't use .await in a closure
                let connection_id = id.to_string();
                let state_connections = connections.clone();
                tokio::spawn(async move {
                    state_connections
                        .remove_from_connecting(&connection_id)
                        .await;
                });
            }
            error
        };

        // Create node address from parsed components
        let node_addr = NodeAddr::new(node_id.clone());

        debug!("Created NodeAddr: {:?}", node_addr);
        debug!("Our endpoint ID: {}", endpoint.node_id());
        debug!("ALPN Protocol: {}", String::from_utf8_lossy(ALPN_PROTOCOL));

        // Attempt to establish connection
        info!("Connecting to remote endpoint...");
        let connection_span = info_span!("endpoint_connect", 
        remote_node_id = %node_addr.node_id,
        addresses = ?node_addr.direct_addresses.len());
        debug!("Attempting connection to node_id: {}", node_id);
        debug!("NodeAddr created: {:?}", node_addr);
        debug!("Using ALPN: {}", String::from_utf8_lossy(ALPN_PROTOCOL));

        let connect_result = endpoint
            .connect(node_addr.clone(), ALPN_PROTOCOL)
            .instrument(connection_span)
            .await;

        // Handle connection result
        let conn = match connect_result {
            Ok(conn) => {
                info!("Connection established successfully");
                conn
            }
            Err(e) => {
                error!("Connection failed: {}", e);
                debug!("Node addr used: {:?}", node_addr);

                let error = P2PError::Connection(format!("Connection failed: {}", e));
                return Err(cleanup_connecting(Some(&connection_id), error));
            }
        };

        // Proceed with handshake
        info!("Starting handshake process");
        let handshake_span =
            info_span!("handshake", initiator = true, connection_type = ?conn_type);

        // Perform handshake
        let handshake_result = self
            .perform_handshake_and_create_peer(&conn, true, Some(conn_type), action)
            .instrument(handshake_span)
            .await;

        // Clean up connecting state if handshake fails
        // If handshake succeeds, the connection will be in the established connections map
        // and handle_peer_and_create_connection will call insert_connection which removes from connecting
        match handshake_result {
            Ok(peer_connection) => {
                // Return the new peer connection
                Ok(Some(peer_connection))
            }
            Err(e) => {
                // Clean up connecting state
                let state_guard = self.state.lock().await;
                if let Some(state) = &*state_guard {
                    state
                        .connections
                        .remove_from_connecting(&connection_id)
                        .await;
                }
                Err(e)
            }
        }
    }
}
