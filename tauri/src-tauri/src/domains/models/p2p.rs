use crate::domains::models::device::Device;
use crate::domains::models::folder::Folder;
use crate::domains::models::resource::Resource;
use crate::domains::models::sync_record::{DeviceRecord, DeviceRecordStatus, SyncRecord};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::time;

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
    Resource(Resource),
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
    FileTransfer { name: String, data: Vec<u8> },
    Error,
    SyncEvent { event: String, payload: String },
}

#[derive(Serialize, Deserialize)]
pub struct ConnectionTicket {
    pub node_id: String,
    pub addresses: Vec<String>,
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
