//! Node - Node registry (which users have Kunki nodes)
//!
//! Stored in Sled as: nodes/{user_did} → NodeInfo

use chrono::Local;
use serde::{Deserialize, Serialize};

/// NodeInfo - Information about a user's Kunki node
///
/// Stored in Sled nodes tree under key "{user_did}".
/// Used for P2P routing and discovering available nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    /// The user who owns this node
    pub user_did: String,
    /// The node's own DID (nodes have separate identity from user)
    pub node_did: String,
    /// Which device is the node
    pub device_id: String,
    /// Iroh network node ID for P2P connections
    pub iroh_node_id: String,
    /// Serialized NodeAddr for direct connection (optional)
    pub node_addr: Option<String>,
    /// Is the node currently online?
    pub is_online: bool,
    /// Last time we saw this node
    pub last_seen_at: Option<i64>,
    /// When this node was registered
    pub registered_at: i64,
}

impl NodeInfo {
    pub fn new(
        user_did: String,
        node_did: String,
        device_id: String,
        iroh_node_id: String,
    ) -> Self {
        Self {
            user_did,
            node_did,
            device_id,
            iroh_node_id,
            node_addr: None,
            is_online: false,
            last_seen_at: None,
            registered_at: Local::now().timestamp_millis(),
        }
    }

    pub fn with_node_addr(mut self, addr: String) -> Self {
        self.node_addr = Some(addr);
        self
    }

    pub fn set_online(&mut self, online: bool) {
        self.is_online = online;
        if online {
            self.last_seen_at = Some(Local::now().timestamp_millis());
        }
    }

    pub fn update_last_seen(&mut self) {
        self.last_seen_at = Some(Local::now().timestamp_millis());
    }
}
