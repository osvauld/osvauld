//! Page - Sub-application instance within a Space
//!
//! Terminology:
//! - Space: Container that groups Pages
//! - Page: A sub-application instance with its own template and Layers
//! - Layer: CRDT data containers within a Page
//!
//! ## Key Design Decisions:
//! - Each Page has one AES-256 key that encrypts ALL its Layers
//! - `encrypted_key` stores the AES key encrypted for THIS user (owner or recipient)
//! - Layers stored at hierarchical keys: `layers/{page_id}/{layer_name}`
//! - Layer names come from permit template's `documents` map

use chrono::Local;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// PageType - Type of page content
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PageType {
    /// Main HUML content
    Content,
    /// Thread comments
    Comments,
    /// Form submissions
    Submissions,
    /// Private chat
    PrivateChat,
}

impl Default for PageType {
    fn default() -> Self {
        Self::Content
    }
}

/// PageMeta - Page metadata stored in redb
///
/// Key design: no `layer_ids` field - layers are accessed via hierarchical keys:
/// `layers/{page_id}/{layer_name}` where layer_name comes from permit template.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageMeta {
    pub id: String,
    pub space_id: String,
    pub name: String,
    pub page_type: PageType,
    /// AES-256 key encrypted for THIS user (owner or recipient)
    /// Each user stores their own encrypted_key in their own DB
    /// Encrypted using X25519 ECIES via herald::encrypt()
    pub encrypted_key: Vec<u8>,
    pub owner_did: String,
    /// For private conversations between owner and viewer
    pub is_private: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

impl PageMeta {
    pub fn new(name: String, space_id: String, owner_did: String) -> Self {
        let now = Local::now().timestamp_millis();
        Self {
            id: Uuid::new_v4().to_string(),
            space_id,
            name,
            page_type: PageType::default(),
            encrypted_key: Vec::new(),
            owner_did,
            is_private: false,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn with_type(mut self, page_type: PageType) -> Self {
        self.page_type = page_type;
        self
    }

    /// Set the encrypted AES key for this page
    pub fn with_encrypted_key(mut self, encrypted_key: Vec<u8>) -> Self {
        self.encrypted_key = encrypted_key;
        self
    }

    pub fn as_private(mut self) -> Self {
        self.is_private = true;
        self
    }
}

/// PageData - Complete page data with embedded shares
///
/// Stored in redb as: pages/{space_id}/{page_id} → PageData
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageData {
    pub meta: PageMeta,
    /// Embedded shares: user_did → UCAN token string
    pub shares: HashMap<String, String>,
}

impl PageData {
    pub fn new(meta: PageMeta) -> Self {
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

/// Page - Runtime page representation (for API responses)
///
/// Note: Does not include `encrypted_key` - that stays in PageMeta
/// for security. This is what gets sent to frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Page {
    pub id: String,
    pub space_id: String,
    pub name: String,
    pub page_type: PageType,
    pub owner_did: String,
    pub is_private: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

impl From<PageMeta> for Page {
    fn from(meta: PageMeta) -> Self {
        Self {
            id: meta.id,
            space_id: meta.space_id,
            name: meta.name,
            page_type: meta.page_type,
            owner_did: meta.owner_did,
            is_private: meta.is_private,
            created_at: meta.created_at,
            updated_at: meta.updated_at,
        }
    }
}

impl From<PageData> for Page {
    fn from(data: PageData) -> Self {
        Self::from(data.meta)
    }
}

/// A fully decrypted Page with all layer data
///
/// This is the domain object returned by Butler after decryption.
/// Contains all data needed by handlers/FE.
#[derive(Debug, Clone)]
pub struct DecryptedPage {
    pub id: String,
    pub space_id: String,
    pub name: String,
    pub page_type: PageType,
    pub owner_did: String,
    pub is_private: bool,
    pub created_at: i64,
    pub updated_at: i64,
    /// The user's permit for this page (if shared)
    pub permit: Option<String>,
    /// Decrypted layer data: layer_name -> raw bytes
    pub docs: HashMap<String, Vec<u8>>,
}

impl DecryptedPage {
    pub fn new(page: Page, permit: Option<String>, docs: HashMap<String, Vec<u8>>) -> Self {
        Self {
            id: page.id,
            space_id: page.space_id,
            name: page.name,
            page_type: page.page_type,
            owner_did: page.owner_did,
            is_private: page.is_private,
            created_at: page.created_at,
            updated_at: page.updated_at,
            permit,
            docs,
        }
    }

    /// Convert docs to JSON format expected by FE: {"doc_name": [byte1, byte2, ...], ...}
    pub fn docs_to_json(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();
        for (name, bytes) in &self.docs {
            // Convert bytes to JSON array of numbers
            let byte_array: Vec<serde_json::Value> = bytes
                .iter()
                .map(|b| serde_json::Value::Number((*b).into()))
                .collect();
            map.insert(name.clone(), serde_json::Value::Array(byte_array));
        }
        serde_json::Value::Object(map)
    }
}
