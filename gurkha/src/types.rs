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

// Role enum removed in v3 migration - identity derived from facts instead
// Relationship stored as string in facts: "owner", "node", "viewer", "user"

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

// ConnectionTokenType and ResourceTokenType enums removed in v3 migration
// Token types now stored as strings in facts.token_type field:
// - Connection: "one_time_connection", "owner_connection", "node_connection", "viewer_auth", "viewer_connection"
// - Resource/Space: "resource_owner", "resource_share", "resource_viewer", "space_owner", "space_share", "space_viewer"

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
