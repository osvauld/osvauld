use chrono::Local;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

use super::Device;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UserRole {
    User,
    Owner,
    Viewer,
    Server,
    Owned,
}

impl fmt::Display for UserRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UserRole::User => write!(f, "user"),
            UserRole::Owner => write!(f, "owner"),
            UserRole::Viewer => write!(f, "viewer"),
            UserRole::Server => write!(f, "server"),
            UserRole::Owned => write!(f, "owned"),
        }
    }
}

impl FromStr for UserRole {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "user" => Ok(UserRole::User),
            "owner" => Ok(UserRole::Owner),
            "viewer" => Ok(UserRole::Viewer),
            "server" => Ok(UserRole::Server),
            "owned" => Ok(UserRole::Owned),
            _ => Err(format!("Invalid user role: {}", s)),
        }
    }
}

impl Default for UserRole {
    fn default() -> Self {
        UserRole::Viewer
    }
}

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
