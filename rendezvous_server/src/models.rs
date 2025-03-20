use axum::extract::ws::Message;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{mpsc, Mutex};

#[derive(Clone, Debug)]
pub struct ClientInfo {
    pub sender: mpsc::UnboundedSender<Message>,
    pub last_activity: Instant,
}

pub type Clients = Arc<Mutex<HashMap<String, ClientInfo>>>;

#[derive(Clone, Debug)]
pub struct UserClientMapping {
    pub user_id: String,
    pub client_id: String,
}

pub type UserClientMappings = Arc<Mutex<Vec<UserClientMapping>>>;

#[derive(Serialize, Debug, Deserialize, Clone)]
pub struct ClientStatus {
    pub user_id: String,
    pub connection_status: String,
}

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
    GetAllClients,
    GetConnectionStatus {
        user_ids: Vec<String>,
    },
    GetConnectionStatusResponse {
        data: Vec<ClientStatus>,
    },
    UserConnectionNotificationRequest {
        user_ids: Vec<String>,
    },
    UserConnectionNotification {
        online_user_id: String,
    },
}
