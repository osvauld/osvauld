use crate::database::schema::{credentials, devices, folders, sync_records};
use crate::domains::models::{
    credential::Credential as DomainCredential,
    device::Device as DomainDevice,
    folder::Folder as DomainFolder,
    sync_record::{SyncRecord as DomainSyncRecord, SyncStatus},
};
use diesel::prelude::*;

#[derive(Queryable, Insertable)]
#[diesel(table_name = folders)]
pub struct FolderModel {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub deleted: bool,
    pub deleted_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

impl From<&DomainFolder> for FolderModel {
    fn from(folder: &DomainFolder) -> Self {
        Self {
            id: folder.id.clone(),
            name: folder.name.clone(),
            description: folder.description.clone(),
            deleted: folder.deleted,
            deleted_at: folder.deleted_at,
            created_at: folder.created_at,
            updated_at: folder.updated_at,
        }
    }
}

impl From<FolderModel> for DomainFolder {
    fn from(model: FolderModel) -> Self {
        Self {
            id: model.id,
            name: model.name,
            description: model.description,
            created_at: model.created_at,
            updated_at: model.updated_at,
            deleted_at: model.deleted_at,
            deleted: model.deleted,
        }
    }
}

#[derive(Queryable, Insertable, Selectable, QueryableByName, Debug, Clone)]
#[diesel(table_name = sync_records)]
pub struct SyncRecordModel {
    pub id: String,
    pub resource_id: String,
    pub resource_type: String,
    pub operation_type: String,
    pub source_device_id: String,
    pub target_device_id: String,
    pub status: String,
    pub folder_id: Option<String>,
    pub credential_id: Option<String>,
    pub synced_from: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

impl From<&DomainSyncRecord> for SyncRecordModel {
    fn from(record: &DomainSyncRecord) -> Self {
        Self {
            id: record.id.clone(),
            resource_id: record.resource_id.clone(),
            resource_type: record.resource_type.to_string(),
            operation_type: record.operation_type.to_string(),
            source_device_id: record.source_device_id.clone(),
            target_device_id: record.target_device_id.clone(),
            status: record.status.to_string(),
            folder_id: record.folder_id.clone(),
            credential_id: record.credential_id.clone(),
            synced_from: record.synced_from.clone(),
            created_at: record.created_at.clone(),
            updated_at: record.updated_at.clone(),
        }
    }
}

impl SyncRecordModel {
    pub fn to_domain_records(models: Vec<SyncRecordModel>) -> Vec<DomainSyncRecord> {
        models.into_iter().map(DomainSyncRecord::from).collect()
    }

    pub fn from_domain_records(records: Vec<DomainSyncRecord>) -> Vec<SyncRecordModel> {
        records
            .into_iter()
            .map(|r| SyncRecordModel::from(&r))
            .collect()
    }
}

impl From<SyncRecordModel> for DomainSyncRecord {
    fn from(model: SyncRecordModel) -> Self {
        Self {
            id: model.id,
            resource_id: model.resource_id,
            resource_type: model.resource_type.into(),
            operation_type: model.operation_type.into(),
            source_device_id: model.source_device_id,
            target_device_id: model.target_device_id,
            status: model.status.into(),
            folder_id: model.folder_id,
            credential_id: model.credential_id,
            synced_from: model.synced_from,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}

#[derive(Queryable, Insertable)]
#[diesel(table_name = credentials)]
pub struct CredentialModel {
    pub id: String,
    pub credential_type: String,
    pub data: String,
    pub folder_id: String,
    pub signature: String,
    pub encrypted_key: String,
    pub favorite: bool,
    pub last_accessed: i64,
    pub deleted: bool,
    pub deleted_at: Option<i64>,
    pub updated_at: i64,
    pub created_at: i64,
}

impl From<&DomainCredential> for CredentialModel {
    fn from(credential: &DomainCredential) -> Self {
        Self {
            id: credential.id.clone(),
            credential_type: credential.credential_type.clone(),
            data: credential.data.clone(),
            folder_id: credential.folder_id.clone(),
            signature: credential.signature.clone(),
            encrypted_key: credential.encrypted_key.clone(),
            favorite: credential.favorite,
            last_accessed: credential.last_accessed,
            deleted: credential.deleted,
            deleted_at: credential.deleted_at,
            created_at: credential.created_at,
            updated_at: credential.updated_at,
        }
    }
}

impl From<CredentialModel> for DomainCredential {
    fn from(model: CredentialModel) -> Self {
        Self {
            id: model.id,
            credential_type: model.credential_type,
            data: model.data,
            folder_id: model.folder_id,
            signature: model.signature,
            encrypted_key: model.encrypted_key,
            created_at: model.created_at,
            updated_at: model.updated_at,
            last_accessed: model.last_accessed,
            favorite: model.favorite,
            deleted: model.deleted,
            deleted_at: model.deleted_at,
        }
    }
}

impl CredentialModel {
    // Convert Vec<CredentialModel> to Vec<DomainCredential>
    pub fn to_domain_credentials(models: Vec<CredentialModel>) -> Vec<DomainCredential> {
        models.into_iter().map(DomainCredential::from).collect()
    }

    // Convert Vec<DomainCredential> to Vec<CredentialModel>
    pub fn from_domain_credentials(credentials: Vec<DomainCredential>) -> Vec<CredentialModel> {
        credentials
            .into_iter()
            .map(|c| CredentialModel::from(&c))
            .collect()
    }
}

#[derive(Queryable, Insertable)]
#[diesel(table_name = devices)]
pub struct DeviceModel {
    pub id: String,
    pub device_key: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub last_synced_at: Option<i64>,
}

impl From<&DomainDevice> for DeviceModel {
    fn from(device: &DomainDevice) -> Self {
        Self {
            id: device.id.clone(),
            device_key: device.device_key.clone(),
            created_at: device.created_at,
            updated_at: device.updated_at,
            last_synced_at: device.last_synced_at,
        }
    }
}

impl From<DeviceModel> for DomainDevice {
    fn from(model: DeviceModel) -> Self {
        Self {
            id: model.id,
            device_key: model.device_key,
            created_at: model.created_at,
            updated_at: model.updated_at,
            last_synced_at: model.last_synced_at,
        }
    }
}

impl DeviceModel {
    // Convert Vec<DeviceModel> to Vec<DomainDevice>
    pub fn to_domain_devices(models: Vec<DeviceModel>) -> Vec<DomainDevice> {
        models.into_iter().map(DomainDevice::from).collect()
    }

    // Convert Vec<DomainDevice> to Vec<DeviceModel>
    pub fn from_domain_devices(devices: Vec<DomainDevice>) -> Vec<DeviceModel> {
        devices.into_iter().map(|d| DeviceModel::from(&d)).collect()
    }
}
