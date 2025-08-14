use chrono::Local;
use serde::{Deserialize, Serialize};

use super::Device;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub id: String,
    pub username: String,
    pub public_key: String,
    pub created_at: i64,
    pub signature: String,
    pub ucan_token: String,
    pub ucan_pub_key: String,
    pub ucan_cid: String,
    pub first_sync: bool,
    pub updated_at: i64,
    pub owner: bool,
    pub deleted: bool,
    pub deleted_at: Option<i64>,
}

impl User {
    pub fn new(
        username: String,
        id: String,
        public_key: String,
        signature: String,
        owner: bool,
        first_sync: bool,
        ucan_token: String,
        ucan_cid: String,
        ucan_pub_key: String,
    ) -> Self {
        let now = Local::now().timestamp_millis();

        Self {
            id,
            username,
            public_key,
            signature,
            owner,
            first_sync,
            ucan_token,
            ucan_pub_key,
            ucan_cid,
            created_at: now,
            updated_at: now,
            deleted: false,
            deleted_at: None,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserWithDeviceIds {
    pub user_id: String,
    pub device_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserWithDevices {
    pub user: User,
    pub devices: Vec<Device>,
}
