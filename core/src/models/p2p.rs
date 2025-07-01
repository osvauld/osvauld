use super::ResourceSyncData;
use super::device::Device;
use super::folder::Folder;
use super::resource::{ResourceKeyPair, ResourceManifestData};
use super::share_record::ShareRecord;
use super::sync_record::{
    DeviceRecord, DeviceRecordStatus, StatusChangeSet, SyncRecord, SyncRecordSet,
};
use super::sync_types::SyncOperations;
use super::user::{User, UserWithDeviceIds, UserWithDevices};
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
    Error,
    MergeUpdate(ResourceUpdateMsg),
    LiveEdit(LiveEditMessage),
    // Disconnect(DisconnectStatus),
    DeviceManifestRequest(DeviceManifestRequestPayload),
    DeviceManifestResponse(DeviceManifestComparisonResult),
    DeviceNetworkSync(DeviceNetworkSyncPayload),
    DeviceManifestAck,
    DeviceNetworkSyncAck,
    ResourceAddtionRequest(ResourceSyncData),
    ResourceAddtionComplete,
    FirstUserConnection(FirstUserExchange),
    UserManifestPayload(UserManifestPayload),
    UserNetworkSync(UserNetworkSyncPayload),
    UserNetworkSyncAck,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum FirstUserExchange {
    Request(UserWithDevices),
    Response(UserWithDevices),
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
        share_records: Vec<ShareRecord>,
    },
    // Acknowledgment that sync is complete
    VectorClockResponse {
        resource_id: String,
        update_clock: Vec<ResourceVectorClock>,
        add_clock: Vec<ResourceVectorClock>,
        share_records: Vec<ShareRecord>,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionAction {
    DeviceSync,
    UserFirstConnection,
    AddDevice,
    LiveEdit,
    UserSync,
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

    DocumentUpdate {
        resource_id: String,
        client_id: u32,
        updates: Vec<u8>,
    },
    AwarenessUpdate {
        resource_id: String,
        client_id: u32,
        awareness_data: Vec<u8>,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum DeviceConnection {
    // Initial request with the new device
    Request {
        device: Device,
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceManifestRequestPayload {
    pub known_device_ids: Vec<String>,
    pub other_users: Vec<UserWithDeviceIds>,
    pub folder_ids: Vec<String>,
    pub resources: Vec<ResourceManifestData>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserManifestRequestPayload {
    pub user: Vec<UserWithDeviceIds>,
    pub resources: Vec<ResourceManifestData>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UserManifestPayload {
    Request(UserManifestRequestPayload),
    Response(UserManifestComparisonResult),
    Ack,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserNetworkSyncPayload {
    pub users: Vec<UserWithDevices>,
    pub devices: Vec<Device>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserManifestDifferences {
    pub unknown_users: Vec<String>,
    pub unknown_devices_from_common_users: Vec<UserWithDeviceIds>,
    pub unknown_resources: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserManifestComparisonResult {
    pub local_missing: UserManifestDifferences,
    pub remote_missing: UserManifestDifferences,
    pub resources_requiring_sync: Vec<String>,
}

impl UserManifestComparisonResult {
    /// Inverse the perspective for sending to remote peer
    /// What's local_missing becomes remote_missing and vice versa
    pub fn inverse(&self) -> UserManifestComparisonResult {
        UserManifestComparisonResult {
            local_missing: self.remote_missing.clone(),
            remote_missing: self.local_missing.clone(),
            resources_requiring_sync: self.resources_requiring_sync.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct UserComparisonResult {
    pub users_only_local_has: Vec<String>,
    pub users_only_remote_has: Vec<String>,
    pub common_users: Vec<String>,
    pub devices_from_common_users_only_local_has: Vec<UserWithDeviceIds>,
    pub devices_from_common_users_only_remote_has: Vec<UserWithDeviceIds>,
}
#[derive(Debug, Clone)]
pub struct ResourceComparisonResult {
    pub resources_only_local_has: Vec<String>,
    pub resources_only_remote_has: Vec<String>,
    pub common_resources: Vec<String>,
    pub resources_requiring_sync: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceManifestDifferences {
    pub unknown_users: Vec<String>,
    pub unknown_devices_from_common_users: Vec<UserWithDeviceIds>,
    pub unknown_devices_from_current_user: Vec<String>,
    pub unknown_resources: Vec<String>,
    pub unknown_folders: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceManifestComparisonResult {
    pub local_missing: DeviceManifestDifferences,
    pub remote_missing: DeviceManifestDifferences,
    pub resources_requiring_sync: Vec<String>,
}

impl DeviceManifestComparisonResult {
    /// Inverse the perspective for sending to remote peer
    /// What's local_missing becomes remote_missing and vice versa
    pub fn inverse(&self) -> DeviceManifestComparisonResult {
        DeviceManifestComparisonResult {
            local_missing: self.remote_missing.clone(),
            remote_missing: self.local_missing.clone(),
            resources_requiring_sync: self.resources_requiring_sync.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceNetworkSyncPayload {
    pub unknown_users_with_devices: Vec<UserWithDevices>,
    pub unknown_devices_from_common_users: Vec<Device>,
    pub unknown_devices_from_current_user: Vec<Device>,
    pub unknown_folders: Vec<Folder>,
}
