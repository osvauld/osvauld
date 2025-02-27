use crate::database::schema::{
    device_record_status, device_records, devices, folders, resources, sync_records, users,
};
use crate::domains::models::{
    device::Device as DomainDevice,
    folder::Folder as DomainFolder,
    resource::Resource as DomainResource,
    sync_record::DeviceRecord as DomainDeviceRecord,
    sync_record::DeviceRecordStatus as DomainDeviceRecordStatus,
    sync_record::SyncRecord as DomainSyncRecord,
    sync_types::{OperationType, ResourceType, SyncStatus},
    user::User as DomainUser,
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

#[derive(Queryable, Insertable, Selectable, Debug)]
#[diesel(table_name = sync_records)]
pub struct SyncRecordModel {
    pub id: String,
    pub resource_id: String,
    pub resource_type: String,
    pub operation_type: String,
    pub source_device_id: String,
    pub created_at: i64,
    pub updated_at: i64,
}

impl SyncRecordModel {
    pub fn to_domain(&self) -> DomainSyncRecord {
        DomainSyncRecord {
            id: self.id.clone(),
            resource_id: self.resource_id.clone(),
            resource_type: ResourceType::from(self.resource_type.clone()),
            operation_type: OperationType::from(self.operation_type.clone()),
            source_device_id: self.source_device_id.clone(),
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

impl From<&DomainSyncRecord> for SyncRecordModel {
    fn from(record: &DomainSyncRecord) -> Self {
        Self {
            id: record.id.clone(),
            resource_id: record.resource_id.clone(),
            resource_type: record.resource_type.to_string(),
            operation_type: record.operation_type.to_string(),
            source_device_id: record.source_device_id.clone(),
            created_at: record.created_at,
            updated_at: record.updated_at,
        }
    }
}

#[derive(Queryable, Insertable, Selectable, Debug)]
#[diesel(table_name = device_records)]
pub struct DeviceRecordModel {
    pub id: String,
    pub sync_record_id: String,
    pub device_id: String,
    pub status: String,
    pub synced: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

impl DeviceRecordModel {
    pub fn to_domain(&self) -> DomainDeviceRecord {
        DomainDeviceRecord {
            id: self.id.clone(),
            sync_record_id: self.sync_record_id.clone(),
            device_id: self.device_id.clone(),
            status: SyncStatus::from(self.status.clone()),
            synced: self.synced,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

impl From<&DomainDeviceRecord> for DeviceRecordModel {
    fn from(record: &DomainDeviceRecord) -> Self {
        Self {
            id: record.id.clone(),
            sync_record_id: record.sync_record_id.clone(),
            device_id: record.device_id.clone(),
            status: record.status.to_string(),
            synced: record.synced,
            created_at: record.created_at,
            updated_at: record.updated_at,
        }
    }
}

#[derive(Queryable, Insertable, Selectable, Debug)]
#[diesel(table_name = device_record_status)]
pub struct DeviceRecordStatusModel {
    pub id: String,
    pub device_record_id: String,
    pub aware_device_id: String,
    pub synced: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

impl DeviceRecordStatusModel {
    pub fn to_domain(&self) -> DomainDeviceRecordStatus {
        DomainDeviceRecordStatus {
            id: self.id.clone(),
            device_record_id: self.device_record_id.clone(),
            aware_device_id: self.aware_device_id.clone(),
            synced: self.synced,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

impl From<&DomainDeviceRecordStatus> for DeviceRecordStatusModel {
    fn from(status: &DomainDeviceRecordStatus) -> Self {
        Self {
            id: status.id.clone(),
            device_record_id: status.device_record_id.clone(),
            aware_device_id: status.aware_device_id.clone(),
            synced: status.synced,
            created_at: status.created_at,
            updated_at: status.updated_at,
        }
    }
}

#[derive(Queryable, Insertable)]
#[diesel(table_name = resources)]
pub struct ResourceModel {
    pub id: String,
    pub resource_type: String,
    pub data: String,
    pub folder_id: String,
    pub signature: String,
    pub favourite: bool,
    pub last_accessed: i64,
    pub deleted: bool,
    pub deleted_at: Option<i64>,
    pub updated_at: i64,
    pub created_at: i64,
}

impl From<&DomainResource> for ResourceModel {
    fn from(resource: &DomainResource) -> Self {
        Self {
            id: resource.id.clone(),
            resource_type: resource.resource_type.clone(),
            data: resource.data.clone(),
            folder_id: resource.folder_id.clone(),
            signature: resource.signature.clone(),
            favourite: resource.favourite,
            last_accessed: resource.last_accessed,
            deleted: resource.deleted,
            deleted_at: resource.deleted_at,
            created_at: resource.created_at,
            updated_at: resource.updated_at,
        }
    }
}

impl From<ResourceModel> for DomainResource {
    fn from(model: ResourceModel) -> Self {
        Self {
            id: model.id,
            resource_type: model.resource_type,
            data: model.data,
            folder_id: model.folder_id,
            signature: model.signature,
            created_at: model.created_at,
            updated_at: model.updated_at,
            last_accessed: model.last_accessed,
            favourite: model.favourite,
            deleted: model.deleted,
            deleted_at: model.deleted_at,
        }
    }
}

impl ResourceModel {
    // Convert Vec<DomainModel> to Vec<DomainDomain>
    pub fn to_domain_resources(models: Vec<ResourceModel>) -> Vec<DomainResource> {
        models.into_iter().map(DomainResource::from).collect()
    }

    // Convert Vec<DomainDomain> to Vec<DomainModel>
    pub fn from_domain_resources(resources: Vec<DomainResource>) -> Vec<ResourceModel> {
        resources
            .into_iter()
            .map(|c| ResourceModel::from(&c))
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

#[derive(Queryable, Insertable)]
#[diesel(table_name = users)]
pub struct UserModel {
    pub id: String,
    pub username: String,
    pub public_key: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub signature: String,
    pub owner: bool,
    pub deleted: bool,
    pub deleted_at: Option<i64>,
}

impl From<&DomainUser> for UserModel {
    fn from(user: &DomainUser) -> Self {
        Self {
            id: user.id.clone(),
            username: user.username.clone(),
            public_key: user.public_key.clone(),
            deleted: user.deleted,
            signature: user.signature.clone(),
            owner: user.owner,
            deleted_at: user.deleted_at,
            created_at: user.created_at,
            updated_at: user.updated_at,
        }
    }
}

impl From<UserModel> for DomainUser {
    fn from(model: UserModel) -> Self {
        Self {
            id: model.id,
            username: model.username,
            public_key: model.public_key,
            deleted: model.deleted,
            signature: model.signature,
            owner: model.owner,
            deleted_at: model.deleted_at,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}
impl UserModel {
    pub fn to_domain_users(models: Vec<UserModel>) -> Vec<DomainUser> {
        models.into_iter().map(DomainUser::from).collect()
    }
}
