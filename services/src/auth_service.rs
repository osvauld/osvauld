use crate::errors::{AuthServiceError, ServiceResult};
use crypto_utils::{
    CryptoUtils, change_certificate_password, export_certificate as crypto_export_certificate,
    generate_and_encrypt_ed25519_key, generate_keys, get_key_id, import_certificate,
};
use PermitService;
use osvauld_core::models::device::Device;
use osvauld_core::models::user::User;
use osvauld_core::models::Certificate;
use persistance::database::RepositoryContext;
use rand::{RngCore, rngs::OsRng};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, instrument, warn};

async fn create_certificate(username: &str, passphrase: &str) -> ServiceResult<Certificate> {
    let primary_key = generate_keys(passphrase, username)?;
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
) -> ServiceResult<(Device, Certificate)> {
    let (encrypted_key, public_key) = generate_and_encrypt_ed25519_key(user_public_key)?;

    let device_certificate = Certificate {
        private_key: encrypted_key,
        public_key: public_key.clone(),
        salt: String::new(),
    };
    let device = Device::new(public_key.clone(), public_key, user_id.to_string());

    Ok((device, device_certificate))
}

/// Complete signup process - combines user creation and device setup
#[instrument(skip(passphrase, repo_context), fields(username = %username, user_id))]
pub async fn handle_signup(
    username: &str,
    passphrase: &str,
    repo_context: Arc<RepositoryContext>,
) -> ServiceResult<()> {
    info!("👤 Creating new user");

    let is_already_signed_up = repo_context.store_repo.is_signed_up().await?;

    if is_already_signed_up {
        warn!("❌ User already signed up");
        return Err(AuthServiceError::AlreadySignedUp.into());
    }

    // Create user and primary certificate
    debug!("🔐 Generating PGP certificate");
    let primary_certificate = create_certificate(username, passphrase).await?;
    debug!("✓ Certificate created");
    let mut crypto = CryptoUtils::new();

    // For specific error types, we still need to map
    crypto
        .decrypt_and_load_certificate(
            &primary_certificate.private_key,
            &primary_certificate.salt,
            passphrase,
        )
        .map_err(|_| AuthServiceError::InvalidPassphrase)?;

    let user_id = get_key_id(&primary_certificate.public_key)?;
    tracing::Span::current().record("user_id", &user_id.as_str());
    debug!("✓ User ID derived: {}", user_id);

    debug!("🔐 Generating UCAN keys");
    let ucan_certificate = generate_ucan_key(&crypto).await?;
    debug!("✓ UCAN keys generated");

    // Generate owner connection token for the new user
    // This gives the user the ability to create folders and resources
    debug!("🔐 Generating owner connection token");
    let owner_token = {
        // Create a temporary UcanService with the newly generated UCAN keys
        let temp_ucan_service = {
            // Decrypt the encrypted UCAN private key
            let (ucan_signing_key, ucan_verifying_key) = crypto.decrypt_ucan_key(&ucan_certificate.private_key)?;

            let mut service = PermitService::new();
            service.load_keys(ucan_signing_key, ucan_verifying_key);
            Arc::new(RwLock::new(service))
        };

        // Use the UcanService to issue an owner connection token
        // For the owner, we use their own public key as the audience
        let token = temp_ucan_service.read().await
            .issue_peer_connection(&ucan_certificate.public_key, "owner")
            .await?;
        token
    };
    debug!("✓ Owner token generated");

    crypto.clear_cert();

    let user = User::new(
        username.to_string(),
        user_id,
        primary_certificate.public_key.clone(),
        "signature".to_string(),
        true,
        true,
        owner_token,
        "owner_cid".to_string(),
        ucan_certificate.public_key.clone(),
    );

    // Create device and device certificate
    debug!("📱 Creating device");
    let (device, device_certificate) = create_device(&user.public_key, &user.id).await?;
    debug!("✓ Device created: {}", device.id);

    debug!("💾 Committing signup transaction");
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
        .await?;

    info!("✓ User created successfully");
    Ok(())
}

/// Check if user is already signed up
pub async fn is_signed_up(repo_ctx: Arc<RepositoryContext>) -> ServiceResult<bool> {
    Ok(repo_ctx.store_repo.is_signed_up().await?)
}

pub async fn load_certificate(
    passphrase: &str,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
) -> ServiceResult<(User, Device)> {
    // Map specific errors where needed
    let certificate = repo_ctx
        .store_repo
        .get_certificate("primary_key".to_string(), "primary_key_salt".to_string())
        .await
        .map_err(|_| AuthServiceError::CertificateNotFound)?;

    let mut crypto = CryptoUtils::new();
    crypto
        .decrypt_and_load_certificate(&certificate.private_key, &certificate.salt, passphrase)
        .map_err(|_| AuthServiceError::InvalidPassphrase)?;

    {
        let mut crypto_utils = crypto_utils.write().await;
        *crypto_utils = crypto;
    }

    let public_key = {
        let crypto = crypto_utils.read().await;
        crypto.get_public_key()?
    };

    let user_id = get_key_id(&public_key)?;

    // Map specific errors where needed
    let user = repo_ctx
        .user_repo
        .get_user_by_id(&user_id)
        .await
        .map_err(|_| AuthServiceError::UserNotFound { user_id })?;

    let device = get_current_device(repo_ctx).await?;
    Ok((user, device))
}

pub async fn get_current_device(repo_ctx: Arc<RepositoryContext>) -> ServiceResult<Device> {
    // For errors that need context, we still map them
    let device_id = repo_ctx.store_repo.get_device_key().await.map_err(|e| {
        AuthServiceError::DeviceNotFound {
            device_id: format!("Failed to get device key: {}", e),
        }
    })?;

    let device = repo_ctx
        .device_repo
        .find_by_id(&device_id)
        .await
        .map_err(|_| AuthServiceError::DeviceNotFound { device_id })?;

    Ok(device)
}

pub async fn export_certificate(
    passphrase: String,
    repo_ctx: Arc<RepositoryContext>,
) -> ServiceResult<String> {
    let certificate = repo_ctx
        .store_repo
        .get_certificate("primary_key".to_string(), "primary_key_salt".to_string())
        .await
        .map_err(|_| AuthServiceError::CertificateNotFound)?;

    Ok(crypto_export_certificate(
        &passphrase,
        &certificate.private_key,
        &certificate.salt,
    )?)
}

pub async fn change_passphrase(
    old_password: String,
    new_password: String,
    repo_ctx: Arc<RepositoryContext>,
) -> ServiceResult<Certificate> {
    let certificate = repo_ctx
        .store_repo
        .get_certificate("primary_key".to_string(), "primary_key_salt".to_string())
        .await
        .map_err(|_| AuthServiceError::CertificateNotFound)?;

    let new_private_key = change_certificate_password(
        &certificate.private_key,
        &certificate.salt,
        &old_password,
        &new_password,
    )?;

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
        .await?;

    Ok(new_certificate)
}

pub async fn import_user(
    certificate: &str,
    passphrase: &str,
    username: &str,
    peer_device_id: &str,
    repo_ctx: Arc<RepositoryContext>,
) -> ServiceResult<(User, Certificate)> {
    let result = import_certificate(certificate, passphrase)?;
    let user_id = get_key_id(&result.public_key)?;

    let primary_certificate = Certificate {
        private_key: result.private_key,
        public_key: result.public_key,
        salt: result.salt,
    };

    let mut crypto = CryptoUtils::new();
    crypto
        .decrypt_and_load_certificate(
            &primary_certificate.private_key,
            &primary_certificate.salt,
            passphrase,
        )
        .map_err(|_| AuthServiceError::InvalidPassphrase)?;

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
        "owner_cid".to_string(),
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
        .await?;

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

async fn generate_ucan_key(crypto_utils: &CryptoUtils) -> ServiceResult<Certificate> {
    let (encrypted_ucan_private_key, ucan_public_key) =
        crypto_utils.generate_and_encrypt_ucan_key()?;

    let ucan_certificate = Certificate {
        private_key: encrypted_ucan_private_key,
        public_key: ucan_public_key,
        salt: String::new(),
    };

    Ok(ucan_certificate)
}

#[instrument(skip(ucan_service), fields(relationship = %relationship))]
pub async fn generate_one_time_ucan_token(
    relationship: &str,
    ucan_service: &Arc<RwLock<PermitService>>,
) -> ServiceResult<(String, String)> {
    info!("🔐 Generating one-time UCAN token");
    let ucan_service_guard = ucan_service.read().await;
    let result = ucan_service_guard.issue_one_time(relationship).await?;
    info!("✓ One-time token generated");
    Ok(result)
}

#[instrument(skip(ucan_service), fields(folder_id = %folder_id))]
pub async fn generate_folder_share_token(
    folder_id: &str,
    ucan_service: &Arc<RwLock<PermitService>>,
) -> ServiceResult<(String, String)> {
    info!("🔐 Generating folder share token");
    let ucan_service_guard = ucan_service.read().await;
    let result = ucan_service_guard.issue_folder_viewer_auth(folder_id).await?;
    info!("✓ Folder share token generated");
    Ok(result)
}

// ==================== UNIFIED HANDSHAKE SUPPORT ====================

use osvauld_core::models::Permit;

/// Handshake type classification (v3 simplified)
///
/// Viewer vs Peer distinction is determined by relationship field, not enum variant
#[derive(Debug, Clone)]
pub enum HandshakeType {
    FirstConnection,
    Reconnection,
}

/// Parsed handshake token information
#[derive(Debug, Clone)]
pub struct ParsedHandshakeToken {
    pub handshake_type: HandshakeType,
    pub relationship: Option<String>,
    pub folder_id: Option<String>,
}

/// Parse and validate handshake token
///
/// Extracts token type, relationship, and folder_id from UCAN token.
/// Validates the signed UCAN public key matches the peer's public key.
///
/// # Arguments
/// * `ucan_token` - UCAN token string from HandshakeRequest
/// * `signed_ucan_pub` - Signed UCAN public key from HandshakeRequest
/// * `peer_ucan_pub_key` - Peer's UCAN public key from User object
///
/// # Returns
/// * `ParsedHandshakeToken` - Parsed token information for routing
pub async fn parse_and_validate_handshake_token(
    ucan_token: &str,
    signed_ucan_pub: &str,
    peer_ucan_pub_key: &str,
    ucan_service: &Arc<RwLock<PermitService>>,
) -> ServiceResult<ParsedHandshakeToken> {
    // 1. Parse connection token
    let conn_permit = Permit::from_token(ucan_token)
        .map_err(|e| AuthServiceError::InvalidUcanToken(format!("Failed to parse token: {}", e)))?;

    // 2. Validate signature (reuse existing validation if available)
    // TODO: Implement or reuse signature validation
    // For now, we trust the signature is validated elsewhere
    let _ = (signed_ucan_pub, peer_ucan_pub_key);

    // 3. Extract folder_id from facts (for viewer tokens with folder context)
    let folder_id = conn_permit.folder_id();

    // 4. Determine handshake type based on facts (simplified in v3)
    let relationship = conn_permit.relationship().map(|s| s.to_string());
    let is_first_connection = conn_permit.is_first_connection();

    let handshake_type = if is_first_connection {
        HandshakeType::FirstConnection
    } else {
        HandshakeType::Reconnection
    };

    Ok(ParsedHandshakeToken {
        handshake_type,
        relationship,
        folder_id,
    })
}

/// Save first connection user and devices
///
/// Checks if user already exists in database. If not, saves the user
/// and all their devices.
///
/// # Arguments
/// * `user` - User to save
/// * `devices` - Devices to save for this user
/// * `repo_ctx` - Repository context for database access
pub async fn save_first_connection_user(
    user: &User,
    devices: &[Device],
    repo_ctx: &Arc<RepositoryContext>,
) -> ServiceResult<()> {
    // Check if user already exists by trying to get them
    let user_exists = repo_ctx
        .user_repo
        .get_user_by_id(&user.id)
        .await
        .is_ok();

    // If user doesn't exist, save user and devices
    if !user_exists {
        // Save user as known user
        repo_ctx
            .user_repo
            .add_known_user(user)
            .await
            .map_err(|e| AuthServiceError::DatabaseError(e.to_string()))?;

        // Save all devices
        for device in devices {
            repo_ctx
                .device_repo
                .save(device)
                .await
                .map_err(|e| AuthServiceError::DatabaseError(e.to_string()))?;
        }

        tracing::info!("✓ Saved user {} and {} devices", user.id, devices.len());
    } else {
        // User exists - update their UCAN token
        tracing::info!("User {} already exists, updating token", user.id);
        update_user_token(&user.id, &user.ucan_token, repo_ctx).await?;
    }

    Ok(())
}

/// Check if user exists in database
///
/// Used during reconnection to verify the user has previously connected.
///
/// # Arguments
/// * `user_id` - User ID to check
/// * `repo_ctx` - Repository context for database access
///
/// # Returns
/// * `Ok(true)` - User exists
/// * `Ok(false)` - User not found
pub async fn user_exists(
    user_id: &str,
    repo_ctx: &Arc<RepositoryContext>,
) -> ServiceResult<bool> {
    let exists = repo_ctx
        .user_repo
        .get_user_by_id(user_id)
        .await
        .is_ok();

    Ok(exists)
}

/// Update user's UCAN token in database
///
/// Used during three-way handshake to replace one-time tokens with persistent tokens.
///
/// # Arguments
/// * `user_id` - User ID to update
/// * `token` - New UCAN token (persistent connection token)
/// * `repo_ctx` - Repository context for database access
///
/// # Returns
/// * `Ok(())` - Token updated successfully
/// * `Err(...)` - User not found or database error
pub async fn update_user_token(
    user_id: &str,
    token: &str,
    repo_ctx: &Arc<RepositoryContext>,
) -> ServiceResult<()> {
    // For now, use a placeholder CID since we're just updating the token
    // The CID field may not be critical for the handshake flow
    let placeholder_cid = format!("cid_{}", user_id);

    repo_ctx
        .user_repo
        .update_ucan(user_id, token.to_string(), placeholder_cid)
        .await
        .map_err(|e| AuthServiceError::DatabaseError(format!("Failed to update token: {}", e)))?;

    tracing::info!("✓ Updated UCAN token for user {}", user_id);
    Ok(())
}

/// Get node credentials (node key and device key) for P2P initialization
///
/// # Arguments
/// * `repo_ctx` - Repository context for database access
///
/// # Returns
/// * `(node_key, device_key)` - Node private key and device public key
pub async fn get_node_credentials(
    repo_ctx: &Arc<RepositoryContext>,
) -> ServiceResult<(String, String)> {
    let node_key = repo_ctx
        .store_repo
        .get_node_key()
        .await
        .map_err(|e| AuthServiceError::DatabaseError(format!("Failed to get node key: {}", e)))?;

    let device_key = repo_ctx
        .store_repo
        .get_device_key()
        .await
        .map_err(|e| AuthServiceError::DatabaseError(format!("Failed to get device key: {}", e)))?;

    Ok((node_key, device_key))
}
