use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ShareOperation {
    Share,
    Revoke,
    UpdatePermission,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ShareStatus {
    Pending,
    Accepted,
    Rejected,
    Completed,
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
            ShareOperation::UpdatePermission => "update_permission".to_string(),
        }
    }
}

impl ToString for ShareStatus {
    fn to_string(&self) -> String {
        match self {
            ShareStatus::Pending => "pending".to_string(),
            ShareStatus::Accepted => "accepted".to_string(),
            ShareStatus::Rejected => "rejected".to_string(),
            ShareStatus::Completed => "completed".to_string(),
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

impl From<String> for ShareOperation {
    fn from(s: String) -> Self {
        match s.as_str() {
            "share" => ShareOperation::Share,
            "revoke" => ShareOperation::Revoke,
            "update_permission" => ShareOperation::UpdatePermission,
            _ => panic!("Invalid ShareOperation string: {}", s),
        }
    }
}

impl From<String> for ShareStatus {
    fn from(s: String) -> Self {
        match s.as_str() {
            "pending" => ShareStatus::Pending,
            "accepted" => ShareStatus::Accepted,
            "rejected" => ShareStatus::Rejected,
            "completed" => ShareStatus::Completed,
            _ => panic!("Invalid ShareStatus string: {}", s),
        }
    }
}

impl From<String> for PermissionLevel {
    fn from(s: String) -> Self {
        match s.as_str() {
            "read" => PermissionLevel::Read,
            "write" => PermissionLevel::Write,
            "admin" => PermissionLevel::Admin,
            _ => panic!("Invalid PermissionLevel string: {}", s),
        }
    }
}
