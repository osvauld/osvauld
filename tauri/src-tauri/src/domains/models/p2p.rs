use crate::application::services::SyncPayload;
use crate::domains::models::device::Device;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::time;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Message {
    Chat(String),
    Ping,
    Pong,
    SyncRequest,
    SyncResponse(SyncPayload),
    SyncAck(String),
    SyncComplete,
    AddDevice(Device),
    AddDeviceAck(Device),
    FileTransfer { name: String, data: Vec<u8> },
}

#[derive(Serialize, Deserialize)]
pub struct ConnectionTicket {
    pub node_id: String,
    pub addresses: Vec<String>,
    pub relay_url: String,
}
#[derive(Error, Debug, Serialize, Deserialize)]
pub enum HandshakeError {
    #[error("Invalid signature: {0}")]
    InvalidSignature(String),
    #[error("Invalid challenge: {0}")]
    InvalidChallenge(String),
    #[error("Serialization error: {0}")]
    Serialization(String), // Changed from serde_json::Error
    #[error("Auth service error: {0}")]
    AuthService(String),
    #[error("Connection error: {0}")]
    Connection(String),
    #[error("Timeout error: {0}")]
    Timeout(String), // Changed from time::error::Elapsed
}
impl From<serde_json::Error> for HandshakeError {
    fn from(err: serde_json::Error) -> Self {
        HandshakeError::Serialization(err.to_string())
    }
}

impl From<time::error::Elapsed> for HandshakeError {
    fn from(err: time::error::Elapsed) -> Self {
        HandshakeError::Timeout(err.to_string())
    }
}

// The HandshakeMessage type remains the same
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HandshakeMessage {
    pub challenge: String,
    pub signature: String,
    pub device: Device,
}
