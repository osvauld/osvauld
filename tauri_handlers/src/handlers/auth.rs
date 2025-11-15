use crate::config::HandlerConfig;
use crate::types::{
    AddDeviceInput, BaseCryptoResponse, ExportedCertificate, LoadPvtKeyInput, PasswordChangeInput,
    SavePassphraseInput, UcanOneTimeTokenOut,
};
use crate::user_state::UserState;
use crypto_utils::CryptoUtils;
use tracing::{error, info, instrument};
use network::P2PService;
use osvauld_core::models::UserRole;
use persistance::database::RepositoryContext;
use search_indexer::SearchIndexManager;
use services::{
    change_passphrase, export_certificate, generate_one_time_ucan_token, handle_signup,
    import_user, is_signed_up, load_certificate,
};
use std::sync::Arc;
use tauri::State;
use tokio::sync::{Mutex, RwLock};

/// Check if user has completed the signup process
#[tauri::command]
#[instrument(skip(repo_ctx))]
pub async fn check_signup_status(
    repo_ctx: State<'_, Arc<RepositoryContext>>,
) -> Result<BaseCryptoResponse, String> {
    let is_signed_up = is_signed_up(repo_ctx.inner().clone())
        .await
        .map_err(|e| e.to_string())?;
    Ok(BaseCryptoResponse::IsSignedUp { is_signed_up })
}

#[tauri::command]
#[instrument(skip(user_state))]
pub async fn get_user_details(user_state: State<'_, UserState>) -> Result<BaseCryptoResponse, String> {
    let user = user_state.get_user().await?;
    let device = user_state.get_device().await?;
    Ok(BaseCryptoResponse::UserDetails {
        user_id: user.id,
        username: user.username,
        device_id: device.id,
        public_key: user.public_key,
        device_key: device.device_key,
    })
}

#[tauri::command]
#[instrument(skip(input, config, repo_ctx), fields(username = %input.username))]
pub async fn handle_sign_up(
    input: SavePassphraseInput,
    config: State<'_, HandlerConfig>,
    repo_ctx: State<'_, Arc<RepositoryContext>>,
) -> Result<BaseCryptoResponse, String> {
    let _result = handle_signup(
        &input.username,
        &input.passphrase,
        repo_ctx.inner().clone(),
        &config.domain,
    )
    .await
    .map_err(|e| e.to_string())?;
    Ok(BaseCryptoResponse::Success)
}

#[tauri::command]
#[instrument(skip(crypto_utils))]
pub async fn check_private_key_loaded(
    crypto_utils: State<'_, Arc<RwLock<CryptoUtils>>>,
) -> Result<BaseCryptoResponse, String> {
    let crypto = crypto_utils.read().await;
    Ok(BaseCryptoResponse::CheckPvtKeyLoaded(crypto.is_cert_loaded()))
}

#[tauri::command]
#[instrument(skip_all)]
pub async fn login(
    input: LoadPvtKeyInput,
    user_state: State<'_, UserState>,
    _p2p_service: State<'_, Arc<P2PService>>,
    crypto_utils: State<'_, Arc<RwLock<CryptoUtils>>>,
    ucan_service: State<'_, Arc<RwLock<gurkha::UcanService>>>,
    repo_ctx: State<'_, Arc<RepositoryContext>>,
    search_manager: State<'_, Arc<Mutex<SearchIndexManager>>>,
) -> Result<BaseCryptoResponse, String> {
    let (user, current_device) =
        load_certificate(&input.passphrase, repo_ctx.inner().clone(), &crypto_utils)
            .await
            .map_err(|e| e.to_string())?;
    {
        let mut current_user_state = user_state.current_user.write().await;
        current_user_state.user = Some(user.clone());
        current_user_state.device = Some(current_device.clone());
    }

    // Load UCAN keys into ucan_service
    let encrypted_ucan_key = repo_ctx
        .store_repo
        .get_ucan_key()
        .await
        .map_err(|e| e.to_string())?;
    let (signing_key, verifying_key) = {
        let crypto = crypto_utils.read().await;
        crypto
            .decrypt_ucan_key(&encrypted_ucan_key)
            .map_err(|e| e.to_string())?
    };
    {
        let mut ucan_guard = ucan_service.write().await;
        ucan_guard.load_keys(signing_key, verifying_key);
    }

    // TODO: Re-implement P2P service startup after Loro migration
    // let p2p_service_clone = p2p_service.inner().clone();
    // let device_clone = current_device.clone();
    // let user_clone = user.clone();
    // tokio::spawn(async move {
    //     if let Err(e) = p2p_service_clone
    //         .start_p2p_service(&device_clone, &user_clone)
    //         .await
    //     {
    //         error!("Failed to start P2P service: {}", e);
    //     }
    // });

    let search_manager_clone = search_manager.inner().clone();
    let crypto_utils_clone = crypto_utils.inner().clone();
    let repo_ctx_clone = repo_ctx.inner().clone();
    let user_pub_key = user.public_key.clone();
    tokio::spawn(async move {
        match search_manager_clone
            .lock()
            .await
            .initialize(&crypto_utils_clone, &repo_ctx_clone, user_pub_key)
            .await
        {
            Ok(_) => {
                SearchIndexManager::start_scheduled_save(
                    search_manager_clone.clone(),
                    repo_ctx_clone.clone(),
                    5, // Save every 5 minutes
                );
                info!("Search index initialized successfully")
            }
            Err(e) => error!("Failed to initialize search index: {}", e),
        }
    });
    Ok(BaseCryptoResponse::User {
        user_id: user.id.clone(),
        username: user.username,
        public_key: user.public_key,
    })
}

#[tauri::command]
#[instrument(skip(input, repo_ctx), fields(username = %input.username, device_id = %input.device_id))]
pub async fn handle_add_device(
    input: AddDeviceInput,
    repo_ctx: State<'_, Arc<RepositoryContext>>,
) -> Result<BaseCryptoResponse, String> {
    import_user(
        &input.certificate,
        &input.passphrase,
        &input.username,
        &input.device_id,
        repo_ctx.inner().clone(),
    )
    .await
    .map_err(|e| e.to_string())?;
    Ok(BaseCryptoResponse::Success)
}

#[tauri::command]
#[instrument(skip_all)]
pub async fn handle_export_certificate(
    input: ExportedCertificate,
    repo_ctx: State<'_, Arc<RepositoryContext>>,
) -> Result<BaseCryptoResponse, String> {
    let exported_cert = export_certificate(input.passphrase, repo_ctx.inner().clone())
        .await
        .map_err(|e| e.to_string())?;
    Ok(BaseCryptoResponse::ExportedCertificate(exported_cert))
}

#[tauri::command]
#[instrument(skip_all)]
pub async fn handle_change_passphrase(
    input: PasswordChangeInput,
    repo_ctx: State<'_, Arc<RepositoryContext>>,
) -> Result<BaseCryptoResponse, String> {
    let _new_certificate = change_passphrase(
        input.old_password,
        input.new_password,
        repo_ctx.inner().clone(),
    )
    .await
    .map_err(|e| e.to_string())?;

    Ok(BaseCryptoResponse::Success)
}

#[tauri::command]
#[instrument(skip_all)]
pub async fn handle_logout(
    crypto_utils: State<'_, Arc<RwLock<CryptoUtils>>>,
    ucan_service: State<'_, Arc<RwLock<gurkha::UcanService>>>,
) -> Result<BaseCryptoResponse, String> {
    let mut crypto = crypto_utils.write().await;
    crypto.clear_cert();

    // Clear UCAN keys
    let mut ucan_guard = ucan_service.write().await;
    ucan_guard.clear_keys();

    Ok(BaseCryptoResponse::Success)
}

#[tauri::command]
#[instrument(skip_all)]
pub async fn get_one_time_ucan_token(
    config: State<'_, HandlerConfig>,
    ucan_service: State<'_, Arc<RwLock<gurkha::UcanService>>>,
) -> Result<BaseCryptoResponse, String> {
    let (ucan_token, ucan_pub_key) = generate_one_time_ucan_token(
        &config.domain,
        &UserRole::User.to_string(),
        &ucan_service,
    )
    .await
    .map_err(|e| e.to_string())?;
    Ok(BaseCryptoResponse::OneTimeUcanToken(UcanOneTimeTokenOut {
        ucan_token,
        ucan_pub_key,
    }))
}
