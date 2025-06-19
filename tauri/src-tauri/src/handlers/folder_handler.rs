use crate::types::{AddFolderInput, CryptoResponse, FolderResponse, SoftDeleteFolder};
use osvauld_db::database::RepositoryContext;
use osvauld_services::{create_folder, get_all_folders, soft_delete_folder};
use tauri::State;

#[tauri::command]
pub async fn handle_add_folder(
    input: AddFolderInput,
    repo_ctx: State<'_, RepositoryContext>,
) -> Result<CryptoResponse, String> {
    log::info!("Adding folder: ");
    let folder = create_folder(input.name, Some(input.description), &repo_ctx)
        .await
        .map_err(|e| e.to_string())?;

    Ok(CryptoResponse::FolderCreated(folder))
}

#[tauri::command]
pub async fn handle_get_folders(
    repo_ctx: State<'_, RepositoryContext>,
) -> Result<CryptoResponse, String> {
    let folders = get_all_folders(&repo_ctx)
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
    repo_ctx: State<'_, RepositoryContext>,
) -> Result<CryptoResponse, String> {
    soft_delete_folder(&input.folder_id, &repo_ctx)
        .await
        .map_err(|e| e.to_string())?;
    // sync_service
    //     .add_soft_deletion_sync_record(input.folder_id, ResourceType::Folder)
    //     .await
    //     .map_err(|e| e.to_string())?;
    Ok(CryptoResponse::Success)
}
