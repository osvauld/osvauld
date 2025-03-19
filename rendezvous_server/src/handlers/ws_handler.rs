use crate::error::AppError;
use crate::models::{ClientInfo, Clients, UserClientMapping, UserClientMappings, WsMessage};
use crate::services::connection_service::{self, ConnectionStatus};
use crate::storage::Storage;
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

pub async fn simple_ws_handler(ws: WebSocketUpgrade) -> impl IntoResponse {
    tracing::info!("Simple WebSocket connection request received");

    ws.on_upgrade(|socket| async move {
        tracing::info!("Simple WebSocket upgrade successful");

        let (mut sender, mut receiver) = socket.split();

        // Echo incoming messages
        while let Some(Ok(message)) = receiver.next().await {
            tracing::info!("Received message: {:?}", message);
            if sender.send(message).await.is_err() {
                break;
            }
        }

        tracing::info!("Simple WebSocket connection closed");
    })
}

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
        // Update last activity timestamp
        if let Some(client_info) = handler.clients.lock().await.get_mut(&handler.client_id) {
            client_info.last_activity = Instant::now();
        }

        if let Ok(text) = message.to_text() {
            info!("Received message: {}", text);

            match serde_json::from_str::<WsMessage>(text) {
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
        WsMessage::RequestConnection { .. }
        | WsMessage::UserConnectionStringResponse { .. }
        | WsMessage::GetConnectionStatusResponse { .. } => {
            info!("Received server-side message, ignoring");
            Ok(())
        }
    }
}

async fn handle_register(user_id: String, handler: &ClientHandler) -> Result<(), AppError> {
    info!("Registering user: {}", user_id);

    handler.user_mappings.lock().await.push(UserClientMapping {
        user_id: user_id.clone(),
        client_id: handler.client_id.clone(),
    });

    handler
        .storage
        .save_client(&user_id, &handler.client_id)
        .await?;
    handler
        .storage
        .update_connection_status(&user_id, ConnectionStatus::Online)
        .await?;

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

    match handler
        .storage
        .get_ws_connection_id(&target_user_id)
        .await?
    {
        Some(connection_id) => {
            let request = WsMessage::RequestConnection {
                target_connection_id: handler.client_id.clone(),
            };

            match connection_service::send_message(&handler.clients, &connection_id, request).await
            {
                Ok(_) => {
                    info!("Connection request sent to user: {}", target_user_id);
                    Ok(())
                }
                Err(_) => {
                    connection_service::send_offline_response(
                        target_user_id,
                        handler.client_id.clone(),
                        handler.clients.clone(),
                    )
                    .await
                }
            }
        }
        None => {
            info!("No active connection found for user: {}", target_user_id);
            connection_service::send_offline_response(
                target_user_id,
                handler.client_id.clone(),
                handler.clients.clone(),
            )
            .await
        }
    }
}

async fn handle_get_all_clients(handler: &ClientHandler) -> Result<(), AppError> {
    match handler.storage.get_all_clients().await {
        Ok(clients) => {
            info!("Clients: {:#?}", clients);
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
