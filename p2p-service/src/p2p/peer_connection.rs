use crate::p2p::emitter::{P2PEvent, P2PEventEmitter};
use crate::p2p::phase_management::PhaseState;
use iroh::endpoint::Connection;
use osvauld_core::models::device::Device;
use osvauld_core::models::p2p::{ConnectionAction, ConnectionType,PhaseType, Phase,PhaseAction, Message};
use osvauld_core::models::user::User;
use osvauld_services::{AuthService,  SyncService, UserService};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tracing::{Instrument, debug, error, info, info_span, instrument, trace, warn};
use iroh_quinn::VarInt;

/// Context struct containing all service dependencies
pub struct ServiceContext {
    pub auth_service: Arc<AuthService>,
    pub user_service: Arc<UserService>,
    pub sync_service: Arc<SyncService>,
    pub current_user: Arc<RwLock<Option<User>>>,
    pub current_device: Arc<RwLock<Option<Device>>>,
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

    pub pending_resource_ids: Arc<Mutex<Vec<String>>>,
    pub phase: PhaseState,
    pub action: Option<ConnectionAction>,
    pub is_live_editing: Arc<Mutex<bool>>,
    pub on_close: Arc<Mutex<Option<Box<dyn Fn(String) + Send + Sync>>>>,
        pub disconnection_timer: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,

}

impl PeerConnection {
    /// Creates a new PeerConnection
    #[instrument(skip(connection, device, user, context, event_emitter, on_close), 
        fields(
            connection_type = ?connection_type,
            device_id = %device.id,
            user_id = %user.id,
            is_initiator = is_initiator
        ),
        level = "info")]
    pub fn new(
        connection: Arc<Connection>,
        connection_type: ConnectionType,
        device: Device,
        user: User,
        is_initiator: bool,
        context: Arc<ServiceContext>,
        event_emitter: P2PEventEmitter,
        pending_resource_ids: Vec<String>,
        action: Option<ConnectionAction>,
            on_close: Option<Box<dyn Fn(String) + Send + Sync>>,
    ) -> Self {
        info!("Creating new peer connection");

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
            pending_resource_ids: Arc::new(Mutex::new(pending_resource_ids)),
            phase: PhaseState::new(),
            action,
        is_live_editing: Arc::new(Mutex::new(false)),
                    on_close: Arc::new(Mutex::new(on_close)),
                    disconnection_timer: Arc::new(Mutex::new(None)),
        };

        debug!("Starting message handler for the connection");

        // Start the message handler and store its task handle
        peer_connection.task_handle = peer_connection.start_message_handler();

        info!(
            "Peer connection created successfully: {}",
            peer_connection.get_id()
        );
        peer_connection
    }

    /// Gets the unique identifier for this connection (user_id:device_id)
    pub fn get_id(&self) -> String {
        format!("{}:{}", self.user.id, self.device.id)
    }

   // Add method to close the connection
    #[instrument(skip(self), fields(connection_id = %self.get_id()), level = "info")]
    pub async fn close_connection(&self) -> Result<(), String> {
        info!("Closing connection: {}", self.get_id());
        self.cancel_disconnection_timer().await;
        
        // Close the Iroh connection
        self.connection.close(VarInt::from_u32(0), b"Connection closed normally");
        
        // Call the closure callback if set
        let connection_id = self.get_id();
        let on_close_guard = self.on_close.lock().await;
        if let Some(callback) = &*on_close_guard {
            info!("Executing connection closure callback for: {}", connection_id);
            (callback)(connection_id);
        }
        
        info!("Connection closed successfully: {}", self.get_id());
        Ok(())
    }

    /// Starts the message handler task
    fn start_message_handler(&self) -> tokio::task::JoinHandle<()> {
        let _connection = self.connection.clone();
        let self_clone = self.clone();
        let conn_id = self.get_id();

        // Create the span first, before the value is moved
        let span = info_span!("message_handler", connection_id = %conn_id);

        tokio::spawn(
            async move {
                info!("Starting message listener for connection {}", conn_id);
                self_clone.handle_messages().await;
                info!("Message listener stopped for connection {}", conn_id);
            }
            .instrument(span),
        )
    }

    /// Handles incoming messages from the peer
    #[instrument(skip(self), level = "debug")]
    async fn handle_messages(&self) {
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
                                        Ok(message) => {
                                            info!(
                                                "Successfully deserialized message: {:?}",
                                                message
                                            );

                                            // Process the message in a separate span
                                            let process_span = info_span!("process_message", 
                                                message_type = ?std::mem::discriminant(&message));

                                            // Process the message
                                            if let Err(e) = self
                                                .process_message(&message)
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
    async fn process_message(&self, message: &Message) -> Result<(), String> {
        match message {
            Message::SyncAck(updated_data) => {
                info!("Received SyncAck");
                self.handle_sync_ack(updated_data.clone()).await
            }
            Message::FirstDeviceConnection(records) => {
                info!("Received AddDevice request");
                self.process_first_device_connection(records).await
            }
            Message::AddDeviceAck => {
                info!("Received AddDeviceAck");
                self.complete_current_phase().await
            }
            Message::SyncResponse(payload) => {
                info!("Received SyncResponse");
                self.handle_sync_response(payload.clone()).await
            }
            Message::Chat(content) => {
                info!("Received chat message: {}", content);
                // No response needed for chat messages
                Ok(())
            }
            Message::Ping => {
                debug!("Received ping");
                Ok(())
            }
            Message::Pong => {
                debug!("Received pong");
                Ok(())
            }
            Message::AckComplete(device_sync_record_ids) => {
                self.ack_complete(device_sync_record_ids.clone()).await
            }
            Message::SyncEvent { event, payload } => {
                info!("Received SyncEvent: {:?}", event);
                self.event_emitter.emit(P2PEvent::EditingEvent { payload: payload.clone() });
                Ok(())
            }
            Message::UserConnection(payload) => {
                self.process_user_connection_payload(payload).await
            }
            Message::Error => {
                error!("Received error message from peer");
                self.event_emitter.emit(P2PEvent::Error {
                    message: "Received error message from peer".to_string(),
                    source: "peer".to_string(),
                });
                Ok(())
            }
            Message::Phase(phase) => {
                self.handle_phase_message(phase).await
            }
            Message::MergeUpdate(payload) => {
                self.process_merge_payload(payload).await
            }
            Message::LiveEdit(payload) => {
                self.handle_live_edit_flow(payload).await
            }
            Message::Disconnect(status) => {
                self.handle_disconnect_message(status).await
            }
        }
    }

    /// Sends a message to the peer
    #[instrument(skip(self, message), fields(message_type = ?std::mem::discriminant(&message)), level = "debug")]
    pub async fn send_message(&self, message: Message) -> Result<(), String> {
        info!("sending message {:?}", message);

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
            Ok(stream) => {
                stream
            }
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

    /// Sends a chat message to the peer
    #[instrument(skip(self, content), fields(content_len = content.len()), level = "info")]
    pub async fn send_chat_message(&self, content: String) -> Result<(), String> {
        debug!("Sending chat message: {}", content);
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
            pending_resource_ids: self.pending_resource_ids.clone(),
            phase: self.phase.clone(),
            action: self.action.clone(),
            is_live_editing: self.is_live_editing.clone(),
            on_close: self.on_close.clone(),
                    disconnection_timer: self.disconnection_timer.clone(),
        }
    }
    pub async fn get_local_user(&self) -> Option<User> {
        let user_guard = self.context.current_user.read().await;
        user_guard.clone()
    }

    pub async fn get_local_device(&self) -> Option<Device> {
        let device_guard = self.context.current_device.read().await;
        device_guard.clone()
    }

     #[instrument(skip(self), fields(action = ?self.action, is_initiator = self.is_initiator), level = "info")]
    pub async fn execute_connection_action(&self) -> Result<(), String> {
        // Only execute if we have an action and we're the initiator
        if let Some(action) = &self.action {
            if !self.is_initiator {
                info!("Not executing action {:?} as this peer is not the initiator", action);
                return Ok(());
            }

            info!("Executing connection action: {:?}", action);
            
            // Execute the appropriate action
            match action {
                ConnectionAction::DeviceSync => {
                    info!("Initiator: Starting device sync phase");
                    self.start_phased_sync().await
                   
                }
                ConnectionAction::UserFirstConnection => {
                    info!("Initiator: Starting user first connection phase");
                    // Use the existing method which already handles phase management
                    self.initiate_user_first_connection().await
                }
                ConnectionAction::AddDevice => {
                    info!("Initiator: Starting add device phase");
                    self.phase.reset_for_new_phase(PhaseType::AddDevice).await;
                    
                    // Send the Phase message to notify the other side
                    let phase_message = Message::Phase(Phase {
                        action: PhaseAction::Init,
                        phase_type: PhaseType::AddDevice,
                    });
                    
                    self.send_message(phase_message).await
                }
                ConnectionAction::LiveEdit => {
                    info!("live edit triggered");
                    let connection_id = self.get_id();
                    self.event_emitter.emit(P2PEvent::LiveEditConnected { connection_id });
                    Ok(())
                }
            }
        } else {
            debug!("No connection action to execute");
            Ok(())
        }
    }

#[instrument(skip(self), fields(connection_id = %self.get_id()), level = "info")]
    pub async fn set_live_editing_active(&self) {
        let mut is_editing = self.is_live_editing.lock().await;
        if !*is_editing {
            info!("Setting connection as active for live editing: {}", self.get_id());
            *is_editing = true;
        }
    }
    
    #[instrument(skip(self), fields(connection_id = %self.get_id()), level = "info")]
    pub async fn set_live_editing_inactive(&self) {
        let mut is_editing = self.is_live_editing.lock().await;
        if *is_editing {
            info!("Setting connection as inactive for live editing: {}", self.get_id());
            *is_editing = false;
        }
    }
    
    #[instrument(skip(self), fields(connection_id = %self.get_id()), level = "trace")]
    pub async fn is_live_editing(&self) -> bool {
        let is_editing = self.is_live_editing.lock().await;
        trace!(
            "Checking if connection is active for live editing: {}, result: {}",
            self.get_id(),
            *is_editing
        );
        *is_editing
    }
}
