use crate::error::AppError;
use crate::models::{ClientInfo, Clients, UserClientMapping, UserClientMappings, WsMessage};
use crate::services::connection_service::{self, ConnectionStatus};
use crate::storage::{DbClients, Storage};
use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::IntoResponse,
    Extension,
};
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc;
use tracing::log::{error, info};
use uuid::Uuid;

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Extension(storage): Extension<Arc<Storage>>,
    Extension(clients): Extension<Clients>,
    Extension(user_mappings): Extension<UserClientMappings>,
) -> impl IntoResponse {
    info!("New WebSocket connection request");
    ws.on_upgrade(|socket| handle_socket(socket, storage, clients, user_mappings))
}

async fn handle_socket(
    socket: WebSocket,
    storage: Arc<Storage>,
    clients: Clients,
    user_mappings: UserClientMappings,
) {
    info!("WebSocket connection established");
    let client_id = Uuid::new_v4().to_string();
    info!("Generated client ID: {}", client_id);

    let (tx, rx) = mpsc::unbounded_channel();
    clients.lock().await.insert(
        client_id.clone(),
        ClientInfo {
            sender: tx,
            last_activity: Instant::now(),
        },
    );

    let (mut sender, mut receiver) = socket.split();

    let client_handler = ClientHandler {
        client_id: client_id.clone(),
        storage: storage.clone(),
        clients: clients.clone(),
        user_mappings: user_mappings.clone(),
    };

    let mut send_task = tokio::spawn(async move {
        handle_outgoing_messages(rx, &mut sender).await;
    });

    let mut recv_task = tokio::spawn(async move {
        handle_incoming_messages(&mut receiver, client_handler).await;
    });

    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    }

    handle_disconnect(client_id, clients, user_mappings, storage).await;
}

async fn handle_outgoing_messages(
    mut rx: mpsc::UnboundedReceiver<Message>,
    sender: &mut futures_util::stream::SplitSink<WebSocket, Message>,
) {
    while let Some(message) = rx.recv().await {
        if sender.send(message).await.is_err() {
            break;
        }
    }
}

struct ClientHandler {
    client_id: String,
    storage: Arc<Storage>,
    clients: Clients,
    user_mappings: UserClientMappings,
}

async fn handle_incoming_messages(
    receiver: &mut futures_util::stream::SplitStream<WebSocket>,
    handler: ClientHandler,
) {
    while let Some(Ok(message)) = receiver.next().await {
        // Update last activity timestamp for client for any message type
        if let Some(client_info) = handler.clients.lock().await.get_mut(&handler.client_id) {
            client_info.last_activity = Instant::now();
        }

        match message {
            Message::Text(text) => {
                info!("Received text message: {}", text);

                match serde_json::from_str::<WsMessage>(&text) {
                    Ok(ws_message) => {
                        if let Err(e) = process_message(ws_message, &handler).await {
                            error!("Error processing message: {}", e);
                        }
                    }
                    Err(e) => {
                        error!("Error parsing message: {}", e);
                    }
                }
            }
            Message::Ping(data) => {
                // Handle WebSocket protocol ping frame
                if !data.is_empty() {
                    // Try to parse the timestamp data if present
                    match std::str::from_utf8(&data) {
                        Ok(ping_data_str) => {
                            info!(
                                "Received WebSocket ping from client {} with data: {}",
                                handler.client_id, ping_data_str
                            );
                        }
                        Err(_) => {
                            info!(
                                "Received WebSocket ping from client {} (non-UTF8 data)",
                                handler.client_id
                            );
                        }
                    }
                } else {
                    info!("Received WebSocket ping from client {}", handler.client_id);
                }

                // Automatically respond with a pong containing the same data
                if let Some(client_info) = handler.clients.lock().await.get_mut(&handler.client_id)
                {
                    if let Err(e) = client_info.sender.send(Message::Pong(data)) {
                        error!("Failed to send pong response: {}", e);
                    } else {
                        info!("Sent pong response to client {}", handler.client_id);
                    }
                }
            }
            Message::Pong(data) => {
                // Just log pong reception
                if !data.is_empty() {
                    match std::str::from_utf8(&data) {
                        Ok(pong_data_str) => {
                            info!(
                                "Received WebSocket pong from client {} with data: {}",
                                handler.client_id, pong_data_str
                            );
                        }
                        Err(_) => {
                            info!(
                                "Received WebSocket pong from client {} (non-UTF8 data)",
                                handler.client_id
                            );
                        }
                    }
                } else {
                    info!("Received WebSocket pong from client {}", handler.client_id);
                }
            }
            Message::Binary(data) => {
                info!("Received binary data of size: {} bytes", data.len());
            }
            Message::Close(frame) => {
                info!("Received close frame from client: {:?}", frame);
                break;
            }
        }
    }
}
async fn process_message(message: WsMessage, handler: &ClientHandler) -> Result<(), AppError> {
    match message {
        WsMessage::Register { user_id } => handle_register(user_id, handler).await,
        WsMessage::ConnectionResponse {
            user_id,
            target_connection_id,
            connection_string,
        } => {
            handle_connection_response(user_id, target_connection_id, connection_string, handler)
                .await
        }
        WsMessage::RequestUserConnectionString { target_user_id } => {
            handle_request_user_connection(target_user_id, handler).await
        }
        WsMessage::GetAllClients => handle_get_all_clients(handler).await,
        WsMessage::GetConnectionStatus { user_ids } => {
            handle_get_connection_status(user_ids, handler).await
        }
        WsMessage::UserConnectionNotificationRequest { user_ids } => {
            handle_connection_notification_request(handler, user_ids).await
        }
        WsMessage::RequestConnection { .. }
        | WsMessage::UserConnectionNotification { .. }
        | WsMessage::UserConnectionStringResponse { .. }
        | WsMessage::GetConnectionStatusResponse { .. } => {
            info!("Received server-side message, ignoring");
            Ok(())
        }
    }
}
async fn handle_ping(timestamp: i64, handler: &ClientHandler) -> Result<(), AppError> {
    // Update the client's last activity timestamp
    if let Some(client_info) = handler.clients.lock().await.get_mut(&handler.client_id) {
        client_info.last_activity = Instant::now();
    }

    // Log ping reception at debug level to avoid too much noise in logs
    info!(
        "Received ping from client {} with timestamp {}",
        handler.client_id, timestamp
    );

    // For now, just acknowledge the ping without sending a response
    Ok(())
}

async fn handle_register(user_id: String, handler: &ClientHandler) -> Result<(), AppError> {
    info!("Registering user: {}", user_id);

    handler.user_mappings.lock().await.push(UserClientMapping {
        user_id: user_id.clone(),
        client_id: handler.client_id.clone(),
    });

    match handler.storage.get_client_details(&user_id).await? {
        Some(client_info) => {
            send_client_connection_notification(
                handler,
                &user_id,
                client_info.connection_requested,
            )
            .await?;
            let db_client = DbClients {
                user_id,
                ws_connection_id: handler.client_id.clone(),
                connection_string: None,
                connection_status: ConnectionStatus::Online,
                connection_requested: Vec::new(),
            };
            handler.storage.update_client_info(&db_client).await?
        }
        None => {
            handler
                .storage
                .save_client(&user_id, &handler.client_id)
                .await?;
            handler
                .storage
                .update_connection_status(&user_id, ConnectionStatus::Online)
                .await?;
        }
    }

    Ok(())
}

async fn send_client_connection_notification(
    handler: &ClientHandler,
    user_id: &str,
    requested_by: Vec<String>,
) -> Result<(), AppError> {
    info!(
        "Sending connection notifications to user: {:?}",
        requested_by
    );

    let mappings = handler.user_mappings.lock().await;

    for id in requested_by {
        if let Some(ws_client_id) = mappings
            .iter()
            .find(|mapping| mapping.user_id == id.clone())
            .map(|mapping| mapping.client_id.clone())
        {
            let message = WsMessage::UserConnectionNotification {
                online_user_id: user_id.to_owned(),
            };
            if let Err(_err) =
                connection_service::send_message(&handler.clients, &ws_client_id, message).await
            {
                error!("Error sending connection notification to: {}", id)
            }
        }
    }
    Ok(())
}

async fn handle_connection_notification_request(
    handler: &ClientHandler,
    user_ids: Vec<String>,
) -> Result<(), AppError> {
    info!("Saving connection notification request");
    for id in user_ids {
        handle_request_user_connection(id, handler).await?;
    }
    Ok(())
}

async fn handle_connection_response(
    user_id: String,
    target_connection_id: String,
    connection_string: String,
    handler: &ClientHandler,
) -> Result<(), AppError> {
    info!("Saving connection string for user: {}", user_id);

    handler
        .storage
        .save_connection_string(&user_id, &connection_string)
        .await?;

    let response = WsMessage::UserConnectionStringResponse {
        user_id: user_id.clone(),
        connection_string: Some(connection_string),
        connection_status: ConnectionStatus::Online.as_str().to_string(),
    };

    connection_service::send_message(&handler.clients, &target_connection_id, response).await
}

async fn handle_request_user_connection(
    target_user_id: String,
    handler: &ClientHandler,
) -> Result<(), AppError> {
    info!("Fetching connection string for user: {}", target_user_id);

    match handler.storage.get_client_details(&target_user_id).await? {
        Some(client_details) => match client_details.connection_status {
            ConnectionStatus::Online => {
                let request = WsMessage::RequestConnection {
                    target_connection_id: handler.client_id.clone(),
                };

                match connection_service::send_message(
                    &handler.clients,
                    &client_details.ws_connection_id,
                    request,
                )
                .await
                {
                    Ok(_) => {
                        info!("Connection request sent to user: {}", target_user_id);
                        Ok(())
                    }
                    Err(_) => {
                        handle_offline_connection_request(handler, &target_user_id).await?;
                        Ok(())
                    }
                }
            }
            ConnectionStatus::Offline => {
                info!("No active connection found for user: {}", target_user_id);
                handle_offline_connection_request(handler, &target_user_id).await?;
                Ok(())
            }
        },
        None => {
            info!("No active connection found for user: {}", target_user_id);
            handle_offline_connection_request(handler, &target_user_id).await?;
            Ok(())
        }
    }
}

async fn handle_offline_connection_request(
    handler: &ClientHandler,
    user_id: &str,
) -> Result<(), AppError> {
    info!("Updating connection requested by for: {}", user_id);
    let mappings = handler.user_mappings.lock().await;
    if let Some(requested_by) = mappings
        .iter()
        .find(|mapping| mapping.client_id == handler.client_id)
        .map(|mapping| mapping.user_id.clone())
    {
        if let Err(_) = handler
            .storage
            .update_connection_requested_by(user_id, &requested_by)
            .await
        {
            handler
                .storage
                .create_new_user_for_connection_request(user_id, &requested_by)
                .await?;
        }
    }
    Ok(())
}

async fn handle_get_all_clients(handler: &ClientHandler) -> Result<(), AppError> {
    match handler.storage.get_all_clients_detailes().await {
        Ok(clients) => {
            info!("Clients: {:#?}", clients);
            info!("UserMappings: {:#?}", handler.user_mappings);
            Ok(())
        }
        Err(e) => {
            error!("Error fetching clients: {}", e);
            Err(AppError::WebSocketError(format!(
                "Client {} not found",
                handler.client_id
            )))
        }
    }
}

async fn handle_get_connection_status(
    user_ids: Vec<String>,
    handler: &ClientHandler,
) -> Result<(), AppError> {
    let db_clients = match handler.storage.get_all_clients().await {
        Ok(clients) => Some(clients),
        Err(e) => {
            error!("Error fetching connection status: {}", e);
            None
        }
    };

    let connection_statuses = match db_clients {
        Some(clients) => user_ids
            .into_iter()
            .map(|id| match clients.iter().find(|c| c.user_id == id) {
                Some(client) => crate::models::ClientStatus {
                    user_id: id,
                    connection_status: client.connection_status.as_str().into(),
                },
                None => crate::models::ClientStatus {
                    user_id: id,
                    connection_status: ConnectionStatus::Offline.as_str().to_string(),
                },
            })
            .collect(),
        None => user_ids
            .into_iter()
            .map(|id| crate::models::ClientStatus {
                user_id: id,
                connection_status: ConnectionStatus::Offline.as_str().to_string(),
            })
            .collect(),
    };

    let response = WsMessage::GetConnectionStatusResponse {
        data: connection_statuses,
    };

    match connection_service::send_message(&handler.clients, &handler.client_id, response).await {
        Ok(_) => {
            info!("Sent connection status response");
            Ok(())
        }
        Err(e) => {
            error!("Error sending connection status response: {}", e);
            Err(e)
        }
    }
}

async fn handle_disconnect(
    client_id: String,
    clients: Clients,
    user_mappings: UserClientMappings,
    storage: Arc<Storage>,
) {
    info!("Client {} disconnected", client_id);
    clients.lock().await.remove(&client_id);

    let user_mappings = user_mappings.lock().await;
    if let Some(mapping) = user_mappings.iter().find(|m| m.client_id == client_id) {
        if let Err(e) = storage
            .update_connection_status(&mapping.user_id, ConnectionStatus::Offline)
            .await
        {
            error!("Error updating connection status on disconnect: {}", e);
        }
    }
}
