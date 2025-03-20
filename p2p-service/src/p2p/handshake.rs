use crate::p2p::constants::*;
use crate::p2p::errors::{ P2PError, HandshakeError};
use crate::p2p::p2p_service::P2PService;
use crate::p2p::peer_connection::PeerConnection;
use crate::p2p::P2PEvent;
use iroh::endpoint::{Connection, RecvStream, SendStream};
use osvauld_core::models::p2p::{ConnectionType, HandshakeMessage};
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
    #[instrument(skip(self, conn, is_initiator, connection_type), 
        fields(initiator = is_initiator, connection_type = ?connection_type),
        level = "info")]
    pub async fn perform_handshake_and_create_peer(
        &self,
        conn: &Connection,
        is_initiator: bool,
        connection_type: Option<ConnectionType>,
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
                return Err(P2PError::Connection(format!("Stream establishment failed: {}", e)));
            }
            Err(e) => {
                error!("Timeout while establishing bi-directional stream: {}", e);
                return Err(P2PError::Timeout(format!("Stream timeout: {}", e)));
            }
        };

        let handshake_message = if is_initiator {
            let conn_type = connection_type.unwrap_or(ConnectionType::Device);
            debug!("Initiating handshake as {:?}", conn_type);
            
            match self.initiate_handshake(&mut send, &mut recv, conn_type).await {
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
        let resources_needing_update = self.sync_service.get_resources_needing_sync(&handshake_message.device.id).await.map_err(|e| P2PError::SyncService(e.to_string()))?;

        let peer_connection = PeerConnection::new(
            connection_arc,
            handshake_message.connection_type.clone(),
            handshake_message.device.clone(),
            handshake_message.user.clone(),
            is_initiator,
            state.service_context.clone(),
            self.event_emitter.clone(),
            resources_needing_update,
        );

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
        self.event_emitter.emit(P2PEvent::HandshakeCompleted);

        info!("Handshake and peer creation successful: {}", peer_connection_arc.get_id());
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
                        let err = format!("Message too large: {} bytes (max: {})", buffer.len(), MAX_HANDSHAKE_SIZE);
                        error!("{}", err);
                        return Err(HandshakeError::Connection(err));
                    }

                    // Check if we now have a valid UTF-8 string that can be parsed as valid JSON
                    if let Ok(message_str) = String::from_utf8(buffer.clone()) {
                        match serde_json::from_str::<HandshakeMessage>(&message_str) {
                            Ok(_) => {
                                // Success! We have a complete JSON message
                                debug!("Successfully parsed complete JSON message ({} bytes)", message_str.len());
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
                error!("Incomplete JSON after reading {} bytes. Preview: {}...", total_read, preview);
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
    #[instrument(skip(self, send, recv), fields(connection_type = ?connection_type), level = "debug")]
    async fn initiate_handshake(
        &self,
        send: &mut SendStream,
        recv: &mut RecvStream,
        connection_type: ConnectionType,
    ) -> Result<HandshakeMessage, HandshakeError> {
        info!("Initiator: Sending handshake message");
        
        // Get our device information
        let device = match self.auth_service.get_current_device().await {
            Ok(device) => {
                debug!("Got current device: {}", device.id);
                device
            },
            Err(e) => {
                error!("Failed to get current device: {}", e);
                return Err(HandshakeError::AuthService(e.to_string()));
            }
        };

        // Sign a challenge to prove our identity
        let (challenge, signature) = match self.auth_service.sign_random_challenge().await {
            Ok(cs) => {
                debug!("Created and signed challenge");
                cs
            },
            Err(e) => {
                error!("Failed to sign challenge: {}", e);
                return Err(HandshakeError::AuthService(e.to_string()));
            }
        };

        // Get our user information
        let user = match self.user_service.get_current_user().await {
            Ok(user) => {
                debug!("Got current user: {}", user.id);
                user
            },
            Err(e) => {
                error!("Failed to get current user: {}", e);
                return Err(HandshakeError::AuthService(e.to_string()));
            }
        };
        
        // Construct the handshake message
        let handshake_message = HandshakeMessage {
            connection_type,
            challenge,
            signature,
            device,
            user,
        };

        // Serialize and send the handshake message
        let serialized = match serde_json::to_string(&handshake_message) {
            Ok(json) => {
                debug!("Serialized handshake message: {} bytes", json.len());
                trace!("DIAGNOSTIC: Handshake message size is {} bytes", json.len());
                json
            },
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
        if let Err(e) = send.flush().await {
            error!("Failed to flush handshake message: {}", e);
            return Err(HandshakeError::Connection(e.to_string()));
        }

        info!("Initiator: Waiting for handshake response");
        
        // Read the response
        let message_str = match self.read_complete_message(recv).await {
            Ok(msg) => {
                debug!("Received handshake response: {} bytes", msg.len());
                msg
            },
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
            },
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
            },
            Err(e) => {
                error!("Failed to read handshake message: {}", e);
                return Err(e);
            }
        };

        // Parse the JSON once we have the complete message
        let handshake_message: HandshakeMessage = match serde_json::from_str::<HandshakeMessage>(&message_str) {
            Ok(msg) => {
                debug!("Successfully parsed handshake message");
                debug!("Connection type: {:?}", msg.connection_type);
                trace!("From user: {}", msg.user.id);
                msg
            }
            Err(e) => {
                error!("Failed to parse handshake JSON: {}", e);
                error!("Message preview: {}", if message_str.len() > 100 { &message_str[..100] } else { &message_str });
                return Err(HandshakeError::Serialization(e.to_string()));
            }
        };

        // TODO: Verify the incoming handshake signature here

        info!("Successfully received and parsed handshake message");

        // Get our device and create response
        let device = match self.auth_service.get_current_device().await {
            Ok(d) => {
                debug!("Got current device: {}", d.id);
                d
            },
            Err(e) => {
                error!("Failed to get current device: {}", e);
                return Err(HandshakeError::AuthService(e.to_string()));
            }
        };

        let user = match self.user_service.get_current_user().await {
            Ok(u) => {
                debug!("Got current user: {}", u.id);
                u
            },
            Err(e) => {
                error!("Failed to get current user: {}", e);
                return Err(HandshakeError::AuthService(e.to_string()));
            }
        };

        let (challenge, signature) = match self.auth_service.sign_random_challenge().await {
            Ok(cs) => {
                debug!("Created and signed challenge");
                cs
            },
            Err(e) => {
                error!("Failed to sign challenge: {}", e);
                return Err(HandshakeError::AuthService(e.to_string()));
            }
        };

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
            },
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
        if let Err(e) = send.flush().await {
            error!("Failed to flush response: {}", e);
            return Err(HandshakeError::Connection(e.to_string()));
        }

        debug!("Sent handshake response successfully");
        info!("Receiver: Handshake completed successfully");
        
        Ok(handshake_message)
    }
}
