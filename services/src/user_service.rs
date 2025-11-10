use std::sync::Arc;

use crate::errors::ServiceResult;
use crypto_utils::{CryptoUtils, get_key_id};
use log::{error, info};
use osvauld_core::models::{Device, ShareOperation, User, UserWithDevices};
use persistance::database::RepositoryContext;
use tokio::sync::RwLock;

pub async fn add_known_user(
    username: String,
    user_public_key: String,
    device_public_key: String,
    one_time_token: String,
    ucan_pub_key: String,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
) -> ServiceResult<(User, Device)> {
    let user_id = get_key_id(&user_public_key)?;

    let signature = {
        let crypto = crypto_utils.read().await;
        crypto.sign_message(&user_public_key)?
    };

    let ucan_cid = crate::ucan_service::get_cid(&one_time_token)?;

    let user = User::new(
        username,
        user_id.clone(),
        user_public_key,
        signature,
        false,
        false,
        one_time_token,
        ucan_cid,
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
        .await?;

    Ok((user, device))
}

pub async fn get_known_users(repo_ctx: Arc<RepositoryContext>) -> ServiceResult<Vec<User>> {
    Ok(repo_ctx.user_repo.get_known_users().await?)
}

pub async fn get_my_user_devices(
    user_id: &str,
    repo_ctx: Arc<RepositoryContext>,
) -> ServiceResult<Vec<Device>> {
    Ok(repo_ctx.device_repo.get_devices_by_user_id(user_id).await?)
}

pub async fn get_shared_user_devices_for_note(
    note_id: &str,
    current_user_id: &str,
    current_device_id: &str,
    skip_current_user: bool,
    repo_ctx: Arc<RepositoryContext>,
) -> ServiceResult<(Vec<String>, Vec<User>)> {
    // Get all share records for this note - direct use of ?
    let share_records = repo_ctx
        .share_repo
        .find_by_resource_and_operation(note_id, &ShareOperation::Share.to_string())
        .await?;

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
        .await?;

    for user_id in shared_user_ids {
        if skip_current_user && user_id == current_user_id {
            continue;
        }

        // Get all devices for this user
        match repo_ctx.device_repo.get_devices_by_user_id(&user_id).await {
            Ok(devices) => {
                for device in devices {
                    if current_device_id != device.id {
                        shared_device_ids.push(device.id.clone());
                    }
                }
            }
            Err(e) => {
                // Log error but continue processing other users
                error!("Failed to get devices for user {}: {:?}", user_id, e);
            }
        }
    }

    Ok((shared_device_ids, shared_users))
}

pub async fn get_ucan_pub_key(
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
) -> ServiceResult<String> {
    crate::ucan_service::get_ucan_public_key(crypto_utils, &repo_ctx).await
}

pub async fn issue_connect_ucan_token(
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
    domain: &str,
    peer_ucan_pub_key: &str,
    role: &str,
) -> ServiceResult<String> {
    crate::ucan_service::issue_peer_connection_token(
        domain,
        peer_ucan_pub_key,
        role,
        crypto_utils,
        &repo_ctx,
    )
    .await
    .map_err(|e| {
        error!("Failed to issue connect and share token: {}", e);
        e
    })
}

pub async fn sign_ucan_pub_key(
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
    repo_ctx: Arc<RepositoryContext>,
) -> ServiceResult<String> {
    let ucan_pub_key = get_ucan_pub_key(repo_ctx, crypto_utils).await?;

    let crypto = crypto_utils.read().await;
    crypto.sign_clear_text_message(&ucan_pub_key).map_err(|e| {
        error!("Failed to sign local UCAN public key: {}", e);
        e.into()
    })
}

/// Parsed connection string data
#[derive(Debug, Clone)]
pub struct ConnectionStringData {
    pub username: String,
    pub user_public_key: String,
    pub device_public_key: String,
    pub ucan_token: String,
    pub ucan_pub_key: String,
    pub folder_id: Option<String>,
}

/// Parse and decode a connection string (base64-encoded JSON)
/// Returns all fields needed for both sovereign node and viewer connections
pub fn parse_connection_string(
    connection_string: &str
) -> ServiceResult<ConnectionStringData> {
    use base64::{Engine as _, engine::general_purpose};
    use crate::errors::UserServiceError;

    // 1. Decode base64
    let decoded = general_purpose::STANDARD
        .decode(connection_string)
        .map_err(|e| UserServiceError::InvalidUserData {
            field: "connection_string".into(),
            reason: format!("Failed to decode base64: {}", e),
        })?;

    // 2. Convert to UTF-8
    let json_str = String::from_utf8(decoded)
        .map_err(|e| UserServiceError::InvalidUserData {
            field: "connection_string".into(),
            reason: format!("Invalid UTF-8: {}", e),
        })?;

    // 3. Parse JSON
    let details: serde_json::Value = serde_json::from_str(&json_str)
        .map_err(|e| UserServiceError::InvalidUserData {
            field: "connection_string".into(),
            reason: format!("Invalid JSON: {}", e),
        })?;

    // 4. Extract all fields
    Ok(ConnectionStringData {
        username: details["username"].as_str()
            .ok_or_else(|| UserServiceError::InvalidUserData {
                field: "username".into(),
                reason: "Missing username in connection string".into(),
            })?
            .to_string(),
        user_public_key: details["user_public_key"].as_str()
            .ok_or_else(|| UserServiceError::InvalidUserData {
                field: "user_public_key".into(),
                reason: "Missing user_public_key in connection string".into(),
            })?
            .to_string(),
        device_public_key: details["device_public_key"].as_str()
            .ok_or_else(|| UserServiceError::InvalidUserData {
                field: "device_public_key".into(),
                reason: "Missing device_public_key in connection string".into(),
            })?
            .to_string(),
        ucan_token: details["ucan_token"].as_str()
            .ok_or_else(|| UserServiceError::InvalidUserData {
                field: "ucan_token".into(),
                reason: "Missing ucan_token in connection string".into(),
            })?
            .to_string(),
        ucan_pub_key: details["ucan_pub_key"].as_str()
            .ok_or_else(|| UserServiceError::InvalidUserData {
                field: "ucan_pub_key".into(),
                reason: "Missing ucan_pub_key in connection string".into(),
            })?
            .to_string(),
        folder_id: details["folder_id"].as_str().map(|s| s.to_string()),
    })
}
