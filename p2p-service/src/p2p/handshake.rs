use crate::p2p::constants::*;
use crate::p2p::service::P2PService;
use crate::p2p::P2PEvent;
use iroh::endpoint::{Connection, RecvStream, SendStream};
use iroh::NodeAddr;
use log::{error, info};
use osvauld_core::models::p2p::{
    ConnectionTicket, ConnectionType, HandshakeError, HandshakeMessage,
};
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::time::timeout;

impl P2PService {
    pub async fn connect_with_ticket(
        &self,
        ticket_str: &str,
        conn_type: ConnectionType,
    ) -> Result<(), String> {
        self.ensure_initialized().await?;

        println!("Starting connection process with ticket: {}", ticket_str);

        let (endpoint, node_addr) = {
            let state_guard = self.state.lock().await;
            let state = state_guard.as_ref().ok_or("P2P not initialized")?;
            if let Some(state) = state_guard.as_ref() {
                let mut conn_type_guard = state.connection_type.lock().await;
                *conn_type_guard = Some(conn_type);
            }

            let ticket: ConnectionTicket = serde_json::from_str(ticket_str)
                .map_err(|e| format!("Invalid ticket format: {}", e))?;

            info!("Addresses from ticket: {:?}", ticket.addresses);

            let node_addr = NodeAddr::from_parts(
                ticket
                    .node_id
                    .parse()
                    .map_err(|e| format!("Invalid node ID: {}", e))?,
                None,
                ticket
                    .addresses
                    .iter()
                    .filter_map(|a| {
                        let addr = a.parse();
                        addr.ok()
                    })
                    .collect::<Vec<_>>(),
            );

            println!("Created NodeAddr: {:?}", node_addr);
            println!("Our endpoint ID: {}", state.endpoint.node_id());
            println!(
                "ALPN Protocol being used: {}",
                String::from_utf8_lossy(ALPN_PROTOCOL)
            );

            (state.endpoint.clone(), node_addr)
        };

        info!("Starting endpoint.connect() call...");
        let connect_result = endpoint.connect(node_addr.clone(), ALPN_PROTOCOL).await;

        match &connect_result {
            Ok(_conn) => {
                info!("Connection successful!");
                self.event_emitter.emit(P2PEvent::Connected)
            }
            Err(e) => {
                println!("Connection failed. Error details:");
                println!("Error: {}", e);
                error!("Node addr used: {:?}", node_addr);
                // Try to get any additional endpoint state that might be helpful
                info!("Endpoint bound sockets: {:?}", endpoint.bound_sockets());
                if let Ok(cur_addr) = endpoint.node_addr().await {
                    info!("Current endpoint addr: {:?}", cur_addr);
                }
            }
        }

        let conn = connect_result.map_err(|e| format!("Connection failed: {e}"))?;

        info!("Starting handshake process...");
        let handshake_result = self.perform_handshake(&conn, true).await;

        match &handshake_result {
            Ok(_) => info!("Handshake completed successfully"),
            Err(e) => error!("Handshake failed: {}", e),
        }

        handshake_result?;

        Ok(())
    }

    pub async fn perform_handshake(
        &self,
        conn: &Connection,
        is_initiator: bool,
    ) -> Result<(), String> {
        let (mut send, mut recv) = match timeout(CONNECTION_TIMEOUT, async {
            {
                let mut initiator = self.is_initiator.lock().await;
                *initiator = Some(is_initiator);
            }
            if is_initiator {
                info!("Initiator: Opening bi-directional stream");
                conn.open_bi().await
            } else {
                info!("Receiver: Accepting bi-directional stream");
                conn.accept_bi().await
            }
        })
        .await
        .map_err(|e| format!("Stream timeout: {}", e))?
        {
            Ok(stream) => stream,
            Err(e) => return Err(format!("Stream establishment failed: {}", e)),
        };

        if is_initiator {
            self.initiate_handshake(&mut send, &mut recv)
                .await
                .map_err(|e| e.to_string())?;
        } else {
            self.accept_handshake(&mut send, &mut recv)
                .await
                .map_err(|e| e.to_string())?;
        }

        {
            let state_guard = self.state.lock().await;
            let state = state_guard.as_ref().unwrap();
            let mut active_conn = state.active_connection.lock().await;
            *active_conn = Some(Arc::new(conn.clone()));
        }

        let self_clone = self.clone();
        tokio::spawn(async move {
            info!(
                "Starting message listener for {}",
                if is_initiator {
                    "initiator"
                } else {
                    "receiver"
                }
            );
            self_clone.handle_messages().await;
        });

        Ok(())
    }

    async fn read_complete_message(&self, recv: &mut RecvStream) -> Result<String, HandshakeError> {
        info!("Reading complete message (potentially fragmented)");
        let mut buffer = Vec::with_capacity(MAX_HANDSHAKE_SIZE);
        let mut temp_buf = [0u8; 8192];
        let mut total_read = 0;
        let mut read_attempts = 0;

        // Continue reading until we get a complete JSON object or error out
        loop {
            read_attempts += 1;

            match timeout(HANDSHAKE_TIMEOUT, recv.read(&mut temp_buf)).await {
                Ok(Ok(Some(n))) => {
                    if n == 0 {
                        // End of stream
                        info!("End of stream reached after reading {} bytes", total_read);
                        break;
                    }

                    total_read += n;
                    info!(
                        "Read chunk {}: {} bytes (total: {} bytes)",
                        read_attempts, n, total_read
                    );

                    // Add the new chunk to our buffer
                    buffer.extend_from_slice(&temp_buf[..n]);

                    // Check if we've exceeded max size
                    if buffer.len() > MAX_HANDSHAKE_SIZE {
                        error!(
                            "Message too large: {} bytes (max: {})",
                            buffer.len(),
                            MAX_HANDSHAKE_SIZE
                        );
                        return Err(HandshakeError::Connection(format!(
                            "Message too large: {} bytes",
                            buffer.len()
                        )));
                    }

                    // Check if we now have a valid UTF-8 string that can be parsed as valid JSON
                    if let Ok(message_str) = String::from_utf8(buffer.clone()) {
                        match serde_json::from_str::<HandshakeMessage>(&message_str) {
                            Ok(_) => {
                                // Success! We have a complete JSON message
                                info!(
                                    "Successfully parsed complete JSON message ({} bytes)",
                                    message_str.len()
                                );
                                return Ok(message_str);
                            }
                            Err(e) if e.is_eof() || e.is_data() => {
                                // JSON is incomplete or invalid, continue reading
                                info!("JSON parsing incomplete, continuing to read: {}", e);
                            }
                            Err(e) => {
                                // Other JSON error, log but continue trying
                                info!("JSON parsing error (continuing): {}", e);
                            }
                        }
                    }
                }
                Ok(Ok(None)) => {
                    // End of stream reached
                    info!("End of stream reached after reading {} bytes", total_read);
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
            return Err(HandshakeError::Connection("Empty message received".into()));
        }

        // Try to convert to a string for better error reporting
        match String::from_utf8(buffer) {
            Ok(s) => {
                error!(
                    "Incomplete JSON after reading {} bytes. Preview: {}...",
                    total_read,
                    if s.len() > 100 { &s[..100] } else { &s }
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

    async fn initiate_handshake(
        &self,
        send: &mut SendStream,
        recv: &mut RecvStream,
    ) -> Result<(), HandshakeError> {
        info!("Initiator: Sending hello");
        let device = self
            .auth_service
            .get_current_device()
            .await
            .map_err(|e| HandshakeError::AuthService(e.to_string()))?;

        let (challenge, signature) = self
            .auth_service
            .sign_random_challenge()
            .await
            .map_err(|e| HandshakeError::AuthService(e.to_string()))?;

        let connection_type = {
            let state_guard = self.state.lock().await;
            let state = state_guard
                .as_ref()
                .ok_or(HandshakeError::Connection("P2P not initialized".into()))?;
            let conn_type_guard = state.connection_type.lock().await;
            conn_type_guard.clone().unwrap_or(ConnectionType::Device) // Default to Device if not set
        };
        let user = match connection_type {
            ConnectionType::User => match self.user_service.get_current_user().await {
                Ok(user) => Some(user),
                Err(_) => None,
            },
            ConnectionType::Device => None,
        };
        let handshake_message = HandshakeMessage {
            connection_type,
            challenge,
            signature,
            device,
            user,
        };

        let serialized = serde_json::to_string(&handshake_message)
            .map_err(|e| HandshakeError::Serialization(e.to_string()))?;
        info!(
            "DIAGNOSTIC: Handshake message size is {} bytes",
            serialized.len()
        );
        send.write_all(serialized.as_bytes())
            .await
            .map_err(|e| HandshakeError::Connection(e.to_string()))?;

        send.flush()
            .await
            .map_err(|e| HandshakeError::Connection(e.to_string()))?;

        info!("Initiator: Waiting for handshake response");
        let message_str = self.read_complete_message(recv).await?;

        let response: HandshakeMessage = serde_json::from_str(&message_str)?;

        match response.connection_type {
            ConnectionType::Device => {
                let mut device = self.device.lock().await;
                *device = Some(response.device.clone());
            }
            ConnectionType::User => {
                // Store device info if provided
                let mut device = self.device.lock().await;
                *device = Some(response.device.clone());

                // User info is required for user connections
                if let Some(user_info) = response.user {
                    let mut user = self.user.lock().await;
                    *user = Some(user_info);
                } else {
                    return Err(HandshakeError::Connection("Missing user info".into()));
                }
            }
        }

        info!("Initiator: Handshake completed successfully");
        Ok(())
    }

    async fn accept_handshake(
        &self,
        send: &mut SendStream,
        recv: &mut RecvStream,
    ) -> Result<(), HandshakeError> {
        info!("Receiver: Waiting for handshake message");

        // Use the read_complete_message helper to get the entire message
        let message_str = self.read_complete_message(recv).await?;

        // Parse the JSON once we have the complete message
        let handshake_message: HandshakeMessage = match serde_json::from_str(&message_str) {
            Ok(msg) => {
                info!("Successfully parsed handshake message");
                msg
            }
            Err(e) => {
                error!(
                    "Failed to parse handshake JSON: {}. Message was: {}",
                    e, &message_str
                );
                return Err(HandshakeError::Serialization(e.to_string()));
            }
        };

        // Store connection type in state
        {
            let state_guard = self.state.lock().await;
            if let Some(state) = state_guard.as_ref() {
                let mut conn_type_guard = state.connection_type.lock().await;
                *conn_type_guard = Some(handshake_message.connection_type.clone());
            }
        }

        info!("Successfully received and parsed handshake message");

        match handshake_message.connection_type {
            ConnectionType::Device => {
                let mut device = self.device.lock().await;
                *device = Some(handshake_message.device);
            }
            ConnectionType::User => {
                // Store device info if provided
                let mut device = self.device.lock().await;
                *device = Some(handshake_message.device);

                // User info is required for user connections
                if let Some(user_info) = handshake_message.user {
                    let mut user = self.user.lock().await;
                    *user = Some(user_info);
                } else {
                    return Err(HandshakeError::Connection("Missing user info".into()));
                }
            }
        }

        // Get our device and create response
        let device = match self.auth_service.get_current_device().await {
            Ok(d) => d,
            Err(e) => {
                error!("Failed to get current device: {}", e);
                return Err(HandshakeError::AuthService(e.to_string()));
            }
        };

        let user = match handshake_message.connection_type {
            ConnectionType::User => match self.user_service.get_current_user().await {
                Ok(user) => Some(user),
                Err(_) => None,
            },
            ConnectionType::Device => None,
        };

        let (challenge, signature) = match self.auth_service.sign_random_challenge().await {
            Ok(cs) => cs,
            Err(e) => {
                error!("Failed to sign challenge: {}", e);
                return Err(HandshakeError::AuthService(e.to_string()));
            }
        };

        let response = HandshakeMessage {
            challenge,
            signature,
            device,
            user,
            connection_type: handshake_message.connection_type,
        };

        info!("Created handshake response message");

        // Serialize and send our response
        let serialized = match serde_json::to_string(&response) {
            Ok(s) => s,
            Err(e) => {
                error!("Failed to serialize response: {}", e);
                return Err(HandshakeError::Serialization(e.to_string()));
            }
        };

        if let Err(e) = send.write_all(serialized.as_bytes()).await {
            error!("Failed to write response: {}", e);
            return Err(HandshakeError::Connection(e.to_string()));
        }

        if let Err(e) = send.flush().await {
            error!("Failed to flush response: {}", e);
            return Err(HandshakeError::Connection(e.to_string()));
        }

        info!("Receiver: Handshake completed successfully");
        Ok(())
    }
}
