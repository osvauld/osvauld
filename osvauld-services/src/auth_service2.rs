use osvauld_core::models::auth::Certificate;
use osvauld_core::models::device::Device;
use osvauld_core::models::user::User;
use osvauld_core::repositories::{DeviceRepository, RepositoryError, StoreRepository};

use crypto_utils::{
    CryptoUtils, change_certificate_password, export_certificate as crypto_export_certificate,
    generate_keys, generate_keys_without_password, get_key_id, import_certificate,
};
use osvauld_db::database::RepositoryContext;
use std::sync::Arc;
use tokio::sync::Mutex;

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
        .user_repo
        .commit_signup_transaction(&user, &primary_certificate, &device, &device_certificate)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Check if user is already signed up
pub async fn is_signed_up(repo_ctx: &RepositoryContext) -> Result<bool, String> {
    repo_ctx
        .store_repo
        .is_signed_up()
        .await
        .map_err(|e| e.to_string())
}

pub async fn load_certificate(
    passphrase: &str,
    repo_ctx: &RepositoryContext,
    crypto_utils: &Arc<Mutex<CryptoUtils>>,
) -> Result<(User, Device), String> {
    let certificate = repo_ctx
        .store_repo
        .get_certificate("primary_key".to_string(), "primary_key_salt".to_string())
        .await
        .map_err(|e| e.to_string())?;

    let mut crypto = CryptoUtils::new();
    crypto
        .decrypt_and_load_certificate(&certificate.private_key, &certificate.salt, passphrase)
        .map_err(|e| e.to_string())?;

    {
        let mut crypto_utils = crypto_utils.lock().await;
        *crypto_utils = crypto;
    }

    let public_key = {
        let crypto = crypto_utils.lock().await;
        crypto
            .get_public_key()
            .map_err(|e| format!("Failed to get public key: {}", e))?
    };

    let user_id = get_key_id(&public_key).map_err(|e| e.to_string())?;
    let user = repo_ctx
        .user_repo
        .get_user_by_id(&user_id)
        .await
        .map_err(|e| e.to_string())?;
    let device = get_current_device(repo_ctx).await?;
    Ok((user, device))
}

pub async fn get_current_device(repo_ctx: &RepositoryContext) -> Result<Device, String> {
    let device_id = repo_ctx
        .store_repo
        .get_device_key()
        .await
        .map_err(|e| e.to_string())?;
    let device = repo_ctx
        .device_repo
        .find_by_id(&device_id)
        .await
        .map_err(|e| e.to_string())?;
    Ok(device)
}
pub async fn export_certificate(
    passphrase: String,
    repo_ctx: &RepositoryContext,
) -> Result<String, String> {
    let certificate = repo_ctx
        .store_repo
        .get_certificate("primary_key".to_string(), "primary_key_salt".to_string())
        .await
        .map_err(|e| e.to_string())?;

    crypto_export_certificate(&passphrase, &certificate.private_key, &certificate.salt)
        .map_err(|e| format!("Error exporting certificate: {}", e))
}

pub async fn change_passphrase(
    old_password: String,
    new_password: String,
    repo_ctx: &RepositoryContext,
) -> Result<Certificate, String> {
    let certificate = repo_ctx
        .store_repo
        .get_certificate("primary_key".to_string(), "primary_key_salt".to_string())
        .await
        .map_err(|e| e.to_string())?;

    let new_private_key = {
        change_certificate_password(
            &certificate.private_key,
            &certificate.salt,
            &old_password,
            &new_password,
        )
        .map_err(|e| format!("Error changing certificate password: {}", e))?
    };

    let new_certificate = Certificate {
        private_key: new_private_key,
        public_key: certificate.public_key,
        salt: certificate.salt,
    };

    repo_ctx
        .store_repo
        .store_certificate(
            &new_certificate,
            "primary_key".to_string(),
            "primary_key_salt".to_string(),
        )
        .await
        .map_err(|e| e.to_string())?;

    Ok(new_certificate)
}
pub async fn import_user(
    certificate: &str,
    passphrase: &str,
    username: &str,
    repo_ctx: &RepositoryContext,
) -> Result<(User, Certificate), String> {
    let result = import_certificate(certificate, passphrase).map_err(|e| e.to_string())?;
    let user_id = get_key_id(&result.public_key).map_err(|e| e.to_string())?;
    let certificate = Certificate {
        private_key: result.private_key,
        public_key: result.public_key,
        salt: result.salt,
    };
    let user = User::new(
        username.to_string(),
        user_id,
        certificate.public_key.clone(),
        "signature".to_string(),
        true,
        true,
    );
    let (device, device_certificate) = create_device(&user.id, username).await?;

    repo_ctx
        .user_repo
        .commit_signup_transaction(&user, &certificate, &device, &device_certificate)
        .await
        .map_err(|e| e.to_string())?;
    Ok((user, certificate))
}
