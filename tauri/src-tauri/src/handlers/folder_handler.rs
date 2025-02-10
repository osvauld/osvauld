use crate::application::services::{FolderService, SyncService};
use crate::domains::models::sync_types::ResourceType;
use crate::types::{AddFolderInput, CryptoResponse, FolderResponse, SoftDeleteFolder};
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn handle_add_folder(
    input: AddFolderInput,
    folder_service: State<'_, Arc<FolderService>>,
    sync_service: State<'_, Arc<SyncService>>,
) -> Result<CryptoResponse, String> {
    log::info!("Adding folder: ");
    let folder = folder_service
        .create_folder(input.name, Some(input.description))
        .await
        .map_err(|e| e.to_string())?;
    let _ = sync_service.add_folder_to_sync(folder.clone()).await;

    Ok(CryptoResponse::FolderCreated(folder))
}

#[tauri::command]
pub async fn handle_get_folders(
    folder_service: State<'_, Arc<FolderService>>,
) -> Result<CryptoResponse, String> {
    let folders = folder_service
        .get_all_folders()
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
pub async fn soft_delete_folder(
    input: SoftDeleteFolder,
    folder_service: State<'_, Arc<FolderService>>,
    sync_service: State<'_, Arc<SyncService>>,
) -> Result<CryptoResponse, String> {
    folder_service
        .soft_delete_folder(&input.folder_id)
        .await
        .map_err(|e| e.to_string())?;
    sync_service
        .add_soft_deletion_sync_record(input.folder_id, ResourceType::Folder)
        .await
        .map_err(|e| e.to_string())?;
    Ok(CryptoResponse::Success)
}
