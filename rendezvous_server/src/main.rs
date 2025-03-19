use axum::{routing::get, Router};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

// Import modules
mod error;
mod handlers;
mod models;
mod services;
mod storage;

use handlers::ws_handler::{simple_ws_handler, ws_handler};
use models::{Clients, UserClientMappings};
use services::connection_monitor::start_connection_monitor;
use storage::Storage;

#[tokio::main]
async fn main() {
    // Initialize tracing for better logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    // Initialize storage
    let storage = Arc::new(Storage::new("clients_db").unwrap());

    // Initialize shared state
    let clients: Clients = Arc::new(Mutex::new(HashMap::new()));
    let user_mappings: UserClientMappings = Arc::new(Mutex::new(Vec::new()));

    // Configure connection monitoring
    let check_interval = Duration::from_secs(60); // Check every minute
    let timeout = Duration::from_secs(300); // 5 minute timeout

    // Start the connection monitor as a background task
    let monitor_storage = storage.clone();
    let monitor_clients = clients.clone();
    let monitor_mappings = user_mappings.clone();

    tokio::spawn(async move {
        start_connection_monitor(
            monitor_clients,
            monitor_storage,
            monitor_mappings,
            check_interval,
            timeout,
        )
        .await;
    });

    // Build application with routes and extensions
    let app = Router::new()
        .route("/ws", get(ws_handler))
        .route("/simple-ws", get(simple_ws_handler))
        .layer(axum::Extension(storage.clone()))
        .layer(axum::Extension(clients.clone()))
        .layer(axum::Extension(user_mappings.clone()));

    // Start the server
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3030")
        .await
        .unwrap();
    tracing::info!("WebSocket server running at ws://127.0.0.1:3030/ws");

    axum::serve(listener, app).await.unwrap();
}
