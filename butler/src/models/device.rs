//! Device2 - New device models for Butler/Sled storage
//!
//! Replaces old Device model after migration complete.

use chrono::Local;
use serde::{Deserialize, Serialize};

/// Device2 - Device data for our own devices
///
/// Stored in Sled as: devices/{device_id} → DeviceData
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceData {
    pub id: String,
    /// Device's public key
    pub device_key: String,
    /// Human-readable name ("iPhone", "Laptop", etc.)
    pub name: Option<String>,
    /// Is this the device we're currently running on?
    pub is_current: bool,
    /// Is this device a Kunki node?
    pub is_node: bool,
    /// Last time this device was seen online
    pub last_seen_at: Option<i64>,
    pub created_at: i64,
}

impl DeviceData {
    pub fn new(id: String, device_key: String) -> Self {
        Self {
            id,
            device_key,
            name: None,
            is_current: false,
            is_node: false,
            last_seen_at: None,
            created_at: Local::now().timestamp_millis(),
        }
    }

    pub fn with_name(mut self, name: String) -> Self {
        self.name = Some(name);
        self
    }

    pub fn as_current(mut self) -> Self {
        self.is_current = true;
        self
    }

    pub fn as_node(mut self) -> Self {
        self.is_node = true;
        self
    }

    pub fn update_last_seen(&mut self) {
        self.last_seen_at = Some(Local::now().timestamp_millis());
    }
}
