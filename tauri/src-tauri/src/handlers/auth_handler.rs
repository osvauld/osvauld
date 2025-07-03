use crate::types::{
    AddDeviceInput, CryptoResponse, ExportedCertificate, FirstDeviceConnectInput, LoadPvtKeyInput,
    PasswordChangeInput, SavePassphraseInput,
};
use crate::user_state::UserState;
use crypto_utils::CryptoUtils;
use log::{error, info};
use osvauld_core::models::p2p::{ConnectionAction, ConnectionType};
use osvauld_db::database::RepositoryContext;
use osvauld_services::{
    change_passphrase, create_default_folder, export_certificate, get_rendezvous_payload,
    handle_signup, import_user, is_signed_up, load_certificate,
};
use p2p_service::P2PService;
use rendezvous_client::rendezvous_service::RendezvousService;
use std::sync::Arc;
use tauri::State;
use tokio::sync::Mutex;

#[tauri::command]
pub async fn check_signup_status(
    repo_ctx: State<'_, RepositoryContext>,
) -> Result<CryptoResponse, String> {
    let is_signed_up = is_signed_up(&*repo_ctx).await?;
    Ok(CryptoResponse::IsSignedUp { is_signed_up })
}

#[tauri::command]
pub async fn get_user_details(user_state: State<'_, UserState>) -> Result<CryptoResponse, String> {
    let user = user_state.get_user().await?;
    let device = user_state.get_device().await?;
    Ok(CryptoResponse::UserDetails {
        user_id: user.id,
        username: user.username,
        device_id: device.id,
        public_key: user.public_key,
        device_key: device.device_key,
    })
}

#[tauri::command]
pub async fn handle_sign_up(
    input: SavePassphraseInput,
    repo_ctx: State<'_, RepositoryContext>,
) -> Result<CryptoResponse, String> {
    let _result = handle_signup(&input.username, &input.passphrase, &repo_ctx).await?;
    let _ = create_default_folder(&repo_ctx)
        .await
        .map_err(|e| e.to_string())?;
    Ok(CryptoResponse::Success)
}

#[tauri::command]
pub async fn check_private_key_loaded(
    crypto_utils: State<'_, Arc<Mutex<CryptoUtils>>>,
) -> Result<CryptoResponse, String> {
    let crypto = crypto_utils.lock().await;
    Ok(CryptoResponse::CheckPvtKeyLoaded(crypto.is_cert_loaded()))
}

#[tauri::command]
pub async fn login(
    input: LoadPvtKeyInput,
    rendezvous_service: State<'_, Arc<RendezvousService>>,
    user_state: State<'_, UserState>,
    p2p_service: State<'_, Arc<P2PService>>,
    crypto_utils: State<'_, Arc<Mutex<CryptoUtils>>>,
    repo_ctx: State<'_, RepositoryContext>,
) -> Result<CryptoResponse, String> {
    let (user, current_device) =
        load_certificate(&input.passphrase, &repo_ctx, &crypto_utils).await?;
    let rendezvous_clone = rendezvous_service.inner().clone();
    {
        let mut current_user_state = user_state.current_user.write().await;
        current_user_state.user = Some(user.clone());
        current_user_state.device = Some(current_device.clone());
    }
    p2p_service
        .start_p2p_service(&current_device, &user)
        .await?;

    // Spawn a background task to handle WebSocket connection

    let user_clone = user.clone();
    let rendezvous_payload = get_rendezvous_payload(&current_device.id, &repo_ctx).await?;

    tokio::spawn(async move {
        match rendezvous_clone
            .initialize(
                format!("{}:{}", user_clone.id.clone(), current_device.id.clone()),
                rendezvous_payload,
            )
            .await
        {
            Ok(_) => {
                info!(
                    "Successfully connected to rendezvous server with user ID: {}",
                    user_clone.id.clone()
                );
            }
            Err(e) => {
                error!("Failed to connect to rendezvous server: {}", e);
            }
        }
    });

    Ok(CryptoResponse::User {
        user_id: user.id.clone(),
        username: user.username,
        public_key: user.public_key,
    })
}

#[tauri::command]
pub async fn handle_add_device(
    input: AddDeviceInput,
    repo_ctx: State<'_, RepositoryContext>,
) -> Result<CryptoResponse, String> {
    import_user(
        &input.certificate,
        &input.passphrase,
        &input.username,
        &input.device_id,
        &repo_ctx,
    )
    .await?;
    Ok(CryptoResponse::Success)
}

#[tauri::command]
pub async fn handle_export_certificate(
    input: ExportedCertificate,
    repo_ctx: State<'_, RepositoryContext>,
) -> Result<CryptoResponse, String> {
    let exported_cert = export_certificate(input.passphrase, &*repo_ctx).await?;
    Ok(CryptoResponse::ExportedCertificate(exported_cert))
}

#[tauri::command]
pub async fn handle_change_passphrase(
    input: PasswordChangeInput,
    repo_ctx: State<'_, RepositoryContext>,
) -> Result<CryptoResponse, String> {
    let _new_certificate =
        change_passphrase(input.old_password, input.new_password, &repo_ctx).await?;

    Ok(CryptoResponse::Success)
}

#[tauri::command]
pub async fn handle_logout(
    crypto_utils: State<'_, Arc<Mutex<CryptoUtils>>>,
) -> Result<CryptoResponse, String> {
    let mut crypto = crypto_utils.lock().await;
    crypto.clear_cert();
    Ok(CryptoResponse::Success)
}
