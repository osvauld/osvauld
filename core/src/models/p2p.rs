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
    ResourceAdditionRequest(ResourceSyncData),
    ResourceAdditionComplete,
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
