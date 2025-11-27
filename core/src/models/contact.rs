//! Contact - Known users (peers we've connected with)
//!
//! Stored in Sled as: contacts/{user_did} → ContactData

use chrono::Local;
use serde::{Deserialize, Serialize};

/// ContactData - A known user/peer
///
/// These are users we've established connections with.
/// Stored in Sled contacts tree under key "{user_did}".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContactData {
    /// User's DID (did:key:...)
    pub did: String,
    /// Ed25519 public key
    pub public_key: String,
    /// Display username
    pub username: String,
    /// Public key for UCAN verification (may be different from identity key)
    pub ucan_pub_key: String,
    /// When we added this contact
    pub added_at: i64,
    /// Last time we saw this user online
    pub last_seen_at: Option<i64>,
}

impl ContactData {
    pub fn new(did: String, public_key: String, username: String, ucan_pub_key: String) -> Self {
        Self {
            did,
            public_key,
            username,
            ucan_pub_key,
            added_at: Local::now().timestamp_millis(),
            last_seen_at: None,
        }
    }

    pub fn update_last_seen(&mut self) {
        self.last_seen_at = Some(Local::now().timestamp_millis());
    }
}
