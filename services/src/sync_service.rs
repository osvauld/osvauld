use crate::errors::{ServiceResult, SyncServiceError};
use crypto_utils::CryptoUtils;
use osvauld_core::models::{
    ConnectionType, Device, DeviceManifestComparisonResult, DeviceManifestDifferences,
    DeviceManifestRequestPayload, DeviceNetworkSyncPayload, ResourceComparisonResult,
    ResourceManifestData, ResourceSyncData, ResourceVectorClock, User, UserComparisonResult,
    UserManifestComparisonResult, UserManifestDifferences, UserManifestRequestPayload,
    UserNetworkSyncPayload, UserWithDeviceIds, UserWithDevices,
};
use persistance::database::RepositoryContext;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
pub struct SetComparison {
    pub only_local: HashSet<String>,
    pub only_remote: HashSet<String>,
    pub common: HashSet<String>,
}

pub async fn get_device_manifest(
    repo_ctx: Arc<RepositoryContext>,
    current_user_id: &str,
) -> ServiceResult<DeviceManifestRequestPayload> {
    let resource_manfest = repo_ctx
        .resource_repo
        .get_resource_manifest_data(None)
        .await?;

    let user_and_devices = repo_ctx.user_repo.get_other_users_with_device_ids().await?;

    let current_user_devices = repo_ctx
        .device_repo
        .get_device_ids_by_user_id(current_user_id)
        .await?;

    let folders = repo_ctx.folder_repo.find_all().await?;

    let folder_ids: Vec<String> = folders.into_iter().map(|folder| folder.id).collect();

    let message = DeviceManifestRequestPayload {
        resources: resource_manfest,
        other_users: user_and_devices,
        known_device_ids: current_user_devices,
        folder_ids,
    };

    Ok(message)
}

pub async fn process_device_manifest_request(
    remote_manifest_payload: &DeviceManifestRequestPayload,
    repo_ctx: Arc<RepositoryContext>,
    current_user_id: &str,
) -> ServiceResult<DeviceManifestComparisonResult> {
    let local_manifest_payload = get_device_manifest(repo_ctx, current_user_id).await?;

    let (local_user_ids, local_device_ids, local_resource_ids, local_folder_ids) =
        create_comparison_sets(&local_manifest_payload);
    let (remote_user_ids, remote_device_ids, remote_resource_ids, remote_folder_ids) =
        create_comparison_sets(remote_manifest_payload);

    // Create HashSets for remote payload
    let user_comparison = compare_sets(&local_user_ids, &remote_user_ids);
    let device_comparison = compare_sets(&local_device_ids, &remote_device_ids);
    let resource_comparison = compare_sets(&local_resource_ids, &remote_resource_ids);
    let folder_comparison = compare_sets(&local_folder_ids, &remote_folder_ids);

    let common_user_ids = Vec::from_iter(user_comparison.common);

    let (devices_from_common_users_only_local_has, devices_from_common_users_only_remote_has) =
        compare_devices_for_common_users(
            &local_manifest_payload.other_users,
            &remote_manifest_payload.other_users,
            &common_user_ids,
        );

    let resources_requiring_sync = compare_resources_for_common_resources(
        &local_manifest_payload.resources,
        &remote_manifest_payload.resources,
        &resource_comparison.common,
    );

    let unknown_users_to_local: Vec<String> = user_comparison.only_remote.into_iter().collect();
    let unknown_users_to_remote: Vec<String> = user_comparison.only_local.into_iter().collect();
    let unknown_resources_to_local: Vec<String> =
        resource_comparison.only_remote.into_iter().collect();
    let unknown_resources_to_remote: Vec<String> =
        resource_comparison.only_local.into_iter().collect();

    let result = DeviceManifestComparisonResult {
        local_missing: DeviceManifestDifferences {
            unknown_users: unknown_users_to_local,
            unknown_devices_from_common_users: devices_from_common_users_only_remote_has,
            unknown_devices_from_current_user: device_comparison.only_remote.into_iter().collect(),
            unknown_resources: unknown_resources_to_local,
            unknown_folders: folder_comparison.only_remote.into_iter().collect(),
        },
        remote_missing: DeviceManifestDifferences {
            unknown_users: unknown_users_to_remote,
            unknown_devices_from_common_users: devices_from_common_users_only_local_has,
            unknown_devices_from_current_user: device_comparison.only_local.into_iter().collect(),
            unknown_resources: unknown_resources_to_remote,
            unknown_folders: folder_comparison.only_local.into_iter().collect(),
        },
        resources_requiring_sync,
    };

    Ok(result)
}

fn compare_sets(local_set: &HashSet<String>, remote_set: &HashSet<String>) -> SetComparison {
    SetComparison {
        only_local: local_set - remote_set,
        only_remote: remote_set - local_set,
        common: local_set & remote_set,
    }
}

fn create_comparison_sets(
    payload: &DeviceManifestRequestPayload,
) -> (
    HashSet<String>,
    HashSet<String>,
    HashSet<String>,
    HashSet<String>,
) {
    let user_ids: HashSet<String> = payload
        .other_users
        .iter()
        .map(|user| user.user_id.clone())
        .collect();

    let device_ids: HashSet<String> = payload.known_device_ids.iter().cloned().collect();

    let resource_ids: HashSet<String> = payload
        .resources
        .iter()
        .map(|resource| resource.resource_id.clone())
        .collect();

    let folder_ids: HashSet<String> = payload.folder_ids.iter().cloned().collect();

    (user_ids, device_ids, resource_ids, folder_ids)
}

fn compare_devices_for_common_users(
    local_payload: &[UserWithDeviceIds],
    remote_payload: &[UserWithDeviceIds],
    common_user_ids: &[String],
) -> (Vec<UserWithDeviceIds>, Vec<UserWithDeviceIds>) {
    // Create lookup maps for users
    let local_users_map: HashMap<String, &UserWithDeviceIds> = local_payload
        .iter()
        .map(|user| (user.user_id.clone(), user))
        .collect();

    let remote_users_map: HashMap<String, &UserWithDeviceIds> = remote_payload
        .iter()
        .map(|user| (user.user_id.clone(), user))
        .collect();

    let mut users_with_devices_only_local_knows = Vec::new();
    let mut users_with_devices_only_remote_knows = Vec::new();

    // For each common user, compare their device lists
    for user_id in common_user_ids {
        if let (Some(local_user), Some(remote_user)) =
            (local_users_map.get(user_id), remote_users_map.get(user_id))
        {
            let local_user_devices: HashSet<String> =
                local_user.device_ids.iter().cloned().collect();
            let remote_user_devices: HashSet<String> =
                remote_user.device_ids.iter().cloned().collect();

            // Find devices that only local knows for this user
            let devices_only_local: HashSet<String> = &local_user_devices - &remote_user_devices;
            if !devices_only_local.is_empty() {
                users_with_devices_only_local_knows.push(UserWithDeviceIds {
                    user_id: user_id.clone(),
                    device_ids: devices_only_local.into_iter().collect(),
                });
            }

            // Find devices that only remote knows for this user
            let devices_only_remote: HashSet<String> = &remote_user_devices - &local_user_devices;
            if !devices_only_remote.is_empty() {
                users_with_devices_only_remote_knows.push(UserWithDeviceIds {
                    user_id: user_id.clone(),
                    device_ids: devices_only_remote.into_iter().collect(),
                });
            }
        }
    }

    (
        users_with_devices_only_local_knows,
        users_with_devices_only_remote_knows,
    )
}

fn compare_share_records(local_share_records: &[String], remote_share_records: &[String]) -> bool {
    let local_set: HashSet<String> = local_share_records.iter().cloned().collect();
    let remote_set: HashSet<String> = remote_share_records.iter().cloned().collect();

    // Return true if there are differences
    local_set != remote_set
}

fn vector_clocks_need_update(
    local_vector_clocks: &[ResourceVectorClock],
    remote_vector_clocks: &[ResourceVectorClock],
) -> bool {
    // Check if any records are missing on either side
    for local_clock in local_vector_clocks {
        let remote_has_device = remote_vector_clocks
            .iter()
            .any(|remote| remote.device_id == local_clock.device_id);
        if !remote_has_device {
            return true; // Missing record in remote
        }
    }

    for remote_clock in remote_vector_clocks {
        let local_has_device = local_vector_clocks
            .iter()
            .any(|local| local.device_id == remote_clock.device_id);
        if !local_has_device {
            return true; // Missing record in local
        }
    }

    // Check for clock value mismatches
    for local_clock in local_vector_clocks {
        if let Some(remote_clock) = remote_vector_clocks
            .iter()
            .find(|remote| remote.device_id == local_clock.device_id)
        {
            if local_clock.clock_value != remote_clock.clock_value {
                return true; // Clock value mismatch
            }
        }
    }

    false
}

fn compare_resources_for_common_resources(
    local_resources: &[ResourceManifestData],
    remote_resources: &[ResourceManifestData],
    common_resource_ids: &HashSet<String>,
) -> Vec<String> {
    // Create lookup maps for resources
    let local_resources_map: HashMap<String, &ResourceManifestData> = local_resources
        .iter()
        .map(|resource| (resource.resource_id.clone(), resource))
        .collect();

    let remote_resources_map: HashMap<String, &ResourceManifestData> = remote_resources
        .iter()
        .map(|resource| (resource.resource_id.clone(), resource))
        .collect();

    let mut resources_needing_update = Vec::new();

    // For each common resource, check if update is needed
    for resource_id in common_resource_ids {
        if let (Some(local_resource), Some(remote_resource)) = (
            local_resources_map.get(resource_id),
            remote_resources_map.get(resource_id),
        ) {
            // Check share records first
            let share_records_differ = compare_share_records(
                &local_resource.share_record_ids,
                &remote_resource.share_record_ids,
            );

            if share_records_differ {
                // If share records differ, resource needs update - no need to check vector clocks
                resources_needing_update.push(resource_id.clone());
            } else {
                // Only check vector clocks if share records are the same
                let vector_clocks_need_sync = vector_clocks_need_update(
                    &local_resource.vector_clocks,
                    &remote_resource.vector_clocks,
                );

                if vector_clocks_need_sync {
                    resources_needing_update.push(resource_id.clone());
                }
            }
        }
    }

    resources_needing_update
}

pub async fn create_device_network_sync_payload(
    manifest_diff: &DeviceManifestDifferences,
    repo_ctx: Arc<RepositoryContext>,
) -> ServiceResult<DeviceNetworkSyncPayload> {
    let unknown_users_with_devices = repo_ctx
        .user_repo
        .get_users_with_devices_by_user_ids(&manifest_diff.unknown_users)
        .await?;

    // Get unknown devices from common users
    let common_user_device_ids: Vec<String> = manifest_diff
        .unknown_devices_from_common_users
        .iter()
        .flat_map(|user_with_devices| &user_with_devices.device_ids)
        .cloned()
        .collect();

    let unknown_devices_from_common_users = repo_ctx
        .device_repo
        .get_devices_by_ids(&common_user_device_ids)
        .await?;

    // Get unknown devices from current user
    let unknown_devices_from_current_user = repo_ctx
        .device_repo
        .get_devices_by_ids(&manifest_diff.unknown_devices_from_current_user)
        .await?;

    // Get unknown folders
    let unknown_folders = repo_ctx
        .folder_repo
        .get_folders_by_ids(&manifest_diff.unknown_folders)
        .await?;

    let payload = DeviceNetworkSyncPayload {
        unknown_folders,
        unknown_devices_from_current_user,
        unknown_devices_from_common_users,
        unknown_users_with_devices,
    };

    Ok(payload)
}

pub async fn process_device_network_sync(
    payload: &mut DeviceNetworkSyncPayload,
    repo_ctx: Arc<RepositoryContext>,
) -> ServiceResult<()> {
    for user_with_device in &mut payload.unknown_users_with_devices {
        user_with_device.user.owner = false;
    }

    repo_ctx
        .user_repo
        .add_users_with_devices_bulk(&payload.unknown_users_with_devices)
        .await?;

    repo_ctx
        .folder_repo
        .add_folders_bulk(&payload.unknown_folders)
        .await?;

    repo_ctx
        .device_repo
        .save_many(&payload.unknown_devices_from_common_users)
        .await?;

    repo_ctx
        .device_repo
        .save_many(&payload.unknown_devices_from_current_user)
        .await?;

    Ok(())
}

pub async fn get_resource_for_remote_addition(
    resource_id: &str,
    device: &Device,
    repo_ctx: Arc<RepositoryContext>,
) -> ServiceResult<ResourceSyncData> {
    let new_vector_clock =
        ResourceVectorClock::create_entry_for_new_device(resource_id, &device.id);

    repo_ctx
        .vector_clock_repo
        .save_vector_clock(&new_vector_clock)
        .await?;

    let resource_payload = repo_ctx
        .resource_repo
        .get_resource_sync_data(resource_id)
        .await?;

    Ok(resource_payload)
}

pub async fn add_resource_sync(
    payload: &mut ResourceSyncData,
    repo_ctx: Arc<RepositoryContext>,
    connection_type: &ConnectionType,
) -> ServiceResult<()> {
    match connection_type {
        ConnectionType::User => {
            let default_folder = repo_ctx
                .folder_repo
                .get_default_folder()
                .await
                .map_err(|_| SyncServiceError::DefaultFolderNotFound)?;

            payload.resource.folder_id = default_folder.id.clone();
        }
        ConnectionType::Device => {}
    }

    repo_ctx
        .resource_repo
        .save_resource_sync_data(&payload)
        .await?;

    Ok(())
}

pub async fn process_first_user_connection_request(
    user_with_devices: UserWithDevices,
    repo_ctx: Arc<RepositoryContext>,
) -> ServiceResult<()> {
    repo_ctx
        .user_repo
        .add_users_with_devices_bulk(&[user_with_devices])
        .await?;

    Ok(())
}

pub async fn get_user_manifest(
    repo_ctx: Arc<RepositoryContext>,
    peer_user_id: &str,
    current_user_id: &str,
) -> ServiceResult<UserManifestRequestPayload> {
    let share_records = repo_ctx
        .share_repo
        .get_user_share_records(peer_user_id)
        .await?;

    let mut unique_resource_ids = HashSet::new();
    let mut unique_user_ids = HashSet::new();

    for record in share_records {
        unique_user_ids.insert(record.recipient_user_id);
        unique_resource_ids.insert(record.resource_id);
    }

    // Inserting current and peer users, because they know each other
    unique_user_ids.insert(current_user_id.to_string());
    unique_user_ids.insert(peer_user_id.to_string());

    let unique_user_ids_vec: Vec<String> = unique_user_ids.into_iter().collect();
    let unique_resource_ids_vec: Vec<String> = unique_resource_ids.into_iter().collect();

    let user_manifest = repo_ctx
        .user_repo
        .get_users_with_device_ids_by_user_ids(&unique_user_ids_vec)
        .await?;

    let resource_manifest = repo_ctx
        .resource_repo
        .get_resource_manifest_data(Some(&unique_resource_ids_vec))
        .await?;

    let payload = UserManifestRequestPayload {
        users: user_manifest,
        resources: resource_manifest,
    };

    Ok(payload)
}

pub async fn process_user_manifest_request(
    remote_payload: &UserManifestRequestPayload,
    repo_ctx: Arc<RepositoryContext>,
    peer_user_id: &str,
    current_user_id: &str,
) -> ServiceResult<UserManifestComparisonResult> {
    let local_payload = get_user_manifest(repo_ctx, peer_user_id, current_user_id).await?;

    let user_gaps = process_user_gaps(&local_payload, remote_payload);
    let resource_gaps = process_resource_gaps(&local_payload, remote_payload);

    Ok(UserManifestComparisonResult {
        local_missing: UserManifestDifferences {
            unknown_users: user_gaps.users_only_remote_has.clone(),
            unknown_devices_from_common_users: user_gaps
                .devices_from_common_users_only_remote_has
                .clone(),
            unknown_resources: resource_gaps.resources_only_remote_has.clone(),
        },
        remote_missing: UserManifestDifferences {
            unknown_users: user_gaps.users_only_local_has.clone(),
            unknown_devices_from_common_users: user_gaps
                .devices_from_common_users_only_local_has
                .clone(),
            unknown_resources: resource_gaps.resources_only_local_has.clone(),
        },
        resources_requiring_sync: resource_gaps.resources_requiring_sync.clone(),
    })
}

pub fn process_user_gaps(
    local_payload: &UserManifestRequestPayload,
    remote_payload: &UserManifestRequestPayload,
) -> UserComparisonResult {
    // Extract user IDs from both payloads
    let local_user_ids: HashSet<String> = local_payload
        .users
        .iter()
        .map(|user| user.user_id.clone())
        .collect();

    let remote_user_ids: HashSet<String> = remote_payload
        .users
        .iter()
        .map(|user| user.user_id.clone())
        .collect();

    // Find users that only exist on one side
    let users_only_local_has: Vec<String> =
        (&local_user_ids - &remote_user_ids).into_iter().collect();

    let users_only_remote_has: Vec<String> =
        (&remote_user_ids - &local_user_ids).into_iter().collect();

    // Find common users
    let common_users: Vec<String> = (&local_user_ids & &remote_user_ids).into_iter().collect();

    // For common users, compare their devices
    let (devices_from_common_users_only_local_has, devices_from_common_users_only_remote_has) =
        compare_devices_for_common_users(
            &local_payload.users,
            &remote_payload.users,
            &common_users,
        );

    UserComparisonResult {
        users_only_local_has,
        users_only_remote_has,
        common_users,
        devices_from_common_users_only_local_has,
        devices_from_common_users_only_remote_has,
    }
}

pub fn process_resource_gaps(
    local_payload: &UserManifestRequestPayload,
    remote_payload: &UserManifestRequestPayload,
) -> ResourceComparisonResult {
    // Extract resource IDs from both payloads
    let local_resource_ids: HashSet<String> = local_payload
        .resources
        .iter()
        .map(|resource| resource.resource_id.clone())
        .collect();

    let remote_resource_ids: HashSet<String> = remote_payload
        .resources
        .iter()
        .map(|resource| resource.resource_id.clone())
        .collect();

    // Find resources that only exist on one side
    let resources_only_local_has: Vec<String> = (&local_resource_ids - &remote_resource_ids)
        .into_iter()
        .collect();

    let resources_only_remote_has: Vec<String> = (&remote_resource_ids - &local_resource_ids)
        .into_iter()
        .collect();

    // Find common resources
    let common_resources: Vec<String> = (&local_resource_ids & &remote_resource_ids)
        .into_iter()
        .collect();

    // For common resources, check which ones need syncing
    let common_resources_set: HashSet<String> = common_resources.iter().cloned().collect();
    let resources_requiring_sync = compare_resources_for_common_resources(
        &local_payload.resources,
        &remote_payload.resources,
        &common_resources_set,
    );

    ResourceComparisonResult {
        resources_only_local_has,
        resources_only_remote_has,
        common_resources,
        resources_requiring_sync,
    }
}

pub async fn create_user_network_sync_payload(
    manifest_diff: &UserManifestDifferences,
    peer_user: &User,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<Mutex<CryptoUtils>>,
    domain: &str,
) -> ServiceResult<UserNetworkSyncPayload> {
    // Get unknown users with their devices
    let mut unknown_users_with_devices = repo_ctx
        .user_repo
        .get_users_with_devices_by_user_ids(&manifest_diff.unknown_users)
        .await?;

    let encrypted_ucan_pvt_key = repo_ctx.store_repo.get_ucan_key().await?;

    for user_with_devices in &mut unknown_users_with_devices {
        // Generate delegated token for peer to connect to this user
        let delegated_token = {
            let crypto = crypto_utils.lock().await;
            //  CryptoError propagates automatically
            crypto
                .issue_delegated_user_connect_token(
                    &encrypted_ucan_pvt_key,
                    domain,
                    &user_with_devices.user.id,
                    &peer_user.ucan_pub_key,
                    &user_with_devices.user.ucan_token,
                )
                .await?
        };

        // Store the delegated token in the user object for transmission
        user_with_devices.user.ucan_token = delegated_token;
    }

    // Get unknown devices from common users
    let common_user_device_ids: Vec<String> = manifest_diff
        .unknown_devices_from_common_users
        .iter()
        .flat_map(|user_with_devices| &user_with_devices.device_ids)
        .cloned()
        .collect();

    let unknown_devices_from_common_users = repo_ctx
        .device_repo
        .get_devices_by_ids(&common_user_device_ids)
        .await?;

    Ok(UserNetworkSyncPayload {
        users: unknown_users_with_devices,
        devices: unknown_devices_from_common_users,
    })
}

pub async fn process_user_network_sync_payload(
    payload: &mut UserNetworkSyncPayload,
    repo_ctx: Arc<RepositoryContext>,
) -> ServiceResult<()> {
    for user_with_device in &mut payload.users {
        user_with_device.user.owner = false;
        user_with_device.user.first_sync = false;
    }

    repo_ctx
        .user_repo
        .add_users_with_devices_bulk(&payload.users)
        .await?;

    repo_ctx.device_repo.save_many(&payload.devices).await?;

    Ok(())
}
