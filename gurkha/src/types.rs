use serde::{Deserialize, Serialize};

/// Permission level for document access (domain concept)
/// Hierarchy: Collaborator >= Submitter >= Viewer (for capability checking)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Capability {
    /// Receive-only, no sending updates (was: crud/readonly)
    /// Lowest privilege
    Viewer,

    /// Send full snapshots to isolated namespace (was: crud/submit)
    Submitter,

    /// Full bidirectional CRDT sync (was: crud/merge)
    /// Highest privilege
    Collaborator,
}

impl Capability {
    pub fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "collaborator" => Ok(Capability::Collaborator),
            "viewer" => Ok(Capability::Viewer),
            "submitter" => Ok(Capability::Submitter),
            _ => Err(format!("Unknown capability: {}", s)),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Capability::Collaborator => "collaborator",
            Capability::Viewer => "viewer",
            Capability::Submitter => "submitter",
        }
    }

    pub fn can_write(&self) -> bool {
        matches!(self, Capability::Collaborator | Capability::Submitter)
    }

    pub fn can_sync_bidirectional(&self) -> bool {
        matches!(self, Capability::Collaborator)
    }
}

/// User role in UCAN token (domain concept)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
    /// Resource creator, full control
    Owner,
    /// Sovereign node hosting resource
    Node,
    /// Peer user (P2P collaboration)
    User,
    /// Limited access (viewer mode)
    Viewer,
}

impl Role {
    pub fn from_str(s: &str) -> Result<Self, String> {
        match s.to_lowercase().as_str() {
            "owner" => Ok(Role::Owner),
            "node" => Ok(Role::Node),
            "user" => Ok(Role::User),
            "viewer" => Ok(Role::Viewer),
            _ => Err(format!("Unknown role: {}", s)),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Role::Owner => "owner",
            Role::Node => "node",
            Role::User => "user",
            Role::Viewer => "viewer",
        }
    }
}

/// Document type (domain concept with behavior)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DocType {
    /// CRDT document (Loro, state vectors, merge)
    Crdt,
    /// Binary asset (blob storage, no merge)
    Asset,
}

impl DocType {
    pub fn from_str(s: &str) -> Result<Self, String> {
        match s.to_lowercase().as_str() {
            "crdt" => Ok(DocType::Crdt),
            "asset" => Ok(DocType::Asset),
            _ => Err(format!("Unknown doc type: {}", s)),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            DocType::Crdt => "crdt",
            DocType::Asset => "asset",
        }
    }

    pub fn supports_merge(&self) -> bool {
        matches!(self, DocType::Crdt)
    }

    pub fn is_binary(&self) -> bool {
        matches!(self, DocType::Asset)
    }
}

/// Connection/Handshake token types (separate hierarchy from resource tokens)
/// These tokens are used for device-to-device connections and authentication
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionTokenType {
    /// One-time use token for initial device handshake (any role)
    /// Used in: FirstConnectRequest.one_time_ucan
    /// Lifespan: Single use
    /// Purpose: Prove device pairing authorization
    OneTimeConnection,

    /// Owner device-to-device connection token
    /// Used in: FirstConnectRequest.issued_ucan, UcanAndUserExchange.ucan_token
    /// Lifespan: Long-lived
    /// Purpose: Owner device sync and full control operations
    OwnerConnection,

    /// Node device-to-device connection token
    /// Used in: FirstConnectRequest.issued_ucan, UcanAndUserExchange.ucan_token
    /// Lifespan: Long-lived
    /// Purpose: Node device sync and hosting operations
    NodeConnection,

    /// User device-to-device connection token (P2P collaboration)
    /// Used in: FirstConnectRequest.issued_ucan, UcanAndUserExchange.ucan_token
    /// Lifespan: Long-lived
    /// Purpose: User device sync and collaboration operations
    UserConnection,

    /// Viewer authentication token (from shareable link)
    /// Used in: ViewerHandshakeRequest.viewer_auth_token, WebsiteRequest.ucan_token
    /// Lifespan: Single use or short-lived
    /// Purpose: Initial viewer authentication
    ViewerAuth,

    /// Viewer connection token (persistent after first connection)
    /// Used in: HandshakeResponse.issued_ucan (after ViewerAuth validation)
    /// Lifespan: Indefinite (long-lived)
    /// Purpose: Persistent viewer connection, can delete self
    ViewerConnection,
}

impl ConnectionTokenType {
    pub fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "one_time_connection" => Ok(ConnectionTokenType::OneTimeConnection),
            "owner_connection" => Ok(ConnectionTokenType::OwnerConnection),
            "node_connection" => Ok(ConnectionTokenType::NodeConnection),
            "user_connection" => Ok(ConnectionTokenType::UserConnection),
            "viewer_auth" => Ok(ConnectionTokenType::ViewerAuth),
            "viewer_connection" => Ok(ConnectionTokenType::ViewerConnection),
            _ => Err(format!("Unknown connection token type: {}", s)),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            ConnectionTokenType::OneTimeConnection => "one_time_connection",
            ConnectionTokenType::OwnerConnection => "owner_connection",
            ConnectionTokenType::NodeConnection => "node_connection",
            ConnectionTokenType::UserConnection => "user_connection",
            ConnectionTokenType::ViewerAuth => "viewer_auth",
            ConnectionTokenType::ViewerConnection => "viewer_connection",
        }
    }

    pub fn is_one_time(&self) -> bool {
        matches!(
            self,
            ConnectionTokenType::OneTimeConnection | ConnectionTokenType::ViewerAuth
        )
    }
}

/// Resource/Folder token types (for sync and resource operations)
/// These tokens control access to specific resources and folders
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResourceTokenType {
    /// Resource owner token (full control)
    /// Role: Owner
    /// Contains: Delegation templates for Node/User/Viewer
    ResourceOwner,

    /// Resource share token (for Node or User role)
    /// Role: Node or User
    /// Contains: Delegated capabilities, may contain further delegation templates
    ResourceShare,

    /// Resource viewer token (for Viewer role)
    /// Role: Viewer
    /// Issued by Node after ViewerAuth validation
    /// Used in: UpdateUcanMessage.new_ucan_token
    /// Purpose: Viewer access to specific resource
    ResourceViewer,

    /// Folder owner token (full control)
    /// Role: Owner
    /// Contains: Delegation templates for Node/User/Viewer
    FolderOwner,

    /// Folder share token (for Node or User role)
    /// Role: Node or User
    /// Contains: Delegated capabilities, may contain further delegation templates
    FolderShare,

    /// Folder viewer token (for Viewer role)
    /// Role: Viewer
    /// Issued by Node after ViewerAuth validation
    /// Purpose: Viewer access to specific folder
    FolderViewer,
}

impl ResourceTokenType {
    pub fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "resource_owner" => Ok(ResourceTokenType::ResourceOwner),
            "resource_share" => Ok(ResourceTokenType::ResourceShare),
            "resource_viewer" => Ok(ResourceTokenType::ResourceViewer),
            "folder_owner" => Ok(ResourceTokenType::FolderOwner),
            "folder_share" => Ok(ResourceTokenType::FolderShare),
            "folder_viewer" => Ok(ResourceTokenType::FolderViewer),
            _ => Err(format!("Unknown resource token type: {}", s)),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            ResourceTokenType::ResourceOwner => "resource_owner",
            ResourceTokenType::ResourceShare => "resource_share",
            ResourceTokenType::ResourceViewer => "resource_viewer",
            ResourceTokenType::FolderOwner => "folder_owner",
            ResourceTokenType::FolderShare => "folder_share",
            ResourceTokenType::FolderViewer => "folder_viewer",
        }
    }
}

/// Sync decision for a document (domain concept)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncDecision {
    SendIncrementalUpdates,
    SendFullSnapshot,
    DontSend,
}

/// Resource-level actions (domain concept)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResourceAction {
    GetShareLink,
    RevokeAccess,
    UpdateMetadata,
    Delete,
    Export,
}

impl ResourceAction {
    pub fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "get_share_link" => Ok(ResourceAction::GetShareLink),
            "revoke_access" => Ok(ResourceAction::RevokeAccess),
            "update_metadata" => Ok(ResourceAction::UpdateMetadata),
            "delete" => Ok(ResourceAction::Delete),
            "export" => Ok(ResourceAction::Export),
            _ => Err(format!("Unknown resource action: {}", s)),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            ResourceAction::GetShareLink => "get_share_link",
            ResourceAction::RevokeAccess => "revoke_access",
            ResourceAction::UpdateMetadata => "update_metadata",
            ResourceAction::Delete => "delete",
            ResourceAction::Export => "export",
        }
    }
}

/// Sync behavior facts from UCAN (data structure)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SyncFacts {
    /// Documents that stay local (was: dont_send_to_node)
    pub local_only: Vec<String>,

    /// Documents that don't accept incoming updates (was: no_update_from_node)
    pub no_incoming_updates: Vec<String>,

    /// Documents that send full snapshots (was: full_doc_send)
    pub send_full_snapshot: Vec<String>,
}

/// Document metadata (combines domain types with data)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocMetadata {
    /// Data: document name
    pub name: String,
    /// Domain: Crdt or Asset
    pub doc_type: DocType,
    /// Data: MIME patterns (e.g., ["image/*", "video/*"])
    pub allowed_mimes: Option<Vec<String>>,
    /// Data: size limit in MB
    pub max_size_mb: Option<u64>,
}
