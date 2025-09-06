use std::sync::Arc;

use crate::{
    types::{
        AddFolderInput, CryptoResponse, FolderResponse, FolderShareUsersInput, ShareFolder,
        SoftDeleteFolder,
    },
    user_state::UserState,
};
use crypto_utils::CryptoUtils;
use network::P2PService;
use persistance::database::RepositoryContext;
use services::{
    create_folder, get_all_folders, get_folder_shared_users, share_folder, soft_delete_folder,
};
use tauri::State;
use tokio::sync::Mutex;

#[tauri::command]
pub async fn handle_add_folder(
    input: AddFolderInput,
    repo_ctx: State<'_, Arc<RepositoryContext>>,
    crypto_utils: State<'_, Arc<Mutex<CryptoUtils>>>,
    user_state: State<'_, UserState>,
) -> Result<CryptoResponse, String> {
    let user = user_state.get_user().await?;
    let folder = create_folder(
        input.name,
        Some(input.description),
        repo_ctx.inner().clone(),
        &crypto_utils,
        "livnote",
        &user,
    )
    .await
    .map_err(|e| e.to_string())?;

    Ok(CryptoResponse::FolderCreated(folder))
}

#[tauri::command]
pub async fn handle_get_folders(
    repo_ctx: State<'_, Arc<RepositoryContext>>,
) -> Result<CryptoResponse, String> {
    let folders = get_all_folders(repo_ctx.inner().clone())
        .await
        .map_err(|e| e.to_string())?;

    let folder_responses: Vec<FolderResponse> = folders
        .into_iter()
        .map(|folder| FolderResponse {
            id: folder.id,
            name: folder.name,
            description: folder.description.unwrap_or_default(),
        })
        .collect();

    Ok(CryptoResponse::Folders(folder_responses))
}

#[tauri::command]
pub async fn handle_soft_delete_folder(
    input: SoftDeleteFolder,
    repo_ctx: State<'_, Arc<RepositoryContext>>,
) -> Result<CryptoResponse, String> {
    soft_delete_folder(&input.folder_id, repo_ctx.inner().clone())
        .await
        .map_err(|e| e.to_string())?;
    Ok(CryptoResponse::Success)
}

#[tauri::command]
pub async fn handle_get_shared_folder_users(
    input: FolderShareUsersInput,
    repo_ctx: State<'_, Arc<RepositoryContext>>,
) -> Result<CryptoResponse, String> {
    let users = get_folder_shared_users(&input.folder_id, repo_ctx.inner().clone())
        .await
        .map_err(|e| e.to_string())?;
    Ok(CryptoResponse::Users(users))
}
#[tauri::command]
pub async fn handle_share_folder(
    input: ShareFolder,
    repo_ctx: State<'_, Arc<RepositoryContext>>,
    user_state: State<'_, UserState>,
    crypto_utils: State<'_, Arc<Mutex<CryptoUtils>>>,
    p2p_service: State<'_, Arc<P2PService>>,
) -> Result<CryptoResponse, String> {
    let user = user_state.get_user().await?;
    share_folder(
        &input.folder_id,
        &input.user_id,
        input.permissions,
        &user,
        repo_ctx.inner().clone(),
        &crypto_utils,
        "livnote",
    )
    .await
    .map_err(|e| e.to_string())?;
    p2p_service
        .sync_folders(&input.folder_id)
        .await
        .map_err(|e| e.to_string())?;
    Ok(CryptoResponse::Success)
}
