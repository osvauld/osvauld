use crate::domains::models::device::Device;
use chrono::Local;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ResourceType {
    Folder,
    Credential,
    SyncStatus,
    Device,
    Record,
}

impl ToString for ResourceType {
    fn to_string(&self) -> String {
        match self {
            ResourceType::Folder => "folder".to_string(),
            ResourceType::Credential => "credential".to_string(),
            ResourceType::SyncStatus => "sync_status".to_string(),
            ResourceType::Device => "device".to_string(),
            ResourceType::Record => "record".to_string(),
        }
    }
}
impl From<String> for ResourceType {
    fn from(s: String) -> Self {
        match s.as_str() {
            "folder" => ResourceType::Folder,
            "credential" => ResourceType::Credential,
            "sync_status" => ResourceType::SyncStatus,
            "device" => ResourceType::Device,
            "record" => ResourceType::Record,
            _ => ResourceType::Folder, // Default case - or you could use Option/Result instead
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum OperationType {
    Create,
    Update,
    Delete,
    SoftDelete,
    StatusChange,
}

impl ToString for OperationType {
    fn to_string(&self) -> String {
        match self {
            OperationType::Create => "create".to_string(),
            OperationType::Update => "update".to_string(),
            OperationType::Delete => "delete".to_string(),
            OperationType::StatusChange => "status_change".to_string(),
            OperationType::SoftDelete => "soft_delete".to_string(),
        }
    }
}

impl From<String> for OperationType {
    fn from(s: String) -> Self {
        match s.as_str() {
            "create" => OperationType::Create,
            "update" => OperationType::Update,
            "delete" => OperationType::Delete,
            "status_change" => OperationType::StatusChange,
            "soft_delete" => OperationType::SoftDelete,
            _ => panic!("Invalid OperationType string: {}", s), // You might want to handle this differently
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SyncStatus {
    Pending,
    Completed,
    Failed,
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

impl From<String> for SyncStatus {
    fn from(s: String) -> Self {
        match s.as_str() {
            "pending" => SyncStatus::Pending,
            "completed" => SyncStatus::Completed,
            "failed" => SyncStatus::Failed,
            _ => todo!(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncRecord {
    pub id: String,
    pub resource_id: String,
    pub resource_type: ResourceType,
    pub operation_type: OperationType,
    pub source_device_id: String,
    pub target_device_id: String,
    pub status: SyncStatus,
    pub folder_id: Option<String>,
    pub credential_id: Option<String>,
    pub synced_from: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

impl SyncRecord {
    pub fn new_folder_sync(
        folder_id: String,
        operation_type: OperationType,
        source_device_id: String,
        target_device_id: String,
    ) -> Self {
        let now = Local::now().timestamp_millis();
        Self {
            id: Uuid::new_v4().to_string(),
            resource_id: folder_id.clone(),
            resource_type: ResourceType::Folder,
            operation_type,
            source_device_id,
            target_device_id,
            status: SyncStatus::Pending,
            folder_id: Some(folder_id),
            credential_id: None,
            created_at: now,
            updated_at: now,
            synced_from: None,
        }
    }

    pub fn new_credential_sync(
        credential_id: String,
        operation_type: OperationType,
        source_device_id: String,
        target_device_id: String,
    ) -> Self {
        let now = Local::now().timestamp_millis();
        Self {
            id: Uuid::new_v4().to_string(),
            resource_id: credential_id.clone(),
            resource_type: ResourceType::Credential,
            operation_type,
            source_device_id,
            target_device_id,
            status: SyncStatus::Pending,
            folder_id: None,
            credential_id: Some(credential_id),
            created_at: now,
            updated_at: now,
            synced_from: None,
        }
    }

    pub fn update_status(&mut self, new_status: SyncStatus) {
        self.status = new_status;
        self.updated_at = Local::now().timestamp_millis();
    }
    pub fn create_new_sync_for_device(&self, new_device_id: String) -> Self {
        let now = Local::now().timestamp_millis();
        Self {
            id: Uuid::new_v4().to_string(), // New unique ID for this sync record
            resource_id: self.resource_id.clone(), // Same resource being synced
            resource_type: self.resource_type.clone(),
            operation_type: self.operation_type.clone(),
            source_device_id: self.source_device_id.clone(),
            target_device_id: new_device_id,   // New target device
            status: SyncStatus::Pending,       // Always starts as pending
            folder_id: self.folder_id.clone(), // Maintain folder relationship if any
            credential_id: self.credential_id.clone(), // Maintain credential relationship if any
            created_at: now,                   // New timestamps for this sync record
            updated_at: now,
            synced_from: None,
        }
    }

    /// Creates new SyncRecord objects for syncing multiple records to another device
    pub fn create_new_syncs_for_device(records: &[SyncRecord], new_device_id: String) -> Vec<Self> {
        records
            .iter()
            .map(|record| record.create_new_sync_for_device(new_device_id.clone()))
            .collect()
    }

    pub fn new_device_sync(
        device_id: String,
        operation_type: OperationType,
        source_device_id: String,
        target_device_id: String,
    ) -> Self {
        let now = Local::now().timestamp_millis();
        Self {
            id: Uuid::new_v4().to_string(),
            resource_id: device_id.clone(),
            resource_type: ResourceType::Device,
            operation_type,
            source_device_id,
            target_device_id,
            status: SyncStatus::Pending,
            folder_id: None,
            credential_id: None,
            created_at: now,
            updated_at: now,
            synced_from: None,
        }
    }
    pub fn create_status_change_records(
        completed_sync: &SyncRecord,
        current_device_id: String,
        devices: Vec<Device>,
    ) -> Vec<SyncRecord> {
        let now = Local::now().timestamp_millis();

        devices
            .into_iter()
            .filter(|device| {
                // Filter out both source and target devices of the original sync
                device.id != completed_sync.source_device_id
                    && device.id != completed_sync.target_device_id
            })
            .map(|device| {
                SyncRecord {
                    id: Uuid::new_v4().to_string(),
                    resource_id: completed_sync.id.clone(),
                    resource_type: ResourceType::Record,
                    operation_type: OperationType::StatusChange,
                    source_device_id: current_device_id.clone(),
                    target_device_id: device.id,
                    status: SyncStatus::Pending,
                    folder_id: None,
                    credential_id: None,
                    synced_from: None, // Changed this to None since it's a status notification
                    created_at: now,
                    updated_at: now,
                }
            })
            .collect()
    }

    pub fn create_sync_record_sync(
        sync_record: &SyncRecord,
        source_device_id: String,
        target_device_id: String,
    ) -> SyncRecord {
        let now = Local::now().timestamp_millis();

        SyncRecord {
            id: Uuid::new_v4().to_string(),
            resource_id: sync_record.id.clone(),
            resource_type: ResourceType::Record,
            operation_type: OperationType::Create,
            source_device_id: source_device_id.clone(),
            target_device_id,
            status: SyncStatus::Pending,
            folder_id: None,
            credential_id: None,
            synced_from: Some(source_device_id),
            created_at: now,
            updated_at: now,
        }
    }
}
