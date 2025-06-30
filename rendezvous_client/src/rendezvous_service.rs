use crate::ws::{UserConnectionStatus, WsClient, WsMessage};
use log::{debug, error, info};
use osvauld_core::models::p2p::{ConnectionAction, ConnectionType};
use osvauld_services::{UserService, get_rendezvous_payload};
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
    connection_id: Arc<Mutex<Option<String>>>,
    user_service: Arc<UserService>,
    live_edit_connections: Arc<Mutex<HashSet<String>>>,
}

impl RendezvousService {
    /// Create a new RendezvousService instance
    pub fn new(p2p_service: Arc<P2PService>, ws_url: &str, user_service: Arc<UserService>) -> Self {
        Self {
            client: Arc::new(Mutex::new(WsClient::new())),
            p2p_service,
            ws_url: ws_url.to_string(),
            pending_first_connections: Arc::new(Mutex::new(HashSet::new())),
            connection_id: Arc::new(Mutex::new(None)),
            user_service,
            live_edit_connections: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    /// Initialize the service with the provided user
    pub async fn initialize(
        &self,
        user: String,
        connection_strings: Vec<String>,
    ) -> Result<(), String> {
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
        let live_edit_connections = self.live_edit_connections.clone();
        let user_arc = self.connection_id.clone();

        // Start message handler in a separate task
        tokio::spawn(async move {
            Self::handle_messages(
                client,
                p2p_service,
                pending_first_connections,
                live_edit_connections,
                user_arc,
            )
            .await;
        });

        self.request_user_connection_notifications(connection_strings)
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
        live_edit_connections: Arc<Mutex<HashSet<String>>>,
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
                                    &live_edit_connections,
                                    &user,
                                    &user_id,
                                    conn_string,
                                )
                                .await;
                            }
                        }
                    }
                    WsMessage::UserConnectionNotification { online_user_id } => {
                        info!("User came online: {}", online_user_id);

                        // When a user comes online, attempt to establish a connection
                        let client_clone = client.clone();
                        let user_id = online_user_id.clone();

                        tokio::spawn(async move {
                            let client_lock = client_clone.lock().await;
                            if let Err(e) =
                                client_lock.request_user_connection_string(&user_id).await
                            {
                                error!(
                                    "Failed to request connection to newly online user {}: {}",
                                    user_id, e
                                );
                            }
                        });
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

    async fn process_connection_string(
        p2p_service: &Arc<P2PService>,
        pending_first_connections: &Arc<Mutex<HashSet<String>>>,
        live_edit_connections: &Arc<Mutex<HashSet<String>>>,
        user: &Arc<Mutex<Option<String>>>,
        response_user_id: &str,
        conn_string: String,
    ) -> Result<(), String> {
        // Check if this is a first connection
        let is_first_connection = {
            let mut pending = pending_first_connections.lock().await;
            let is_first = pending.contains(response_user_id);
            if is_first {
                pending.remove(response_user_id);
            }
            is_first
        };

        let is_live_edit = {
            let live_edit = live_edit_connections.lock().await;
            // Check if the exact user_id:device_id is in our live edit set
            live_edit.contains(response_user_id)
        };

        let p2p_service_clone = p2p_service.clone();
        let ticket = conn_string.clone();
        let user_id = response_user_id.to_string();

        // Create connection ID using the response_user_id
        let connection_id = format!("{}", response_user_id);
        let (response_user_id, device_id) = response_user_id
            .split_once(':')
            .ok_or("Invalid connection_id format")?;
        let current_user_id = {
            let user_lock = user.lock().await;
            match &*user_lock {
                Some(connection_id) => {
                    let (user_id, _) = connection_id
                        .split_once(':')
                        .ok_or("Invalid stored connection_id format")?;
                    user_id.to_string()
                }
                None => return Err("User connection_id not initialized".to_string()),
            }
        };
        info!("current_user_id {}", current_user_id);
        let connection_type = if current_user_id == response_user_id {
            ConnectionType::Device
        } else {
            ConnectionType::User
        };
        // Determine the appropriate action based on whether this is a first connection

        let action = if is_first_connection {
            info!("This is a first connection with user: {}", response_user_id);
            Some(ConnectionAction::UserFirstConnection)
        } else if is_live_edit {
            info!(
                "This is a live edit connection with user: {}",
                response_user_id
            );
            Some(ConnectionAction::LiveEdit)
        } else {
            info!(
                "This is a regular connection with user: {}",
                response_user_id
            );
            match connection_type {
                ConnectionType::Device => {
                    info!(
                        "This is a regular device connection with: {}",
                        response_user_id
                    );
                    Some(ConnectionAction::DeviceSync)
                }
                ConnectionType::User => {
                    info!(
                        "This is a regular user connection with: {}",
                        response_user_id
                    );
                    Some(ConnectionAction::UserSync)
                }
            }
        };

        info!(
            "Processing connection string for user: {}, action: {:?}",
            response_user_id, action
        );

        tokio::spawn(async move {
            match p2p_service_clone
                .connect_with_ticket(&ticket, connection_type, Some(&connection_id), action)
                .await
            {
                Ok(Some(_connection)) => {
                    // The action is executed as part of connect_with_ticket via execute_connection_action
                    info!(
                        "Successfully connected to user {} and initiated action",
                        user_id
                    );
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
        Ok(())
    }
    /// Handle incoming connection request
    async fn handle_connection_request(
        client: &Arc<Mutex<WsClient>>,
        p2p_service: &Arc<P2PService>,
        target_connection_id: &str,
        user: &Arc<Mutex<Option<String>>>,
    ) -> Result<(), String> {
        // Start P2P listener
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

    pub async fn request_user_connection_notifications(
        &self,
        user_ids: Vec<String>,
    ) -> Result<(), String> {
        let client = self.client.lock().await;
        info!(
            "Requesting connection notifications for {} users",
            user_ids.len()
        );
        client.request_user_connection_notifications(user_ids).await
    }

    pub async fn initialize_live_editing(&self, shared_users: Vec<String>) -> Result<(), String> {
        info!(
            "Initializing live editing for {} shared users",
            shared_users.len()
        );

        // Clear previous live edit connections
        {
            let mut live_edit = self.live_edit_connections.lock().await;
            live_edit.clear();
        }

        // Process each shared user
        for user_device_id in shared_users {
            // First check if connection already exists
            let connection_exists =
                match self.p2p_service.get_connection_by_id(&user_device_id).await {
                    Ok(connection) => {
                        let connection_id = connection.get_id();
                        connection
                            .event_emitter
                            .emit(p2p_service::p2p::P2PEvent::LiveEditConnected { connection_id });
                        info!("Connection already exists for {}", user_device_id);
                        true
                    }
                    Err(_) => false,
                };

            // If connection doesn't exist, mark for live edit and request connection
            if !connection_exists {
                // Mark for live editing
                {
                    let mut live_edit = self.live_edit_connections.lock().await;
                    live_edit.insert(user_device_id.clone());
                }

                // Request connection to this user:device
                info!("Requesting connection for live editing: {}", user_device_id);
                if let Err(e) = self.request_user_connection(&user_device_id).await {
                    error!(
                        "Failed to request connection to {} for live editing: {}",
                        user_device_id, e
                    );
                    // Continue with other users even if one fails
                }
            } else {
                // Even if connection exists, we still want to mark it for live edit
                // in case it gets disconnected and reconnects
                let mut live_edit = self.live_edit_connections.lock().await;
                live_edit.insert(user_device_id.clone());
                info!(
                    "Marked existing connection for live editing: {}",
                    user_device_id
                );
            }
        }

        Ok(())
    }
}
