use serde::{Deserialize, Serialize};

/// Permission level for layer access (simplified for Lua FFI)
/// Hierarchy: Admin > ReadWrite > ReadOnly
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Capability {
    /// Read-only access, no write permissions
    /// Lowest privilege
    ReadOnly,

    /// Read and write access to layer
    ReadWrite,

    /// Full administrative access
    /// Highest privilege
    Admin,
}

impl Capability {
    pub fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "read_only" | "readonly" | "viewer" => Ok(Capability::ReadOnly),
            "read_write" | "readwrite" | "collaborator" => Ok(Capability::ReadWrite),
            "admin" | "owner" => Ok(Capability::Admin),
            _ => Err(format!("Unknown capability: {}", s)),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Capability::ReadOnly => "read_only",
            Capability::ReadWrite => "read_write",
            Capability::Admin => "admin",
        }
    }

    pub fn can_write(&self) -> bool {
        matches!(self, Capability::ReadWrite | Capability::Admin)
    }

    pub fn can_admin(&self) -> bool {
        matches!(self, Capability::Admin)
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
    #[serde(default)]
    pub local_only: Vec<String>,

    /// Documents that don't accept incoming updates (was: no_update_from_node)
    #[serde(default)]
    pub no_incoming_updates: Vec<String>,

    /// Documents that send full snapshots (was: full_doc_send)
    #[serde(default)]
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
