//! Node - Node registry (which users have Kunki nodes)
//!
//! Stored in redb as: nodes/{user_did} → NodeInfo
//! Sovereign nodes: sovereign_nodes/{node_id} → SovereignNode

use base64::{engine::general_purpose, Engine as _};
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

// ==================== CONNECTION STRING ====================

/// Connection string data (parsed from base64-encoded JSON)
///
/// This is what kunki generates and sthalam parses when adding a sovereign node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionString {
    /// User's Ed25519 public signing key (base64)
    pub user_public_key: String,
    /// Device's X25519 public key (base64)
    pub device_public_key: String,
    /// Iroh NodeId (hex string)
    pub node_id: String,
    /// Username
    pub username: String,
    /// One-time permit (UCAN token)
    pub permit: String,
    /// Optional relay URL
    pub relay: Option<String>,
}

impl ConnectionString {
    /// Parse a connection string from base64-encoded JSON
    pub fn parse(input: &str) -> Result<Self, String> {
        // Decode base64
        let decoded = general_purpose::STANDARD
            .decode(input.trim())
            .map_err(|e| format!("Invalid base64: {}", e))?;

        // Parse JSON
        let json_str = String::from_utf8(decoded)
            .map_err(|e| format!("Invalid UTF-8: {}", e))?;

        serde_json::from_str(&json_str)
            .map_err(|e| format!("Invalid JSON: {}", e))
    }
}

// ==================== SOVEREIGN NODE ====================

/// SovereignNode - An external node we connect to (from sthalam's perspective)
///
/// Stored in redb sovereign_nodes table under key "{node_id}".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SovereignNode {
    /// Iroh NodeId (primary key)
    pub node_id: String,
    /// Username of the node owner
    pub username: String,
    /// User's Ed25519 public signing key (base64)
    pub user_public_key: String,
    /// Device's X25519 public key (base64)
    pub device_public_key: String,
    /// The permit we received from them (for first connection)
    pub their_permit: String,
    /// The permit they gave us (long-lived, after handshake) - for reconnecting TO them
    pub our_permit: Option<String>,
    /// The permit we issued TO them (long-lived) - they use this to reconnect to us
    pub permit_for_them: Option<String>,
    /// Relay URL for connection
    pub relay_url: Option<String>,
    /// Is currently connected?
    pub is_connected: bool,
    /// Last connection timestamp
    pub last_connected_at: Option<i64>,
    /// When we first added this node
    pub added_at: i64,
}

impl SovereignNode {
    /// Create a new SovereignNode from a connection string
    pub fn from_connection_string(conn: &ConnectionString) -> Self {
        Self {
            node_id: conn.node_id.clone(),
            username: conn.username.clone(),
            user_public_key: conn.user_public_key.clone(),
            device_public_key: conn.device_public_key.clone(),
            their_permit: conn.permit.clone(),
            our_permit: None,
            permit_for_them: None,
            relay_url: conn.relay.clone(),
            is_connected: false,
            last_connected_at: None,
            added_at: Local::now().timestamp_millis(),
        }
    }

    /// Set connected status
    pub fn set_connected(&mut self, connected: bool) {
        self.is_connected = connected;
        if connected {
            self.last_connected_at = Some(Local::now().timestamp_millis());
        }
    }

    /// Store the long-lived permit we received after handshake
    pub fn set_our_permit(&mut self, permit: String) {
        self.our_permit = Some(permit);
    }

    /// Set the permit we issued TO them
    pub fn set_permit_for_them(&mut self, permit: String) {
        self.permit_for_them = Some(permit);
    }
}

// ==================== OWNER INFO (for Node side) ====================

/// OwnerInfo - Information about the owner stored on the Node
///
/// Stored in redb owner_info table under key "owner" (only one owner per node).
/// Created during first_connection handshake when owner pairs with node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OwnerInfo {
    /// The owner's DID
    pub owner_did: String,
    /// The owner's username
    pub username: String,
    /// The owner's Ed25519 signing public key
    pub signing_public_key: Vec<u8>,
    /// The owner's X25519 encryption public key (optional)
    pub encryption_public_key: Option<Vec<u8>>,
    /// The owner's iroh device public key
    pub device_public_key: Vec<u8>,
    /// The permit we issued TO the owner
    pub permit_for_owner: Option<String>,
    /// The permit we received FROM the owner
    pub permit_from_owner: Option<String>,
    /// When the owner first paired with this node
    pub paired_at: i64,
    /// Last time owner connected
    pub last_connected_at: Option<i64>,
}

impl OwnerInfo {
    /// Create new OwnerInfo during first connection
    pub fn new(
        owner_did: String,
        username: String,
        signing_public_key: Vec<u8>,
        device_public_key: Vec<u8>,
    ) -> Self {
        Self {
            owner_did,
            username,
            signing_public_key,
            encryption_public_key: None,
            device_public_key,
            permit_for_owner: None,
            permit_from_owner: None,
            paired_at: Local::now().timestamp_millis(),
            last_connected_at: None,
        }
    }

    /// Store the permit we issued TO the owner
    pub fn set_permit_for_owner(&mut self, permit: String) {
        self.permit_for_owner = Some(permit);
    }

    /// Store the permit we received FROM the owner
    pub fn set_permit_from_owner(&mut self, permit: String) {
        self.permit_from_owner = Some(permit);
    }

    /// Update last connected timestamp
    pub fn update_last_connected(&mut self) {
        self.last_connected_at = Some(Local::now().timestamp_millis());
    }
}
