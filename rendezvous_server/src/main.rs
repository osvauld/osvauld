use axum::{
    routing::get,
    Router,
};
use std::sync::Arc;
use crate::storage::Storage;
use crate::websocket::ws_handler;

mod storage;
mod websocket;

#[tokio::main]
async fn main() {
    // Initialize the Sled database for local storage
    let storage = Arc::new(Storage::new("clients_db").unwrap());

    // Set up the Axum router with a WebSocket route
    let app = Router::new()
        .route("/ws", get(ws_handler))
        .layer(axum::Extension(storage));

    // Start the server on localhost:3030
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3030").await.unwrap();
    println!("WebSocket server running at ws://127.0.0.1:3030/ws");
    axum::serve(listener, app).await.unwrap();
}