use std::sync::Arc;

use crypto_utils::{CryptoUtils, get_key_id};
use log::{debug, error, info};
use osvauld_core::models::{Device, ShareOperation, User, UserWithDevices};
use persistance::database::RepositoryContext;
use tokio::sync::Mutex;

pub async fn add_known_user(
    username: String,
    user_public_key: String,
    device_public_key: String,
    one_time_token: String,
    ucan_pub_key: String,
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
        one_time_token,
        ucan_pub_key,
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
) -> Result<(Vec<String>, Vec<User>), String> {
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

    let mut shared_device_ids = Vec::new();
    let shared_user_ids: Vec<String> = share_records
        .into_iter()
        .map(|sr| sr.recipient_user_id.clone())
        .collect();
    let shared_users = repo_ctx
        .user_repo
        .get_users_by_ids(&shared_user_ids)
        .await
        .map_err(|e| e.to_string())?;

    for user_id in shared_user_ids {
        // Get the user_id from the record
        info!("user{:?}", user_id);

        // Skip if this is the current user
        if skip_current_user && user_id == current_user_id {
            continue;
        }

        // Get all devices for this user
        match repo_ctx.device_repo.get_devices_by_user_id(&user_id).await {
            Ok(devices) => {
                info!("user devices {:?}", devices);
                for device in devices {
                    if current_device_id != device.id {
                        shared_device_ids.push(device.id.clone());
                    }
                }
            }
            Err(e) => {
                error!("Failed to get devices for user {}: {:?}", user_id, e);
            }
        }
    }

    Ok((shared_device_ids, shared_users))
}

pub async fn get_ucan_pub_key(
    repo_ctx: &RepositoryContext,
    crypto_utils: &Arc<Mutex<CryptoUtils>>,
) -> Result<String, String> {
    let encrypted_ucan_pvt_key = repo_ctx
        .store_repo
        .get_ucan_key()
        .await
        .map_err(|e| e.to_string())?;
    let crypto = crypto_utils.lock().await;
    crypto
        .get_public_ucan_key(&encrypted_ucan_pvt_key)
        .await
        .map_err(|e| e.to_string())
}

pub async fn issue_connect_ucan_token(
    repo_ctx: &RepositoryContext,
    crypto_utils: &Arc<Mutex<CryptoUtils>>,
    domain: &str,
    peer_ucan_pub_key: &str,
) -> Result<String, String> {
    let encrypted_pvt_key = repo_ctx.store_repo.get_ucan_key().await.map_err(|e| {
        error!("Failed to get UCAN key for issuing new token: {}", e);
        e.to_string()
    })?;
    let crypto = crypto_utils.lock().await;
    crypto
        .issue_connect_and_share_user_token(&encrypted_pvt_key, domain, peer_ucan_pub_key)
        .await
        .map_err(|e| {
            error!("Failed to issue connect and share token: {}", e);
            e.to_string()
        })
}

pub async fn sign_ucan_pub_key(
    crypto_utils: &Arc<Mutex<CryptoUtils>>,
    repo_ctx: &RepositoryContext,
) -> Result<String, String> {
    let ucan_pub_key = get_ucan_pub_key(repo_ctx, crypto_utils).await?;
    let crypto = crypto_utils.lock().await;
    crypto.sign_clear_text_message(&ucan_pub_key).map_err(|e| {
        error!("Failed to sign local UCAN public key: {}", e);
        e.to_string()
    })
}
