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
    ViewerNode,  // Node from viewer's perspective (server side, no escalated privileges)
    User,
}

impl PeerRole {
    /// Create PeerRole from string (extracted from UCAN token)
    pub fn from_string(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "owner" => PeerRole::Owner,
            "node" => PeerRole::Node,
            "viewer" => PeerRole::Viewer,
            "viewer_node" => PeerRole::ViewerNode,
            _ => PeerRole::User, // Default
        }
    }

    /// Convert PeerRole to string (for UCAN token facts)
    pub fn as_str(&self) -> &str {
        match self {
            PeerRole::Owner => "owner",
            PeerRole::Node => "node",
            PeerRole::Viewer => "viewer",
            PeerRole::ViewerNode => "viewer_node",
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

    // Resource sync protocol (CRDT merging, resource transfer, etc.)
    Resource(ResourceMessage),

    // Folder sync protocol (simple push and token management)
    Folder(FolderMessage),
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

/// Resource sync messages (CRDT merging and resource transfer)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ResourceMessage {
    // CRDT merge protocol
    MergeUpdate(ResourceUpdateMsg),

    // Resource request/transfer protocol
    ResourceSyncRequest(ResourceSyncRequestMsg),
    ResourceNotFoundRequest(ResourceNotFoundRequestMsg),
    ResourceTransfer(ResourceTransferMsg),
    ResourceTransferAck,

    // Asset transfer protocol (for static files)
    AssetTransfer(AssetTransferMsg),

    // Simple push sync (used for initial folder sync)
    ResourceDataSync(ResourceDataSync),
}

/// Folder sync messages (simple push and token management)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum FolderMessage {
    // Simple push sync
    FolderDataSync(FolderDataSync),

    // CRDT folder sync protocol
    FolderSyncRequest(FolderSyncRequestMsg),
    FolderSyncResponse(FolderSyncResponseMsg),

    // Token request/response for shareable links
    FolderTokenRequest(FolderTokenRequest),
    FolderTokenResponse(FolderTokenResponse),
}

/// Unified handshake protocol for all roles (owner, node, user, viewer)
/// Three-way handshake for first connection, two-way for reconnection
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum HandshakeMessage {
    // Step 1: Initiator → Responder (all connections)
    HandshakeRequest(HandshakeRequest),

    // Step 2 & 3: First connection (3-way handshake)
    FirstConnectionResponse(FirstConnectionResponse),
    FirstConnectionComplete(FirstConnectionComplete),

    // Step 2: Reconnection (2-way handshake)
    ReconnectionResponse(ReconnectionResponse),
}

/// Step 1: Handshake request for all connection types
/// Token contents (token_type, role, first_connection) determine behavior
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HandshakeRequest {
    /// UCAN token (OneTimeConnection, ViewerAuth, or persistent connection token)
    pub ucan_token: String,
    /// Peer user information
    pub peer_user: User,
    /// Peer device information
    pub peer_device: Device,
    /// Signed UCAN public key for validation
    pub signed_ucan_pub: String,
}

/// Step 2: First connection response (responder issues token to initiator)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FirstConnectionResponse {
    /// NEW persistent token for initiator
    pub issued_ucan: String,
    /// Peer user information
    pub peer_user: User,
    /// Peer device information
    pub peer_device: Device,
    /// Device list
    pub devices: Vec<Device>,
    /// Signed UCAN public key for validation
    pub signed_ucan_pub: String,
}

/// Step 3: First connection complete (initiator issues token to responder)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FirstConnectionComplete {
    /// NEW persistent token for responder
    pub issued_ucan: String,
    /// Peer user ID (initiator's user ID) so responder knows which user to update
    pub peer_user_id: String,
}

/// Step 2: Reconnection response (no token exchange needed)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReconnectionResponse {
    /// Peer user information
    pub peer_user: User,
    /// Peer device information
    pub peer_device: Device,
    /// Device list
    pub devices: Vec<Device>,
    /// Signed UCAN public key for validation
    pub signed_ucan_pub: String,
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

/// Step 1: Initiator sends UCAN with state vectors to check if responder has resource and start sync
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResourceSyncRequestMsg {
    /// Resource UCAN token (contains resource_id)
    pub resource_ucan: String,
    /// Initiator's folder UCAN (for validation when responder sends resource back)
    pub folder_ucan: String,
    /// Initiator's state vectors for CRDT docs
    /// JSON format: {"doc_name": {"state_vector": [1,2,3,...], "asset_ids": ["id1", "id2"]}}
    pub state_vectors: String,
    /// Full documents for crud/submit capability (viewer submissions)
    /// JSON format: {"submissions_doc": {"doc_type": "loro", "data": "base64..."}}
    pub full_docs: String,
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

/// Folder sync request - discover resources in folder
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FolderSyncRequestMsg {
    /// Folder UCAN proving access to folder
    pub folder_ucan: String,
    /// Initiator's resource list with state vectors
    /// JSON format: {"resource_id": {"state_vectors": {...}, "asset_ids": [...]}}
    pub resources: String,
}

/// Folder sync response - identify missing and existing resources
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FolderSyncResponseMsg {
    /// Folder ID being synced
    pub folder_id: String,
    /// Resource IDs that responder doesn't have (need full transfer)
    pub missing_resource_ids: Vec<String>,
    /// Resources that responder has (will use individual ResourceSyncRequest)
    /// JSON format: {"resource_id": {"state_vectors": {...}, "asset_ids": [...]}}
    pub existing_resources: String,
}

/// Asset transfer message for static files
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AssetTransferMsg {
    /// Resource ID this asset belongs to
    pub resource_id: String,
    /// Asset ID
    pub asset_id: String,
    /// Binary asset data
    pub asset_data: Vec<u8>,
    /// Metadata JSON (mime_type, size, etc.)
    pub metadata: String,
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

