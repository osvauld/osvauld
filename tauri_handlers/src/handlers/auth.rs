//! Auth handlers - Simplified for new architecture
//!
//! Uses Butler for authentication and identity management

use crate::types::{BaseCryptoResponse, LoadPvtKeyInput, SavePassphraseInput, OneTimePermitOut};
use butler::Butler;
use tracing::{info, instrument};
use std::sync::Arc;
use tauri::State;

/// Check if user has completed the signup process
#[tauri::command]
#[instrument(skip(butler))]
pub async fn check_signup_status(
    butler: State<'_, Arc<Butler>>,
) -> Result<BaseCryptoResponse, String> {
    let is_signed_up = butler::is_signed_up(butler.store())
        .map_err(|e| e.to_string())?;

    Ok(BaseCryptoResponse::IsSignedUp { is_signed_up })
}

#[tauri::command]
#[instrument(skip(butler))]
pub async fn get_user_details(
    butler: State<'_, Arc<Butler>>,
) -> Result<BaseCryptoResponse, String> {
    let user = butler.user_info().await
        .map_err(|e| e.to_string())?;
    let identity = butler.get_identity().await
        .map_err(|e| e.to_string())?;

    Ok(BaseCryptoResponse::UserDetails {
        user_id: user.did.clone(),
        username: user.username,
        device_id: identity.did().to_string(), // Device ID is same as DID for now
        public_key: base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &user.public_key),
        device_key: base64::Engine::encode(&base64::engine::general_purpose::STANDARD, identity.public_device_key()),
    })
}

#[tauri::command]
#[instrument(skip(input, butler), fields(username = %input.username))]
pub async fn handle_sign_up(
    input: SavePassphraseInput,
    butler: State<'_, Arc<Butler>>,
) -> Result<BaseCryptoResponse, String> {
    let result = butler::signup(butler.store(), &input.username, &input.passphrase)
        .map_err(|e| e.to_string())?;

    info!("Signup successful for {}", input.username);

    // Return the mnemonic seed phrase for user to backup
    Ok(BaseCryptoResponse::SeedPhrase {
        mnemonic: result.mnemonic,
    })
}

#[tauri::command]
#[instrument(skip(butler))]
pub async fn check_private_key_loaded(
    butler: State<'_, Arc<Butler>>,
) -> Result<BaseCryptoResponse, String> {
    let loaded = butler.is_logged_in().await;
    Ok(BaseCryptoResponse::CheckPvtKeyLoaded(loaded))
}

#[tauri::command]
#[instrument(skip_all)]
pub async fn login(
    input: LoadPvtKeyInput,
    butler: State<'_, Arc<Butler>>,
) -> Result<BaseCryptoResponse, String> {
    // Login with Butler (returns Herald Identity)
    let identity = butler::login(butler.store(), &input.passphrase)
        .map_err(|e| e.to_string())?;

    // Get username from identity data
    let identity_data = butler::get_identity_data(butler.store())
        .map_err(|e| e.to_string())?
        .ok_or("No identity data found")?;

    info!("Login successful for {}", identity_data.username);

    // Store identity in Butler (owns identity state now)
    butler.set_identity(identity.clone()).await;

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
    butler: State<'_, Arc<Butler>>,
) -> Result<BaseCryptoResponse, String> {
    butler::change_passphrase(butler.store(), &input.old_password, &input.new_password)
        .map_err(|e| e.to_string())?;

    info!("Passphrase changed successfully");
    Ok(BaseCryptoResponse::Success)
}

#[tauri::command]
#[instrument(skip_all)]
pub async fn handle_logout(
    butler: State<'_, Arc<Butler>>,
) -> Result<BaseCryptoResponse, String> {
    butler.clear_identity().await;
    Ok(BaseCryptoResponse::Success)
}

#[tauri::command]
#[instrument(skip_all)]
pub async fn get_one_time_permit(
    butler: State<'_, Arc<Butler>>,
) -> Result<BaseCryptoResponse, String> {
    // Relationship is "node_owner" = node issuing permit TO owner
    let (permit, permit_pub_key) = butler.issue_one_time_permit("node_owner")
        .await
        .map_err(|e| format!("Failed to generate token: {}", e))?;

    Ok(BaseCryptoResponse::OneTimePermit(OneTimePermitOut {
        permit,
        permit_pub_key,
    }))
}
