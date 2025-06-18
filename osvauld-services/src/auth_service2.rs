use osvauld_core::models::auth::Certificate;
use osvauld_core::models::device::Device;
use osvauld_core::models::user::User;
use osvauld_core::repositories::{DeviceRepository, RepositoryError, StoreRepository};

use crypto_utils::{generate_keys, generate_keys_without_password, get_key_id};
use osvauld_db::database::RepositoryContext;
use std::sync::Arc;

/// Handle user signup - generates user and certificate
async fn create_user(username: &str, passphrase: &str) -> Result<(User, Certificate), String> {
    // Generate primary keys for the user
    let primary_key = generate_keys(passphrase, username).map_err(|e| e.to_string())?;

    // Create primary certificate
    let certificate = Certificate {
        private_key: primary_key.private_key.clone(),
        public_key: primary_key.public_key.clone(),
        salt: primary_key.salt.clone(),
    };

    let user_id = get_key_id(&certificate.public_key).map_err(|e| e.to_string())?;
    let user = User::new(
        username.to_string(),
        user_id,
        certificate.public_key.clone(),
        "signature".to_string(),
        true,
        true,
    );

    Ok((user, certificate))
}

/// Create device objects for a user - generates device, certificate, and sync records
async fn create_device(user_id: &str, username: &str) -> Result<(Device, Certificate), String> {
    // Generate device keys and get device ID
    let (device_key, device_id) = {
        let keys = generate_keys_without_password(username).map_err(|e| e.to_string())?;
        let id = get_key_id(&keys.public_key).map_err(|e| e.to_string())?;
        (keys, id)
    };

    // Create device certificate from generated keys
    let device_certificate = Certificate {
        private_key: device_key.private_key.clone(),
        public_key: device_key.public_key.clone(),
        salt: device_key.salt.clone(),
    };

    let device = Device::new(
        device_id.clone(),
        device_key.public_key,
        user_id.to_string(),
    );

    Ok((device, device_certificate))
}

/// Complete signup process - combines user creation and device setup
pub async fn handle_signup(
    username: &str,
    passphrase: &str,
    repo_context: &RepositoryContext,
) -> Result<(), String> {
    // Create user and primary certificate
    let (user, primary_certificate) = create_user(username, passphrase).await?;

    // Create device and device certificate
    let (device, device_certificate) = create_device(&user.id, username).await?;
    repo_context
        .user_repository
        .commit_signup_transaction(&user, &primary_certificate, &device, &device_certificate)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Check if user is already signed up
pub async fn is_signed_up(store_repo: &Arc<dyn StoreRepository>) -> Result<bool, String> {
    store_repo.is_signed_up().await.map_err(|e| e.to_string())
}

/// Get current device information
pub async fn get_current_device(
    store_repo: &Arc<dyn StoreRepository>,
    device_repo: &Arc<dyn DeviceRepository>,
) -> Result<Device, RepositoryError> {
    let device_id = store_repo.get_device_key().await?;
    let device = device_repo.find_by_id(&device_id).await?;
    Ok(device)
}
