// In share_record.rs
use chrono::Local;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ShareOperation {
    Share,
    Revoke,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum PermissionLevel {
    Read,
    Write,
    Admin,
}

impl ToString for ShareOperation {
    fn to_string(&self) -> String {
        match self {
            ShareOperation::Share => "share".to_string(),
            ShareOperation::Revoke => "revoke".to_string(),
        }
    }
}

impl From<String> for ShareOperation {
    fn from(s: String) -> Self {
        match s.as_str() {
            "share" => ShareOperation::Share,
            "revoke" => ShareOperation::Revoke,
            _ => ShareOperation::Share, // Default to share if invalid
        }
    }
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
            _ => PermissionLevel::Read, // Default to read if invalid
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareRecord {
    pub id: String,
    pub resource_id: String,
    pub shared_by_user_id: String,
    pub recipient_user_id: String,
    pub permission_level: PermissionLevel,
    pub ucan_token: String,
    pub ucan_cid: String,
    pub operation_type: ShareOperation,
    pub created_at: i64,
    pub updated_at: i64,
}

impl ShareRecord {
    pub fn prepare_share_record(
        resource_id: String,
        shared_by_user_id: String,
        recipient_user_id: String,
        permission_level: PermissionLevel,
        ucan_token: String,
        ucan_cid: String,
    ) -> ShareRecord {
        let now = Local::now().timestamp_millis();

        ShareRecord {
            id: Uuid::new_v4().to_string(),
            resource_id,
            shared_by_user_id,
            recipient_user_id,
            permission_level,
            ucan_token,
            ucan_cid,
            operation_type: ShareOperation::Share,
            created_at: now,
            updated_at: now,
        }
    }
}
