use crate::database::schema::folder_share_records;
use crate::database::schema::{
    devices, folders, resources, share_records, users,
};
use diesel::associations::Associations;
use diesel::prelude::*;
use osvauld_core::models::folder_share_record::FolderShareRecord as DomainFolderShareRecord;
use osvauld_core::models::{
    device::Device as DomainDevice,
    folder::Folder as DomainFolder,
    resource::EncryptedResource as DomainEncryptedResource,
    share_record::{PermissionLevel, ShareOperation, ShareRecord as DomainShareRecord},
    user::User as DomainUser,
};
use serde_json;
#[derive(Queryable, Insertable)]
#[diesel(table_name = folders)]
pub struct FolderModel {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub default_folder: bool,
    pub parent_folder_id: Option<String>,
    pub ucan: String,
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
            default_folder: folder.default_folder,
            parent_folder_id: folder.parent_folder_id.clone(),
            ucan: folder.ucan.clone(),
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
            default_folder: model.default_folder,
            parent_folder_id: model.parent_folder_id,
            ucan: model.ucan,
            created_at: model.created_at,
            updated_at: model.updated_at,
            deleted_at: model.deleted_at,
            deleted: model.deleted,
        }
    }
}

#[derive(Queryable, Insertable, Identifiable, Selectable)]
#[diesel(table_name = resources)]
pub struct ResourceModel {
    pub id: String,
    pub folder_id: String,
    pub encrypted_data: String,
    pub encrypted_key: String,
    pub ucan_token: String,
    pub metadata: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

impl From<&DomainEncryptedResource> for ResourceModel {
    fn from(resource: &DomainEncryptedResource) -> Self {
        Self {
            id: resource.id.clone(),
            folder_id: resource.folder_id.clone(),
            encrypted_data: resource.encrypted_data.clone(),
            encrypted_key: resource.encrypted_key.clone(),
            ucan_token: resource.ucan_token.clone(),
            metadata: Some(serde_json::to_string(&resource.metadata).unwrap_or_default()),
            created_at: resource.created_at,
            updated_at: resource.updated_at,
        }
    }
}

impl From<ResourceModel> for DomainEncryptedResource {
    fn from(model: ResourceModel) -> Self {
        let metadata = model.metadata
            .and_then(|m| serde_json::from_str(&m).ok())
            .unwrap_or(serde_json::Value::Null);

        Self {
            id: model.id,
            folder_id: model.folder_id,
            encrypted_data: model.encrypted_data,
            encrypted_key: model.encrypted_key,
            ucan_token: model.ucan_token,
            metadata,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}

impl ResourceModel {
    // Convert Vec<ResourceModel> to Vec<DomainEncryptedResource>
    pub fn to_domain_resources(models: Vec<ResourceModel>) -> Vec<DomainEncryptedResource> {
        models.into_iter().map(DomainEncryptedResource::from).collect()
    }

    // Convert Vec<DomainEncryptedResource> to Vec<ResourceModel>
    pub fn from_domain_resources(resources: Vec<DomainEncryptedResource>) -> Vec<ResourceModel> {
        resources
            .into_iter()
            .map(|c| ResourceModel::from(&c))
            .collect()
    }
}

#[derive(Queryable, Insertable, Identifiable, Associations)]
#[diesel(belongs_to(UserModel, foreign_key = user_id))]
#[diesel(table_name = devices)]
pub struct DeviceModel {
    pub id: String,
    pub device_key: String,
    pub user_id: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub last_synced_at: Option<i64>,
}

impl From<&DomainDevice> for DeviceModel {
    fn from(device: &DomainDevice) -> Self {
        Self {
            id: device.id.clone(),
            device_key: device.device_key.clone(),
            user_id: device.user_id.clone(),
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
            user_id: model.user_id,
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

#[derive(Queryable, Insertable, Identifiable, Selectable)]
#[diesel(table_name = users)]
pub struct UserModel {
    pub id: String,
    pub username: String,
    pub public_key: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub signature: String,
    pub ucan_token: String,
    pub ucan_pub_key: String,
    pub ucan_cid: String,
    pub owner: bool,
    pub first_sync: bool,
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
            ucan_token: user.ucan_token.clone(),
            ucan_pub_key: user.ucan_pub_key.clone(),
            ucan_cid: user.ucan_cid.clone(),
            first_sync: user.first_sync,
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
            first_sync: model.first_sync,
            owner: model.owner,
            ucan_token: model.ucan_token,
            ucan_pub_key: model.ucan_pub_key,
            ucan_cid: model.ucan_cid,
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

#[derive(Queryable, Insertable, Identifiable, Associations)]
#[diesel(belongs_to(ResourceModel, foreign_key = resource_id))]
#[diesel(table_name = share_records)]
pub struct ShareRecordModel {
    pub id: String,
    pub resource_id: String,
    pub shared_by_user_id: String,
    pub recipient_user_id: String,
    pub permission_level: String,
    pub ucan_token: String,
    pub ucan_cid: String,
    pub operation_type: String,
    pub created_at: i64,
    pub updated_at: i64,
}

impl ShareRecordModel {
    pub fn to_domain(&self) -> DomainShareRecord {
        DomainShareRecord {
            id: self.id.clone(),
            resource_id: self.resource_id.clone(),
            shared_by_user_id: self.shared_by_user_id.clone(),
            recipient_user_id: self.recipient_user_id.clone(),
            ucan_token: self.ucan_token.clone(),
            ucan_cid: self.ucan_cid.clone(),
            permission_level: PermissionLevel::from(self.permission_level.clone()),
            operation_type: ShareOperation::from(self.operation_type.clone()),
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }

    pub fn to_domain_records(models: Vec<ShareRecordModel>) -> Vec<DomainShareRecord> {
        models.into_iter().map(|m| m.to_domain()).collect()
    }
}

impl From<&DomainShareRecord> for ShareRecordModel {
    fn from(record: &DomainShareRecord) -> Self {
        Self {
            id: record.id.clone(),
            resource_id: record.resource_id.clone(),
            shared_by_user_id: record.shared_by_user_id.clone(),
            recipient_user_id: record.recipient_user_id.clone(),
            permission_level: record.permission_level.to_string(),
            ucan_token: record.ucan_token.clone(),
            ucan_cid: record.ucan_cid.clone(),
            operation_type: record.operation_type.to_string(),
            created_at: record.created_at,
            updated_at: record.updated_at,
        }
    }
}

#[derive(Queryable, Insertable, Identifiable, Associations)]
#[diesel(belongs_to(FolderModel, foreign_key = folder_id))]
#[diesel(table_name = folder_share_records)]
pub struct FolderShareRecordModel {
    pub id: String,
    pub folder_id: String,
    pub shared_by_user_id: String,
    pub recipient_user_id: String,
    pub permission_level: String,
    pub ucan_token: String,
    pub ucan_cid: String,
    pub operation_type: String,
    pub created_at: i64,
    pub updated_at: i64,
}

impl FolderShareRecordModel {
    pub fn to_domain(&self) -> DomainFolderShareRecord {
        DomainFolderShareRecord {
            id: self.id.clone(),
            folder_id: self.folder_id.clone(),
            shared_by_user_id: self.shared_by_user_id.clone(),
            recipient_user_id: self.recipient_user_id.clone(),
            ucan_token: self.ucan_token.clone(),
            ucan_cid: self.ucan_cid.clone(),
            permission_level: PermissionLevel::from(self.permission_level.clone()),
            operation_type: ShareOperation::from(self.operation_type.clone()),
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }

    pub fn to_domain_records(models: Vec<FolderShareRecordModel>) -> Vec<DomainFolderShareRecord> {
        models.into_iter().map(|m| m.to_domain()).collect()
    }
}

impl From<&DomainFolderShareRecord> for FolderShareRecordModel {
    fn from(record: &DomainFolderShareRecord) -> Self {
        Self {
            id: record.id.clone(),
            folder_id: record.folder_id.clone(),
            shared_by_user_id: record.shared_by_user_id.clone(),
            recipient_user_id: record.recipient_user_id.clone(),
            permission_level: record.permission_level.to_string(),
            ucan_token: record.ucan_token.clone(),
            ucan_cid: record.ucan_cid.clone(),
            operation_type: record.operation_type.to_string(),
            created_at: record.created_at,
            updated_at: record.updated_at,
        }
    }
}
