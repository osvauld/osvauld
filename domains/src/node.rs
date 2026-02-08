//! Node models
//!
//! - SovereignNode: External nodes the owner connects to (stored in SOVEREIGN_NODES)
//! - OwnerInfo: Owner info stored on the node (stored in OWNER_INFO)
//! - ConnectionString: Parsed from base64-encoded JSON for first connection
//!
//! Note: Methods requiring gurkha/herald (space_id, node_did) are provided
//! as extension functions in butler, not on the domain models.

use base64::{engine::general_purpose, Engine as _};
use chrono::Local;
use serde::{Deserialize, Serialize};

/// Type of connection to a node
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ConnectionType {
    /// Owner connecting to their own node (should auto-reconnect)
    Owner,
    /// Viewer connecting to view content (should NOT auto-reconnect)
    Viewer,
}

/// Connection string data (parsed from base64-encoded JSON)
///
/// This is what kunki generates and sthalam parses when adding a sovereign node.
/// Node is a special trusted user - has verifying key (ID) + encryption key.
/// Note: node_id is derived from device_public_key (base64 -> hex).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionString {
    /// Node's Ed25519 verifying key (base64) = ID
    pub node_public_key: String,
    /// Node's X25519 encryption key (base64) - for ECDH
    pub node_encryption_key: String,
    /// Iroh device key for P2P (base64) - node_id is derived from this
    pub device_public_key: String,
    /// Node name
    pub name: String,
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
        let json_str =
            String::from_utf8(decoded).map_err(|e| format!("Invalid UTF-8: {}", e))?;

        serde_json::from_str(&json_str).map_err(|e| format!("Invalid JSON: {}", e))
    }

    /// Get the iroh node_id derived from device_public_key
    pub fn node_id(&self) -> String {
        general_purpose::STANDARD
            .decode(&self.device_public_key)
            .map(|bytes| bytes.iter().map(|b| format!("{:02x}", b)).collect())
            .unwrap_or_else(|_| String::new())
    }
}

/// SovereignNode - An external node we connect to (from owner's perspective)
///
/// Node is a special trusted user with:
/// - did = verifying key (ID)
/// - encryption_key = X25519 for ECDH
///
/// Stored in SOVEREIGN_NODES table under key "{node_id}".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SovereignNode {
    /// Node's verifying key = ID
    pub did: String,
    /// Node's X25519 encryption key (for ECDH)
    pub encryption_key: String,
    /// Node name
    pub name: String,
    /// Iroh NodeId (for P2P connection)
    pub node_id: String,
    /// Iroh device key (base64)
    pub device_public_key: String,
    /// The permit for owner<->node auth (single permit)
    pub permit: Option<String>,
    /// Relay URL for connection
    pub relay_url: Option<String>,
    /// Is currently connected?
    pub is_connected: bool,
    /// Last connection timestamp
    pub last_connected_at: Option<i64>,
    /// When we first added this node
    pub added_at: i64,
    /// Type of connection (Owner or Viewer)
    /// Owner connections auto-reconnect, Viewer connections don't
    #[serde(default = "default_connection_type")]
    pub connection_type: ConnectionType,
}

/// Default to Owner for backwards compatibility with existing stored nodes
fn default_connection_type() -> ConnectionType {
    ConnectionType::Owner
}

impl SovereignNode {
    /// Create a new SovereignNode with pre-computed values
    ///
    /// Use `from_connection_string` in butler for full creation with DID derivation.
    pub fn new(
        did: String,
        encryption_key: String,
        name: String,
        node_id: String,
        device_public_key: String,
        permit: Option<String>,
        relay_url: Option<String>,
        connection_type: ConnectionType,
    ) -> Self {
        Self {
            did,
            encryption_key,
            name,
            node_id,
            device_public_key,
            permit,
            relay_url,
            is_connected: false,
            last_connected_at: None,
            added_at: Local::now().timestamp_millis(),
            connection_type,
        }
    }

    /// Set connected status
    pub fn set_connected(&mut self, connected: bool) {
        self.is_connected = connected;
        if connected {
            self.last_connected_at = Some(Local::now().timestamp_millis());
        }
    }

    /// Set the permit
    pub fn set_permit(&mut self, permit: String) {
        self.permit = Some(permit);
    }
}

/// OwnerInfo - Information about the owner stored on the Node
///
/// Owner is a user with:
/// - did = verifying key (ID)
/// - encryption_key = X25519 for ECDH
///
/// Stored in OWNER_INFO table under key "owner" (only one owner per node).
/// Created during first_connection handshake when owner pairs with node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OwnerInfo {
    /// Owner's verifying key = ID
    pub did: String,
    /// Owner's X25519 encryption key (for ECDH)
    pub encryption_key: String,
    /// Owner's username
    pub username: String,
    /// The permit for owner<->node auth (single permit)
    pub permit: Option<String>,
    /// When the owner first paired with this node
    pub paired_at: i64,
    /// Last time owner connected
    pub last_connected_at: Option<i64>,
}

impl OwnerInfo {
    /// Create new OwnerInfo during first connection
    pub fn new(did: String, encryption_key: String, username: String) -> Self {
        Self {
            did,
            encryption_key,
            username,
            permit: None,
            paired_at: Local::now().timestamp_millis(),
            last_connected_at: None,
        }
    }

    /// Set the permit
    pub fn set_permit(&mut self, permit: String) {
        self.permit = Some(permit);
    }

    /// Update last connected timestamp
    pub fn update_last_connected(&mut self) {
        self.last_connected_at = Some(Local::now().timestamp_millis());
    }
}
