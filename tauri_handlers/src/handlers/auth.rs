//! Auth handlers - Simplified for new architecture
//!
//! Uses Butler + Herald for authentication

use crate::types::{BaseCryptoResponse, LoadPvtKeyInput, SavePassphraseInput, OneTimePermitOut};
use crate::user_state::UserState;
use butler::RedbStore;
use ed25519_dalek::SigningKey;
use gurkha::PermitService;
use tracing::{error, info, instrument};
use std::sync::Arc;
use tauri::State;
use tokio::sync::RwLock;

/// Check if user has completed the signup process
#[tauri::command]
#[instrument(skip(redb_store))]
pub async fn check_signup_status(
    redb_store: State<'_, Arc<RedbStore>>,
) -> Result<BaseCryptoResponse, String> {
    let is_signed_up = butler::is_signed_up(&redb_store)
        .map_err(|e| e.to_string())?;

    Ok(BaseCryptoResponse::IsSignedUp { is_signed_up })
}

#[tauri::command]
#[instrument(skip(user_state))]
pub async fn get_user_details(
    user_state: State<'_, UserState>,
) -> Result<BaseCryptoResponse, String> {
    let user = user_state.get_user().await?;
    let identity = user_state.get_identity().await?;

    Ok(BaseCryptoResponse::UserDetails {
        user_id: user.did.clone(),
        username: user.username,
        device_id: identity.did().to_string(), // Device ID is same as DID for now
        public_key: base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &user.public_key),
        device_key: base64::Engine::encode(&base64::engine::general_purpose::STANDARD, identity.public_device_key()),
    })
}

#[tauri::command]
#[instrument(skip(input, redb_store), fields(username = %input.username))]
pub async fn handle_sign_up(
    input: SavePassphraseInput,
    redb_store: State<'_, Arc<RedbStore>>,
) -> Result<BaseCryptoResponse, String> {
    let result = butler::signup(&redb_store, &input.username, &input.passphrase)
        .map_err(|e| e.to_string())?;

    info!("Signup successful for {}", input.username);

    // Return the mnemonic seed phrase for user to backup
    Ok(BaseCryptoResponse::SeedPhrase {
        mnemonic: result.mnemonic,
    })
}

#[tauri::command]
#[instrument(skip(user_state))]
pub async fn check_private_key_loaded(
    user_state: State<'_, UserState>,
) -> Result<BaseCryptoResponse, String> {
    let loaded = user_state.is_logged_in().await;
    Ok(BaseCryptoResponse::CheckPvtKeyLoaded(loaded))
}

#[tauri::command]
#[instrument(skip_all)]
pub async fn login(
    input: LoadPvtKeyInput,
    user_state: State<'_, UserState>,
    permit_service: State<'_, Arc<RwLock<PermitService>>>,
    redb_store: State<'_, Arc<RedbStore>>,
) -> Result<BaseCryptoResponse, String> {
    // Login with Butler (returns Herald Identity)
    let identity = butler::login(&redb_store, &input.passphrase)
        .map_err(|e| e.to_string())?;

    // Get username from identity data
    let identity_data = butler::get_identity_data(&redb_store)
        .map_err(|e| e.to_string())?
        .ok_or("No identity data found")?;

    info!("Login successful for {}", identity_data.username);

    // Store identity in user state
    user_state.set_identity(identity.clone(), identity_data.username.clone()).await;

    // Load signing key into PermitService
    let signing_key = SigningKey::from_bytes(&identity.secret_signing_key());
    let verifying_key = signing_key.verifying_key();
    {
        let mut permit_guard = permit_service.write().await;
        permit_guard.load_keys(signing_key, verifying_key);
    }
    info!("PermitService loaded with Herald's signing key");

    Ok(BaseCryptoResponse::User {
        user_id: identity.did().to_string(),
        username: identity_data.username,
        public_key: base64::Engine::encode(&base64::engine::general_purpose::STANDARD, identity.public_signing_key()),
    })
}

#[tauri::command]
#[instrument(skip_all)]
pub async fn handle_add_device(
    _input: crate::types::AddDeviceInput,
) -> Result<BaseCryptoResponse, String> {
    // TODO: Implement device import using Herald
    Err("Device import not yet implemented".to_string())
}

#[tauri::command]
#[instrument(skip_all)]
pub async fn handle_export_certificate(
    _input: crate::types::ExportedCertificate,
) -> Result<BaseCryptoResponse, String> {
    // TODO: Implement certificate export using Herald
    Err("Certificate export not yet implemented".to_string())
}

#[tauri::command]
#[instrument(skip_all)]
pub async fn handle_change_passphrase(
    input: crate::types::PasswordChangeInput,
    redb_store: State<'_, Arc<RedbStore>>,
) -> Result<BaseCryptoResponse, String> {
    butler::change_passphrase(&redb_store, &input.old_password, &input.new_password)
        .map_err(|e| e.to_string())?;

    info!("Passphrase changed successfully");
    Ok(BaseCryptoResponse::Success)
}

#[tauri::command]
#[instrument(skip_all)]
pub async fn handle_logout(
    user_state: State<'_, UserState>,
    permit_service: State<'_, Arc<RwLock<PermitService>>>,
) -> Result<BaseCryptoResponse, String> {
    user_state.clear().await;

    let mut permit_guard = permit_service.write().await;
    permit_guard.clear_keys();

    Ok(BaseCryptoResponse::Success)
}

#[tauri::command]
#[instrument(skip_all)]
pub async fn get_one_time_permit(
    permit_service: State<'_, Arc<RwLock<PermitService>>>,
) -> Result<BaseCryptoResponse, String> {
    let permit_guard = permit_service.read().await;

    let (permit, _cid) = permit_guard
        .issue_one_time("owner")
        .await
        .map_err(|e| format!("Failed to generate token: {:?}", e))?;

    let permit_pub_key = permit_guard
        .get_public_key()
        .map_err(|e| format!("Failed to get public key: {:?}", e))?;

    Ok(BaseCryptoResponse::OneTimePermit(OneTimePermitOut {
        permit,
        permit_pub_key,
    }))
}
