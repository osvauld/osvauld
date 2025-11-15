use crate::config::HandlerConfig;
use crate::types::{
    AddFolderInput, BaseCryptoResponse, FolderResponse, FolderShareUsersInput, ShareFolder,
    SoftDeleteFolder,
};
use crate::user_state::UserState;
use crypto_utils::CryptoUtils;
use network::P2PService;
use persistance::database::RepositoryContext;
use tracing::instrument;
use services::{
    create_folder, get_all_folders, get_folder_shared_users, share_folder,
    soft_delete_folder,
};
use std::sync::Arc;
use tauri::State;
use tokio::sync::RwLock;

#[tauri::command]
#[instrument(skip(input, config, repo_ctx, ucan_service, user_state), fields(folder_name = %input.name))]
pub async fn handle_add_folder(
    input: AddFolderInput,
    config: State<'_, HandlerConfig>,
    repo_ctx: State<'_, Arc<RepositoryContext>>,
    ucan_service: State<'_, Arc<RwLock<gurkha::UcanService>>>,
    user_state: State<'_, UserState>,
) -> Result<BaseCryptoResponse, String> {
    let user = user_state.get_user().await?;
    let folder = create_folder(
        input.name,
        Some(input.description),
        input.folder_template_json,
        repo_ctx.inner().clone(),
        &config.domain,
        &user,
        &ucan_service,
    )
    .await
    .map_err(|e| e.to_string())?;

    Ok(BaseCryptoResponse::FolderCreated(folder))
}

#[tauri::command]
#[instrument(skip(repo_ctx))]
pub async fn handle_get_folders(
    repo_ctx: State<'_, Arc<RepositoryContext>>,
) -> Result<BaseCryptoResponse, String> {
    let folders = get_all_folders(repo_ctx.inner().clone())
        .await
        .map_err(|e| e.to_string())?;

    let folder_responses: Vec<FolderResponse> = folders
        .into_iter()
        .map(|folder| FolderResponse {
            id: folder.id,
            name: folder.name,
            description: folder.description.unwrap_or_default(),
            default: folder.default_folder,
        })
        .collect();

    Ok(BaseCryptoResponse::Folders(folder_responses))
}

#[tauri::command]
#[instrument(skip(input, repo_ctx), fields(folder_id = %input.folder_id))]
pub async fn handle_soft_delete_folder(
    input: SoftDeleteFolder,
    repo_ctx: State<'_, Arc<RepositoryContext>>,
) -> Result<BaseCryptoResponse, String> {
    soft_delete_folder(&input.folder_id, repo_ctx.inner().clone())
        .await
        .map_err(|e| e.to_string())?;
    Ok(BaseCryptoResponse::Success)
}

#[tauri::command]
#[instrument(skip(input, repo_ctx), fields(folder_id = %input.folder_id))]
pub async fn handle_get_shared_folder_users(
    input: FolderShareUsersInput,
    repo_ctx: State<'_, Arc<RepositoryContext>>,
) -> Result<BaseCryptoResponse, String> {
    let users = get_folder_shared_users(&input.folder_id, repo_ctx.inner().clone())
        .await
        .map_err(|e| e.to_string())?;
    Ok(BaseCryptoResponse::Users(users))
}

#[tauri::command]
#[instrument(skip(input, repo_ctx, user_state, crypto_utils, ucan_service, p2p_service), fields(folder_id = %input.folder_id, recipient = %input.user_id, role = %input.recipient_role))]
pub async fn handle_share_folder(
    input: ShareFolder,
    repo_ctx: State<'_, Arc<RepositoryContext>>,
    user_state: State<'_, UserState>,
    crypto_utils: State<'_, Arc<RwLock<CryptoUtils>>>,
    ucan_service: State<'_, Arc<RwLock<gurkha::UcanService>>>,
    p2p_service: State<'_, Arc<P2PService>>,
) -> Result<BaseCryptoResponse, String> {
    let user = user_state.get_user().await?;

    // 1. Share folder using template-based permissions
    share_folder(
        &input.folder_id,
        &input.user_id,
        &input.recipient_role,
        &user,
        repo_ctx.inner().clone(),
        &ucan_service,
    )
    .await
    .map_err(|e| e.to_string())?;

    // 2. Trigger P2P sync (fire-and-forget, async task spawned internally)
    network::p2p::sync_handler::send_folder(
        input.folder_id.clone(),
        input.user_id.clone(),
        user.clone(),
        repo_ctx.inner().clone(),
        crypto_utils.inner().clone(),
        p2p_service.inner().clone(),
    )
    .await
    .map_err(|e| e.to_string())?;

    Ok(BaseCryptoResponse::Success)
}
