use crate::storage::Storage;
use axum::{extract::ws::Message, routing::get, Router};
use std::time::Duration;
use std::{collections::HashMap, sync::Arc, time::Instant};
use tokio::sync::{mpsc, Mutex};
mod storage;
mod websocket;

// Import the types from websocket.rs or define them again
type ClientInfo = (mpsc::UnboundedSender<Message>, Instant);
type Clients = Arc<Mutex<HashMap<String, ClientInfo>>>;

// Define the user client mapping type
type UserClientMappings = Arc<Mutex<Vec<websocket::UserClientMapping>>>;

#[tokio::main]
async fn main() {
    let storage = Arc::new(Storage::new("clients_db").unwrap());
    let clients: Clients = Arc::new(Mutex::new(HashMap::new()));
    let user_mappings: UserClientMappings = Arc::new(Mutex::new(Vec::new()));

    // Configure connection monitoring
    let check_interval = Duration::from_secs(60); // Check every minute
    let timeout = Duration::from_secs(300); // 5 minute timeout

    // Start the connection monitor
    tokio::spawn(websocket::start_connection_monitor(
        clients.clone(),
        storage.clone(),
        user_mappings.clone(),
        check_interval,
        timeout,
    ));

    let app = Router::new()
        .route("/ws", get(websocket::ws_handler))
        .layer(axum::Extension(storage.clone()))
        .layer(axum::Extension(clients.clone()))
        .layer(axum::Extension(user_mappings.clone())); // Add user_mappings to the extensions

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3030").await.unwrap();
    println!("WebSocket server running at ws://0.0.0.0:3030/ws");

    axum::serve(listener, app).await.unwrap();
}
