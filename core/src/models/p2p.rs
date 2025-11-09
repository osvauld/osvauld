use super::resource::EncryptedResource;
use super::device::Device;
use super::folder::Folder;
use super::folder_share_record::FolderShareRecord;
use super::share_record::ShareRecord;
use super::user::User;
use serde::{Deserialize, Serialize};

/// Represents the role/type of a peer in P2P connections
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PeerRole {
    Owner,
    Node,
    Viewer,
    User,
}

impl PeerRole {
    /// Create PeerRole from string (extracted from UCAN token)
    pub fn from_string(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "owner" => PeerRole::Owner,
            "node" => PeerRole::Node,
            "viewer" => PeerRole::Viewer,
            _ => PeerRole::User, // Default
        }
    }

    /// Convert PeerRole to string (for UCAN token facts)
    pub fn as_str(&self) -> &str {
        match self {
            PeerRole::Owner => "owner",
            PeerRole::Node => "node",
            PeerRole::Viewer => "viewer",
            PeerRole::User => "user",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Message {
    // Connection management
    Ping,
    Pong,
    Error,
    Handshake(HandshakeMessage),
    RetryRequest,

    // Resource sync (unified UCAN-based protocol for owner connections)
    MergeUpdate(ResourceUpdateMsg),
    ResourceAdditionRequest(EncryptedResource),
    ResourceAdditionComplete,
    AssetTransfer(AssetTransferMessage),

    // Resource request protocol (when peer doesn't have resource)
    ResourceSyncRequest(ResourceSyncRequestMsg),
    ResourceNotFoundRequest(ResourceNotFoundRequestMsg),
    ResourceTransfer(ResourceTransferMsg),
    ResourceTransferAck,

    // Folder sync (simple push protocol)
    FolderDataSync(FolderDataSync),
    ResourceDataSync(ResourceDataSync),

    // Folder token request (for generating shareable links)
    FolderTokenRequest(FolderTokenRequest),
    FolderTokenResponse(FolderTokenResponse),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ResourceUpdateMsg {
    /// Step 1: Initiator sends their state vectors and asset IDs
    /// state_vectors: JSON format {"doc_name": {"state_vector": [1,2,3,...]}}
    /// asset_ids: List of asset IDs this peer has
    StateVectorRequest {
        resource_id: String,
        state_vectors: String,  // JSON with Loro state vectors per doc
        asset_ids: Vec<String>, // Asset IDs this peer has
        ucan_token: String,
    },

    /// Step 2: Responder sends updates and their state
    /// updates: JSON format {"doc_name": {"updates": [...], "state_vector": [...]}}
    /// missing_asset_ids: Asset IDs responder needs from initiator
    UpdatesResponse {
        resource_id: String,
        updates: String,              // JSON with Loro updates per doc
        state_vectors: String,        // Responder's current state vectors
        missing_asset_ids: Vec<String>, // Assets responder doesn't have
        ucan_token: String,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum HandshakeMessage {
    HandshakeFirstConnectRequest(FirstConnectRequest),
    HandshakeFirstConnectResponse(FirstConnectResponse),
    HandshakeExchange(UcanAndUserExchange),
    HandshakeWebsiteRequest(WebsiteHandshakeRequest),
    HandshakeWebsiteResponse(WebsiteHandshakeResponse),
    HandshakeWebsiteReconnectRequest(WebsiteReconnectRequest),
    HandshakeWebsiteReconnectResponse(WebsiteReconnectResponse),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FirstConnectRequest {
    pub devices: Vec<Device>,
    pub issued_ucan: String,
    pub signed_ucan_pub: String,
    pub one_time_ucan: String,
    pub peer_device: Device,
    pub peer_user: User,
    // Note: connection_type removed - inferred from UCAN token role
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
    // Note: connection_type removed - inferred from UCAN token role
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WebsiteHandshakeRequest {
    pub ucan_token: String,
    pub viewer_user: User,
    pub viewer_device: Device,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WebsiteHandshakeResponse {
    pub node_user: User,
    pub node_device: Device,
    pub viewer_specific_token: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WebsiteReconnectRequest {
    pub ucan_token: String,
    pub viewer_user: User,
    pub viewer_device: Device,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WebsiteReconnectResponse {
    pub node_user: User,
    pub node_device: Device,
}


/// Asset transfer messages for static files (non-CRDT)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AssetTransferMessage {
    /// Request specific assets by ID
    AssetRequest {
        resource_id: String,
        asset_ids: Vec<String>,
        ucan_token: String,
    },
    /// Response with requested asset data
    AssetResponse {
        resource_id: String,
        assets: Vec<Asset>,
    },
}

/// Represents a static asset (image, file, etc.)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Asset {
    pub asset_id: String,
    pub data: Vec<u8>,
    pub mime_type: Option<String>,
}

// Folder sync protocol - simple push after share_folder()

/// Folder data with share record for syncing to node
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FolderDataSync {
    pub folder: Folder,
    pub folder_share_record: FolderShareRecord,
}

/// Resource data with share records for syncing to node
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResourceDataSync {
    /// Re-encrypted resource for node
    pub resource: EncryptedResource,
    /// All share records for this resource (enables node to forward viewer updates)
    pub share_records: Vec<ShareRecord>,
    /// Owner's folder UCAN token (proves add_resources permission)
    pub owner_folder_ucan: String,
}

// Resource request protocol - initiated when peer doesn't have resource

/// Step 1: Initiator sends UCAN to check if responder has resource and start sync
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResourceSyncRequestMsg {
    /// Resource UCAN token (contains resource_id)
    pub resource_ucan: String,
    /// Initiator's folder UCAN (for validation when responder sends resource back)
    pub folder_ucan: String,
}

/// Step 2: Responder requests full resource (doesn't have it locally)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResourceNotFoundRequestMsg {
    /// ID of the resource being requested
    pub resource_id: String,
    /// Responder's folder UCAN proving they should have access
    pub folder_ucan: String,
}

/// Step 3: Initiator sends complete resource to responder
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResourceTransferMsg {
    /// Re-encrypted resource for responder
    pub resource: EncryptedResource,
    /// All share records for this resource
    pub share_records: Vec<ShareRecord>,
}

/// Request folder token for generating shareable link
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FolderTokenRequest {
    pub folder_id: String,
    pub folder_ucan: String,
}

/// Response with generated folder token (shareable link)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FolderTokenResponse {
    pub folder_id: String,
    pub connection_string: String,
}

