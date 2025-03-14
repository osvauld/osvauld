use crate::ws::{UserConnectionStatus, WsClient, WsMessage};
use log::{debug, error, info};
use osvauld_core::models::p2p::ConnectionType;
use osvauld_core::models::user::User; // Import the User model
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
    user: Arc<Mutex<Option<User>>>, // Store the entire User object
}

impl RendezvousService {
    /// Create a new RendezvousService instance
    pub fn new(p2p_service: Arc<P2PService>, ws_url: &str) -> Self {
        Self {
            client: Arc::new(Mutex::new(WsClient::new())),
            p2p_service,
            ws_url: ws_url.to_string(),
            pending_first_connections: Arc::new(Mutex::new(HashSet::new())),
            user: Arc::new(Mutex::new(None)), // Initialize with None
        }
    }

    /// Initialize the service with the provided user
    pub async fn initialize(&self, user: User) -> Result<(), String> {
        let user_id = user.id.clone(); // Get user_id from the User object

        // Store the user in the service state
        {
            let mut user_lock = self.user.lock().await;
            *user_lock = Some(user);
        }

        // Connect to the rendezvous server with the user_id
        self.connect_and_register(&user_id).await?;

        // Create clones of the fields we need in the async task
        let client = self.client.clone();
        let p2p_service = self.p2p_service.clone();
        let pending_first_connections = self.pending_first_connections.clone();
        let user_arc = self.user.clone(); // Pass the stored user

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
        user: Arc<Mutex<Option<User>>>, // Replace user_service with user
    ) {
        let mut receiver = {
            let client_lock = client.lock().await;
            client_lock.subscribe()
        };
        loop {
            match receiver.recv().await {
                // Process message based on its type
                Ok(msg) => {
                    match msg {
                        WsMessage::RequestConnection {
                            target_connection_id,
                        } => {
                            info!("Received connection request from: {}", target_connection_id);

                            // Handle connection request
                            if let Err(e) = Self::handle_connection_request(
                                &client,
                                &p2p_service,
                                &target_connection_id,
                                &user, // Pass the user reference
                            )
                            .await
                            {
                                error!("Failed to handle connection request: {}", e);
                            }
                        }
                        WsMessage::UserConnectionStringResponse {
                            user_id: response_user_id,
                            connection_string,
                            connection_status,
                        } => {
                            info!(
                                "Received connection string response from user {}, status: {}",
                                response_user_id, connection_status
                            );

                            // Process connection string if available
                            if let Some(conn_string) = connection_string {
                                if !conn_string.is_empty() {
                                    // Check if this is a first connection
                                    let mut pending = pending_first_connections.lock().await;
                                    let is_first_connection = pending.contains(&response_user_id);

                                    if is_first_connection {
                                        // Remove from pending set
                                        pending.remove(&response_user_id);

                                        // Get current user for first connection process
                                        let maybe_user = {
                                            let user_lock = user.lock().await;
                                            user_lock.clone()
                                        };

                                        // Handle first connection process
                                        if let Some(current_user) = maybe_user {
                                            let p2p_service_clone = p2p_service.clone();
                                            let ticket = conn_string.clone();
                                            let response_user_id = response_user_id.clone();
                                            let user_clone = current_user.clone();

                                            tokio::spawn(async move {
                                                // Connect with the ticket
                                                match p2p_service_clone
                                                    .connect_with_ticket(
                                                        &ticket,
                                                        ConnectionType::User,
                                                    )
                                                    .await
                                                {
                                                    Ok(_) => {
                                                        // Initiate first connection
                                                        match p2p_service_clone
                                                            .initiate_first_user_connection(
                                                                &user_clone,
                                                                &ticket,
                                                            )
                                                            .await
                                                        {
                                                            Ok(_) => {
                                                                info!(
                                                                    "Successfully initiated first connection with user: {}",
                                                                    response_user_id
                                                                );
                                                            }
                                                            Err(e) => {
                                                                error!(
                                                                    "Failed to initiate first user connection: {}",
                                                                    e
                                                                );
                                                            }
                                                        }
                                                    }
                                                    Err(e) => {
                                                        error!(
                                                            "Failed to connect with ticket: {}",
                                                            e
                                                        );
                                                    }
                                                }
                                            });
                                        } else {
                                            error!(
                                                "User not available for first connection process"
                                            );
                                        }
                                    } else {
                                        // Regular connection
                                        info!(
                                            "Handling regular connection with ticket: {}",
                                            conn_string
                                        );

                                        // Connect using the connection string
                                        tokio::spawn({
                                            let p2p_service = p2p_service.clone();
                                            let ticket = conn_string.clone();

                                            async move {
                                                if let Err(e) = p2p_service
                                                    .connect_with_ticket(
                                                        &ticket,
                                                        ConnectionType::User,
                                                    )
                                                    .await
                                                {
                                                    error!("Failed to connect with ticket: {}", e);
                                                } else {
                                                    info!(
                                                        "Successfully connected to peer using ticket"
                                                    );
                                                }
                                            }
                                        });
                                    }
                                }
                            }
                        }
                        // Handle other message types
                        _ => {
                            debug!("Received other message: {:?}", msg);
                        }
                    }
                }
                Err(e) => {
                    error!("Error receiving message: {}", e);
                    // Check if it's a lagged error or connection closed
                    match e {
                        tokio::sync::broadcast::error::RecvError::Closed => {
                            info!("WebSocket connection closed, stopping message handler");
                            break;
                        }
                        tokio::sync::broadcast::error::RecvError::Lagged(n) => {
                            info!("Receiver lagged behind by {} messages", n);
                            // Continue receiving
                        }
                    }
                }
            }
        }
    }

    /// Handle incoming connection request
    async fn handle_connection_request(
        client: &Arc<Mutex<WsClient>>,
        p2p_service: &Arc<P2PService>,
        target_connection_id: &str,
        user: &Arc<Mutex<Option<User>>>,
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
                Some(user) => user.id.clone(),
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
}
