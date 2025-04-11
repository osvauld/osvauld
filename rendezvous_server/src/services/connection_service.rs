use crate::error::AppError;
use crate::models::{Clients, WsMessage};
use axum::extract::ws::Message;
use serde::{Deserialize, Serialize};
use std::fmt;
use tracing::{error, info};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ConnectionStatus {
    Online,
    Offline,
}

impl ConnectionStatus {
    pub fn as_str(&self) -> &str {
        match self {
            ConnectionStatus::Online => "online",
            ConnectionStatus::Offline => "offline",
        }
    }
}

impl fmt::Display for ConnectionStatus {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

pub async fn send_message(
    clients: &Clients,
    client_id: &str,
    message: WsMessage,
) -> Result<(), AppError> {
    let json =
        serde_json::to_string(&message).map_err(|e| AppError::SerializationError(e.to_string()))?;

    let clients_lock = clients.lock().await;
    if let Some(client_info) = clients_lock.get(client_id) {
        client_info
            .sender
            .send(Message::Text(json.into()))
            .map_err(|e| AppError::WebSocketError(format!("Failed to send message: {}", e)))?;
        info!("Message sent to client {}", client_id);
        Ok(())
    } else {
        Err(AppError::WebSocketError(format!(
            "Client {} not found",
            client_id
        )))
    }
}

pub async fn send_offline_response(
    target_user_id: String,
    client_id: String,
    clients: Clients,
) -> Result<(), AppError> {
    let response = WsMessage::UserConnectionStringResponse {
        user_id: target_user_id.clone(),
        connection_string: None,
        connection_status: ConnectionStatus::Offline.as_str().to_string(),
    };

    match send_message(&clients, &client_id, response).await {
        Ok(_) => {
            info!("Sent offline status for user: {}", target_user_id);
            Ok(())
        }
        Err(e) => {
            error!("Error sending offline response: {}", e);
            Err(e)
        }
    }
}
