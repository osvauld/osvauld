use chrono::Local;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Credential {
    pub id: String,
    pub credential_type: String,
    pub data: String,
    pub folder_id: String,
    pub signature: String,
    pub encrypted_key: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub favorite: bool,
    pub last_accessed: i64,
    pub deleted: bool,
    pub deleted_at: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct DecryptedCredential {
    pub id: String,
    pub credential_type: String,
    pub data: Value,
    pub last_accessed: i64,
    pub favourite: bool,
    pub folder_id: String,
}

impl Credential {
    pub fn new(
        credential_type: String,
        data: String,
        folder_id: String,
        encrypted_key: String,
        signature: String,
    ) -> Self {
        let now = Local::now().timestamp_millis();

        Self {
            id: uuid::Uuid::new_v4().to_string(),
            credential_type,
            data,
            folder_id,
            signature,
            encrypted_key,
            created_at: now.clone(),
            updated_at: now,
            favorite: false,
            last_accessed: now,
            deleted: false,
            deleted_at: None,
        }
    }
}
