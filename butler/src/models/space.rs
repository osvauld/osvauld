//! Space - Container for Pages (sub-applications)
//!
//! Terminology:
//! - Space: Container that groups Pages and defines permission boundaries
//! - Page: A sub-application instance with its own template
//! - Layer: CRDT data containers within a Page

use chrono::Local;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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

/// SpaceData - Complete space data with embedded shares
///
/// Stored in redb as: spaces/{space_id} → SpaceData
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpaceData {
    pub meta: SpaceMeta,
    /// Embedded shares: user_did → UCAN token string
    pub shares: HashMap<String, String>,
}

impl SpaceData {
    pub fn new(meta: SpaceMeta) -> Self {
        Self {
            meta,
            shares: HashMap::new(),
        }
    }

    /// Add or update a share for a user
    pub fn add_share(&mut self, user_did: String, ucan: String) {
        self.shares.insert(user_did, ucan);
    }

    /// Remove a share for a user
    pub fn remove_share(&mut self, user_did: &str) -> Option<String> {
        self.shares.remove(user_did)
    }

    /// Check if a user has access
    pub fn has_share(&self, user_did: &str) -> bool {
        self.shares.contains_key(user_did)
    }

    /// Get UCAN for a user
    pub fn get_share(&self, user_did: &str) -> Option<&String> {
        self.shares.get(user_did)
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
