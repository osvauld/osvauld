//! Folder handlers - Simplified for new architecture
//!
//! Uses Butler facade

use crate::types::{
    AddFolderInput, BaseCryptoResponse, FolderResponse, FolderShareUsersInput,
    RequestFolderResourcesInput, ShareFolder, SoftDeleteFolder,
};
use butler::Butler;
use tracing::{info, instrument};
use std::sync::Arc;
use tauri::State;

#[tauri::command]
#[instrument(skip(input, butler), fields(folder_name = %input.name))]
pub async fn handle_add_folder(
    input: AddFolderInput,
    butler: State<'_, Arc<Butler>>,
) -> Result<BaseCryptoResponse, String> {
    let user = butler.user_info().await
        .map_err(|e| e.to_string())?;

    let space = butler
        .create_space(input.name.clone(), user.did.clone(), &input.folder_template_json)
        .await
        .map_err(|e| format!("Failed to create space: {}", e))?;

    info!("Space created: {} (id: {})", space.name, space.id);

    Ok(BaseCryptoResponse::FolderCreated(space))
}

#[tauri::command]
#[instrument(skip(butler))]
pub async fn handle_get_folders(
    butler: State<'_, Arc<Butler>>,
) -> Result<BaseCryptoResponse, String> {
    let spaces = butler
        .list_spaces()
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
#[instrument(skip(input, butler), fields(folder_id = %input.folder_id))]
pub async fn handle_soft_delete_folder(
    input: SoftDeleteFolder,
    butler: State<'_, Arc<Butler>>,
) -> Result<BaseCryptoResponse, String> {
    butler
        .delete_space(&input.folder_id)
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
    let _ = input; // Silence unused warning
    Ok(BaseCryptoResponse::Users(vec![]))
}

#[tauri::command]
#[instrument(skip(input, butler), fields(folder_id = %input.folder_id))]
pub async fn handle_share_folder(
    input: ShareFolder,
    butler: State<'_, Arc<Butler>>,
) -> Result<BaseCryptoResponse, String> {
    // Track that space was shared with user (pubkey only, actual permit in Contact.shares)
    butler
        .share_space(&input.folder_id, input.user_id.clone())
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
