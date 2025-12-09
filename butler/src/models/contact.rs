//! Contact - Known users (peers we've connected with) or nodes (for viewer → node connections)
//!
//! Stored in redb as: CONTACTS/{did} → ContactData

use chrono::Local;
use serde::{Deserialize, Serialize};

/// Type of contact
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ContactType {
    /// Regular user contact
    User,
    /// Node contact (for viewer → node connections)
    Node,
}

impl Default for ContactType {
    fn default() -> Self {
        Self::User
    }
}

/// Device info for a contact
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    /// Device ID (single key)
    pub id: String,
    /// Device name
    pub name: String,
}

/// ContactData - A known user/peer or node
///
/// All key-based identity:
/// - did = verifying key (Ed25519)
/// - encryption_key = X25519 for ECDH
///
/// For Node type contacts (viewer → node):
/// - node_id = Iroh NodeId for P2P connection
/// - permit = Authentication permit for the node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContactData {
    /// User's DID (verifying key = ID)
    pub did: String,
    /// X25519 encryption public key for ECDH
    pub encryption_key: String,
    /// Display username/name
    pub username: String,
    /// User's devices
    pub devices: Vec<DeviceInfo>,
    /// When we added this contact
    pub added_at: i64,
    /// Type of contact (User or Node)
    pub contact_type: ContactType,
    /// Iroh NodeId for P2P connection (only for Node type)
    pub node_id: Option<String>,
    /// Permit for authenticating to this node (only for Node type)
    pub permit: Option<String>,
}

impl ContactData {
    pub fn new(did: String, encryption_key: String, username: String) -> Self {
        Self {
            did,
            encryption_key,
            username,
            devices: Vec::new(),
            added_at: Local::now().timestamp_millis(),
            contact_type: ContactType::User,
            node_id: None,
            permit: None,
        }
    }

    /// Create a node contact (for viewer → node connections)
    pub fn new_node(
        did: String,
        encryption_key: String,
        name: String,
        node_id: String,
        permit: String,
    ) -> Self {
        Self {
            did,
            encryption_key,
            username: name,
            devices: Vec::new(),
            added_at: Local::now().timestamp_millis(),
            contact_type: ContactType::Node,
            node_id: Some(node_id),
            permit: Some(permit),
        }
    }

    /// Add a device to this contact
    pub fn add_device(&mut self, id: String, name: String) {
        // Check if device already exists
        if !self.devices.iter().any(|d| d.id == id) {
            self.devices.push(DeviceInfo { id, name });
        }
    }

    /// Get device by ID
    pub fn get_device(&self, device_id: &str) -> Option<&DeviceInfo> {
        self.devices.iter().find(|d| d.id == device_id)
    }

    /// Check if this is a node contact
    pub fn is_node(&self) -> bool {
        self.contact_type == ContactType::Node
    }
}

impl DeviceInfo {
    pub fn new(id: String, name: String) -> Self {
        Self { id, name }
    }
}
