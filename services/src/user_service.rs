use std::sync::Arc;

use crate::errors::{ServiceResult, ServiceError, UserServiceError};
use crypto_utils::{CryptoUtils, get_key_id};
use PermitService;
use tracing::{debug, error, info, instrument};
use osvauld_core::models::{Device, ShareOperation, User, UserWithDevices};
use persistance::database::RepositoryContext;
use tokio::sync::RwLock;

#[instrument(skip(repo_ctx), fields(
    username = %username,
    user_id
))]
pub async fn add_known_user(
    username: String,
    user_public_key: String,
    device_public_key: String,
    one_time_token: String,
    ucan_pub_key: String,
    repo_ctx: Arc<RepositoryContext>,
) -> ServiceResult<(User, Device)> {
    info!("👤 Adding known user");

    let user_id = get_key_id(&user_public_key)?;
    tracing::Span::current().record("user_id", &user_id.as_str());
    debug!("✓ Generated user ID from public key: {}", user_id);

    // No longer using PGP signature - signature field kept for backwards compatibility
    let signature = String::new();

    // TODO: Get CID from token if needed
    let ucan_cid = String::new();

    debug!("📝 Creating User object");
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

    debug!("📱 Creating Device object");
    let device = Device::new(device_public_key.clone(), device_public_key, user_id);

    let user_data = UserWithDevices {
        user: user.clone(),
        devices: vec![device.clone()],
    };

    debug!("💾 Saving user and device to database");
    repo_ctx
        .user_repo
        .add_users_with_devices_bulk(&[user_data])
        .await?;
    debug!("✓ User and device saved");

    info!("✓ Known user added successfully");
    Ok((user, device))
}

#[instrument(skip(repo_ctx), fields(users_count))]
pub async fn get_known_users(repo_ctx: Arc<RepositoryContext>) -> ServiceResult<Vec<User>> {
    info!("📋 Fetching known users");
    let users = repo_ctx.user_repo.get_known_users().await?;
    tracing::Span::current().record("users_count", users.len());
    info!("✓ Found {} known users", users.len());
    Ok(users)
}

#[instrument(skip(repo_ctx), fields(
    user_id = %user_id,
    devices_count
))]
pub async fn get_my_user_devices(
    user_id: &str,
    repo_ctx: Arc<RepositoryContext>,
) -> ServiceResult<Vec<Device>> {
    info!("📱 Fetching devices for user");
    let devices = repo_ctx.device_repo.get_devices_by_user_id(user_id).await?;
    tracing::Span::current().record("devices_count", devices.len());
    info!("✓ Found {} devices", devices.len());
    Ok(devices)
}

#[instrument(skip(repo_ctx), fields(
    note_id = %note_id,
    current_user_id = %current_user_id,
    current_device_id = %current_device_id,
    skip_current_user = %skip_current_user,
    share_records_count,
    devices_count,
    users_count
))]
pub async fn get_shared_user_devices_for_note(
    note_id: &str,
    current_user_id: &str,
    current_device_id: &str,
    skip_current_user: bool,
    repo_ctx: Arc<RepositoryContext>,
) -> ServiceResult<(Vec<String>, Vec<User>)> {
    info!("🔗 Fetching shared user devices for note");

    // Get all share records for this note - direct use of ?
    let share_records = repo_ctx
        .share_repo
        .find_by_resource_and_operation(note_id, &ShareOperation::Share.to_string())
        .await?;

    tracing::Span::current().record("share_records_count", share_records.len());
    debug!("Found {} share records for note", share_records.len());

    let mut shared_device_ids = Vec::new();
    let shared_user_ids: Vec<String> = share_records
        .into_iter()
        .map(|sr| sr.recipient_user_id.clone())
        .collect();

    debug!("📥 Fetching user details");
    let shared_users = repo_ctx
        .user_repo
        .get_users_by_ids(&shared_user_ids)
        .await?;

    debug!("📱 Collecting devices from {} users", shared_user_ids.len());
    for user_id in shared_user_ids {
        if skip_current_user && user_id == current_user_id {
            debug!("⏭️ Skipping current user: {}", user_id);
            continue;
        }

        // Get all devices for this user
        match repo_ctx.device_repo.get_devices_by_user_id(&user_id).await {
            Ok(devices) => {
                debug!("Found {} devices for user {}", devices.len(), user_id);
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

    tracing::Span::current().record("devices_count", shared_device_ids.len());
    tracing::Span::current().record("users_count", shared_users.len());
    info!("✓ Found {} devices from {} users", shared_device_ids.len(), shared_users.len());

    Ok((shared_device_ids, shared_users))
}

#[instrument(skip_all)]
pub async fn get_ucan_pub_key(
    _repo_ctx: Arc<RepositoryContext>,
    _crypto_utils: &Arc<RwLock<CryptoUtils>>,
    ucan_service: &Arc<RwLock<PermitService>>,
) -> ServiceResult<String> {
    info!("🔑 Getting UCAN public key");
    let pub_key = ucan_service.read().await.get_public_key()?;
    debug!("✓ UCAN public key retrieved");
    Ok(pub_key)
}

#[instrument(skip_all, fields(
    domain = %domain,
    role = %role
))]
pub async fn issue_connect_ucan_token(
    _repo_ctx: Arc<RepositoryContext>,
    _crypto_utils: &Arc<RwLock<CryptoUtils>>,
    domain: &str,
    peer_ucan_pub_key: &str,
    role: &str,
    ucan_service: &Arc<PermitService>,
) -> ServiceResult<String> {
    info!("🔐 Issuing connection UCAN token");

    let token = ucan_service.issue_peer_connection(
        peer_ucan_pub_key,
        role,
    )
    .await
    .map_err(|e| {
        error!("❌ Failed to issue connect and share token: {}", e);
        ServiceError::from(e)
    })?;

    debug!("✓ Connection token issued");
    Ok(token)
}

#[instrument(skip_all)]
pub async fn sign_ucan_pub_key(
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
    repo_ctx: Arc<RepositoryContext>,
    ucan_service: &Arc<RwLock<PermitService>>,
) -> ServiceResult<String> {
    info!("✍️ Signing UCAN public key");

    debug!("🔑 Retrieving UCAN public key");
    let ucan_pub_key = get_ucan_pub_key(repo_ctx, crypto_utils, ucan_service).await?;

    debug!("🔐 Signing public key");
    let crypto = crypto_utils.read().await;
    let signature = crypto.sign_clear_text_message(&ucan_pub_key).map_err(|e| {
        error!("❌ Failed to sign local UCAN public key: {}", e);
        ServiceError::Crypto(e)
    })?;

    debug!("✓ UCAN public key signed");
    Ok(signature)
}

/// Update user's UCAN token and CID
#[instrument(skip(repo_ctx, new_ucan_token), fields(
    user_id = %user_id
))]
pub async fn update_ucan(
    user_id: &str,
    new_ucan_token: String,
    new_ucan_cid: String,
    repo_ctx: Arc<RepositoryContext>,
) -> ServiceResult<()> {
    info!("🔄 Updating UCAN token for user");

    repo_ctx
        .user_repo
        .update_ucan(user_id, new_ucan_token, new_ucan_cid)
        .await?;

    info!("✓ UCAN token updated");
    Ok(())
}

/// Parsed connection string data
#[derive(Debug, Clone)]
pub struct ConnectionStringData {
    pub username: String,
    pub user_public_key: String,
    pub device_public_key: String,
    /// The permit (UCAN token) for authorization
    pub permit: String,
    pub folder_id: Option<String>,
}

/// Parse and decode a connection string (base64-encoded JSON)
/// Returns all fields needed for both sovereign node and viewer connections
#[instrument(skip(connection_string), fields(has_folder_id))]
pub fn parse_connection_string(
    connection_string: &str
) -> ServiceResult<ConnectionStringData> {
    use base64::{Engine as _, engine::general_purpose};
    use crate::errors::UserServiceError;

    info!("🔍 Parsing connection string");

    // 1. Decode base64
    debug!("📥 Decoding base64");
    let decoded = general_purpose::STANDARD
        .decode(connection_string)
        .map_err(|e| {
            error!("❌ Failed to decode base64: {}", e);
            UserServiceError::InvalidUserData {
                field: "connection_string".into(),
                reason: format!("Failed to decode base64: {}", e),
            }
        })?;

    // 2. Convert to UTF-8
    debug!("📄 Converting to UTF-8");
    let json_str = String::from_utf8(decoded)
        .map_err(|e| {
            error!("❌ Invalid UTF-8: {}", e);
            UserServiceError::InvalidUserData {
                field: "connection_string".into(),
                reason: format!("Invalid UTF-8: {}", e),
            }
        })?;

    // 3. Parse JSON
    debug!("📋 Parsing JSON");
    let details: serde_json::Value = serde_json::from_str(&json_str)
        .map_err(|e| {
            error!("❌ Invalid JSON: {}", e);
            UserServiceError::InvalidUserData {
                field: "connection_string".into(),
                reason: format!("Invalid JSON: {}", e),
            }
        })?;

    // 4. Extract all fields
    debug!("🔍 Extracting fields from JSON");
    let data = ConnectionStringData {
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
        permit: details["permit"].as_str()
            .ok_or_else(|| UserServiceError::InvalidUserData {
                field: "permit".into(),
                reason: "Missing permit in connection string".into(),
            })?
            .to_string(),
        folder_id: details["folder_id"].as_str().map(|s| s.to_string()),
    };

    tracing::Span::current().record("has_folder_id", data.folder_id.is_some());
    info!("✓ Connection string parsed successfully (username: {}, has_folder_id: {})",
        data.username, data.folder_id.is_some());

    Ok(data)
}

/// Get a user by ID
///
/// # Arguments
/// * `user_id` - User ID
/// * `repo_ctx` - Repository context
///
/// # Returns
/// * `User` - The user
pub async fn get_user_by_id(
    user_id: &str,
    repo_ctx: Arc<RepositoryContext>,
) -> ServiceResult<User> {
    repo_ctx
        .user_repo
        .get_user_by_id(user_id)
        .await
        .map_err(|e| {
            error!("Failed to get user {}: {}", user_id, e);
            ServiceError::User(UserServiceError::UserNotFound { user_id: user_id.to_string() })
        })
}

/// Get a device by ID
///
/// # Arguments
/// * `device_id` - Device ID
/// * `repo_ctx` - Repository context
///
/// # Returns
/// * `Device` - The device
pub async fn get_device_by_id(
    device_id: &str,
    repo_ctx: Arc<RepositoryContext>,
) -> ServiceResult<Device> {
    let devices = repo_ctx
        .device_repo
        .get_devices_by_ids(&[device_id.to_string()])
        .await
        .map_err(|e| {
            error!("Failed to get device {}: {}", device_id, e);
            ServiceError::User(UserServiceError::DeviceNotFound { device_id: device_id.to_string() })
        })?;

    devices
        .into_iter()
        .next()
        .ok_or_else(|| {
            error!("Device {} not found in results", device_id);
            ServiceError::User(UserServiceError::DeviceNotFound { device_id: device_id.to_string() })
        })
}
