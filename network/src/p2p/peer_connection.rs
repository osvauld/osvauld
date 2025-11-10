use crate::p2p::{
    emitter::{P2PEvent, P2PEventEmitter},
    errors::{MessageError, P2PError, P2PResult, SyncError},
    folder_sync, resource_sync,
};
use crypto_utils::CryptoUtils;
use iroh::endpoint::Connection;
use iroh_quinn::VarInt;
use osvauld_core::models::{Device, Message, PeerRole, User};
use persistance::database::RepositoryContext;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, error, info, info_span, instrument, trace, warn, Instrument};

/// Connection type inferred from UCAN token role
#[derive(Clone, Debug)]
pub enum ConnectionType {
    Owner,  // Owner ↔ Owner or Owner ↔ User connection
    Node,   // Node ↔ Owner connection
    Viewer, // Viewer ↔ Node connection (not yet implemented)
}

impl ConnectionType {
    /// Create ConnectionType from PeerRole
    pub fn from_peer_role(peer_role: &PeerRole) -> Self {
        match peer_role {
            PeerRole::Owner => ConnectionType::Owner,
            PeerRole::Node => ConnectionType::Node,
            PeerRole::Viewer => ConnectionType::Viewer,
            PeerRole::User => ConnectionType::Owner, // Default to Owner
        }
    }
}

/// Context struct containing all service dependencies
pub struct ServiceContext {
    pub current_user: Arc<RwLock<Option<User>>>,
    pub current_device: Arc<RwLock<Option<Device>>>,
}

/// Represents a peer-to-peer connection with another device or user
pub struct PeerConnection {
    pub connection: Arc<Connection>,
    pub connection_type: Arc<RwLock<Option<ConnectionType>>>,
    pub device: Arc<RwLock<Device>>,
    pub node_id: String,
    pub user: Arc<RwLock<User>>,
    pub is_initiator: bool,
    pub handshake_complete: Arc<Mutex<bool>>,
    pub task_handle: tokio::task::JoinHandle<()>,
    pub context: Arc<ServiceContext>,
    pub event_emitter: P2PEventEmitter,
    pub on_close: Arc<Mutex<Option<Box<dyn Fn(String) + Send + Sync>>>>,
    pub disconnection_timer: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    pub crypto_utils: Arc<RwLock<CryptoUtils>>,
    pub repo_ctx: Arc<RepositoryContext>,
    pub challenge: String,
    pub domain: String,
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
        crypto_utils: Arc<RwLock<CryptoUtils>>,
        repo_ctx: Arc<RepositoryContext>,
        node_id: String,
        local_user: User,
        local_device: Device,
        challenge: String,
        domain: String,
    ) -> Self {
        info!("Creating new peer connection");

        let task_handle = tokio::spawn(async {});

        let mut peer_connection = Self {
            connection,
            connection_type: Arc::new(RwLock::new(None)), // Will be set during handshake
            device: Arc::new(RwLock::new(local_device)),
            node_id,
            user: Arc::new(RwLock::new(local_user)),
            is_initiator,
            handshake_complete: Arc::new(Mutex::new(false)),
            task_handle,
            context,
            event_emitter,
            on_close: Arc::new(Mutex::new(on_close)),
            disconnection_timer: Arc::new(Mutex::new(None)),
            repo_ctx,
            crypto_utils,
            challenge,
            domain,
        };

        debug!("Starting message handler for the connection");
        peer_connection.task_handle = peer_connection.start_message_handler();

        info!("Peer connection created successfully");
        peer_connection
    }

    pub fn get_id(&self) -> String {
        self.node_id.clone()
    }

    /// Set connection type based on UCAN role
    pub async fn set_connection_type_from_role(&self, role: &str) {
        let conn_type = match role {
            "owner" | "node" => ConnectionType::Owner,
            "viewer" => ConnectionType::Viewer,
            _ => {
                warn!("Unknown role '{}', defaulting to Owner", role);
                ConnectionType::Owner
            }
        };

        let mut type_guard = self.connection_type.write().await;
        *type_guard = Some(conn_type);
        info!("Set connection type based on role: {} -> {:?}", role, type_guard);
    }

    /// Set connection type directly
    pub async fn set_connection_type(&self, conn_type: ConnectionType) {
        let mut type_guard = self.connection_type.write().await;
        *type_guard = Some(conn_type);
    }

    pub async fn get_peer_device(&self) -> Device {
        self.device.read().await.clone()
    }

    pub async fn get_peer_user(&self) -> User {
        self.user.read().await.clone()
    }

    pub async fn set_peer_user_and_device(&self, new_user: User, new_device: Device) {
        let mut user_guard = self.user.write().await;
        let mut device_guard = self.device.write().await;
        *user_guard = new_user;
        *device_guard = new_device;
    }


    pub async fn get_connection_type(&self) -> ConnectionType {
        let conn_type = self.connection_type.read().await.clone();
        conn_type.unwrap_or(ConnectionType::Owner)
    }

    /// Removes a resource from local_missing.unknown_resources in device manifest
    #[instrument(skip(self), level = "info")]
    pub async fn close_connection(&self) -> P2PResult<()> {
        info!("Closing connection: {}", self.get_id());

        self.connection
            .close(VarInt::from_u32(0), b"Connection closed normally");

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
        let self_clone = self.clone();
        let conn_id = self.get_id();
        tokio::spawn(async move {
            info!("Starting message listener for connection {}", conn_id);
            self_clone.handle_messages().await;
            info!("Message listener stopped for connection {}", conn_id);
        })
    }

    /// Handles incoming messages from the peer
    #[instrument(skip_all, level = "debug")]
    async fn handle_messages(&self) {
        let conn_id = self.get_id();
        info!("Message handler started for connection {}", conn_id);

        loop {
            match self.connection.accept_bi().await {
                Ok((_send, mut recv)) => {
                    debug!("Accepted new bi-directional stream");

                    let mut buffer = Vec::new();
                    let mut temp_buffer = vec![0u8; 8192];

                    loop {
                        match recv.read(&mut temp_buffer).await {
                            Ok(Some(n)) if n > 0 => {
                                trace!("Read {} bytes from stream", n);
                                buffer.extend_from_slice(&temp_buffer[..n]);

                                if let Ok(message_str) = String::from_utf8(buffer.clone()) {
                                    match serde_json::from_str::<Message>(&message_str) {
                                        Ok(mut message) => {
                                            debug!(
                                                "Successfully deserialized message: {:?}",
                                                message
                                            );

                                            let process_span = info_span!("process_message", 
                                                message_type = ?std::mem::discriminant(&message));

                                            if let Err(e) = self
                                                .process_message(&mut message)
                                                .instrument(process_span)
                                                .await
                                            {
                                                error!("Error processing message: {}", e);

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
                                            trace!("Message incomplete, need more data");
                                            continue;
                                        }
                                        Err(e) => {
                                            error!("Failed to deserialize message: {}", e);
                                            buffer.clear();
                                            break;
                                        }
                                    }
                                }
                            }
                            Ok(Some(_)) => continue,
                            Ok(None) => {
                                info!("Connection closed by peer");
                                self.event_emitter.emit(P2PEvent::Disconnected);
                                return;
                            }
                            Err(e) => {
                                error!("Error reading from connection: {}", e);
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
    #[instrument(skip_all, level = "info")]
    async fn process_message(&self, message: &mut Message) -> P2PResult<()> {
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
            Message::MergeUpdate(payload) => {
                match payload {
                    osvauld_core::models::ResourceUpdateMsg::StateVectorRequest {
                        resource_id,
                        state_vectors,
                        asset_ids,
                        ucan_token,
                    } => {
                        resource_sync::handle_state_vector_request(
                            resource_id.clone(),
                            state_vectors.clone(),
                            asset_ids.clone(),
                            ucan_token.clone(),
                            Arc::new(self.clone()),
                            self.repo_ctx.clone(),
                            self.crypto_utils.clone(),
                        )
                        .await
                    }
                    osvauld_core::models::ResourceUpdateMsg::UpdatesResponse {
                        resource_id,
                        updates,
                        state_vectors,
                        missing_asset_ids,
                        ucan_token,
                    } => {
                        resource_sync::handle_updates_response(
                            resource_id.clone(),
                            updates.clone(),
                            state_vectors.clone(),
                            missing_asset_ids.clone(),
                            ucan_token.clone(),
                            Arc::new(self.clone()),
                            self.repo_ctx.clone(),
                            self.crypto_utils.clone(),
                        )
                        .await
                    }
                }
            }
            Message::ResourceAdditionRequest(_payload) => {
                // TODO: Forward to P2PService::handle_resource_addition_request
                info!("Received ResourceAdditionRequest message (handler not yet wired)");
                Ok(())
            }
            Message::ResourceAdditionComplete => {
                // TODO: Forward to P2PService::handle_resource_addition_complete
                info!("Received ResourceAdditionComplete message (handler not yet wired)");
                Ok(())
            }
            Message::AssetTransfer(_payload) => {
                // TODO: Forward to P2PService::handle_asset_transfer
                info!("Received AssetTransfer message (handler not yet wired)");
                Ok(())
            }
            Message::RetryRequest => {
                info!("Received retry request from peer");
                // TODO: Determine what to retry
                Ok(())
            }
            Message::Handshake(payload) => self.handle_handshake_message(payload).await,
            Message::FolderDataSync(payload) => {
                folder_sync::handle_folder_data_sync(
                    payload.clone(),
                    Arc::new(self.clone()),
                    self.repo_ctx.clone(),
                    self.crypto_utils.clone(),
                )
                .await
            }
            Message::ResourceDataSync(payload) => {
                resource_sync::handle_resource_data_sync(
                    payload.clone(),
                    Arc::new(self.clone()),
                    self.repo_ctx.clone(),
                    self.crypto_utils.clone(),
                )
                .await
            }
            Message::ResourceSyncRequest(payload) => {
                resource_sync::handle_resource_sync_request(
                    payload.clone(),
                    Arc::new(self.clone()),
                    self.repo_ctx.clone(),
                    self.crypto_utils.clone(),
                )
                .await
            }
            Message::ResourceNotFoundRequest(payload) => {
                resource_sync::handle_resource_not_found_request(
                    payload.clone(),
                    Arc::new(self.clone()),
                    self.repo_ctx.clone(),
                    self.crypto_utils.clone(),
                )
                .await
            }
            Message::ResourceTransfer(payload) => {
                resource_sync::handle_resource_transfer(
                    payload.clone(),
                    Arc::new(self.clone()),
                    self.repo_ctx.clone(),
                    self.crypto_utils.clone(),
                )
                .await
            }
            Message::ResourceTransferAck => {
                resource_sync::handle_resource_transfer_ack(
                    Arc::new(self.clone()),
                )
                .await
            }
            Message::FolderTokenRequest(payload) => {
                folder_sync::handle_folder_token_request(
                    payload.clone(),
                    Arc::new(self.clone()),
                )
                .await
            }
            Message::FolderTokenResponse(payload) => {
                folder_sync::handle_folder_token_response(
                    payload.clone(),
                    Arc::new(self.clone()),
                )
                .await
            }
        }
    }

    /// Sends a message to the peer
    #[instrument(skip(self, message), fields(message_type = ?std::mem::discriminant(&message)), level = "debug")]
    pub async fn send_message(&self, message: Message) -> P2PResult<()> {
        debug!("sending message {:?}", message);

        let serialized_message = serde_json::to_string(&message)
            .map_err(|e| P2PError::Message(MessageError::SerializationFailed(e)))?;

        trace!("Serialized message to {} bytes", serialized_message.len());

        let (mut send, _) = self
            .connection
            .open_bi()
            .await
            .map_err(|e| P2PError::Message(MessageError::StreamError(e.to_string())))?;

        // Write the message in chunks to handle large payloads
        const CHUNK_SIZE: usize = 8192;
        let bytes = serialized_message.as_bytes();
        let total_chunks = (bytes.len() + CHUNK_SIZE - 1) / CHUNK_SIZE;

        debug!("Sending message in {} chunks", total_chunks);
        for (i, chunk) in bytes.chunks(CHUNK_SIZE).enumerate() {
            trace!(
                "Sending chunk {}/{} ({} bytes)",
                i + 1,
                total_chunks,
                chunk.len()
            );
            send.write_all(chunk).await.map_err(|e| {
                P2PError::Message(MessageError::SendFailed {
                    reason: format!("Failed to write chunk {}: {}", i + 1, e),
                })
            })?;
        }

        send.finish().map_err(|e| {
            P2PError::Message(MessageError::SendFailed {
                reason: format!("Failed to finish sending: {}", e),
            })
        })?;

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
            on_close: self.on_close.clone(),
            disconnection_timer: self.disconnection_timer.clone(),
            repo_ctx: self.repo_ctx.clone(),
            crypto_utils: self.crypto_utils.clone(),
            handshake_complete: self.handshake_complete.clone(),
            challenge: self.challenge.clone(),
            domain: self.domain.clone(),
        }
    }

    pub async fn get_local_user(&self) -> P2PResult<User> {
        let user_guard = self.context.current_user.read().await;
        user_guard
            .clone()
            .ok_or_else(|| P2PError::InvalidState("No user is currently logged in".to_string()))
    }

    pub async fn get_local_device(&self) -> Option<Device> {
        let device_guard = self.context.current_device.read().await;
        device_guard.clone()
    }
}
