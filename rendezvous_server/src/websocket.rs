use crate::storage::Storage;
use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::IntoResponse,
    Extension,
};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{mpsc, Mutex};
use uuid::Uuid;

type Tx = mpsc::UnboundedSender<Message>;
type Clients = Arc<Mutex<HashMap<String, Tx>>>;

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "action", content = "payload")]
enum WsMessage {
    Register {
        user_id: String,
    },
    ConnectionResponse {
        user_id: String,
        connection_string: String,
    },
    GetConnectionRequest,
    GetConnectionStringRequest {
        user_id: String,
    },
    ConnectionStringResponse {
        user_id: String,
        connection_string: Option<String>,
    },
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Extension(storage): Extension<Arc<Storage>>,
    Extension(clients): Extension<Clients>,
) -> impl IntoResponse {
    println!("New WebSocket connection request");
    ws.on_upgrade(|socket| handle_socket(socket, storage, clients))
}

async fn handle_socket(socket: WebSocket, storage: Arc<Storage>, clients: Clients) {
    println!("WebSocket connection established");
    let client_id = Uuid::new_v4().to_string();
    println!("Generated client ID: {}", client_id);

    let (tx, mut rx) = mpsc::unbounded_channel();
    clients.lock().await.insert(client_id.clone(), tx);

    let (mut sender, mut receiver) = socket.split();

    let recv_client_id = client_id.clone();
    let recv_storage = storage.clone();
    let recv_clients = clients.clone();

    let mut send_task = tokio::spawn(async move {
        while let Some(message) = rx.recv().await {
            if sender.send(message).await.is_err() {
                break;
            }
        }
    });

    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(message)) = receiver.next().await {
            if let Ok(text) = message.to_text() {
                println!("Received message: {}", text);

                match serde_json::from_str::<WsMessage>(text) {
                    Ok(ws_message) => match ws_message {
                        WsMessage::Register { user_id } => {
                            println!("Registering user: {}", user_id);
                            if let Err(e) =
                                recv_storage.save_client(&user_id, &recv_client_id).await
                            {
                                eprintln!("Error saving client: {}", e);
                            }
                        }
                        WsMessage::ConnectionResponse {
                            user_id,
                            connection_string,
                        } => {
                            println!("Saving connection string for user: {}", user_id);
                            if let Err(e) = recv_storage
                                .save_connection_string(&user_id, &connection_string)
                                .await
                            {
                                eprintln!("Error saving connection string: {}", e);
                            }
                        }
                        WsMessage::GetConnectionStringRequest { user_id } => {
                            println!("Fetching connection string for user: {}", user_id);
                            let connection_string = match recv_storage
                                .get_connection_string(&user_id)
                                .await
                            {
                                Ok(Some(cs)) => Some(cs),
                                Ok(None) => {
                                    println!("No connection string found for user: {}", user_id);
                                    None
                                }
                                Err(e) => {
                                    eprintln!("Error fetching connection string: {}", e);
                                    None
                                }
                            };

                            let response = WsMessage::ConnectionStringResponse {
                                user_id: user_id.clone(),
                                connection_string,
                            };

                            if let Ok(json) = serde_json::to_string(&response) {
                                if let Some(tx) = recv_clients.lock().await.get(&recv_client_id) {
                                    if let Err(e) = tx.send(Message::Text(json.into())) {
                                        eprintln!(
                                            "Error sending connection string response: {}",
                                            e
                                        );
                                    }
                                }
                            }
                        }
                        WsMessage::GetConnectionRequest => {
                            println!(
                                "Received GetConnectionRequest - this is a server-side message"
                            );
                        }
                        WsMessage::ConnectionStringResponse { .. } => {
                            println!(
                                "Received ConnectionStringResponse - this is a server-side message"
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
}

pub async fn request_connection(
    clients: &Clients,
    client_id: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let clients_map = clients.lock().await;

    if let Some(tx) = clients_map.get(client_id) {
        let msg = WsMessage::GetConnectionRequest;
        let json = serde_json::to_string(&msg)?;
        tx.send(Message::Text(json.into()))?;
        Ok(())
    } else {
        Err("Client not found".into())
    }
}
