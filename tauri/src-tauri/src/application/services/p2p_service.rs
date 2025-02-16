use crate::application::services::auth_service::AuthService;
use crate::application::services::sync_service::SyncService;
use crate::domains::models::device::Device;
use crate::domains::models::p2p::{
    ConnectionTicket, HandshakeError, HandshakeMessage, Message, SyncAckDeviceRecord, SyncAckType,
    SyncPayload,
};
use crate::domains::models::sync_record::SyncRecordSet;
use crate::types::CryptoResponse;
use iroh::endpoint::Connection;
const MAX_HANDSHAKE_SIZE: usize = 8192; // 8KB max size for handshake messages
use iroh::{
    endpoint::{DirectAddrType, RecvStream, SendStream},
    Endpoint, NodeAddr, RelayMode, SecretKey,
};
use iroh_blobs::store::mem::Store;
use log::{debug, error, info, warn};
use std::sync::Arc;
use tauri::AppHandle;
use tauri::Emitter;
use tauri::Manager;
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;
use tokio::time::{timeout, Duration};
use url::Url;

const ALPN_PROTOCOL: &[u8] = b"n0/iroh/examples/magic/0";
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(60);
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(60);

struct P2PState {
    endpoint: Arc<Endpoint>,
    active_connection: Arc<Mutex<Option<Arc<Connection>>>>,
}

#[derive(Clone)]
pub struct P2PService {
    state: Arc<Mutex<Option<P2PState>>>,
    is_initiator: Arc<Mutex<Option<bool>>>,
    device: Arc<Mutex<Option<Device>>>,
    app_handle: AppHandle,
    sync_service: Arc<SyncService>,
    auth_service: Arc<AuthService>,
}

impl P2PService {
    pub fn new(
        app_handle: AppHandle,
        sync_service: Arc<SyncService>,
        auth_service: Arc<AuthService>,
    ) -> Self {
        Self {
            state: Arc::new(Mutex::new(None)),
            is_initiator: Arc::new(Mutex::new(None)),
            device: Arc::new(Mutex::new(None)),
            app_handle,
            sync_service,
            auth_service,
        }
    }
    async fn ensure_initialized(&self) -> Result<(), String> {
        let mut state = self.state.lock().await;
        if state.is_some() {
            return Ok(());
        }

        info!("Initializing P2P endpoint");
        let secret_key = SecretKey::generate(rand::rngs::OsRng);
        let endpoint = Endpoint::builder()
            .secret_key(secret_key)
            .discovery_n0()
            .relay_mode(RelayMode::Default)
            .alpns(vec![ALPN_PROTOCOL.to_vec()])
            .bind()
            .await
            .map_err(|e| format!("Failed to bind endpoint: {}", e))?;

        *state = Some(P2PState {
            endpoint: Arc::new(endpoint),
            active_connection: Arc::new(Mutex::new(None)),
        });

        info!("P2P initialization successful");
        Ok(())
    }

    pub async fn add_device(&self, records: SyncPayload, ticket: String) -> Result<(), String> {
        // First establish connection with the target device using the ticket
        self.connect_with_ticket(&ticket).await?;

        // Once connected, send the AddDevice message
        info!("Connection established, sending AddDevice message");
        let add_device_message = Message::AddDevice(records);
        let serialized = serde_json::to_string(&add_device_message)
            .map_err(|e| format!("Failed to serialize AddDevice message: {}", e))?;

        // Send the message and wait for acknowledgment
        self.send_message(serialized).await?;

        Ok(())
    }

    pub async fn start_listening(&self) -> Result<CryptoResponse, String> {
        // Ensure P2P is initialized
        self.ensure_initialized().await?;
        let state_guard = self.state.lock().await;
        let state = state_guard.as_ref().unwrap();
        let endpoint = state.endpoint.clone();
        let active_connection = state.active_connection.clone();
        let app_handle = self.app_handle.clone();
        let self_clone = self.clone(); // Clone self for use in the spawned task

        tokio::spawn(async move {
            info!("Starting listener for incoming connections");
            while let Some(incoming) = endpoint.accept().await {
                match incoming.accept() {
                    Ok(connecting) => {
                        info!("Accepting incoming connection");
                        app_handle
                            .emit("peer-connected", true)
                            .unwrap_or_else(|e| error!("Failed to emit connection event: {}", e));

                        let app_handle = app_handle.clone();
                        let active_connection = active_connection.clone();
                        let self_clone = self_clone.clone();

                        tokio::spawn(async move {
                            match timeout(CONNECTION_TIMEOUT, connecting).await {
                                Ok(Ok(conn)) => {
                                    info!("Connection established, initiating handshake");
                                    // Perform handshake as the receiver (non-initiator)
                                    match self_clone.perform_handshake(&conn, false).await {
                                        Ok(()) => {
                                            info!("Handshake completed successfully");

                                            let mut active_conn = active_connection.lock().await;
                                            *active_conn = Some(Arc::new(conn));
                                            app_handle.emit("sync-complete", true).unwrap_or_else(
                                                |e| error!("Failed to emit sync event: {}", e),
                                            );
                                        }
                                        Err(e) => {
                                            error!("Handshake failed: {}", e);
                                            app_handle.emit("handshake-failed", e.to_string()).unwrap_or_else(|e| {
                                                error!("Failed to emit handshake failure event: {}", e)
                                            });
                                        }
                                    }
                                }
                                Ok(Err(e)) => error!("Connection failed: {}", e),
                                Err(e) => error!("Connection timeout: {}", e),
                            }
                        });
                    }
                    Err(e) => error!("Failed to accept connection: {}", e),
                }
            }
        });

        Ok(CryptoResponse::Success)
    }

    async fn perform_handshake(&self, conn: &Connection, is_initiator: bool) -> Result<(), String> {
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

        tokio::spawn({
            async move {
                info!(
                    "Starting message listener for {}",
                    if is_initiator {
                        "initiator"
                    } else {
                        "receiver"
                    }
                );
                self_clone.handle_messages().await;
            }
        });
        info!(
            "Handshake completed successfully for {}",
            if is_initiator {
                "initiator"
            } else {
                "receiver"
            }
        );
        if is_initiator {
            self.start_sync().await?;
        }
        //  else {
        //     let app_data_dir = self.app_handle.path().app_data_dir().unwrap();
        //     let test_file_path = app_data_dir.join("test.txt");
        //     self.send_file(test_file_path.to_string_lossy().into_owned())
        //         .await?;
        // }

        Ok(())
    }

    pub async fn start_sync(&self) -> Result<(), String> {
        info!("Starting sync process");
        let connection = self.get_active_connection().await?;
        // Send sync request
        let (mut send, mut recv) = connection
            .open_bi()
            .await
            .map_err(|e| format!("Failed to open bi-directional stream: {}", e))?;

        let sync_request = Message::SyncRequest;
        let serialized = serde_json::to_string(&sync_request)
            .map_err(|e| format!("Serialization error: {}", e))?;

        send.write_all(serialized.as_bytes())
            .await
            .map_err(|e| format!("Failed to send sync request: {}", e))?;
        send.flush()
            .await
            .map_err(|e| format!("Failed to flush sync request: {}", e))?;
        info!("Sync process started");
        Ok(())
    }

    // Helper method to get active connection
    async fn get_active_connection(&self) -> Result<Arc<Connection>, String> {
        let state_guard = self.state.lock().await;
        let state = state_guard.as_ref().ok_or("P2P not initialized")?;
        let active_conn = state.active_connection.lock().await;
        active_conn
            .clone()
            .ok_or_else(|| "No active connection".to_string())
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
        let handshake_message = HandshakeMessage {
            challenge,
            signature,
            device,
        };
        let serialized = serde_json::to_string(&handshake_message)
            .map_err(|e| HandshakeError::Serialization(e.to_string()))?;
        send.write_all(serialized.as_bytes())
            .await
            .map_err(|e| HandshakeError::Connection(e.to_string()))?;
        send.flush()
            .await
            .map_err(|e| HandshakeError::Connection(e.to_string()))?;

        info!("Initiator: Waiting for handshake response");
        let mut buffer = [0u8; MAX_HANDSHAKE_SIZE];
        match timeout(HANDSHAKE_TIMEOUT, recv.read(&mut buffer)).await? {
            Ok(Some(n)) => {
                if n >= MAX_HANDSHAKE_SIZE {
                    return Err(HandshakeError::Connection(
                        "Handshake message too large".into(),
                    ));
                }

                let message_str = String::from_utf8(buffer[..n].to_vec())
                    .map_err(|_| HandshakeError::Connection("Invalid UTF-8 in response".into()))?;

                let response: HandshakeMessage = serde_json::from_str(&message_str)?;
                {
                    let mut device = self.device.lock().await;
                    *device = Some(response.device.clone());
                }

                // Verify the challenge matches
                // if challenge != response.challenge {
                //     return Err(HandshakeError::InvalidChallenge(
                //         "Challenge mismatch".into(),
                //     ));
                // }

                //TODO: Verify signature
                // self.auth_service
                //     .verify_signature(&response.device, &response.challenge, &response.signature)
                //     .await
                //     .map_err(|e| HandshakeError::InvalidSignature(e.to_string()))?;

                info!("Initiator: Handshake completed successfully");
                let _ = self.app_handle.emit("sync-complete", true);
                Ok(())
            }
            Ok(None) => Err(HandshakeError::Connection("Connection closed".into())),
            Err(e) => Err(HandshakeError::Connection(e.to_string())),
        }
    }

    async fn accept_handshake(
        &self,
        send: &mut SendStream,
        recv: &mut RecvStream,
    ) -> Result<(), HandshakeError> {
        info!("Receiver: Waiting for handshake message");
        let mut buffer = [0u8; MAX_HANDSHAKE_SIZE];

        // First receive and validate the incoming handshake
        let handshake_message: HandshakeMessage =
            match timeout(HANDSHAKE_TIMEOUT, recv.read(&mut buffer)).await? {
                Ok(Some(n)) => {
                    if n >= MAX_HANDSHAKE_SIZE {
                        return Err(HandshakeError::Connection(
                            "Handshake message too large".into(),
                        ));
                    }
                    let message_str = String::from_utf8(buffer[..n].to_vec()).map_err(|_| {
                        HandshakeError::Connection("Invalid UTF-8 in response".into())
                    })?;
                    serde_json::from_str(&message_str)?
                }
                Ok(None) => return Err(HandshakeError::Connection("Connection closed".into())),
                Err(e) => return Err(HandshakeError::Connection(e.to_string())),
            };
        //TODO: verify signature

        {
            let mut device = self.device.lock().await;
            *device = Some(handshake_message.device.clone());
        }
        // Get our device and create response
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

        let handshake_message = HandshakeMessage {
            challenge,
            signature,
            device,
        };
        info!("handshake_message {:?}", handshake_message);

        // Send our response
        let serialized = serde_json::to_string(&handshake_message)?;

        send.write_all(serialized.as_bytes())
            .await
            .map_err(|e| HandshakeError::Connection(e.to_string()))?;

        send.flush()
            .await
            .map_err(|e| HandshakeError::Connection(e.to_string()))?;

        info!("Receiver: Handshake completed successfully");
        Ok(())
    }

    pub async fn handle_sync_request(&self) -> Result<(), String> {
        info!("Handling incoming sync request");
        let device = {
            let device_guard = self.device.lock().await;
            (*device_guard
                .as_ref()
                .ok_or_else(|| "Device not set".to_string())?)
            .clone()
        };
        match self.sync_service.get_next_pending_sync(&device).await {
            Ok(Some(payload)) => {
                let message = Message::SyncResponse(payload);
                self.send_message(serde_json::to_string(&message).map_err(|e| e.to_string())?)
                    .await?;
            }
            Ok(None) => {
                info!("No pending syncs, sending sync complete");
                let message = Message::SyncComplete;
                self.send_message(serde_json::to_string(&message).map_err(|e| e.to_string())?)
                    .await?;
            }
            Err(e) => {
                error!("Failed to get pending sync: {}", e);
                return Err(e.to_string());
            }
        }
        Ok(())
    }
    async fn handle_add_device_request(&self, records: SyncPayload) -> Result<(), String> {
        self.sync_service
            .add_new_device_sync(records)
            .await
            .map_err(|e| e.to_string())?;
        // Create and send acknowledgment message
        let ack_message = Message::AddDeviceAck;
        let serialized = serde_json::to_string(&ack_message)
            .map_err(|e| format!("Failed to serialize acknowledgment: {}", e))?;

        // Send the acknowledgment
        self.send_message(serialized).await?;

        info!("Successfully processed device addition request");
        Ok(())
    }

    async fn handle_sync_ack(&self, ack_type: SyncAckType) -> Result<(), String> {
        let device = {
            let device_guard = self.device.lock().await;
            (*device_guard
                .as_ref()
                .ok_or_else(|| "Device not set".to_string())?)
            .clone()
        };
        match ack_type {
            SyncAckType::SyncRecord(sync_id) => {
                info!("Received sync record acknowledgment for ID: {}", sync_id);
                self.sync_service
                    .mark_sync_complete(&sync_id, device.clone())
                    .await
                    .map_err(|e| e.to_string())?;
            }
            SyncAckType::DeviceRecords(device_records) => {
                info!(
                    "Received device records acknowledgment for {} records and {} statuses",
                    device_records.device_records.len(),
                    device_records.status_updates.len()
                );
                self.sync_service
                    .update_sync_status(
                        device_records.device_records,
                        device_records.status_updates,
                    )
                    .await
                    .map_err(|e| e.to_string())?;
            }
        }

        match self.sync_service.get_next_pending_sync(&device).await {
            Ok(Some(payload)) => {
                info!("Sending next sync payload");
                let message = Message::SyncResponse(payload);
                let serialized = serde_json::to_string(&message).map_err(|e| e.to_string())?;
                self.send_message(serialized).await?;
            }
            Ok(None) => {
                info!("No more pending syncs, sending complete");
                let message = Message::SyncComplete;
                let serialized = serde_json::to_string(&message).map_err(|e| e.to_string())?;
                self.send_message(serialized).await?;
            }
            Err(e) => {
                error!("Failed to get next pending sync: {}", e);
                return Err(e.to_string());
            }
        }

        Ok(())
    }

    async fn handle_sync_response(&self, payload: SyncPayload) -> Result<(), String> {
        // Process the sync payload
        let sync_ack = match self.sync_service.process_sync_payload(&payload).await {
            Ok((device_record_ids, status_ids)) => {
                // Determine which type of ack to send based on the payload
                if payload.sync_record.is_some() {
                    Message::SyncAck(SyncAckType::SyncRecord(payload.sync_record.unwrap().id))
                } else {
                    Message::SyncAck(SyncAckType::DeviceRecords(SyncAckDeviceRecord {
                        device_records: device_record_ids,
                        status_updates: status_ids,
                    }))
                }
            }
            Err(e) => {
                error!("Failed to process sync payload: {}", e);
                return Err(e.to_string());
            }
        };

        info!("Processed sync payload successfully");
        self.app_handle
            .emit("sync-completed", true)
            .map_err(|e| e.to_string())?;

        // Send the appropriate ack message
        self.send_message(serde_json::to_string(&sync_ack).map_err(|e| e.to_string())?)
            .await?;

        Ok(())
    }
    async fn handle_sync_complete(&self) -> Result<(), String> {
        info!("Sync process completed");

        // Get is_initiator from state
        let is_initiator = {
            let initiator_guard = self.is_initiator.lock().await;
            *initiator_guard
                .as_ref()
                .ok_or_else(|| "is_initiator not set".to_string())?
        };

        self.app_handle
            .emit("sync-complete", true)
            .map_err(|e| e.to_string())?;

        // If not initiator, start sync
        if !is_initiator {
            self.start_sync().await?;
        }

        Ok(())
    }

    async fn handle_messages(&self) {
        info!("Starting message listener");
        loop {
            match self.get_active_connection().await {
                Ok(connection) => {
                    match connection.accept_bi().await {
                        Ok((mut send, mut recv)) => {
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
                                                    let result = match &message {
                                                        Message::SyncRequest => {
                                                            self.handle_sync_request().await
                                                        }
                                                        Message::SyncAck(updated_data) => {
                                                            self.handle_sync_ack(
                                                                updated_data.clone(),
                                                            )
                                                            .await
                                                        }
                                                        Message::AddDevice(records) => {
                                                            self.handle_add_device_request(
                                                                records.clone(),
                                                            )
                                                            .await
                                                        }
                                                        Message::AddDeviceAck => {
                                                            //let _ =
                                                            // self.handle_add_device_ack().await;
                                                            let _ = self.start_sync().await;
                                                            Ok(())
                                                        }
                                                        Message::SyncResponse(payload) => {
                                                            info!(
                                                                "Received sync payload: {:?}",
                                                                payload
                                                            );
                                                            self.handle_sync_response(
                                                                payload.clone(),
                                                            )
                                                            .await
                                                        }
                                                        Message::SyncComplete => {
                                                            self.handle_sync_complete().await
                                                        }
                                                        Message::Chat(_)
                                                        | Message::Ping
                                                        | Message::Pong => {
                                                            if let Err(e) = self
                                                                .app_handle
                                                                .emit("message-received", message)
                                                            {
                                                                error!(
                                                                    "Failed to emit message: {}",
                                                                    e
                                                                );
                                                            }
                                                            Ok(())
                                                        }
                                                        _ => Ok(()),
                                                    };

                                                    if let Err(e) = result {
                                                        error!("Error handling message: {}", e);
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
                                        break;
                                    }
                                    Err(e) => {
                                        error!("Error reading from connection: {}", e);
                                        break;
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            error!("Failed to accept bi-directional stream: {}", e);
                            break;
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to get active connection: {}", e);
                    break;
                }
            }
        }
        info!("Message listener stopped");
    }

    pub async fn send_file(&self, file_path: String) -> Result<(), String> {
        info!("----------- Starting File Transfer Process -----------");
        info!("Input file path: {}", file_path);

        let path = std::path::Path::new(&file_path);
        info!("Checking if file exists at: {}", path.display());
        if !path.exists() {
            let err = format!("File does not exist at path: {}", path.display());
            error!("{}", err);
            return Err(err);
        }

        let file_name = match path.file_name() {
            Some(name) => {
                let name_str = name.to_string_lossy().into_owned();
                info!("Successfully extracted file name: {}", name_str);
                name_str
            }
            None => {
                let err = format!("Could not extract file name from path: {}", file_path);
                error!("{}", err);
                return Err(err);
            }
        };

        info!("Attempting to read file contents from: {}", file_path);
        let file_data = match tokio::fs::read(&file_path).await {
            Ok(data) => {
                info!("Successfully read file. Size: {} bytes", data.len());
                info!(
                    "First few bytes (up to 10): {:?}",
                    &data.iter().take(10).collect::<Vec<_>>()
                );
                data
            }
            Err(e) => {
                let err = format!("Failed to read file contents: {}", e);
                error!("{}", err);
                return Err(err);
            }
        };

        info!(
            "Creating FileTransfer message with name: {} and data size: {}",
            file_name,
            file_data.len()
        );
        let message = Message::FileTransfer {
            name: file_name.clone(),
            data: file_data.clone(),
        };

        info!("Attempting to serialize FileTransfer message");
        let serialized = match serde_json::to_string(&message) {
            Ok(json) => {
                info!(
                    "Successfully serialized message. JSON size: {} bytes",
                    json.len()
                );
                json
            }
            Err(e) => {
                let err = format!("Failed to serialize FileTransfer message: {}", e);
                error!("{}", err);
                return Err(err);
            }
        };

        info!("Attempting to send serialized message");
        match self.send_message(serialized).await {
            Ok(_) => {
                info!("----------- File Transfer Complete -----------");
                info!("Successfully sent file: {}", file_name);
                info!("Total bytes transferred: {}", file_data.len());
                Ok(())
            }
            Err(e) => {
                let err = format!("Failed to send message through connection: {}", e);
                error!("----------- File Transfer Failed -----------");
                error!("{}", err);
                Err(err)
            }
        }
    }

    async fn handle_add_device_ack(&self, device: Device) -> Result<(), String> {
        self.sync_service
            .add_device_entry(device)
            .await
            .map_err(|e| e.to_string())
    }

    async fn handle_file_receive(&self, name: String, data: Vec<u8>) -> Result<(), String> {
        // Get app data directory and create downloads folder within it
        let app_data_dir = self.app_handle.path().app_data_dir().unwrap();
        let downloads_dir = app_data_dir.join("downloads");

        // Create downloads directory
        tokio::fs::create_dir_all(&downloads_dir)
            .await
            .map_err(|e| format!("Failed to create downloads directory: {}", e))?;

        // Write file to downloads directory
        let file_path = downloads_dir.join(&name);
        tokio::fs::write(&file_path, data)
            .await
            .map_err(|e| format!("Failed to write file: {}", e))?;

        info!("File saved to: {}", file_path.display());

        // Emit event to notify UI
        if let Err(e) = self.app_handle.emit("file-received", name) {
            error!("Failed to emit file received event: {}", e);
        }

        Ok(())
    }
    pub async fn send_chat_message(&self, message: String) -> Result<CryptoResponse, String> {
        let msg = Message::Chat(message);
        let serialized = serde_json::to_string(&msg).map_err(|e| e.to_string())?;
        self.send_message(serialized).await
    }

    async fn send_message(&self, message: String) -> Result<CryptoResponse, String> {
        let state_guard = self.state.lock().await;
        let state = state_guard.as_ref().ok_or("P2P not initialized")?;
        let connection = {
            let active_conn = state.active_connection.lock().await;
            match &*active_conn {
                Some(conn) => Arc::clone(conn),
                None => return Err("No active connection".to_string()),
            }
        };

        let (mut send, _) = connection
            .open_bi()
            .await
            .map_err(|e| format!("Failed to open bi-directional stream: {}", e))?;

        // Write the message in chunks to handle large payloads
        const CHUNK_SIZE: usize = 8192;
        let bytes = message.as_bytes();

        for chunk in bytes.chunks(CHUNK_SIZE) {
            send.write_all(chunk)
                .await
                .map_err(|e| format!("Failed to write chunk: {}", e))?;
        }

        send.finish()
            .map_err(|e| format!("Failed to finish sending: {}", e))?;

        Ok(CryptoResponse::Success)
    }

    pub async fn get_connection_ticket(&self) -> Result<String, String> {
        self.ensure_initialized().await?;
        let state_guard = self.state.lock().await;
        let state = state_guard.as_ref().ok_or("P2P not initialized")?;
        let node_addr = state
            .endpoint
            .node_addr()
            .await
            .map_err(|e| e.to_string())?;
        let addrs = node_addr
            .direct_addresses
            .into_iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>();
        let ticket = ConnectionTicket {
            node_id: state.endpoint.node_id().to_string(),
            addresses: addrs,
        };

        serde_json::to_string(&ticket).map_err(|e| e.to_string())
    }
    pub async fn connect_with_ticket(&self, ticket_str: &str) -> Result<(), String> {
        self.ensure_initialized().await?;

        info!("Starting connection process with ticket: {}", ticket_str);

        let (endpoint, node_addr) = {
            let state_guard = self.state.lock().await;
            let state = state_guard.as_ref().ok_or("P2P not initialized")?;

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

            info!("Created NodeAddr: {:?}", node_addr);
            info!("Our endpoint ID: {}", state.endpoint.node_id());
            info!(
                "ALPN Protocol being used: {}",
                String::from_utf8_lossy(ALPN_PROTOCOL)
            );

            (state.endpoint.clone(), node_addr)
        };

        info!("Starting endpoint.connect() call...");
        let connect_result = endpoint.connect(node_addr.clone(), ALPN_PROTOCOL).await;

        match &connect_result {
            Ok(conn) => {
                info!("Connection successful!");
                info!("Remote address: {}", conn.remote_address());
                info!("Connection stats: {:?}", conn.stats());

                // Try to get additional connection info if possible
                if let Ok(peer_id) = iroh::endpoint::get_remote_node_id(conn) {
                    info!("Connected to peer ID: {}", peer_id);
                }
            }
            Err(e) => {
                error!("Connection failed. Error details:");
                error!("Error: {}", e);
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

        self.app_handle
            .emit("sync-complete", true)
            .map_err(|e| format!("Failed to emit sync event: {}", e))?;

        Ok(())
    }
}
