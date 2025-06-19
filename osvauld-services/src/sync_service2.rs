use osvauld_core::{
    models::{DeviceManifestRequestPayload, UserWithDeviceIds},
    repositories::RepositoryError,
};
use osvauld_db::database::RepositoryContext;
use std::collections::{HashMap, HashSet};
use tracing::{Span, info, instrument};
#[derive(Debug, Clone)]
pub struct SetComparison {
    pub only_local: HashSet<String>,
    pub only_remote: HashSet<String>,
    pub common: HashSet<String>,
}
pub async fn get_device_manifest(
    repo_ctx: &RepositoryContext,
    current_user_id: &str,
) -> Result<DeviceManifestRequestPayload, String> {
    let resource_manfest = repo_ctx
        .resource_repo
        .get_all_resource_manifest_data()
        .await
        .map_err(|e| e.to_string())?;
    let user_and_devices = repo_ctx
        .user_repo
        .get_other_users_with_device_ids()
        .await
        .map_err(|e| e.to_string())?;
    let current_user_devices = repo_ctx
        .device_repo
        .get_device_ids_by_user_id(current_user_id)
        .await
        .map_err(|e| e.to_string())?;
    let folders = repo_ctx
        .folder_repo
        .find_all()
        .await
        .map_err(|e| format!("Failed to get folders: {}", e))?;

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
    repo_ctx: &RepositoryContext,
    current_user_id: &str,
) -> Result<(), String> {
    let local_manifest_payload = get_device_manifest(repo_ctx, current_user_id).await?;
    let (local_user_ids, local_device_ids, local_resource_ids) =
        create_comparison_sets(&local_manifest_payload);
    let (remote_user_ids, remote_device_ids, remote_resource_ids) =
        create_comparison_sets(remote_manifest_payload);
    // Create HashSets for remote payload
    let user_comparison = compare_sets(&local_user_ids, &remote_user_ids);
    let device_comparison = compare_sets(&local_device_ids, &remote_device_ids);
    let resource_comparison = compare_sets(&local_resource_ids, &remote_resource_ids);

    let (devices_only_local_knows, devices_only_remote_knows) = compare_devices_for_common_users(
        &local_manifest_payload,
        remote_manifest_payload,
        &user_comparison.common,
    );
    todo!()
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
) -> (HashSet<String>, HashSet<String>, HashSet<String>) {
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

    (user_ids, device_ids, resource_ids)
}

fn compare_devices_for_common_users(
    local_payload: &DeviceManifestRequestPayload,
    remote_payload: &DeviceManifestRequestPayload,
    common_user_ids: &HashSet<String>,
) -> (Vec<String>, Vec<String>) {
    // Create lookup maps for users
    let local_users_map: HashMap<String, &UserWithDeviceIds> = local_payload
        .other_users
        .iter()
        .map(|user| (user.user_id.clone(), user))
        .collect();

    let remote_users_map: HashMap<String, &UserWithDeviceIds> = remote_payload
        .other_users
        .iter()
        .map(|user| (user.user_id.clone(), user))
        .collect();

    let mut devices_only_local_knows = Vec::new();
    let mut devices_only_remote_knows = Vec::new();

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
            devices_only_local_knows.extend(devices_only_local);

            // Find devices that only remote knows for this user
            let devices_only_remote: HashSet<String> = &remote_user_devices - &local_user_devices;
            devices_only_remote_knows.extend(devices_only_remote);
        }
    }

    (devices_only_local_knows, devices_only_remote_knows)
}
