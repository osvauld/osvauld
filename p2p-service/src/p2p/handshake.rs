use crate::p2p::constants::*;
use crate::p2p::errors::{HandshakeError, P2PError};
use crate::p2p::p2p_service::P2PService;
use crate::p2p::peer_connection::PeerConnection;
use crate::p2p::P2PEvent;
use iroh::endpoint::{Connection, RecvStream, SendStream};
use iroh::NodeAddr;
use osvauld_core::models::p2p::{
    ConnectionAction, ConnectionTicket, ConnectionType, HandshakeMessage,
};
use osvauld_services::sign_random_challenge;
use std::fmt::format;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::time::timeout;
use tracing::{debug, error, info, info_span, instrument, trace, warn, Instrument};

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
        let peer_connection = PeerConnection::new(
            connection_arc,
            handshake_message.connection_type.clone(),
            handshake_message.device.clone(),
            handshake_message.user.clone(),
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

    /// Initiates the handshake process by sending our handshake message and waiting for a response
    ///
    /// This is called by the party that initiated the connection.
    #[instrument(skip_all, level = "debug")]
    async fn initiate_handshake(
        &self,
        send: &mut SendStream,
        recv: &mut RecvStream,
        connection_type: ConnectionType,
    ) -> Result<HandshakeMessage, HandshakeError> {
        info!("Initiator: Sending handshake message");

        // Get our device information
        let current_user = self
            .get_current_user()
            .await
            .map_err(|_| HandshakeError::AuthService("failed handshake".to_string()))?;

        let current_device = self
            .get_current_device()
            .await
            .map_err(|_| HandshakeError::AuthService("failed handshake".to_string()))?;
        // Sign a challenge to prove our identity

        let (challenge, signature) = sign_random_challenge(&self.crypto_utils)
            .await
            .map_err(|_| HandshakeError::InvalidChallenge("failed handshake".to_string()))?;

        // Construct the handshake message
        let handshake_message = HandshakeMessage {
            connection_type,
            challenge,
            signature,
            device: current_device,
            user: current_user,
        };

        // Serialize and send the handshake message
        let serialized = match serde_json::to_string(&handshake_message) {
            Ok(json) => {
                debug!("Serialized handshake message: {} bytes", json.len());
                trace!("DIAGNOSTIC: Handshake message size is {} bytes", json.len());
                json
            }
            Err(e) => {
                error!("Failed to serialize handshake message: {}", e);
                return Err(HandshakeError::Serialization(e.to_string()));
            }
        };

        // Write the message to the stream
        if let Err(e) = send.write_all(serialized.as_bytes()).await {
            error!("Failed to write handshake message: {}", e);
            return Err(HandshakeError::Connection(e.to_string()));
        }

        // Ensure the message is sent
        if let Err(e) = send.finish() {
            error!("Failed to finish sending handshake message: {}", e);
            return Err(HandshakeError::Connection(e.to_string()));
        }
        info!("Initiator: Waiting for handshake response");

        // Read the response
        let message_str = match self.read_complete_message(recv).await {
            Ok(msg) => {
                debug!("Received handshake response: {} bytes", msg.len());
                msg
            }
            Err(e) => {
                error!("Failed to read handshake response: {}", e);
                return Err(e);
            }
        };

        // Parse the response
        let response: HandshakeMessage = match serde_json::from_str(&message_str) {
            Ok(msg) => {
                debug!("Successfully parsed handshake response");
                msg
            }
            Err(e) => {
                error!("Failed to parse handshake response: {}", e);
                return Err(HandshakeError::Deserialization(e));
            }
        };

        // TODO: Verify the response signature here

        info!("Initiator: Handshake completed successfully");
        Ok(response)
    }

    /// Accepts an incoming handshake by receiving a handshake message and responding
    ///
    /// This is called by the party that accepted the connection.
    #[instrument(skip(self, send, recv), level = "debug")]
    async fn accept_handshake(
        &self,
        send: &mut SendStream,
        recv: &mut RecvStream,
    ) -> Result<HandshakeMessage, HandshakeError> {
        info!("Receiver: Waiting for handshake message");

        // Use the read_complete_message helper to get the entire message
        let message_str = match self.read_complete_message(recv).await {
            Ok(msg) => {
                debug!("Received handshake message: {} bytes", msg.len());
                msg
            }
            Err(e) => {
                error!("Failed to read handshake message: {}", e);
                return Err(e);
            }
        };

        // Parse the JSON once we have the complete message
        let handshake_message: HandshakeMessage =
            match serde_json::from_str::<HandshakeMessage>(&message_str) {
                Ok(msg) => {
                    debug!("Successfully parsed handshake message");
                    debug!("Connection type: {:?}", msg.connection_type);
                    trace!("From user: {}", msg.user.id);
                    msg
                }
                Err(e) => {
                    error!("Failed to parse handshake JSON: {}", e);
                    error!(
                        "Message preview: {}",
                        if message_str.len() > 100 {
                            &message_str[..100]
                        } else {
                            &message_str
                        }
                    );
                    return Err(HandshakeError::Serialization(e.to_string()));
                }
            };

        // TODO: Verify the incoming handshake signature here

        info!("Successfully received and parsed handshake message");
        let device = self
            .get_current_device()
            .await
            .map_err(|_| HandshakeError::AuthService("failed handshake".to_string()))?;
        let user = self
            .get_current_user()
            .await
            .map_err(|_| HandshakeError::AuthService("failed handshake".to_string()))?;
        let (challenge, signature) = sign_random_challenge(&self.crypto_utils)
            .await
            .map_err(|_| HandshakeError::AuthService("failed handshake".to_string()))?;

        // Create our response message
        let response = HandshakeMessage {
            challenge,
            signature,
            device,
            user,
            connection_type: handshake_message.connection_type.clone(),
        };

        debug!("Created handshake response message");

        // Serialize and send our response
        let serialized = match serde_json::to_string(&response) {
            Ok(s) => {
                debug!("Serialized response: {} bytes", s.len());
                s
            }
            Err(e) => {
                error!("Failed to serialize response: {}", e);
                return Err(HandshakeError::Serialization(e.to_string()));
            }
        };

        // Write the response to the stream
        if let Err(e) = send.write_all(serialized.as_bytes()).await {
            error!("Failed to write response: {}", e);
            return Err(HandshakeError::Connection(e.to_string()));
        }

        // Ensure the response is sent
        if let Err(e) = send.finish() {
            error!("Failed to finish sending response: {}", e);
            return Err(HandshakeError::Connection(e.to_string()));
        }

        debug!("Sent handshake response successfully");
        info!("Receiver: Handshake completed successfully");

        Ok(handshake_message)
    }

    #[instrument(skip(self, ticket_str, conn_type, connection_id), fields( conn_type = ?conn_type, connection_id = ?connection_id), level = "info")]
    pub async fn connect_with_ticket(
        &self,
        ticket_str: &str,
        conn_type: ConnectionType,
        connection_id: Option<&str>,
        action: Option<ConnectionAction>,
    ) -> Result<Option<Arc<PeerConnection>>, P2PError> {
        info!("Starting connection process with ticket");
        trace!("Using ticket: {}", ticket_str);

        // Ensure P2P service is initialized
        self.ensure_initialized().await?;

        // Get the state for access to connections
        let state_guard = self.state.lock().await;
        let state = state_guard.as_ref().ok_or(P2PError::NotInitialized)?;
        debug!(
            "Connector using endpoint with node ID: {}",
            state.endpoint.node_id()
        );

        // If a connection ID was provided, check if the connection is already active
        if let Some(id) = connection_id {
            // Check if connection is already established or in connecting state
            if state.connections.is_connection_active(id).await {
                debug!("Connection is already active: {}", id);

                // Try to get an established connection
                if let Ok(existing_connection) = state.connections.get_peer_connection(id).await {
                    info!("Using existing established connection: {}", id);
                    return Ok(Some(existing_connection));
                } else {
                    // If we're here, the connection is in connecting state but not yet established
                    info!(
                        "Connection is currently being established (returning None): {}",
                        id
                    );
                    return Ok(None);
                }
            }

            // Mark the connection as connecting
            if let Err(e) = state.connections.mark_as_connecting(id).await {
                // This shouldn't happen given the previous check, but handle it just in case
                warn!("Failed to mark connection as connecting: {}", e);
                return Err(P2PError::Connection(format!(
                    "Failed to mark connection as connecting: {}",
                    e
                )));
            }
        }

        // Helper function to clean up connecting state on error
        let cleanup_connecting = |connection_id: Option<&str>, error: P2PError| -> P2PError {
            if let Some(id) = connection_id {
                // We need to spawn a task because we can't use .await in a closure
                let connection_id = id.to_string();
                let state_connections = state.connections.clone();
                tokio::spawn(async move {
                    state_connections
                        .remove_from_connecting(&connection_id)
                        .await;
                });
            }
            error
        };

        // Parse connection ticket
        let ticket: ConnectionTicket = match serde_json::from_str(ticket_str) {
            Ok(ticket) => {
                debug!("Ticket parsed successfully");
                ticket
            }
            Err(e) => {
                let error = P2PError::Deserialization(format!("Invalid ticket format: {}", e));
                return Err(cleanup_connecting(connection_id, error));
            }
        };

        // Parse node ID from ticket
        let node_id = match ticket.node_id.parse() {
            Ok(id) => id,
            Err(e) => {
                let error = P2PError::Connection(format!("Invalid node ID: {}", e));
                return Err(cleanup_connecting(connection_id, error));
            }
        };

        // Parse addresses from ticket
        let valid_addresses = ticket
            .addresses
            .iter()
            .filter_map(|a| match a.parse() {
                Ok(addr) => Some(addr),
                Err(e) => {
                    warn!("Skipping invalid address {}: {}", a, e);
                    None
                }
            })
            .collect::<Vec<_>>();

        if valid_addresses.is_empty() {
            let error = P2PError::Connection("No valid addresses in ticket".into());
            return Err(cleanup_connecting(connection_id, error));
        }

        debug!("Valid addresses: {}", valid_addresses.len());

        // Create node address from parsed components
        let node_addr = NodeAddr::from_parts(node_id, None, valid_addresses);

        debug!("Created NodeAddr: {:?}", node_addr);
        debug!("Our endpoint ID: {}", state.endpoint.node_id());
        debug!("ALPN Protocol: {}", String::from_utf8_lossy(ALPN_PROTOCOL));

        // Attempt to establish connection
        info!("Connecting to remote endpoint...");
        let connection_span = info_span!("endpoint_connect", 
        remote_node_id = %node_addr.node_id,
        addresses = ?node_addr.direct_addresses.len());
        debug!("Attempting connection to node_id: {}", node_id);
        debug!("NodeAddr created: {:?}", node_addr);
        debug!("Using ALPN: {}", String::from_utf8_lossy(ALPN_PROTOCOL));

        let connect_result = state
            .endpoint
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
                return Err(cleanup_connecting(connection_id, error));
            }
        };

        // Proceed with handshake
        info!("Starting handshake process");
        let handshake_span =
            info_span!("handshake", initiator = true, connection_type = ?conn_type);

        // Drop the state guard before handshake to avoid deadlocks
        drop(state_guard);

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
                if let Some(id) = connection_id {
                    let state_guard = self.state.lock().await;
                    if let Some(state) = &*state_guard {
                        state.connections.remove_from_connecting(id).await;
                    }
                }
                Err(e)
            }
        }
    }
}
