use crate::p2p::emitter::{P2PEvent, P2PEventEmitter};
use crypto_utils::CryptoUtils;
use iroh::endpoint::Connection;
use iroh_quinn::VarInt;
use osvauld_core::models::{
    ConnectionAction, ConnectionType, Device, DeviceManifestComparisonResult, HandshakeInit,
    Message, User, UserManifestComparisonResult,
};
use persistance::database::RepositoryContext;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, error, info, info_span, instrument, trace, warn, Instrument};

/// Context struct containing all service dependencies
pub struct ServiceContext {
    pub current_user: Arc<RwLock<Option<User>>>,
    pub current_device: Arc<RwLock<Option<Device>>>,
}

/// Represents a peer-to-peer connection with another device or user
pub struct PeerConnection {
    /// The underlying connection
    pub connection: Arc<Connection>,

    /// Type of connection (Device or User) - filled during handshake
    pub connection_type: Option<ConnectionType>,

    /// Device information of the peer - filled during handshake
    pub device: Device,

    /// Node ID derived from device key - filled during handshake  
    pub node_id: String,

    /// User information of the peer - filled during handshake
    pub user: User,

    /// Connection action to execute after handshake
    pub action: Option<ConnectionAction>,

    /// Whether this peer initiated the connection
    pub is_initiator: bool,
    pub is_live_edit: bool,

    /// Whether handshake is complete
    pub handshake_complete: Arc<Mutex<bool>>,

    /// Handle to the message handling task
    pub task_handle: tokio::task::JoinHandle<()>,

    /// Service context containing service dependencies
    pub context: Arc<ServiceContext>,

    /// Event emitter for broadcasting events
    pub event_emitter: P2PEventEmitter,

    pub on_close: Arc<Mutex<Option<Box<dyn Fn(String) + Send + Sync>>>>,
    pub disconnection_timer: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    pub crypto_utils: Arc<Mutex<CryptoUtils>>,
    pub repo_ctx: RepositoryContext,
    pub device_manifest_result: Arc<Mutex<Option<DeviceManifestComparisonResult>>>,
    pub user_manifest_result: Arc<Mutex<Option<UserManifestComparisonResult>>>,
    pub challenge: String,
}

impl PeerConnection {
    /// Creates a new PeerConnection
    #[instrument(skip_all, level = "info")]
    pub fn new(
        connection: Arc<Connection>,
        is_initiator: bool,
        context: Arc<ServiceContext>,
        event_emitter: P2PEventEmitter,
        on_close: Option<Box<dyn Fn(String) + Send + Sync>>,
        crypto_utils: Arc<Mutex<CryptoUtils>>,
        repo_ctx: RepositoryContext,
        action: Option<ConnectionAction>,
        connection_type: Option<ConnectionType>,
        node_id: String,
        local_user: User,
        local_device: Device,
        challenge: String,
        live_edit: bool,
    ) -> Self {
        info!("Creating new peer connection");

        // Create a placeholder task handle that will be replaced
        let task_handle = tokio::spawn(async {});

        // Create the PeerConnection instance with all optional fields
        let mut peer_connection = Self {
            connection,
            connection_type,
            device: local_device,
            node_id,
            user: local_user,
            action,
            is_initiator,
            handshake_complete: Arc::new(Mutex::new(false)),
            is_live_edit: live_edit,
            task_handle,
            context,
            event_emitter,
            on_close: Arc::new(Mutex::new(on_close)),
            disconnection_timer: Arc::new(Mutex::new(None)),
            repo_ctx,
            crypto_utils,
            device_manifest_result: Arc::new(Mutex::new(None)),
            user_manifest_result: Arc::new(Mutex::new(None)),
            challenge,
        };

        debug!("Starting message handler for the connection");
        peer_connection.task_handle = peer_connection.start_message_handler();

        info!("Peer connection created successfully");
        peer_connection
    }

    pub fn get_id(&self) -> String {
        self.node_id.clone()
    }
    pub async fn set_device_manifest_comparison_result(
        &self,
        result: DeviceManifestComparisonResult,
    ) {
        let mut manifest_guard = self.device_manifest_result.lock().await;
        *manifest_guard = Some(result);
    }
    pub async fn set_user_manifest_comparison_result(&self, result: UserManifestComparisonResult) {
        let mut manifest_guard = self.user_manifest_result.lock().await;
        *manifest_guard = Some(result);
    }
    #[instrument(skip(self), fields(connection_id = %self.get_id()), level = "debug")]
    pub async fn get_device_manifest_result(
        &self,
    ) -> Result<DeviceManifestComparisonResult, String> {
        let manifest_guard = self.device_manifest_result.lock().await;
        match manifest_guard.clone() {
            Some(manifest) => Ok(manifest),
            None => Err("Device manifest is empty".to_string()),
        }
    }

    #[instrument(skip(self), fields(connection_id = %self.get_id()), level = "debug")]
    pub async fn get_user_manifest_result(&self) -> Result<UserManifestComparisonResult, String> {
        let manifest_guard = self.user_manifest_result.lock().await;
        match manifest_guard.clone() {
            Some(manifest) => Ok(manifest),
            None => Err("user manifest is empty".to_string()),
        }
    }

    /// Removes a resource from local_missing.unknown_resources in device manifest
    #[instrument(skip(self), fields(connection_id = %self.get_id(), resource_id = %resource_id), level = "debug")]
    pub async fn remove_device_local_missing_resource(&self, resource_id: &str) -> bool {
        let mut manifest_result = self.device_manifest_result.lock().await;
        if let Some(ref mut manifest_comparison) = *manifest_result {
            let initial_count = manifest_comparison.local_missing.unknown_resources.len();

            manifest_comparison
                .local_missing
                .unknown_resources
                .retain(|id| id.to_string() != resource_id);

            let remaining_count = manifest_comparison.local_missing.unknown_resources.len();

            debug!(
                initial_missing = initial_count,
                remaining_missing = remaining_count,
                resource_id = %resource_id,
                "Updated local missing resources list"
            );

            remaining_count == 0
        } else {
            true // Consider empty if no manifest exists
        }
    }

    #[instrument(skip(self), fields(connection_id = %self.get_id(), resource_id = %resource_id), level = "debug")]
    pub async fn remove_user_local_missing_resource(&self, resource_id: &str) -> bool {
        let mut manifest_result = self.user_manifest_result.lock().await;
        if let Some(ref mut manifest_comparison) = *manifest_result {
            let initial_count = manifest_comparison.local_missing.unknown_resources.len();

            manifest_comparison
                .local_missing
                .unknown_resources
                .retain(|id| id.to_string() != resource_id);

            let remaining_count = manifest_comparison.local_missing.unknown_resources.len();

            debug!(
                initial_missing = initial_count,
                remaining_missing = remaining_count,
                resource_id = %resource_id,
                "Updated local missing resources list"
            );

            remaining_count == 0
        } else {
            true // Consider empty if no manifest exists
        }
    }

    // Add method to close the connection
    #[instrument(skip(self), level = "info")]
    pub async fn close_connection(&self) -> Result<(), String> {
        info!("Closing connection: {}", self.get_id());

        // Close the Iroh connection
        self.connection
            .close(VarInt::from_u32(0), b"Connection closed normally");

        // Call the closure callback if set
        let connection_id = self.get_id();
        let on_close_guard = self.on_close.lock().await;
        if let Some(callback) = &*on_close_guard {
            info!(
                "Executing connection closure callback for: {}",
                connection_id
            );
            (callback)(connection_id);
        }

        info!("Connection closed successfully: {}", self.get_id());
        Ok(())
    }

    /// Starts the message handler task
    fn start_message_handler(&self) -> tokio::task::JoinHandle<()> {
        let _connection = self.connection.clone();
        let mut self_clone = self.clone();
        let conn_id = self.get_id();

        tokio::spawn(async move {
            info!("Starting message listener for connection {}", conn_id);
            self_clone.handle_messages().await;
            info!("Message listener stopped for connection {}", conn_id);
        })
    }

    /// Handles incoming messages from the peer
    #[instrument(skip_all, level = "debug")]
    async fn handle_messages(&mut self) {
        let conn_id = self.get_id();
        info!("Message handler started for connection {}", conn_id);

        loop {
            match self.connection.accept_bi().await {
                Ok((_send, mut recv)) => {
                    debug!("Accepted new bi-directional stream");

                    // Use a dynamic buffer that can grow as needed
                    let mut buffer = Vec::new();
                    let mut temp_buffer = vec![0u8; 8192]; // Larger temp buffer for reading chunks

                    // Read the entire message
                    loop {
                        match recv.read(&mut temp_buffer).await {
                            Ok(Some(n)) if n > 0 => {
                                trace!("Read {} bytes from stream", n);
                                buffer.extend_from_slice(&temp_buffer[..n]);

                                // Try to parse what we have so far
                                if let Ok(message_str) = String::from_utf8(buffer.clone()) {
                                    match serde_json::from_str::<Message>(&message_str) {
                                        Ok(mut message) => {
                                            debug!(
                                                "Successfully deserialized message: {:?}",
                                                message
                                            );

                                            // Process the message in a separate span
                                            let process_span = info_span!("process_message", 
                                                message_type = ?std::mem::discriminant(&message));

                                            // Process the message
                                            if let Err(e) = self
                                                .process_message(&mut message)
                                                .instrument(process_span)
                                                .await
                                            {
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
                                            buffer.clear();
                                            break;
                                        }
                                        Err(e) if e.is_eof() => {
                                            // Need more data, continue reading
                                            trace!("Message incomplete, need more data");
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

        warn!("Message handler exited for connection {}", conn_id);
    }

    /// Process a received message by delegating to the appropriate handler
    #[instrument(skip(self, message), fields(message_type = ?std::mem::discriminant(message)), level = "debug")]
    async fn process_message(&mut self, message: &mut Message) -> Result<(), String> {
        match message {
            Message::Ping => {
                debug!("Received ping");
                Ok(())
            }
            Message::Pong => {
                debug!("Received pong");
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
            Message::MergeUpdate(payload) => self.process_resource_update_message(payload).await,
            Message::LiveEdit(payload) => self.handle_live_edit_flow(payload).await,
            Message::DeviceManifestRequest(payload) => self.handle_manifest_request(payload).await,
            Message::DeviceManifestResponse(payload) => {
                self.handle_manifest_response(payload).await
            }
            Message::DeviceNetworkSync(payload) => self.handle_device_network_sync(payload).await,
            Message::DeviceManifestAck => self.handle_manifest_ack().await,
            Message::DeviceNetworkSyncAck => self.send_resources().await,
            Message::ResourceAdditionRequest(payload) => {
                self.process_resource_addition_request(payload).await
            }
            Message::ResourceAdditionComplete => self.process_resource_addition_complete().await,
            Message::FirstUserConnection(payload) => {
                self.process_first_connection_exchange(payload).await
            }
            Message::UserManifestPayload(payload) => {
                self.process_user_manifest_payload(payload).await
            }
            Message::UserNetworkSync(payload) => self.process_user_network_sync(payload).await,
            Message::UserNetworkSyncAck => self.send_resources().await,
            Message::RetryRequest => self.execute_connection_action().await,
            Message::Handshake(payload) => self.handle_handshake_message(payload).await,
        }
    }

    /// Sends a message to the peer
    #[instrument(skip(self, message), fields(message_type = ?std::mem::discriminant(&message)), level = "debug")]
    pub async fn send_message(&self, message: Message) -> Result<(), String> {
        debug!("sending message {:?}", message);

        let serialized_message = match serde_json::to_string(&message) {
            Ok(msg) => {
                trace!("Serialized message to {} bytes", msg.len());
                msg
            }
            Err(e) => {
                error!("Failed to serialize message: {}", e);
                return Err(format!("Failed to serialize message: {}", e));
            }
        };

        let (mut send, _) = match self.connection.open_bi().await {
            Ok(stream) => stream,
            Err(e) => {
                error!("Failed to open bi-directional stream: {}", e);
                return Err(format!("Failed to open bi-directional stream: {}", e));
            }
        };

        // Write the message in chunks to handle large payloads
        const CHUNK_SIZE: usize = 8192;
        let bytes = serialized_message.as_bytes();
        let total_chunks = (bytes.len() + CHUNK_SIZE - 1) / CHUNK_SIZE; // Ceiling division

        debug!("Sending message in {} chunks", total_chunks);
        for (i, chunk) in bytes.chunks(CHUNK_SIZE).enumerate() {
            trace!(
                "Sending chunk {}/{} ({} bytes)",
                i + 1,
                total_chunks,
                chunk.len()
            );
            if let Err(e) = send.write_all(chunk).await {
                error!("Failed to write chunk {}: {}", i + 1, e);
                return Err(format!("Failed to write chunk: {}", e));
            }
        }

        if let Err(e) = send.finish() {
            error!("Failed to finish sending: {}", e);
            return Err(format!("Failed to finish sending: {}", e));
        }

        info!("Message sent successfully");
        Ok(())
    }

    /// Enable cloning for PeerConnection
    pub fn clone(&self) -> Self {
        Self {
            connection: self.connection.clone(),
            connection_type: self.connection_type.clone(),
            device: self.device.clone(),
            user: self.user.clone(),
            node_id: self.node_id.clone(),
            is_initiator: self.is_initiator,
            task_handle: tokio::spawn(async {}),
            context: self.context.clone(),
            event_emitter: self.event_emitter.clone(),
            action: self.action.clone(),
            on_close: self.on_close.clone(),
            disconnection_timer: self.disconnection_timer.clone(),
            repo_ctx: self.repo_ctx.clone(),
            crypto_utils: self.crypto_utils.clone(),
            device_manifest_result: self.device_manifest_result.clone(),
            user_manifest_result: self.user_manifest_result.clone(),
            handshake_complete: self.handshake_complete.clone(),
            challenge: self.challenge.clone(),
            is_live_edit: self.is_live_edit.clone(),
        }
    }
    pub async fn get_local_user(&self) -> Result<User, String> {
        let user_guard = self.context.current_user.read().await;
        match user_guard.clone() {
            Some(user) => Ok(user),
            None => Err("No user is currently logged in".to_string()),
        }
    }

    pub async fn get_local_device(&self) -> Option<Device> {
        let device_guard = self.context.current_device.read().await;
        device_guard.clone()
    }
}
