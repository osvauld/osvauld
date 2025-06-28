use std::sync::Arc;

use crypto_utils::{CryptoUtils, get_key_id};
use osvauld_core::models::{Device, User, UserWithDevices};
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
    let device_key_id = get_key_id(&device_public_key).map_err(|e| e.to_string())?;
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
    let device = Device::new(device_key_id, device_public_key, user_id);
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
