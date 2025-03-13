use crate::models::device::Device;
use crate::models::folder::Folder;
use crate::models::resource::ResourceKeyPair;
use crate::models::share_record::{ShareRecord, UserRecord, UserRecordStatus};
use crate::models::sync_record::{DeviceRecord, DeviceRecordStatus, SyncRecord};
use crate::models::user::User;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SyncPayload {
    pub sync_record: Option<SyncRecord>, // Optional because status updates don't have sync record
    pub device_records: Vec<DeviceRecord>,
    pub device_record_statuses: Vec<DeviceRecordStatus>,
    pub data: Option<SyncData>, // The actual folder/resource/device data
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type")]
pub enum SyncData {
    Folder(Folder),
    Resource(ResourceKeyPair),
    Device(Device),
    SyncRecord(SyncRecord),
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Message {
    Chat(String),
    Ping,
    Pong,
    SyncRequest,
    SyncResponse(SyncPayload),
    SyncAck(SyncAckType),
    AckComplete(String),
    SyncComplete,
    AddDevice(SyncPayload),
    AddDeviceAck,
    // FileTransfer { name: String, data: Vec<u8> },
    Error,
    SyncEvent { event: String, payload: String },
    FirstUserConnection(User),
    UserAddAck(String),
    SharePayload(SharePayload),
    ShareComplete,
    // HandshakeMessage(HandshakeMessage),
}

#[derive(Serialize, Deserialize)]
pub struct ConnectionTicket {
    pub node_id: String,
    pub addresses: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ConnectionType {
    Device,
    User,
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

// The HandshakeMessage type remains the same
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HandshakeMessage {
    pub challenge: String,
    pub signature: String,
    pub device: Device,
    pub connection_type: ConnectionType,
    pub user: User,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SyncAckDeviceRecord {
    pub device_records: Vec<String>, // device_record_ids
    pub status_updates: Vec<String>, // device_record_status_ids
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SyncAckType {
    FullSync {
        sync_record_id: String,
        device_record: DeviceRecord,
        device_sync_records: Vec<DeviceRecordStatus>,
    },
    DeviceRecords(Vec<String>), // list of device_record_ids
    DeviceSyncRecord(String),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SharePayload {
    pub share_record: Option<ShareRecord>,
    pub user_records: Vec<UserRecord>,
    pub user_record_statuses: Vec<UserRecordStatus>,
    pub data: Option<ResourceKeyPair>,
}
