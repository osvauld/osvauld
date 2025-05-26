use super::device::Device;
use super::folder::Folder;
use super::resource::ResourceKeyPair;
use super::share_record::ShareRecord;
use super::sync_record::{
    DeviceRecord, DeviceRecordStatus, StatusChangeSet, SyncRecord, SyncRecordSet,
};
use super::sync_types::SyncOperations;
use super::user::User;
use super::vector_clock::ResourceVectorClock;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum SyncPayload {
    DeviceSync {
        sync_record: SyncRecord,
        device_records: Vec<DeviceRecord>,
        device_record_statuses: Vec<DeviceRecordStatus>,
        device: Device,
    },
    UserSync {
        sync_data: Vec<SyncRecordSet>,
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
    ShareSync {
        sync_record: SyncRecord,
        device_records: Vec<DeviceRecord>,
        device_record_statuses: Vec<DeviceRecordStatus>,
        share_record: ShareRecord,
    },
    StatusUpdate(Vec<(DeviceRecord, Vec<DeviceRecordStatus>)>),
    ResourceMerge(ResourceUpdateMsg),
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
    FinalSyncAck,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Message {
    Chat(String),
    Ping,
    Pong,
    SyncResponse(SyncPayload),
    SyncAck(SyncAckType),
    AckComplete(Vec<String>),
    AddDeviceAck,
    Error,
    SyncEvent { event: String, payload: String },
    MergeUpdate(ResourceUpdateMsg),
    UserConnection(UserConnectionPayload),
    Phase(Phase),
    LiveEdit(LiveEditMessage),
    Disconnect(DisconnectStatus),
    FirstDeviceConnection(DeviceConnection),
    DeviceSync(DeviceSyncPayload),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum DisconnectStatus {
    Request,
    Accepted,
    Rejected(String),
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
pub enum ResourceUpdateMsg {
    // Initial message with state vector
    StateVectorRequest {
        resource_id: String,
        state_vector: Vec<u8>,
    },
    // Response with updates and state vector
    UpdatesResponse {
        resource_id: String,
        updates: Vec<u8>,
        state_vector: Vec<u8>,
    },
    FinalUpdateMerge {
        resource_id: String,
        updates: Vec<u8>,
        vector_clocks: Vec<ResourceVectorClock>,
    },
    // Acknowledgment that sync is complete
    VectorClockResponse {
        resource_id: String,
        update_clock: Vec<ResourceVectorClock>,
        add_clock: Vec<ResourceVectorClock>,
    },
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
    UpdateReceived,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum PhaseType {
    AddDevice,
    FirstUserConnection,
    DeviceSync,
    UserSync,
    FolderSync,
    ResourceSync,
    ShareSync,
    UpdateSync,
    DeviceRecordSync,
    Complete,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum PhaseAction {
    Init,
    Ack,
    Complete,
    CompleteAck,
}

// Combined into a single Phase message
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Phase {
    pub action: PhaseAction,
    pub phase_type: PhaseType,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionAction {
    DeviceSync,
    UserFirstConnection,
    AddDevice,
    LiveEdit,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum LiveEditMessage {
    /// Verify both peers are editing the same document
    DocumentCheck {
        resource_id: String,
    },
    NotSameDocument,
    /// Exchange document state vectors for comparison
    StateVectorExchange {
        resource_id: String,
        state_vector: Vec<u8>,
    },
    /// Transfer document updates and pending changes
    /// Contains the update data along with the current buffer state
    UpdateExchange {
        resource_id: String,
        updates: Vec<u8>,
        buffer: Vec<u8>,
        state_vector: Vec<u8>,
    },
    UpdateExchangeResponse {
        resource_id: String,
        updates: Vec<u8>,
        state_vector: Vec<u8>,
    },
    CurrentBufferExchange {
        resource_id: String,
        buffer: Vec<u8>,
    },
    DocumentChange {
        resource_id: String,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum DeviceConnection {
    // Initial request with the new device
    Request {
        device: Device,
        sync_record_set: SyncRecordSet,
    },
    // Comprehensive response with all known user devices
    Response {
        user_devices: Vec<Device>,
        sync_record_sets: Vec<SyncRecordSet>,
        external_users: Vec<User>,
        external_devices: Vec<Device>,
    },
    // Acknowledgment of processing
    Acknowledgment {
        operations: Vec<SyncOperations>,
    },
    // Final confirmation
    Complete {
        device_record_status_ids: Vec<String>,
    },
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum DeviceSyncPayload {
    Request {
        record_sets: Vec<SyncRecordSet>,
        devices: Vec<Device>,
    },

    Response {
        record_sets: Vec<SyncRecordSet>,
        devices: Vec<Device>,
        new_devices_acknowledged: Vec<String>,
    },

    SyncRecordExchange {
        all_sync_record_ids: Vec<String>,
    },

    DeviceRecordRequest {
        sync_record_ids: Vec<String>,
        target_device_ids: Vec<String>,
    },

    DeviceRecordExchange {
        device_records: Vec<DeviceRecord>,
        device_record_statuses: Vec<DeviceRecordStatus>,
    },

    Acknowledgment {
        processed_device_record_ids: Vec<String>,
        processed_status_ids: Vec<String>,
        conflicts_resolved: Vec<String>,
    },

    // Final completion signal
    Complete,
}
