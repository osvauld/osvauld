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
}
