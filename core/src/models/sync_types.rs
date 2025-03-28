use crate::models::sync_record::{DeviceRecord, DeviceRecordStatus};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ResourceType {
    Folder,
    Resource,
    Device,
    User,
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
            ResourceType::Resource => "resource".to_string(),
            ResourceType::Device => "device".to_string(),
            ResourceType::User => "user".to_string(),
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
            "resource" => ResourceType::Resource,
            "device" => ResourceType::Device,
            "user" => ResourceType::User,
            _ => ResourceType::Resource,
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

/// Result structure for sync operations that need to be performed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncOperations {
    /// Records to be added
    pub records_to_add: Vec<DeviceRecord>,
    /// Record statuses to be added
    pub status_records_to_add: Vec<DeviceRecordStatus>,
    /// IDs of records that need to be updated (synced flag)
    pub record_ids_to_update: Vec<String>,
    /// IDs of statuses that need to be updated (synced flag)
    pub status_ids_to_update: Vec<String>,
}

impl SyncOperations {
    /// Create a new empty set of sync operations
    pub fn new() -> Self {
        Self {
            records_to_add: Vec::new(),
            status_records_to_add: Vec::new(),
            record_ids_to_update: Vec::new(),
            status_ids_to_update: Vec::new(),
        }
    }

    /// Check if there are any operations to perform
    pub fn is_empty(&self) -> bool {
        self.records_to_add.is_empty()
            && self.status_records_to_add.is_empty()
            && self.record_ids_to_update.is_empty()
            && self.status_ids_to_update.is_empty()
    }
}

/// Result structure for the merge operation between local and remote sync records
#[derive(Debug)]
pub struct SyncMergeResult {
    /// Operations that need to be performed locally
    pub local_operations: SyncOperations,
    /// Operations that need to be sent back to the remote
    pub remote_operations: SyncOperations,
}

impl SyncMergeResult {
    /// Create a new empty merge result
    pub fn new() -> Self {
        Self {
            local_operations: SyncOperations::new(),
            remote_operations: SyncOperations::new(),
        }
    }
}
