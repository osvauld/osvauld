use crate::p2p::emitter::{P2PEvent, P2PEventEmitter};
use iroh::endpoint::{Connection, RecvStream, SendStream};
use log::{error, info};
use osvauld_core::models::device::Device;
use osvauld_core::models::p2p::{ConnectionType, Message};
use osvauld_core::models::user::User;
use osvauld_services::{AuthService, ShareService, SyncService, UserService};
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;
use tokio::time::timeout;

/// Context struct containing all service dependencies
pub struct ServiceContext {
    pub auth_service: Arc<AuthService>,
    pub user_service: Arc<UserService>,
    pub sync_service: Arc<SyncService>,
    pub share_service: Arc<ShareService>,
}

/// Represents a peer-to-peer connection with another device or user
pub struct PeerConnection {
    /// The underlying connection
    pub connection: Arc<Connection>,

    /// Type of connection (Device or User)
    pub connection_type: ConnectionType,

    /// Device information of the peer
    pub device: Device,

    /// User information of the peer
    pub user: User,

    /// Whether this peer initiated the connection
    pub is_initiator: bool,

    /// Handle to the message handling task
    pub task_handle: tokio::task::JoinHandle<()>,

    /// Service context containing service dependencies
    pub context: Arc<ServiceContext>,

    /// Event emitter for broadcasting events
    pub event_emitter: P2PEventEmitter,
}

impl PeerConnection {
    /// Creates a new PeerConnection
    pub fn new(
        connection: Arc<Connection>,
        connection_type: ConnectionType,
        device: Device,
        user: User,
        is_initiator: bool,
        context: Arc<ServiceContext>,
        event_emitter: P2PEventEmitter,
    ) -> Self {
        // Create a placeholder task handle that will be replaced
        let task_handle = tokio::spawn(async {});

        // Create the PeerConnection instance
        let mut peer_connection = Self {
            connection,
            connection_type,
            device,
            user,
            is_initiator,
            task_handle,
            context,
            event_emitter,
        };

        // Start the message handler and store its task handle
        peer_connection.task_handle = peer_connection.start_message_handler();

        peer_connection
    }

    /// Gets the unique identifier for this connection (user_id:device_id)
    pub fn get_id(&self) -> String {
        format!("{}:{}", self.user.id, self.device.id)
    }

    /// Starts the message handler task
    fn start_message_handler(&self) -> tokio::task::JoinHandle<()> {
        let connection = self.connection.clone();
        let self_clone = self.clone();

        tokio::spawn(async move {
            info!(
                "Starting message listener for connection {}",
                self_clone.get_id()
            );
            self_clone.handle_messages().await;
            info!(
                "Message listener stopped for connection {}",
                self_clone.get_id()
            );
        })
    }

    /// Handles incoming messages from the peer
    async fn handle_messages(&self) {
        info!("Message handler started");

        loop {
            match self.connection.accept_bi().await {
                Ok((_send, mut recv)) => {
                    // Use a dynamic buffer that can grow as needed
                    let mut buffer = Vec::new();
                    let mut temp_buffer = vec![0u8; 8192]; // Larger temp buffer for reading chunks

                    // Read the entire message
                    loop {
                        match recv.read(&mut temp_buffer).await {
                            Ok(Some(n)) if n > 0 => {
                                buffer.extend_from_slice(&temp_buffer[..n]);

                                // Try to parse what we have so far
                                if let Ok(message_str) = String::from_utf8(buffer.clone()) {
                                    match serde_json::from_str::<Message>(&message_str) {
                                        Ok(message) => {
                                            info!("Successfully deserialized message");

                                            // Process the message
                                            if let Err(e) = self.process_message(&message).await {
                                                error!("Error processing message: {}", e);

                                                // Emit error event
                                                self.event_emitter.emit(P2PEvent::Error {
                                                    message: format!(
                                                        "Error processing message: {}",
                                                        e
                                                    ),
                                                    source: "message_handler".to_string(),
                                                });
                                            }
                                            break;
                                        }
                                        Err(e) if e.is_eof() => {
                                            // Need more data, continue reading
                                            continue;
                                        }
                                        Err(e) => {
                                            error!("Failed to deserialize message: {}", e);
                                            break;
                                        }
                                    }
                                }
                            }
                            Ok(Some(_)) => continue, // Got some data, but need more
                            Ok(None) => {
                                info!("Connection closed by peer");

                                // Emit disconnection event
                                self.event_emitter.emit(P2PEvent::Disconnected);

                                return;
                            }
                            Err(e) => {
                                error!("Error reading from connection: {}", e);

                                // Emit error event
                                self.event_emitter.emit(P2PEvent::Error {
                                    message: format!("Error reading from connection: {}", e),
                                    source: "message_handler".to_string(),
                                });

                                return;
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to accept bi-directional stream: {}", e);

                    // Emit error event
                    self.event_emitter.emit(P2PEvent::Error {
                        message: format!("Failed to accept bi-directional stream: {}", e),
                        source: "message_handler".to_string(),
                    });

                    break;
                }
            }
        }
    }

    /// Process a received message by delegating to the appropriate handler
    async fn process_message(&self, message: &Message) -> Result<(), String> {
        match message {
            Message::SyncRequest => {
                info!("Received SyncRequest");
                self.handle_sync_request().await
            }
            Message::SyncAck(updated_data) => {
                info!("Received SyncAck");
                self.handle_sync_ack(updated_data.clone()).await
            }
            Message::AddDevice(records) => {
                info!("Received AddDevice");
                self.handle_add_device_request(records.clone()).await
            }
            Message::AddDeviceAck => {
                info!("Received AddDeviceAck");
                self.start_device_sync().await
            }
            Message::SyncResponse(payload) => {
                info!("Received SyncResponse");
                self.handle_sync_response(payload.clone()).await
            }
            Message::SyncComplete => {
                info!("Received SyncComplete");
                self.handle_sync_complete().await
            }
            Message::Chat(content) => {
                info!("Received chat message: {}", content);
                // No response needed for chat messages
                Ok(())
            }
            Message::Ping => {
                info!("Received ping");
                Ok(())
            }
            Message::Pong => {
                info!("Received pong");
                Ok(())
            }
            Message::AckComplete(device_sync_record_id) => {
                info!("Received AckComplete");
                self.ack_complete(device_sync_record_id.clone()).await
            }
            Message::SyncEvent { event, payload } => {
                info!("Received SyncEvent");
                self.handle_sync_event(event, payload.clone()).await
            }
            Message::FirstUserConnection(user) => {
                info!("Received FirstUserConnection");
                self.handle_first_user_connection(user).await
            }
            Message::UserAddAck(user_id) => {
                info!("Received UserAddAck for user: {}", user_id);
                self.handle_user_add_ack(user_id).await
            }
            Message::SharePayload(payload) => {
                info!("Received SharePayload");
                self.handle_share_payload(payload).await
            }
            Message::ShareComplete => {
                info!("Received ShareComplete");
                self.event_emitter.emit(P2PEvent::ShareComplete);
                Ok(())
            }
            Message::Error => {
                error!("Received error message from peer");
                self.event_emitter.emit(P2PEvent::Error {
                    message: "Received error message from peer".to_string(),
                    source: "peer".to_string(),
                });
                Ok(())
            }
        }
    }

    /// Sends a message to the peer
    pub async fn send_message(&self, message: Message) -> Result<(), String> {
        let serialized_message = serde_json::to_string(&message)
            .map_err(|e| format!("Failed to serialize message: {}", e))?;

        let (mut send, _) = self
            .connection
            .open_bi()
            .await
            .map_err(|e| format!("Failed to open bi-directional stream: {}", e))?;

        // Write the message in chunks to handle large payloads
        const CHUNK_SIZE: usize = 8192;
        let bytes = serialized_message.as_bytes();

        for chunk in bytes.chunks(CHUNK_SIZE) {
            send.write_all(chunk)
                .await
                .map_err(|e| format!("Failed to write chunk: {}", e))?;
        }

        send.finish()
            .map_err(|e| format!("Failed to finish sending: {}", e))?;

        Ok(())
    }

    /// Sends a chat message to the peer
    pub async fn send_chat_message(&self, content: String) -> Result<(), String> {
        self.send_message(Message::Chat(content)).await
    }

    /// Enable cloning for PeerConnection
    pub fn clone(&self) -> Self {
        Self {
            connection: self.connection.clone(),
            connection_type: self.connection_type.clone(),
            device: self.device.clone(),
            user: self.user.clone(),
            is_initiator: self.is_initiator,
            task_handle: tokio::spawn(async {}), // Create a dummy task handle
            context: self.context.clone(),
            event_emitter: self.event_emitter.clone(),
        }
    }
}
