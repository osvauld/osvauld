use crate::types::{
    AddDeviceInput, CryptoResponse, ExportedCertificate, FirstDeviceConnectInput, HashAndSignInput,
    LoadPvtKeyInput, PasswordChangeInput, SavePassphraseInput, SignChallengeInput,
};
use crate::user_state::UserState;
use log::{error, info};
use osvauld_core::models::p2p::{ConnectionAction, ConnectionType};
use osvauld_services::{AuthService, FolderService, TransactionService, UserService};
use p2p_service::P2PService;
use rendezvous_client::rendezvous_service::RendezvousService;
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn check_signup_status(
    auth_service: State<'_, Arc<AuthService>>,
) -> Result<CryptoResponse, String> {
    let is_signed_up = auth_service.is_signed_up().await?;
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
    auth_service: State<'_, Arc<AuthService>>,
    folder_service: State<'_, Arc<FolderService>>,
    transaction_service: State<'_, Arc<TransactionService>>,
) -> Result<CryptoResponse, String> {
    let (user, certificate) = auth_service
        .handle_sign_up(&input.username, &input.passphrase)
        .await?;
    let (device, device_certificate, sync_record_set) = auth_service
        .create_device_objects(&user.id, &input.username)
        .await?;
    info!("device, {:?}", device);
    info!("user: {:?}", user);

    transaction_service
        .handle_sign_up_transaction(
            &user,
            &certificate,
            &device,
            &device_certificate,
            &sync_record_set,
        )
        .await
        .map_err(|e| e.to_string())?;

    let (folder, sync_record_set) = folder_service
        .create_default_folder(&device.id, &user.id)
        .await
        .map_err(|e| e.to_string())?;
    transaction_service
        .handle_add_folder_transaction(&folder, &sync_record_set)
        .await
        .map_err(|e| e.to_string())?;
    Ok(CryptoResponse::SavePassphrase {
        username: user.username,
        device_key: user.public_key.clone(),
        encryption_key: user.public_key,
        user_id: user.id,
    })
}

#[tauri::command]
pub async fn check_private_key_loaded(
    auth_service: State<'_, Arc<AuthService>>,
) -> Result<CryptoResponse, String> {
    let is_loaded = auth_service.check_private_key_loaded().await?;
    Ok(CryptoResponse::CheckPvtKeyLoaded(is_loaded))
}

#[tauri::command]
pub async fn login(
    input: LoadPvtKeyInput,
    auth_service: State<'_, Arc<AuthService>>,
    user_service: State<'_, Arc<UserService>>,
    rendezvous_service: State<'_, Arc<RendezvousService>>,
    user_state: State<'_, UserState>,
    p2p_service: State<'_, Arc<P2PService>>,
) -> Result<CryptoResponse, String> {
    let (public_key, user_id) = auth_service.load_certificate(&input.passphrase).await?;
    let rendezvous_clone = rendezvous_service.inner().clone();
    let user = user_service.get_user_by_id(&user_id).await?;
    let current_device = auth_service
        .get_current_device()
        .await
        .map_err(|e| e.to_string())?;
    {
        let mut current_user_state = user_state.current_user.write().await;
        current_user_state.user = Some(user.clone());
        current_user_state.device = Some(current_device.clone());
    }

    p2p_service.set_current_user(user.clone()).await;
    p2p_service.set_current_device(current_device.clone()).await;

    // Spawn a background task to handle WebSocket connection

    let user_id_clone = user_id.clone();
    tokio::spawn(async move {
        match rendezvous_clone
            .initialize(
                format!("{}:{}", user.id.clone(), current_device.id),
                &current_device.id,
            )
            .await
        {
            Ok(_) => {
                info!(
                    "Successfully connected to rendezvous server with user ID: {}",
                    user_id_clone
                );
            }
            Err(e) => {
                error!("Failed to connect to rendezvous server: {}", e);
                // Connection failed, but we'll still let the login succeed
                // The application can try to reconnect later if needed
            }
        }
    });

    Ok(CryptoResponse::User {
        user_id,
        username: user.username,
        public_key: user.public_key,
    })
}

#[tauri::command]
pub async fn handle_sign_challenge(
    input: SignChallengeInput,
    auth_service: State<'_, Arc<AuthService>>,
) -> Result<CryptoResponse, String> {
    let signature = auth_service.sign_challenge(&input.challenge).await?;
    Ok(CryptoResponse::Signature(signature))
}

#[tauri::command]
pub async fn handle_hash_and_sign(
    input: HashAndSignInput,
    auth_service: State<'_, Arc<AuthService>>,
) -> Result<CryptoResponse, String> {
    let signature = auth_service.hash_and_sign(&input.message).await?;
    Ok(CryptoResponse::Signature(signature))
}

#[tauri::command]
pub async fn handle_add_device(
    input: AddDeviceInput,
    auth_service: State<'_, Arc<AuthService>>,
    transaction_service: State<'_, Arc<TransactionService>>,
) -> Result<CryptoResponse, String> {
    let (user, certificate) = auth_service
        .import_user(&input.certificate, &input.passphrase, "username")
        .await?;
    let (device, device_certificate, sync_record_set) = auth_service
        .create_device_objects(&user.id, &user.username)
        .await?;
    transaction_service
        .handle_sign_up_transaction(
            &user,
            &certificate,
            &device,
            &device_certificate,
            &sync_record_set,
        )
        .await
        .map_err(|e| e.to_string())?;

    Ok(CryptoResponse::Success)
}

#[tauri::command]
pub async fn handle_export_certificate(
    input: ExportedCertificate,
    auth_service: State<'_, Arc<AuthService>>,
) -> Result<CryptoResponse, String> {
    let exported_cert = auth_service.export_certificate(input.passphrase).await?;
    Ok(CryptoResponse::ExportedCertificate(exported_cert))
}

#[tauri::command]
pub async fn handle_change_passphrase(
    input: PasswordChangeInput,
    auth_service: State<'_, Arc<AuthService>>,
) -> Result<CryptoResponse, String> {
    let new_certificate = auth_service
        .change_passphrase(input.old_password, input.new_password)
        .await?;

    Ok(CryptoResponse::ChangedPassphrase(
        new_certificate.private_key,
    ))
}

#[tauri::command]
pub async fn first_device_connect(
    input: FirstDeviceConnectInput,
    p2p_service: State<'_, Arc<P2PService>>,
) -> Result<CryptoResponse, String> {
    info!("attempting first device sync with peer");
    p2p_service
        .connect_with_ticket(
            &input.ticket,
            ConnectionType::Device,
            None,
            Some(ConnectionAction::AddDevice),
        )
        .await
        .map_err(|e| e.to_string())?;
    Ok(CryptoResponse::Success)
}

#[tauri::command]
pub async fn handle_logout(
    auth_service: State<'_, Arc<AuthService>>,
) -> Result<CryptoResponse, String> {
    auth_service.logout().await?;
    Ok(CryptoResponse::Success)
}

#[tauri::command]
pub async fn get_user_id(
    auth_service: State<'_, Arc<AuthService>>,
) -> Result<CryptoResponse, String> {
    let user_id = auth_service.get_user_id().await?;
    Ok(CryptoResponse::UserId(user_id))
}

#[tauri::command]
pub async fn get_public_key(
    auth_service: State<'_, Arc<AuthService>>,
) -> Result<CryptoResponse, String> {
    let public_key = auth_service.get_public_key().await?;
    Ok(CryptoResponse::PublicKey(public_key))
}
