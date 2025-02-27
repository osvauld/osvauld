use chrono::Local;
use serde::{Deserialize, Serialize};
use uuid;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PermissionLevel {
    Read,
    Write,
    Admin,
}

impl ToString for PermissionLevel {
    fn to_string(&self) -> String {
        match self {
            PermissionLevel::Read => "read".to_string(),
            PermissionLevel::Write => "write".to_string(),
            PermissionLevel::Admin => "admin".to_string(),
        }
    }
}

impl From<String> for PermissionLevel {
    fn from(s: String) -> Self {
        match s.as_str() {
            "read" => PermissionLevel::Read,
            "write" => PermissionLevel::Write,
            "admin" => PermissionLevel::Admin,
            _ => PermissionLevel::Read,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ShareStatus {
    Pending,
    Accepted,
    Revoked,
}

impl ToString for ShareStatus {
    fn to_string(&self) -> String {
        match self {
            ShareStatus::Pending => "pending".to_string(),
            ShareStatus::Accepted => "accepted".to_string(),
            ShareStatus::Revoked => "revoked".to_string(),
        }
    }
}

impl From<String> for ShareStatus {
    fn from(s: String) -> Self {
        match s.as_str() {
            "pending" => ShareStatus::Pending,
            "accepted" => ShareStatus::Accepted,
            "revoked" => ShareStatus::Revoked,
            _ => ShareStatus::Pending,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceShare {
    pub id: String,
    pub resource_id: String,
    pub shared_by_user_id: String,
    pub shared_with_user_id: String,
    pub permission_level: PermissionLevel,
    pub share_status: ShareStatus,
    pub created_at: i64,
    pub updated_at: i64,
}

impl ResourceShare {
    pub fn new(
        resource_id: String,
        shared_by_user_id: String,
        shared_with_user_id: String,
        permission_level: PermissionLevel,
    ) -> Self {
        let now = Local::now().timestamp_millis();

        Self {
            id: uuid::Uuid::new_v4().to_string(),
            resource_id,
            shared_by_user_id,
            shared_with_user_id,
            permission_level,
            share_status: ShareStatus::Pending,
            created_at: now,
            updated_at: now,
        }
    }
}
