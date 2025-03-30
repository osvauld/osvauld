use super::device::Device;
use super::folder::Folder;
use super::resource::ResourceKeyPair;
use super::share_record::{ShareRecord, UserRecord, UserRecordStatus};
use super::sync_record::{
    DeviceRecord, DeviceRecordStatus, StatusChangeSet, SyncRecord, SyncRecordSet,
};
use super::sync_types::SyncOperations;
use super::user::User;
use super::vector_clock::ResourceVectorClock;
use serde::{Deserialize, Serialize};

use super::resource::Resource;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum SyncPayload {
    DeviceSync {
        sync_record: SyncRecord,
        device_records: Vec<DeviceRecord>,
        device_record_statuses: Vec<DeviceRecordStatus>,
        device: Device,
    },
    UserSync {
        sync_data: Vec<(SyncRecord, Vec<DeviceRecord>, Vec<DeviceRecordStatus>)>,
        user_data: Vec<(User, Vec<Device>)>,
    },
    ResourceSync {
        sync_record: SyncRecord,
        device_records: Vec<DeviceRecord>,
        device_record_statuses: Vec<DeviceRecordStatus>,
        resource: ResourceKeyPair,
        vector_clocks: Vec<ResourceVectorClock>,
    },
    FolderSync {
        sync_record: SyncRecord,
        device_records: Vec<DeviceRecord>,
        device_record_statuses: Vec<DeviceRecordStatus>,
        folder: Folder,
    },
    StatusUpdate(Vec<(DeviceRecord, Vec<DeviceRecordStatus>)>),
    ResourceUpdate {
        resource: Resource,
        vector_clocks: Vec<ResourceVectorClock>,
    },
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum UserConnectionPayload {
    Request {
        user: User,
        devices: Vec<Device>,
    },
    Response {
        user: User,
        devices: Vec<Device>,
        user_addition_record: SyncRecordSet,
    },
    Acknowledgment {
        user_id: String,
        user_addition_records: SyncRecordSet,
        completion_record: StatusChangeSet,
        updated_device_record_ids: Vec<String>,
        updated_device_record_status_ids: Vec<String>,
    },
    Complete {
        completion_record: StatusChangeSet,
        device_record_status_id: Option<String>,
        updated_device_record_ids: Vec<String>,
        updated_device_record_status_ids: Vec<String>,
    },
    FinalSync {
        device_record_status_id: Option<String>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Message {
    Chat(String),
    Ping,
    Pong,
    SyncRequest,
    SyncResponse(SyncPayload),
    SyncAck(SyncAckType),
    AckComplete(Vec<String>),
    SyncComplete,
    AddDevice(SyncPayload),
    AddDeviceAck,
    // FileTransfer { name: String, data: Vec<u8> },
    Error,
    SyncEvent { event: String, payload: String },
    SharePayload(SharePayload),
    ShareComplete,
    UpdateResource(UpdateResource),
    UserConnection(UserConnectionPayload),
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpdateResource {
    pub encrypted_data: String,
    pub add_vector_clock: Vec<ResourceVectorClock>,
    pub update_vector_clock: Vec<ResourceVectorClock>,
    pub resource_id: String,
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
    FullSync(SyncOperations),
    DeviceSyncRecords(Vec<String>), // list of device_record_ids
    UpdateRecieved(String),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SharePayload {
    pub share_record: Option<ShareRecord>,
    pub user_records: Vec<UserRecord>,
    pub user_record_statuses: Vec<UserRecordStatus>,
    pub data: Option<ResourceKeyPair>,
}
