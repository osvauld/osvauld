use chrono::Local;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceKey {
    pub id: String,
    pub resource_id: String,
    pub user_id: String,
    pub encrypted_key: String,
    pub is_owner: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

impl ResourceKey {
    pub fn new(
        resource_id: String,
        user_id: String,
        encrypted_key: String,
        is_owner: bool,
    ) -> Self {
        let now = Local::now().timestamp_millis();

        Self {
            id: uuid::Uuid::new_v4().to_string(),
            resource_id,
            user_id,
            encrypted_key,
            is_owner,
            created_at: now,
            updated_at: now,
        }
    }
}
