//! Space - Container for Pages (sub-applications)
//!
//! Terminology:
//! - Space: Container that groups Pages and defines permission boundaries
//! - Page: A sub-application instance with its own template
//! - Layer: CRDT data containers within a Page

use chrono::Local;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// SpaceMeta - Space metadata stored in redb
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpaceMeta {
    pub id: String,
    pub name: String,
    pub parent_space_id: Option<String>,
    pub owner_did: String,
    pub is_default: bool,
    pub description: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

impl SpaceMeta {
    pub fn new(name: String, owner_did: String) -> Self {
        let now = Local::now().timestamp_millis();
        Self {
            id: Uuid::new_v4().to_string(),
            name,
            parent_space_id: None,
            owner_did,
            is_default: false,
            description: None,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn with_parent(mut self, parent_id: String) -> Self {
        self.parent_space_id = Some(parent_id);
        self
    }

    pub fn with_description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
    }

    pub fn as_default(mut self) -> Self {
        self.is_default = true;
        self
    }
}

/// SpaceData - Complete space data with permit and share tracking
///
/// Stored in redb as: spaces/{space_id} → SpaceData
///
/// Key design:
/// - `permit` stores MY permit for this space (context-dependent: owner's, node's, or viewer's)
/// - `shares` tracks WHO I've shared with (just pub keys for UI/tracking)
/// - `source_node_id` tracks which node gave us this space (for viewers to sync back)
/// - Actual permits issued to others are stored in Contact.shares (Node-side)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpaceData {
    pub meta: SpaceMeta,
    /// My permit for this space (self-issued for owner, received from parent for others)
    pub permit: Option<String>,
    /// Pub keys of users this space is shared with (tracking only)
    pub shares: Vec<String>,
    /// Node that gave us this space (for viewers to sync back)
    /// None for owner-created spaces, Some(node_id) for received spaces
    #[serde(default)]
    pub source_node_id: Option<String>,
}

impl SpaceData {
    pub fn new(meta: SpaceMeta) -> Self {
        Self {
            meta,
            permit: None,
            shares: Vec::new(),
            source_node_id: None,
        }
    }

    /// Create SpaceData with a source node (for received spaces)
    pub fn with_source(meta: SpaceMeta, source_node_id: String) -> Self {
        Self {
            meta,
            permit: None,
            shares: Vec::new(),
            source_node_id: Some(source_node_id),
        }
    }

    /// Set the source node for this space
    pub fn set_source_node(&mut self, node_id: String) {
        self.source_node_id = Some(node_id);
    }

    /// Set my permit for this space
    pub fn set_permit(&mut self, permit: String) {
        self.permit = Some(permit);
    }

    /// Get my permit for this space
    pub fn get_permit(&self) -> Option<&String> {
        self.permit.as_ref()
    }

    /// Track that we shared with a user (stores pub key only)
    pub fn add_share(&mut self, user_pubkey: String) {
        if !self.shares.contains(&user_pubkey) {
            self.shares.push(user_pubkey);
        }
    }

    /// Remove share tracking for a user
    pub fn remove_share(&mut self, user_pubkey: &str) -> bool {
        if let Some(pos) = self.shares.iter().position(|k| k == user_pubkey) {
            self.shares.remove(pos);
            true
        } else {
            false
        }
    }

    /// Check if we've shared with this user
    pub fn has_share(&self, user_pubkey: &str) -> bool {
        self.shares.contains(&user_pubkey.to_string())
    }
}

/// Space - Runtime space representation (for API responses)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Space {
    pub id: String,
    pub name: String,
    pub parent_space_id: Option<String>,
    pub owner_did: String,
    pub is_default: bool,
    pub description: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

impl From<SpaceMeta> for Space {
    fn from(meta: SpaceMeta) -> Self {
        Self {
            id: meta.id,
            name: meta.name,
            parent_space_id: meta.parent_space_id,
            owner_did: meta.owner_did,
            is_default: meta.is_default,
            description: meta.description,
            created_at: meta.created_at,
            updated_at: meta.updated_at,
        }
    }
}

impl From<SpaceData> for Space {
    fn from(data: SpaceData) -> Self {
        Self::from(data.meta)
    }
}
