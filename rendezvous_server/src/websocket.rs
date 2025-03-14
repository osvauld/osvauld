use crate::storage::{ConnectionStatus, Storage};
use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::IntoResponse,
    Extension,
};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{mpsc, Mutex};
use tokio::time;
use uuid::Uuid;

// Change type definition to include timestamp
type ClientInfo = (mpsc::UnboundedSender<Message>, Instant);
type Clients = Arc<Mutex<HashMap<String, ClientInfo>>>;

pub struct UserClientMapping {
    user_id: String,
    client_id: String,
}
#[derive(Serialize, Debug, Deserialize)]
struct ClientStatus {
    user_id: String,
    connection_status: String,
}
type UserClientMappings = Arc<Mutex<Vec<UserClientMapping>>>;

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "action", content = "payload")]
enum WsMessage {
    Register {
        user_id: String,
    },
    RequestConnection {
        target_connection_id: String,
    },
    ConnectionResponse {
        user_id: String,
        target_connection_id: String,
        connection_string: String,
    },
    RequestUserConnectionString {
        target_user_id: String,
    },
    UserConnectionStringResponse {
        user_id: String,
        connection_string: Option<String>,
        connection_status: String,
    },
    GetAllClients,
    GetConnectionStatus {
        user_ids: Vec<String>,
    },
    GetConnectionStatusResponse {
        data: Vec<ClientStatus>,
    },
}

pub async fn start_connection_monitor(
    clients: Clients,
    storage: Arc<Storage>,
    user_mappings: UserClientMappings,
    check_interval: Duration,
    timeout: Duration,
) {
    let mut interval = time::interval(check_interval);

    println!("Starting websocket connection monitor");

    loop {
        interval.tick().await;
        check_connections(
            clients.clone(),
            storage.clone(),
            user_mappings.clone(),
            timeout,
        )
        .await;
    }
}

async fn check_connections(
    clients: Clients,
    storage: Arc<Storage>,
    user_mappings: UserClientMappings,
    timeout: Duration,
) {
    println!("Checking connection status for all clients");

    let now = Instant::now();
    let mut disconnected_clients = Vec::new();

    {
        let clients_lock = clients.lock().await;
        for (client_id, (_, last_activity)) in clients_lock.iter() {
            if now.duration_since(*last_activity) > timeout {
                disconnected_clients.push(client_id.clone());
            }
        }
    }

    if !disconnected_clients.is_empty() {
        println!("Found {} inactive connections", disconnected_clients.len());

        let mappings_lock = user_mappings.lock().await;

        for client_id in disconnected_clients {
            if let Some(mapping) = mappings_lock.iter().find(|m| m.client_id == client_id) {
                if let Err(e) = storage
                    .update_connection_status(&mapping.user_id, ConnectionStatus::Offline)
                    .await
                {
                    eprintln!("Error updating connection status: {}", e);
                } else {
                    println!("Marked user {} as offline", mapping.user_id);
                }
            }
        }
    }
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Extension(storage): Extension<Arc<Storage>>,
    Extension(clients): Extension<Clients>,
    Extension(user_mappings): Extension<UserClientMappings>,
) -> impl IntoResponse {
    println!("New WebSocket connection request");
    ws.on_upgrade(|socket| handle_socket(socket, storage, clients, user_mappings))
}

async fn handle_socket(
    socket: WebSocket,
    storage: Arc<Storage>,
    clients: Clients,
    user_mappings: UserClientMappings,
) {
    println!("WebSocket connection established");
    let client_id = Uuid::new_v4().to_string();
    println!("Generated client ID: {}", client_id);

    let (tx, mut rx) = mpsc::unbounded_channel();
    // Insert client info with the current timestamp
    clients
        .lock()
        .await
        .insert(client_id.clone(), (tx, Instant::now()));

    let (mut sender, mut receiver) = socket.split();

    let recv_client_id = client_id.clone();
    let recv_storage = storage.clone();
    let recv_clients = clients.clone();
    let recv_user_mappings = user_mappings.clone(); // Fix: Clone user_mappings

    let mut send_task = tokio::spawn(async move {
        while let Some(message) = rx.recv().await {
            if sender.send(message).await.is_err() {
                break;
            }
        }
    });

    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(message)) = receiver.next().await {
            if let Some((_, timestamp)) = recv_clients.lock().await.get_mut(&recv_client_id) {
                *timestamp = Instant::now();
            }
            if let Ok(text) = message.to_text() {
                println!("Received message: {}", text);

                match serde_json::from_str::<WsMessage>(text) {
                    Ok(ws_message) => match ws_message {
                        WsMessage::Register { user_id } => {
                            println!("Registering user: {}", user_id);
                            recv_user_mappings.lock().await.push(UserClientMapping {
                                user_id: user_id.clone(),
                                client_id: recv_client_id.clone(),
                            });

                            if let Err(e) =
                                recv_storage.save_client(&user_id, &recv_client_id).await
                            {
                                eprintln!("Error saving client: {}", e);
                            }
                            if let Err(e) = recv_storage
                                .update_connection_status(&user_id, ConnectionStatus::Online)
                                .await
                            {
                                eprintln!("Error updating connection status: {}", e);
                            }
                        }
                        WsMessage::ConnectionResponse {
                            user_id,
                            target_connection_id,
                            connection_string,
                        } => {
                            println!("Saving connection string for user: {}", user_id);
                            if let Err(e) = recv_storage
                                .save_connection_string(&user_id, &connection_string)
                                .await
                            {
                                eprintln!("Error saving connection string: {}", e);
                            }
                            let response = WsMessage::UserConnectionStringResponse {
                                user_id: user_id.clone(),
                                connection_string: Some(connection_string.clone()),
                                connection_status: ConnectionStatus::Online.as_str().to_string(),
                            };

                            if let Ok(json) = serde_json::to_string(&response) {
                                // Get the sender channel from the client info tuple
                                if let Some((tx, _)) =
                                    recv_clients.lock().await.get(&target_connection_id)
                                {
                                    if let Err(e) = tx.send(Message::Text(json.into())) {
                                        eprintln!(
                                            "Error sending connection string response: {}",
                                            e
                                        );
                                    }
                                }
                            }
                        }
                        WsMessage::RequestUserConnectionString { target_user_id } => {
                            println!("Fetching connection string for user: {}", target_user_id);

                            let source_ws_connection_id =
                                match recv_storage.get_ws_connection_id(&target_user_id).await {
                                    Ok(Some(ws_c_id)) => Some(ws_c_id),
                                    Ok(None) => {
                                        println!(
                                            "No ws connection id found for user: {}",
                                            target_user_id
                                        );
                                        None
                                    }
                                    Err(e) => {
                                        eprintln!("Error fetching connection string: {}", e);
                                        None
                                    }
                                };

                            match source_ws_connection_id {
                                Some(connection_id) => {
                                    let get_connection_string_request =
                                        WsMessage::RequestConnection {
                                            target_connection_id: recv_client_id.clone(),
                                        };

                                    if let Ok(json) =
                                        serde_json::to_string(&get_connection_string_request)
                                    {
                                        match recv_clients.lock().await.get(&connection_id) {
                                            Some((tx, _)) => {
                                                if let Err(e) = tx.send(Message::Text(json.into()))
                                                {
                                                    eprintln!(
                                                        "Error sending connection request: {}",
                                                        e
                                                    );

                                                    send_offline_response(
                                                        target_user_id.clone(),
                                                        recv_client_id.clone(),
                                                        recv_clients.clone(),
                                                    )
                                                    .await;
                                                } else {
                                                    println!(
                                                        "Connection request sent to user: {}",
                                                        target_user_id
                                                    );
                                                }
                                            }
                                            None => {
                                                println!("Client {} exists in database but not in active clients map", connection_id);
                                                send_offline_response(
                                                    target_user_id.clone(),
                                                    recv_client_id.clone(),
                                                    recv_clients.clone(),
                                                )
                                                .await;
                                            }
                                        }
                                    } else {
                                        eprintln!("Error serializing connection request");
                                        send_offline_response(
                                            target_user_id.clone(),
                                            recv_client_id.clone(),
                                            recv_clients.clone(),
                                        )
                                        .await;
                                    }
                                }
                                None => {
                                    // User not found or error retrieving connection ID
                                    println!(
                                        "No active connection found for user: {}",
                                        target_user_id
                                    );
                                    send_offline_response(
                                        target_user_id.clone(),
                                        recv_client_id.clone(),
                                        recv_clients.clone(),
                                    )
                                    .await;
                                }
                            }
                        }
                        WsMessage::GetAllClients => {
                            let db_clients = match recv_storage.get_all_clients().await {
                                Ok(client_info) => Some(client_info),

                                Err(e) => {
                                    eprintln!("Error fetching connection string: {}", e);
                                    None
                                }
                            };
                            match db_clients {
                                Some(clients) => {
                                    println!("Clients: {:#?}", clients)
                                }
                                None => {
                                    println!("No values or error")
                                }
                            }
                        }
                        WsMessage::GetConnectionStatus { user_ids } => {
                            let db_clients = match recv_storage.get_all_clients().await {
                                Ok(client_info) => Some(client_info),

                                Err(e) => {
                                    eprintln!("Error fetching connection string: {}", e);
                                    None
                                }
                            };
                            let mut connection_statuses: Vec<ClientStatus> = Vec::new();
                            match db_clients {
                                Some(clients) => {
                                    for id in user_ids {
                                        if let Some(cl) = clients.iter().find(|c| &c.user_id == &id)
                                        {
                                            connection_statuses.push(ClientStatus {
                                                user_id: id,
                                                connection_status: cl
                                                    .connection_status
                                                    .as_str()
                                                    .to_string(),
                                            });
                                        } else {
                                            connection_statuses.push(ClientStatus {
                                                user_id: id,
                                                connection_status: ConnectionStatus::Offline
                                                    .as_str()
                                                    .to_string(),
                                            });
                                        }
                                    }
                                }
                                None => {
                                    for id in user_ids {
                                        connection_statuses.push(ClientStatus {
                                            user_id: id,
                                            connection_status: ConnectionStatus::Offline
                                                .as_str()
                                                .to_string(),
                                        });
                                    }
                                }
                            }

                            let response = WsMessage::GetConnectionStatusResponse {
                                data: connection_statuses,
                            };

                            if let Ok(json) = serde_json::to_string(&response) {
                                if let Some((tx, _)) =
                                    recv_clients.lock().await.get(&recv_client_id)
                                {
                                    if let Err(e) = tx.send(Message::Text(json.into())) {
                                        eprintln!(
                                            "Error sending connection status response: {}",
                                            e
                                        );
                                    } else {
                                        println!("Sent connection status response");
                                    }
                                }
                            }
                        }
                        WsMessage::RequestConnection { .. } => {
                            println!(
                                "Received GetConnectionRequest - this is a server-side message"
                            );
                        }
                        WsMessage::UserConnectionStringResponse { .. } => {
                            println!(
                                "Received ConnectionStringResponse - this is a server-side message"
                            );
                        }
                        WsMessage::GetConnectionStatusResponse { .. } => {
                            println!(
                                "Received GetConnectionStatusResponse - this is a server-side message"
                            );
                        }
                    },
                    Err(e) => {
                        eprintln!("Error parsing message: {}", e);
                    }
                }
            }
        }
    });

    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    }

    println!("Client {} disconnected", client_id);
    clients.lock().await.remove(&client_id);
    let user_mappings = user_mappings.lock().await;
    if let Some(mapping) = user_mappings.iter().find(|m| m.client_id == client_id) {
        if let Err(e) = storage
            .update_connection_status(&mapping.user_id, ConnectionStatus::Offline)
            .await
        {
            eprintln!("Error updating connection status on disconnect: {}", e);
        }
    }
}

// Define send_offline_response using an async closure
async fn send_offline_response(
    target_user_id: String,
    recv_client_id: String,
    recv_clients: Arc<Mutex<HashMap<String, ClientInfo>>>,
) {
    let response = WsMessage::UserConnectionStringResponse {
        user_id: target_user_id.clone(),
        connection_string: None,
        connection_status: ConnectionStatus::Offline.as_str().to_string(),
    };

    if let Ok(err_json) = serde_json::to_string(&response) {
        if let Some((err_tx, _)) = recv_clients.lock().await.get(&recv_client_id) {
            if let Err(e) = err_tx.send(Message::Text(err_json.into())) {
                eprintln!("Error sending offline response: {}", e);
            } else {
                println!("Sent offline status for user: {}", target_user_id);
            }
        }
    }
}
