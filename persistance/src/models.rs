use crate::database::schema::{
    devices, folders, resource_keys, resource_vector_clocks, resources, share_records, users,
};
use diesel::associations::Associations;
use diesel::prelude::*;
use osvauld_core::models::{
    ResourceType,
    device::Device as DomainDevice,
    folder::Folder as DomainFolder,
    resource::Resource as DomainResource,
    resource_key::ResourceKey as DomainResourceKey,
    share_record::{PermissionLevel, ShareOperation, ShareRecord as DomainShareRecord},
    user::User as DomainUser,
    vector_clock::ResourceVectorClock as DomainResourceVectorClock,
};

#[derive(Queryable, Insertable)]
#[diesel(table_name = folders)]
pub struct FolderModel {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub default_folder: bool,
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
    pub resource_type: String,
    pub data: String,
    pub folder_id: String,
    pub signature: String,
    pub favourite: bool,
    pub created_by: String,
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
            resource_type: resource.resource_type.to_string(),
            data: resource.data.clone(),
            folder_id: resource.folder_id.clone(),
            signature: resource.signature.clone(),
            favourite: resource.favourite,
            last_accessed: resource.last_accessed,
            created_by: resource.created_by.clone(),
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
            resource_type: ResourceType::from_str(&model.resource_type),
            data: model.data,
            folder_id: model.folder_id,
            signature: model.signature,
            created_at: model.created_at,
            updated_at: model.updated_at,
            created_by: model.created_by,
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
#[diesel(table_name = resource_keys)]
pub struct ResourceKeyModel {
    pub id: String,
    pub resource_id: String,
    pub user_id: String,
    pub encrypted_key: String,
    pub is_owner: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

impl From<&DomainResourceKey> for ResourceKeyModel {
    fn from(resource_key: &DomainResourceKey) -> Self {
        Self {
            id: resource_key.id.clone(),
            resource_id: resource_key.resource_id.clone(),
            user_id: resource_key.user_id.clone(),
            encrypted_key: resource_key.encrypted_key.clone(),
            is_owner: resource_key.is_owner,
            created_at: resource_key.created_at,
            updated_at: resource_key.updated_at,
        }
    }
}

impl From<ResourceKeyModel> for DomainResourceKey {
    fn from(model: ResourceKeyModel) -> Self {
        Self {
            id: model.id.clone(),
            resource_id: model.resource_id.clone(),
            user_id: model.user_id.clone(),
            encrypted_key: model.encrypted_key.clone(),
            is_owner: model.is_owner,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}
impl ResourceKeyModel {
    pub fn to_domain_resource(models: Vec<ResourceKeyModel>) -> Vec<DomainResourceKey> {
        models.into_iter().map(DomainResourceKey::from).collect()
    }
}

#[derive(Queryable, Insertable, Identifiable, Associations, Selectable)]
#[diesel(belongs_to(ResourceModel, foreign_key = resource_id))]
#[diesel(table_name = resource_vector_clocks)]
pub struct ResourceVectorClockModel {
    pub id: String,
    pub resource_id: String,
    pub device_id: String,
    pub clock_value: i32,
    pub created_at: i64,
    pub updated_at: i64,
}

impl From<&DomainResourceVectorClock> for ResourceVectorClockModel {
    fn from(clock: &DomainResourceVectorClock) -> Self {
        Self {
            id: clock.id.clone(),
            resource_id: clock.resource_id.clone(),
            device_id: clock.device_id.clone(),
            clock_value: clock.clock_value as i32, // Convert u64 to i32
            created_at: clock.created_at,
            updated_at: clock.updated_at,
        }
    }
}

impl From<ResourceVectorClockModel> for DomainResourceVectorClock {
    fn from(model: ResourceVectorClockModel) -> Self {
        Self {
            id: model.id,
            resource_id: model.resource_id,
            device_id: model.device_id,
            clock_value: model.clock_value as u64, // Convert i32 to u64
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}

impl ResourceVectorClockModel {
    // Helper to convert a collection of models to domain objects
    pub fn to_domain_vector_clocks(
        models: Vec<ResourceVectorClockModel>,
    ) -> Vec<DomainResourceVectorClock> {
        models
            .into_iter()
            .map(DomainResourceVectorClock::from)
            .collect()
    }

    // Helper to convert a collection of domain objects to models
    pub fn from_domain_vector_clocks(
        clocks: &[DomainResourceVectorClock],
    ) -> Vec<ResourceVectorClockModel> {
        clocks
            .iter()
            .map(|clock| ResourceVectorClockModel::from(clock))
            .collect()
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
