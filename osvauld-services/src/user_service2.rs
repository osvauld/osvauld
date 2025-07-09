use std::sync::Arc;

use crypto_utils::{CryptoUtils, get_key_id};
use log::{debug, error, info};
use osvauld_core::models::{Device, ShareOperation, User, UserWithDevices};
use osvauld_db::database::RepositoryContext;
use tokio::sync::Mutex;

pub async fn add_known_user(
    username: String,
    user_public_key: String,
    device_public_key: String,
    repo_ctx: &RepositoryContext,
    crypto_utils: &Arc<Mutex<CryptoUtils>>,
) -> Result<(User, Device), String> {
    let user_id = get_key_id(&user_public_key.clone()).map_err(|e| e.to_string())?;
    let signature = {
        let crypto = crypto_utils.lock().await;
        crypto
            .sign_message(&user_public_key)
            .map_err(|e| e.to_string())?
    };
    let user = User::new(
        username,
        user_id.clone(),
        user_public_key,
        signature,
        false,
        false,
    );
    let device = Device::new(device_public_key.clone(), device_public_key, user_id);
    let user_data = UserWithDevices {
        user: user.clone(),
        devices: vec![device.clone()],
    };
    repo_ctx
        .user_repo
        .add_users_with_devices_bulk(&[user_data])
        .await
        .map_err(|e| e.to_string())?;
    Ok((user, device))
}
pub async fn get_known_users(repo_ctx: &RepositoryContext) -> Result<Vec<User>, String> {
    repo_ctx
        .user_repo
        .get_known_users()
        .await
        .map_err(|e| e.to_string())
}

pub async fn get_my_user_devices(
    user_id: &str,
    repo_ctx: &RepositoryContext,
) -> Result<Vec<Device>, String> {
    repo_ctx
        .device_repo
        .get_devices_by_user_id(user_id)
        .await
        .map_err(|e| e.to_string())
}

pub async fn get_shared_user_devices_for_note(
    note_id: &str,
    current_user_id: &str,
    current_device_id: &str,
    skip_current_user: bool,
    repo_ctx: &RepositoryContext,
) -> Result<Vec<String>, String> {
    // Get all share records for this note
    let share_records = repo_ctx
        .share_repo
        .find_by_resource_and_operation(note_id, &ShareOperation::Share.to_string())
        .await
        .map_err(|e| e.to_string())?;
    info!(
        "Found {} share records for note {}",
        share_records.len(),
        note_id
    );
    info!(
        "current device id {}, current_user_id {}, skip_current_user {}",
        current_device_id, current_user_id, skip_current_user
    );

    let mut shared_device_ids = Vec::new();

    for record in share_records {
        // Get the user_id from the record
        let user_id = record.recipient_user_id;

        // Skip if this is the current user
        if skip_current_user && user_id == current_user_id {
            continue;
        }

        // Get all devices for this user
        match repo_ctx.device_repo.get_devices_by_user_id(&user_id).await {
            Ok(devices) => {
                for device in devices {
                    if current_device_id != device.id {
                        info!("pushing to shared device ids");
                        shared_device_ids.push(device.id.clone());
                    }
                }
            }
            Err(e) => {
                error!("Failed to get devices for user {}: {:?}", user_id, e);
            }
        }
    }

    Ok(shared_device_ids)
}
