use crypto_utils::{
    CryptoUtils, change_certificate_password, export_certificate as crypto_export_certificate,
    generate_and_encrypt_ed25519_key, generate_keys, get_key_id, import_certificate,
};
use osvauld_core::models::Certificate;
use osvauld_core::models::device::Device;
use osvauld_core::models::user::User;
use rand::{RngCore, rngs::OsRng};

use persistance::database::RepositoryContext;
use std::sync::Arc;
use tokio::sync::Mutex;

async fn create_certificate(username: &str, passphrase: &str) -> Result<Certificate, String> {
    // Generate primary keys for the user
    let primary_key = generate_keys(passphrase, username).map_err(|e| e.to_string())?;

    // Create primary certificate
    let certificate = Certificate {
        private_key: primary_key.private_key.clone(),
        public_key: primary_key.public_key.clone(),
        salt: primary_key.salt.clone(),
    };

    Ok(certificate)
}

/// Create device objects for a user - generates device, certificate, and sync records
async fn create_device(
    user_public_key: &str,
    user_id: &str,
) -> Result<(Device, Certificate), String> {
    // Generate device keys and get device ID
    let (encrypted_key, public_key) =
        generate_and_encrypt_ed25519_key(user_public_key).map_err(|e| e.to_string())?;

    // Create device certificate from generated keys
    let device_certificate = Certificate {
        private_key: encrypted_key,
        public_key: public_key.clone(),
        salt: String::new(),
    };
    let device = Device::new(public_key.clone(), public_key, user_id.to_string());

    Ok((device, device_certificate))
}

/// Complete signup process - combines user creation and device setup
pub async fn handle_signup(
    username: &str,
    passphrase: &str,
    repo_context: &RepositoryContext,
) -> Result<(), String> {
    // Create user and primary certificate
    let primary_certificate = create_certificate(username, passphrase).await?;
    let mut crypto = CryptoUtils::new();
    crypto
        .decrypt_and_load_certificate(
            &primary_certificate.private_key,
            &primary_certificate.salt,
            passphrase,
        )
        .map_err(|e| e.to_string())?;
    let ucan_certificate = generate_ucan_key(&crypto).await?;
    crypto.clear_cert();

    let user_id = get_key_id(&primary_certificate.public_key).map_err(|e| e.to_string())?;
    let user = User::new(
        username.to_string(),
        user_id,
        primary_certificate.public_key.clone(),
        "signature".to_string(),
        true,
        true,
        "owner_token".to_string(),
        ucan_certificate.public_key.clone(),
    );
    // Create device and device certificate
    let (device, device_certificate) = create_device(&user.public_key, &user.id).await?;
    repo_context
        .user_repo
        .commit_signup_transaction(
            &user,
            &primary_certificate,
            &device,
            &device_certificate,
            None,
            &ucan_certificate,
        )
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
    peer_device_id: &str,
    repo_ctx: &RepositoryContext,
) -> Result<(User, Certificate), String> {
    let result = import_certificate(certificate, passphrase).map_err(|e| e.to_string())?;
    let user_id = get_key_id(&result.public_key).map_err(|e| e.to_string())?;
    let primary_certificate = Certificate {
        private_key: result.private_key,
        public_key: result.public_key,
        salt: result.salt,
    };
    //TODO: fix signature problem.
    let mut crypto = CryptoUtils::new();
    crypto
        .decrypt_and_load_certificate(
            &primary_certificate.private_key,
            &primary_certificate.salt,
            passphrase,
        )
        .map_err(|e| e.to_string())?;
    let ucan_certificate = generate_ucan_key(&crypto).await?;

    let user = User::new(
        username.to_string(),
        user_id.clone(),
        primary_certificate.public_key.clone(),
        "signature".to_string(),
        true,
        true,
        "owner_token".to_string(),
        ucan_certificate.public_key.clone(),
    );
    let peer_device = Device::new(
        peer_device_id.to_string(),
        peer_device_id.to_string(),
        user_id.clone(),
    );
    let (device, device_certificate) = create_device(&user.public_key, &user.id).await?;
    crypto.clear_cert();

    repo_ctx
        .user_repo
        .commit_signup_transaction(
            &user,
            &primary_certificate,
            &device,
            &device_certificate,
            Some(&peer_device),
            &ucan_certificate,
        )
        .await
        .map_err(|e| e.to_string())?;
    Ok((user, primary_certificate))
}

pub fn generate_challenge() -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    bytes
        .iter()
        .fold(String::with_capacity(64), |mut acc, byte| {
            use std::fmt::Write;
            write!(acc, "{:02x}", byte).unwrap();
            acc
        })
}

async fn generate_ucan_key(crypto_utils: &CryptoUtils) -> Result<Certificate, String> {
    // Generate, derive, and encrypt UCAN key (all in one step)
    let (encrypted_ucan_private_key, ucan_public_key) = {
        crypto_utils
            .generate_and_encrypt_ucan_key()
            .map_err(|e| e.to_string())?
    };

    // Create UCAN certificate from generated keys (same structure as device certificate)
    let ucan_certificate = Certificate {
        private_key: encrypted_ucan_private_key,
        public_key: ucan_public_key,
        salt: String::new(), // UCAN keys don't need separate salt (encrypted with PGP)
    };

    Ok(ucan_certificate)
}

pub async fn generate_one_time_ucan_token(
    capability_str: &str,
    crypto_utils: &Arc<Mutex<CryptoUtils>>,
    repo_ctx: &RepositoryContext,
) -> Result<(String, String), String> {
    let encrypted_ucan_pvt_key = repo_ctx
        .store_repo
        .get_ucan_key()
        .await
        .map_err(|e| e.to_string())?;

    let (ucan_token, ucan_public_key) = {
        let crypto = crypto_utils.lock().await;
        crypto
            .generate_one_time_user_connect_token(&encrypted_ucan_pvt_key, capability_str)
            .await
            .map_err(|e| e.to_string())?
    };
    Ok((ucan_token, ucan_public_key))
}
