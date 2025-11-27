//! Folder handlers - Simplified for new architecture
//!
//! Uses Butler's SpaceService

use crate::types::{
    AddFolderInput, BaseCryptoResponse, FolderResponse, FolderShareUsersInput,
    RequestFolderResourcesInput, ShareFolder, SoftDeleteFolder,
};
use crate::user_state::UserState;
use butler::SpaceService;
use tracing::{info, instrument};
use std::sync::Arc;
use tauri::State;

#[tauri::command]
#[instrument(skip(input, user_state, space_service), fields(folder_name = %input.name))]
pub async fn handle_add_folder(
    input: AddFolderInput,
    user_state: State<'_, UserState>,
    space_service: State<'_, Arc<SpaceService>>,
) -> Result<BaseCryptoResponse, String> {
    let user = user_state.get_user().await?;

    let space = space_service
        .create_space(input.name.clone(), user.did.clone())
        .map_err(|e| format!("Failed to create space: {}", e))?;

    info!("Space created: {} (id: {})", space.name, space.id);

    Ok(BaseCryptoResponse::FolderCreated(space))
}

#[tauri::command]
#[instrument(skip(space_service))]
pub async fn handle_get_folders(
    space_service: State<'_, Arc<SpaceService>>,
) -> Result<BaseCryptoResponse, String> {
    let spaces = space_service
        .list_all_spaces()
        .map_err(|e| e.to_string())?;

    let folder_responses: Vec<FolderResponse> = spaces
        .into_iter()
        .map(|space| FolderResponse {
            id: space.id,
            name: space.name,
            description: space.description.unwrap_or_default(),
            default: space.is_default,
        })
        .collect();

    Ok(BaseCryptoResponse::Folders(folder_responses))
}

#[tauri::command]
#[instrument(skip(input, space_service), fields(folder_id = %input.folder_id))]
pub async fn handle_soft_delete_folder(
    input: SoftDeleteFolder,
    space_service: State<'_, Arc<SpaceService>>,
) -> Result<BaseCryptoResponse, String> {
    space_service
        .delete_space(&input.folder_id, true)
        .map_err(|e| format!("Failed to delete space: {}", e))?;

    info!("Space deleted: {}", input.folder_id);
    Ok(BaseCryptoResponse::Success)
}

#[tauri::command]
#[instrument(skip(input), fields(folder_id = %input.folder_id))]
pub async fn handle_get_shared_folder_users(
    input: FolderShareUsersInput,
) -> Result<BaseCryptoResponse, String> {
    // TODO: Implement with Butler
    Ok(BaseCryptoResponse::Users(vec![]))
}

#[tauri::command]
#[instrument(skip(input, space_service), fields(folder_id = %input.folder_id))]
pub async fn handle_share_folder(
    input: ShareFolder,
    space_service: State<'_, Arc<SpaceService>>,
) -> Result<BaseCryptoResponse, String> {
    space_service
        .share_space(&input.folder_id, input.user_id.clone(), input.recipient_role.clone())
        .map_err(|e| format!("Failed to share space: {}", e))?;

    info!("Space shared: {} with {}", input.folder_id, input.user_id);
    Ok(BaseCryptoResponse::Success)
}

#[tauri::command]
#[instrument(skip(input), fields(folder_id = %input.folder_id))]
pub async fn handle_request_folder_resources(
    input: RequestFolderResourcesInput,
) -> Result<BaseCryptoResponse, String> {
    // TODO: Implement with Courier
    info!("Folder resource request: {}", input.folder_id);
    Ok(BaseCryptoResponse::Success)
}
