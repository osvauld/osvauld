use crate::storage::Storage;
use axum::{extract::ws::Message, routing::get, Router};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{mpsc, Mutex};
mod storage;
mod websocket;

type Clients = Arc<Mutex<HashMap<String, mpsc::UnboundedSender<Message>>>>;

#[tokio::main]
async fn main() {
    let storage = Arc::new(Storage::new("clients_db").unwrap());
    let clients: Clients = Arc::new(Mutex::new(HashMap::new()));

    let app = Router::new()
        .route("/ws", get(websocket::ws_handler))
        .layer(axum::Extension(storage.clone()))
        .layer(axum::Extension(clients.clone()));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3030").await.unwrap();
    println!("WebSocket server running at ws://0.0.0.0:3030/ws");

    axum::serve(listener, app).await.unwrap();
}
