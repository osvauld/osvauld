use super::ResourceSyncData;
use super::device::Device;
use super::share_record::ShareRecord;
use super::sync::{
    DeviceManifestComparisonResult, DeviceManifestRequestPayload, DeviceNetworkSyncPayload,
    UserManifestPayload, UserNetworkSyncPayload,
};
use super::user::{User, UserWithDevices};
use super::vector_clock::ResourceVectorClock;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Message {
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
    ResourceAdditionRequest(ResourceSyncData),
    ResourceAdditionComplete,
    FirstUserConnection(FirstUserExchange),
    UserManifestPayload(UserManifestPayload),
    UserNetworkSync(UserNetworkSyncPayload),
    UserNetworkSyncAck,
    RetryRequest,
    Handshake(HandshakeMessage),
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum HandshakeMessage {
    HandshakeInit(HandshakeInit),
    HandshakeResponse(HandshakeResponse),
    HandshakeConfirm(HandshakeConfirm),
    HandshakeAck,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HandshakeInit {
    pub connection_type: ConnectionType,
    pub user: User,
    pub device: Device,
    pub challenge: String,
    pub timestamp: u64,
    pub action: ConnectionAction,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HandshakeResponse {
    pub user: User,
    pub device: Device,
    pub challenge: String,
    pub timestamp: u64,
    pub challenge_signature: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HandshakeConfirm {
    pub challenge_signature: String,
}

#[derive(Clone, Debug)]
pub struct HandshakeResult {
    pub connection_type: ConnectionType,
    pub device: Device,
    pub user: User,
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
