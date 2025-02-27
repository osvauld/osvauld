use chrono::Local;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub id: String,
    pub username: String,
    pub public_key: String,
    pub created_at: i64,
    pub signature: String,
    pub updated_at: i64,
    pub owner: bool,
    pub deleted: bool,
    pub deleted_at: Option<i64>,
}

impl User {
    pub fn new(username: String, id: String, public_key: String, signature: String) -> Self {
        let now = Local::now().timestamp_millis();

        Self {
            id,
            username,
            public_key,
            signature,
            owner: false,
            created_at: now,
            updated_at: now,
            deleted: false,
            deleted_at: None,
        }
    }
}
