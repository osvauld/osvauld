use axum::{
    extract::ws::{Message, WebSocket},
    response::IntoResponse,
    Extension,
};
use serde::{Deserialize, Serialize};
use serde_json;
use std::sync::Arc;
use crate::storage::Storage;

#[derive(Deserialize)]
struct ClientMessage {
    id: String,
    connection_string: String,
}

#[derive(Serialize)]
struct ClientInfo {
    id: String,
    connection_string: String,
}

#[derive(Serialize)]
struct ClientsResponse {
    clients: Vec<ClientInfo>,
}

pub async fn ws_handler(
    ws: axum::extract::ws::WebSocketUpgrade,
    Extension(storage): Extension<Arc<Storage>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, storage))
}

async fn handle_socket(mut socket: WebSocket, storage: Arc<Storage>) {
    while let Some(msg) = socket.recv().await {
        if let Ok(msg) = msg {
            if let Ok(text) = msg.to_text() {
                // Use helper to process the incoming message.
                if let Err(e) = process_and_respond(text, &mut socket, storage.clone()).await {
                    eprintln!("Error processing message: {}", e);
                }
            }
        }
    }
}


async fn process_and_respond(
    text: &str,
    socket: &mut WebSocket,
    storage: Arc<Storage>,
) -> Result<(), Box<dyn std::error::Error>> {
    let client_msg = parse_client_message(text)?;
    save_client_info(&client_msg, storage.clone()).await?;
    let response = build_clients_response(&client_msg, storage.clone()).await?;
    send_response(socket, &response).await?;
    Ok(())
}

fn parse_client_message(text: &str) -> Result<ClientMessage, serde_json::Error> {
    serde_json::from_str(text)
}

async fn save_client_info(
    client_msg: &ClientMessage,
    storage: Arc<Storage>,
) -> Result<(), Box<dyn std::error::Error>> {
    storage
        .save_client(&client_msg.id, &client_msg.connection_string)
        .await
        .map_err(|e| e.into())
}


async fn build_clients_response(
    client_msg: &ClientMessage,
    storage: Arc<Storage>,
) -> Result<ClientsResponse, Box<dyn std::error::Error>> {
    let clients = storage.get_all_clients().await?;
    let filtered_clients: Vec<ClientInfo> = clients
        .into_iter()
        .filter(|(id, _)| id != &client_msg.id)
        .map(|(id, connection_string)| ClientInfo { id, connection_string })
        .collect();
    Ok(ClientsResponse {
        clients: filtered_clients,
    })
}

async fn send_response(
    socket: &mut WebSocket,
    response: &ClientsResponse,
) -> Result<(), Box<dyn std::error::Error>> {
    let json = serde_json::to_string(response)?;
    socket.send(Message::Text(json)).await.map_err(|e| e.into())
}
