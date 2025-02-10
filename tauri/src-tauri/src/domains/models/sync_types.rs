use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ResourceType {
    Folder,
    Credential,
    Device,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum OperationType {
    Create,
    Update,
    Delete,
    SoftDelete,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SyncStatus {
    Pending,
    Completed,
    Failed,
}

impl ToString for ResourceType {
    fn to_string(&self) -> String {
        match self {
            ResourceType::Folder => "folder".to_string(),
            ResourceType::Credential => "credential".to_string(),
            ResourceType::Device => "device".to_string(),
        }
    }
}

impl ToString for OperationType {
    fn to_string(&self) -> String {
        match self {
            OperationType::Create => "create".to_string(),
            OperationType::Update => "update".to_string(),
            OperationType::Delete => "delete".to_string(),
            OperationType::SoftDelete => "soft_delete".to_string(),
        }
    }
}

impl ToString for SyncStatus {
    fn to_string(&self) -> String {
        match self {
            SyncStatus::Pending => "pending".to_string(),
            SyncStatus::Completed => "completed".to_string(),
            SyncStatus::Failed => "failed".to_string(),
        }
    }
}
impl From<String> for ResourceType {
    fn from(s: String) -> Self {
        match s.as_str() {
            "folder" => ResourceType::Folder,
            "credential" => ResourceType::Credential,
            "device" => ResourceType::Device,
            _ => panic!("Invalid ResourceType string: {}", s),
        }
    }
}

impl From<String> for OperationType {
    fn from(s: String) -> Self {
        match s.as_str() {
            "create" => OperationType::Create,
            "update" => OperationType::Update,
            "delete" => OperationType::Delete,
            "soft_delete" => OperationType::SoftDelete,
            _ => panic!("Invalid OperationType string: {}", s),
        }
    }
}

impl From<String> for SyncStatus {
    fn from(s: String) -> Self {
        match s.as_str() {
            "pending" => SyncStatus::Pending,
            "completed" => SyncStatus::Completed,
            "failed" => SyncStatus::Failed,
            _ => panic!("Invalid SyncStatus string: {}", s),
        }
    }
}
