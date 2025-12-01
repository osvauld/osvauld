//! Contact - Known users (peers we've connected with)
//!
//! Stored in redb as: contacts/{user_did} → ContactData
//! Index: shares_by_page/{page_id}/{user_did} → user_did (for "who has page X" queries)

use chrono::Local;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// ContactData - A known user/peer
///
/// For Node mode: also stores shares we issued TO this user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContactData {
    /// User's DID (did:key:...)
    pub did: String,
    /// Ed25519 public key
    pub public_key: String,
    /// Display username
    pub username: String,
    /// Public key for Permit verification
    pub permit_pub_key: String,
    /// Shares we issued TO this user: page_id → ShareInfo
    pub shares: HashMap<String, ShareInfo>,
    /// When we added this contact
    pub added_at: i64,
}

/// Information about a share we issued to a contact
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareInfo {
    /// The permit token we issued
    pub permit: String,
    /// Parent space ID
    pub space_id: String,
    /// Template used (descriptive: "viewer", "collaborator", etc.)
    pub template_key: String,
    /// When the permit was issued
    pub issued_at: i64,
}

impl ContactData {
    pub fn new(did: String, public_key: String, username: String, permit_pub_key: String) -> Self {
        Self {
            did,
            public_key,
            username,
            permit_pub_key,
            shares: HashMap::new(),
            added_at: Local::now().timestamp_millis(),
        }
    }

    /// Add or update a share for a page
    pub fn add_share(&mut self, page_id: String, share: ShareInfo) {
        self.shares.insert(page_id, share);
    }

    /// Get all page IDs this contact has shares for
    pub fn shared_page_ids(&self) -> Vec<String> {
        self.shares.keys().cloned().collect()
    }
}

impl ShareInfo {
    pub fn new(permit: String, space_id: String, template_key: String) -> Self {
        Self {
            permit,
            space_id,
            template_key,
            issued_at: Local::now().timestamp_millis(),
        }
    }
}
