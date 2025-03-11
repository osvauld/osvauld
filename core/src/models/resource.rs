use crate::models::resource_key::ResourceKey;
use chrono::Local;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::vectorClock::VectorClock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resource {
    pub id: String,
    pub resource_type: String,
    pub data: String,
    pub folder_id: String,
    pub signature: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub favourite: bool,
    pub last_accessed: i64,
    pub deleted: bool,
    pub deleted_at: Option<i64>,
    pub vector_clock: VectorClock,
}

impl Resource {
    pub fn new(resource_type: String, data: String, folder_id: String, signature: String) -> Self {
        let now = Local::now().timestamp_millis();

        Self {
            id: uuid::Uuid::new_v4().to_string(),
            resource_type,
            data,
            folder_id,
            signature,
            created_at: now,
            updated_at: now,
            favourite: false,
            last_accessed: now,
            deleted: false,
            deleted_at: None,
            vector_clock: VectorClock::new(),
        }
    }

    pub fn new_with_user(
        resource_type: String,
        data: String,
        folder_id: String,
        signature: String,
        user_id: &str,
    ) -> Self {
        let mut resource = Self::new(resource_type, data, folder_id, signature);
        resource.vector_clock.increment(user_id);
        resource
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DecryptedResource {
    pub id: String,
    pub resource_type: String,
    pub data: Value,
    pub last_accessed: i64,
    pub favourite: bool,
    pub folder_id: String,
}

#[derive(Debug, Clone)]
pub struct ResourceWithKey {
    pub resource: Resource,
    pub encrypted_key: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceKeyPair {
    pub resource: Resource,
    pub key: ResourceKey,
}
