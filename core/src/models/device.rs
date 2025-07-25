use base64::{Engine as _, engine::general_purpose};
use chrono::Local;
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Device {
    pub id: String,
    pub device_key: String,
    pub user_id: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub last_synced_at: Option<i64>,
}

impl Device {
    pub fn new(id: String, device_public_key: String, user_id: String) -> Self {
        let now = Local::now().timestamp_millis();

        Self {
            id,
            device_key: device_public_key,
            user_id,
            created_at: now,
            updated_at: now,
            last_synced_at: None,
        }
    }
    pub fn get_client_id(&self) -> Result<u32, Box<dyn std::error::Error>> {
        // Decode base64
        let decoded = general_purpose::STANDARD.decode(&self.id)?;

        // Take first 4 bytes
        if decoded.len() < 4 {
            return Err("Device ID too short - need at least 4 bytes".into());
        }

        let first_four_bytes = &decoded[0..4];

        // Convert to u32 (big-endian interpretation)
        let client_id = u32::from_be_bytes([
            first_four_bytes[0],
            first_four_bytes[1],
            first_four_bytes[2],
            first_four_bytes[3],
        ]);

        Ok(client_id)
    }
}
