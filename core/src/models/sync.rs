use super::device::Device;
use super::folder::Folder;
use super::resource::ResourceManifestData;
use super::user::{UserWithDeviceIds, UserWithDevices};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceManifestRequestPayload {
    pub known_device_ids: Vec<String>,
    pub other_users: Vec<UserWithDeviceIds>,
    pub folder_ids: Vec<String>,
    pub resources: Vec<ResourceManifestData>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserManifestRequestPayload {
    pub users: Vec<UserWithDeviceIds>,
    pub resources: Vec<ResourceManifestData>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UserManifestPayload {
    Request(UserManifestRequestPayload),
    Response(UserManifestComparisonResult),
    Ack,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserNetworkSyncPayload {
    pub users: Vec<UserWithDevices>,
    pub devices: Vec<Device>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserManifestDifferences {
    pub unknown_users: Vec<String>,
    pub unknown_devices_from_common_users: Vec<UserWithDeviceIds>,
    pub unknown_resources: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserManifestComparisonResult {
    pub local_missing: UserManifestDifferences,
    pub remote_missing: UserManifestDifferences,
    pub resources_requiring_sync: Vec<String>,
}

impl UserManifestComparisonResult {
    /// Inverse the perspective for sending to remote peer
    /// What's local_missing becomes remote_missing and vice versa
    pub fn inverse(&self) -> UserManifestComparisonResult {
        UserManifestComparisonResult {
            local_missing: self.remote_missing.clone(),
            remote_missing: self.local_missing.clone(),
            resources_requiring_sync: self.resources_requiring_sync.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct UserComparisonResult {
    pub users_only_local_has: Vec<String>,
    pub users_only_remote_has: Vec<String>,
    pub common_users: Vec<String>,
    pub devices_from_common_users_only_local_has: Vec<UserWithDeviceIds>,
    pub devices_from_common_users_only_remote_has: Vec<UserWithDeviceIds>,
}
#[derive(Debug, Clone)]
pub struct ResourceComparisonResult {
    pub resources_only_local_has: Vec<String>,
    pub resources_only_remote_has: Vec<String>,
    pub common_resources: Vec<String>,
    pub resources_requiring_sync: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceManifestDifferences {
    pub unknown_users: Vec<String>,
    pub unknown_devices_from_common_users: Vec<UserWithDeviceIds>,
    pub unknown_devices_from_current_user: Vec<String>,
    pub unknown_resources: Vec<String>,
    pub unknown_folders: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceManifestComparisonResult {
    pub local_missing: DeviceManifestDifferences,
    pub remote_missing: DeviceManifestDifferences,
    pub resources_requiring_sync: Vec<String>,
}

impl DeviceManifestComparisonResult {
    /// Inverse the perspective for sending to remote peer
    /// What's local_missing becomes remote_missing and vice versa
    pub fn inverse(&self) -> DeviceManifestComparisonResult {
        DeviceManifestComparisonResult {
            local_missing: self.remote_missing.clone(),
            remote_missing: self.local_missing.clone(),
            resources_requiring_sync: self.resources_requiring_sync.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceNetworkSyncPayload {
    pub unknown_users_with_devices: Vec<UserWithDevices>,
    pub unknown_devices_from_common_users: Vec<Device>,
    pub unknown_devices_from_current_user: Vec<Device>,
    pub unknown_folders: Vec<Folder>,
}
