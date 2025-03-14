use futures_util::sink::SinkExt;
use log::{debug, error, info};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::{Mutex, broadcast};
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async,
    tungstenite::{Error as WsError, protocol::Message},
};

// Only import what we need, and be explicit
use futures_util::StreamExt;
use futures_util::stream::{SplitSink, SplitStream}; // Only import this StreamExt

type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;
type WsReader = SplitStream<WsStream>;
type WsWriter = SplitSink<WsStream, Message>;

/// WebSocket client using split streams
pub struct WsClient {
    writer: Arc<Mutex<Option<WsWriter>>>,
    tx: broadcast::Sender<WsMessage>,
    rx: broadcast::Receiver<WsMessage>,
}
/// WebSocket message types for communication
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "action", content = "payload")]
pub enum WsMessage {
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
    GetConnectionStatus {
        user_ids: Vec<String>,
    },
    GetConnectionStatusResponse {
        data: Vec<UserConnectionStatus>,
    },
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UserConnectionStatus {
    pub user_id: String,
    pub connection_status: String,
}

impl WsClient {
    /// Create a new instance (doesn't connect yet)
    pub fn new() -> Self {
        // Create a channel for messages
        let (tx, rx) = broadcast::channel(100); // Buffer up to 100 messages

        Self {
            writer: Arc::new(Mutex::new(None)),
            rx,
            tx,
        }
    }
    // Add this new method to get a receiver
    pub fn subscribe(&self) -> broadcast::Receiver<WsMessage> {
        self.tx.subscribe()
    }

    /// Connect to WebSocket server and register
    pub async fn connect_and_register(&mut self, url: &str, user_id: &str) -> Result<(), String> {
        // Connect to the server
        let (ws_stream, _) = match connect_async(url).await {
            Ok(conn) => conn,
            Err(e) => return Err(format!("Failed to connect to WebSocket server: {}", e)),
        };

        info!("Connected to WebSocket server");

        // Split the WebSocket stream using the explicit trait method
        let (writer, reader) = ws_stream.split();

        // Store the writer
        {
            let mut writer_lock = self.writer.lock().await;
            *writer_lock = Some(writer);
        }

        // Start the reader task
        let tx = self.tx.clone();
        tokio::spawn(async move {
            Self::handle_reader(reader, tx).await;
        });

        // Register with the server
        self.register(user_id).await
    }

    /// Handle the reader part of the WebSocket
    async fn handle_reader(mut reader: WsReader, tx: broadcast::Sender<WsMessage>) {
        // Use the fully qualified path for next()
        while let Some(result) = futures_util::StreamExt::next(&mut reader).await {
            match result {
                Ok(msg) => {
                    if let Message::Text(text) = msg {
                        match serde_json::from_str::<WsMessage>(&text) {
                            Ok(ws_message) => {
                                debug!("Received message: {:?}", ws_message);
                                if let Err(e) = tx.send(ws_message) {
                                    error!("Failed to forward message to channel: {}", e);
                                }
                            }
                            Err(e) => {
                                error!("Failed to parse WebSocket message: {}", e);
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("WebSocket error: {}", e);
                    break;
                }
            }
        }

        info!("WebSocket reader closed");
    }
    /// Receive a message (waits until a message is available)
    pub async fn receive_message(&mut self) -> Option<WsMessage> {
        match self.rx.recv().await {
            Ok(msg) => Some(msg),
            Err(_) => None,
        }
    }

    /// Try to receive a message (non-blocking)
    pub fn try_receive_message(&mut self) -> Option<WsMessage> {
        match self.rx.try_recv() {
            Ok(msg) => Some(msg),
            Err(_) => None,
        }
    }

    /// Send a message over the WebSocket
    pub async fn send_message(&self, message: WsMessage) -> Result<(), String> {
        let mut writer_lock = self.writer.lock().await;

        let writer = match writer_lock.as_mut() {
            Some(writer) => writer,
            None => return Err("Not connected to WebSocket server".to_string()),
        };

        let msg_json = match serde_json::to_string(&message) {
            Ok(json) => json,
            Err(e) => return Err(format!("Failed to serialize message: {}", e)),
        };

        match writer.send(Message::Text(msg_json.clone().into())).await {
            Ok(_) => {
                debug!("Sent message: {}", msg_json);
                Ok(())
            }
            Err(e) => Err(format!("Failed to send message: {}", e)),
        }
    }
    /// Register with the server
    pub async fn register(&self, user_id: &str) -> Result<(), String> {
        let message = WsMessage::Register {
            user_id: user_id.to_string(),
        };

        self.send_message(message).await
    }

    /// Request connection to target
    pub async fn request_connection(&self, target_connection_id: &str) -> Result<(), String> {
        let message = WsMessage::RequestConnection {
            target_connection_id: target_connection_id.to_string(),
        };

        self.send_message(message).await
    }

    /// Send connection response
    pub async fn send_connection_response(
        &self,
        user_id: &str,
        target_connection_id: &str,
        connection_string: &str,
    ) -> Result<(), String> {
        let message = WsMessage::ConnectionResponse {
            user_id: user_id.to_string(),
            target_connection_id: target_connection_id.to_string(),
            connection_string: connection_string.to_string(),
        };

        self.send_message(message).await
    }

    /// Request user connection string
    pub async fn request_user_connection_string(&self, target_user_id: &str) -> Result<(), String> {
        info!("requesting connection");
        let message = WsMessage::RequestUserConnectionString {
            target_user_id: target_user_id.to_string(),
        };

        self.send_message(message).await
    }

    /// Close the connection
    pub async fn close(&self) -> Result<(), String> {
        // Close the writer
        let mut writer_lock = self.writer.lock().await;

        if let Some(writer) = writer_lock.as_mut() {
            // Close the writer stream
            if let Err(e) = writer.close().await {
                return Err(format!("Error closing WebSocket connection: {}", e));
            }
        }

        // Clear the writer
        *writer_lock = None;

        // The reader task will eventually notice the connection is closed
        // and exit, so we don't need to do anything else

        Ok(())
    }

    /// Get a clone of the sender that can be used to send messages
    /// to another instance of the client
    pub fn get_sender(&self) -> broadcast::Sender<WsMessage> {
        self.tx.clone()
    }

    /// Request connection status for a list of users and wait for the response
    pub async fn get_connection_status(
        &self,
        user_ids: Vec<String>,
    ) -> Result<Vec<UserConnectionStatus>, String> {
        // Create the request message
        let message = WsMessage::GetConnectionStatus { user_ids };

        // Send the request
        self.send_message(message).await?;

        // Create a temporary subscriber
        let mut rx = self.tx.subscribe();

        // Wait for the response
        // We'll wait for up to 10 seconds for a response
        let timeout = tokio::time::Duration::from_secs(10);

        match tokio::time::timeout(timeout, async {
            loop {
                match rx.recv().await {
                    Ok(WsMessage::GetConnectionStatusResponse { data }) => {
                        return Ok(data);
                    }
                    Ok(_) => {
                        // Not the response we're looking for, continue waiting
                        continue;
                    }
                    Err(e) => {
                        return Err(format!("Error receiving response: {}", e));
                    }
                }
            }
        })
        .await
        {
            Ok(result) => result,
            Err(_) => Err("Timed out waiting for connection status response".to_string()),
        }
    }
}
