use crate::models::ResourceKey;

use super::ResourceSyncData;
use super::device::Device;
use super::folder::Folder;
use super::folder_share_record::FolderShareRecord;
use super::share_record::ShareRecord;
use super::sync::{
    DeviceManifestComparisonResult, DeviceManifestRequestPayload, DeviceNetworkSyncPayload,
    UserManifestPayload, UserNetworkSyncPayload,
};
use super::user::User;
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
    UserManifestPayload(UserManifestPayload),
    UserNetworkSync(UserNetworkSyncPayload),
    UserNetworkSyncAck,
    FolderSync(FolderSyncMessage),
    RetryRequest,
    Handshake(HandshakeMessage),
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
        state_vectors: String,
        ucan_token: String,
    },
    // Response with updates and state vector
    UpdatesResponse {
        resource_id: String,
        updates: String,
        ucan_token: String,
    },
    FinalUpdateMerge {
        resource_id: String,
        updates: String,
        vector_clocks: Vec<ResourceVectorClock>,
        share_records: Vec<ShareRecord>,
        resource_keys: Vec<ResourceKey>,
    },
    // Acknowledgment that sync is complete
    VectorClockResponse {
        resource_id: String,
        update_clock: Vec<ResourceVectorClock>,
        add_clock: Vec<ResourceVectorClock>,
        share_records: Vec<ShareRecord>,
        resource_keys: Vec<ResourceKey>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum HandshakeMessage {
    HandshakeFirstConnectRequest(FirstConnectRequest),
    HandshakeFirstConnectResponse(FirstConnectResponse),
    HandshakeExchange(UcanAndUserExchange),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FirstConnectRequest {
    pub devices: Vec<Device>,
    pub issued_ucan: String,
    pub signed_ucan_pub: String,
    pub one_time_ucan: String,
    pub peer_device: Device,
    pub peer_user: User,
    pub connection_type: ConnectionType,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FirstConnectResponse {
    pub devices: Vec<Device>,
    pub issued_ucan: String,
    pub signed_ucan_pub: String,
    pub ucan_token: String,
    pub peer_user: User,
    pub peer_device: Device,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UcanAndUserExchange {
    pub signed_ucan_pub: String,
    pub ucan_token: String,
    pub peer_user: User,
    pub peer_device: Device,
    pub connection_type: ConnectionType,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionAction {
    DeviceSync,
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
        state_vectors: String,
    },
    /// Transfer document updates and pending changes
    /// Contains the update data along with the current buffer state
    UpdateExchange {
        resource_id: String,
        updates: String,
    },
    UpdateExchangeResponse {
        resource_id: String,
        updates: String,
    },
    DocumentChange {
        resource_id: String,
    },
    DocumentUpdate {
        resource_id: String,
        client_id: u32,
        updates: Vec<u8>,
        doc_type: String,
    },
    AwarenessUpdate {
        resource_id: String,
        client_id: u32,
        awareness_data: Vec<u8>,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum FolderSyncMessage {
    UnknownFoldersPayload(UnknownFoldersPayload),
    FolderRecipientSyncPayload(Vec<FolderRecipientUpdate>),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FolderWithShareRecords {
    pub folder: Folder,
    pub share_records: Vec<FolderShareRecord>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UnknownFoldersPayload {
    pub folder_data: Vec<FolderWithShareRecords>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FolderRecipientUpdate {
    pub folder_id: String,
    pub new_share_records: Vec<FolderShareRecord>,
}
