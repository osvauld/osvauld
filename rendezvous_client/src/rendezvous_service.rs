use crate::ws::{UserConnectionStatus, WsClient, WsMessage};
use log::{debug, error, info};
use osvauld_core::models::p2p::ConnectionType;
use osvauld_core::models::user::User;
use osvauld_services::UserService;
// Import the User model
use p2p_service::P2PService;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Service that handles WebSocket-based rendezvous for P2P connections
pub struct RendezvousService {
    client: Arc<Mutex<WsClient>>,
    p2p_service: Arc<P2PService>,
    ws_url: String,
    pending_first_connections: Arc<Mutex<HashSet<String>>>,
    connection_id: Arc<Mutex<Option<String>>>, // Store the entire User object
    user_service: Arc<UserService>,
}

impl RendezvousService {
    /// Create a new RendezvousService instance
    pub fn new(p2p_service: Arc<P2PService>, ws_url: &str, user_service: Arc<UserService>) -> Self {
        Self {
            client: Arc::new(Mutex::new(WsClient::new())),
            p2p_service,
            ws_url: ws_url.to_string(),
            pending_first_connections: Arc::new(Mutex::new(HashSet::new())),
            connection_id: Arc::new(Mutex::new(None)), // Initialize with None
            user_service,
        }
    }

    /// Initialize the service with the provided user
    pub async fn initialize(&self, user: String, current_device_id: &str) -> Result<(), String> {
        // Store the user in the service state
        {
            let mut user_lock = self.connection_id.lock().await;
            *user_lock = Some(user.clone());
        }

        // Connect to the rendezvous server with the user_id
        self.connect_and_register(&user).await?;
        {
            let client = self.client.lock().await;
            client.start_ping_interval(30); // Ping every 30 seconds
        }

        // Create clones of the fields we need in the async task
        let client = self.client.clone();
        let p2p_service = self.p2p_service.clone();
        let pending_first_connections = self.pending_first_connections.clone();
        let user_arc = self.connection_id.clone(); // Pass the stored user

        // Start message handler in a separate task
        tokio::spawn(async move {
            Self::handle_messages(
                client,
                p2p_service,
                pending_first_connections,
                user_arc, // Pass the user to the handler
            )
            .await;
        });
        self.initialize_sync_with_pending_devices(current_device_id)
            .await?;

        Ok(())
    }

    // Add method to mark a user as needing first connection initialization
    pub async fn mark_for_first_connection(&self, target_user_id: &str) -> Result<(), String> {
        // Add the user ID to the pending set
        let mut pending = self.pending_first_connections.lock().await;
        pending.insert(target_user_id.to_string());

        // Send the connection request
        self.request_user_connection(target_user_id).await
    }

    /// Connect to the rendezvous server and register
    pub async fn connect_and_register(&self, user_id: &str) -> Result<(), String> {
        let mut client = self.client.lock().await;
        client.connect_and_register(&self.ws_url, user_id).await
    }

    /// Request a connection to another peer by connection ID
    pub async fn request_connection(&self, target_connection_id: &str) -> Result<(), String> {
        let client = self.client.lock().await;
        client.request_connection(target_connection_id).await
    }

    /// Request a connection to another peer by user ID
    pub async fn request_user_connection(&self, target_user_id: &str) -> Result<(), String> {
        let client = self.client.lock().await;
        info!("requesting user connection..");
        client.request_user_connection_string(target_user_id).await
    }

    /// Send a connection response with connection string
    pub async fn send_connection_response(
        &self,
        user_id: &str,
        target_connection_id: &str,
        connection_string: &str,
    ) -> Result<(), String> {
        let client = self.client.lock().await;
        client
            .send_connection_response(user_id, target_connection_id, connection_string)
            .await
    }

    /// Close the WebSocket connection
    pub async fn close(&self) -> Result<(), String> {
        let client = self.client.lock().await;
        client.close().await
    }

    /// Handle incoming WebSocket messages
    async fn handle_messages(
        client: Arc<Mutex<WsClient>>,
        p2p_service: Arc<P2PService>,
        pending_first_connections: Arc<Mutex<HashSet<String>>>,
        user: Arc<Mutex<Option<String>>>,
    ) {
        let mut receiver = {
            let client_lock = client.lock().await;
            client_lock.subscribe()
        };

        loop {
            match receiver.recv().await {
                Ok(msg) => match msg {
                    WsMessage::RequestConnection {
                        target_connection_id,
                    } => {
                        info!("Received connection request from: {}", target_connection_id);
                        if let Err(e) = Self::handle_connection_request(
                            &client,
                            &p2p_service,
                            &target_connection_id,
                            &user,
                        )
                        .await
                        {
                            error!("Failed to handle connection request: {}", e);
                        }
                    }
                    WsMessage::UserConnectionStringResponse {
                        user_id,
                        connection_string,
                        connection_status,
                    } => {
                        info!(
                            "Received connection response from user {}, status: {}",
                            user_id, connection_status
                        );

                        if let Some(conn_string) = connection_string {
                            if !conn_string.is_empty() {
                                Self::process_connection_string(
                                    &p2p_service,
                                    &pending_first_connections,
                                    &user,
                                    &user_id,
                                    conn_string,
                                )
                                .await;
                            }
                        }
                    }
                    _ => {
                        debug!("Received other message: {:?}", msg);
                    }
                },
                Err(e) => {
                    error!("Error receiving message: {}", e);
                    match e {
                        tokio::sync::broadcast::error::RecvError::Closed => {
                            info!("WebSocket connection closed, stopping message handler");
                            break;
                        }
                        tokio::sync::broadcast::error::RecvError::Lagged(n) => {
                            info!("Receiver lagged behind by {} messages", n);
                        }
                    }
                }
            }
        }
    }

    // Extract connection string processing into a separate function
    async fn process_connection_string(
        p2p_service: &Arc<P2PService>,
        pending_first_connections: &Arc<Mutex<HashSet<String>>>,
        user: &Arc<Mutex<Option<String>>>,
        response_user_id: &str,
        conn_string: String,
    ) {
        // Check if this is a first connection
        let is_first_connection = {
            let mut pending = pending_first_connections.lock().await;
            let is_first = pending.contains(response_user_id);
            if is_first {
                pending.remove(response_user_id);
            }
            is_first
        };

        let connection_type = ConnectionType::User;
        let p2p_service_clone = p2p_service.clone();
        let ticket = conn_string.clone();
        let user_id = response_user_id.to_string();

        // Create connection ID using the response_user_id
        let connection_id = format!("{}", response_user_id);

        info!(
            "Processing connection string for user: {}",
            response_user_id
        );

        tokio::spawn(async move {
            match p2p_service_clone
                .connect_with_ticket(&ticket, connection_type, Some(&connection_id))
                .await
            {
                Ok(Some(connection)) => {
                    // We successfully created a new connection
                    if is_first_connection {
                        info!("starting first device sync");

                        if let Err(e) = connection.initiate_user_first_connection().await {
                            error!("Failed to initialize first user connection: {}", e);
                        }
                    } else {
                        info!("Successfully connected to peer using ticket");
                        // For regular connections, start device sync
                        if let Err(e) = connection.start_device_sync().await {
                            error!("Failed to initialize sync with user devices: {}", e);
                        }
                    }
                }
                Ok(None) => {
                    // Connection already exists or is being established
                    info!(
                        "Connection to user {} already exists or is being established",
                        user_id
                    );
                }
                Err(e) => {
                    error!("Failed to connect with ticket: {}", e);
                }
            }
        });
    }
    /// Handle incoming connection request
    ///
    async fn handle_connection_request(
        client: &Arc<Mutex<WsClient>>,
        p2p_service: &Arc<P2PService>,
        target_connection_id: &str,
        user: &Arc<Mutex<Option<String>>>,
    ) -> Result<(), String> {
        // Start P2P listener
        if let Err(e) = p2p_service.start_listening().await {
            return Err(format!("Failed to start P2P listener: {}", e));
        }
        // Get connection ticket from P2P service
        let ticket = match p2p_service.get_connection_ticket().await {
            Ok(ticket) => ticket,
            Err(e) => return Err(format!("Failed to get connection ticket: {}", e)),
        };

        // Get the user ID from the stored user
        let user_id = {
            let user_lock = user.lock().await;
            match &*user_lock {
                Some(user) => user.clone(),
                None => return Err("User not initialized".to_string()),
            }
        };

        // Send connection response
        let client_lock = client.lock().await;
        client_lock
            .send_connection_response(&user_id, target_connection_id, &ticket)
            .await
    }

    pub async fn get_connection_status(
        &self,
        user_ids: Vec<String>,
    ) -> Result<Vec<UserConnectionStatus>, String> {
        let client = self.client.lock().await;
        client.get_connection_status(user_ids).await
    }

    pub async fn initialize_sync_with_pending_devices(
        &self,
        current_device_id: &str,
    ) -> Result<(), String> {
        info!("Checking for devices with pending syncs...");

        // Get devices with pending syncs
        match self
            .user_service
            .get_users_with_pending_syncs(current_device_id)
            .await
        {
            Ok(users_with_devices) => {
                info!(
                    "Found {} users with devices that have pending syncs",
                    users_with_devices.len()
                );

                // Process each user's devices
                for (sync_user_id, devices) in users_with_devices {
                    if !devices.is_empty() {
                        info!(
                            "User {} has {} devices with pending syncs",
                            sync_user_id,
                            devices.len()
                        );

                        // Create list of connection IDs to check
                        let device_connection_ids: Vec<String> = devices
                            .iter()
                            .map(|device| format!("{}:{}", sync_user_id, device.id))
                            .collect();

                        // Check which devices are online
                        match self.get_connection_status(device_connection_ids).await {
                            Ok(statuses) => {
                                for status in statuses {
                                    if status.connection_status == "online" {
                                        info!(
                                            "Device {} is online, requesting connection",
                                            status.user_id
                                        );
                                        if let Err(e) =
                                            self.request_user_connection(&status.user_id).await
                                        {
                                            error!(
                                                "Failed to request connection to {}: {}",
                                                status.user_id, e
                                            );
                                        }
                                    } else {
                                        info!(
                                            "Device {} is offline, skipping sync",
                                            status.user_id
                                        );
                                    }
                                }
                            }
                            Err(e) => {
                                error!("Failed to get connection status: {}", e);
                            }
                        }
                    }
                }
                Ok(())
            }
            Err(e) => {
                error!("Failed to get devices with pending syncs: {}", e);
                Err(e)
            }
        }
    }
}
