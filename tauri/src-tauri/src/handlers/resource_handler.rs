use crate::types::{
    AddResourceInput, CryptoResponse, DeleteResourceInput, GetResource, GetResourceForFolderInput,
    ResourceResponse, ShareResource, ToggleFavInput, UpdateLastAccessedInput, UpdateResources,
};
use crate::user_state::UserState;
use crypto_utils::CryptoUtils;
use log::info;
use osvauld_db::database::RepositoryContext;
use osvauld_services::{
    create_resource, delete_resource, get_all_resources, get_resource, get_resource_by_id_direct,
    get_resources_for_folder, share_resource, toggle_fav, update_last_accessed, update_resource,
};
use std::sync::Arc;
use tokio::sync::Mutex;

use tauri::{AppHandle, Emitter, State};
#[tauri::command]
pub async fn handle_add_resource(
    input: AddResourceInput,
    user_state: State<'_, UserState>,
    app_handle: AppHandle,
    crypto_utils: State<'_, Arc<Mutex<CryptoUtils>>>,
    repo_ctx: State<'_, RepositoryContext>,
) -> Result<CryptoResponse, String> {
    let user = user_state.get_user().await?;
    let device = user_state.get_device().await?;
    let resource_added = create_resource(
        input.resource_payload,
        input.resource_type,
        input.folder_id,
        &user,
        &device.id,
        &repo_ctx,
        &crypto_utils,
    )
    .await
    .map_err(|e| e.to_string())?;
    let response = ResourceResponse {
        id: resource_added.id.clone(),
        data: resource_added.data,
        favourite: resource_added.favourite,
        last_accessed: resource_added.last_accessed,
        folder_id: resource_added.folder_id,
    };
    app_handle
        .emit("resource-added", response)
        .map_err(|e| e.to_string())?;

    Ok(CryptoResponse::ResourceCreateted(resource_added.id))
}

#[tauri::command]
pub async fn handle_get_resources_for_folder(
    input: GetResourceForFolderInput,
    crypto_utils: State<'_, Arc<Mutex<CryptoUtils>>>,
    repo_ctx: State<'_, RepositoryContext>,
    user_state: State<'_, UserState>,
) -> Result<CryptoResponse, String> {
    let user = user_state.get_user().await?;
    let resources = get_resources_for_folder(&input.folder_id, &crypto_utils, &user.id, &repo_ctx)
        .await
        .map_err(|e| e.to_string())?;

    let resource_responses = resources
        .into_iter()
        .map(|cred| ResourceResponse {
            id: cred.id,
            data: cred.data,
            favourite: cred.favourite,
            last_accessed: cred.last_accessed,
            folder_id: cred.folder_id,
        })
        .collect();

    Ok(CryptoResponse::Resources(resource_responses))
}

#[tauri::command]
pub async fn soft_delete_resource(
    input: DeleteResourceInput,
    repo_ctx: State<'_, RepositoryContext>,
) -> Result<(), String> {
    info!("deleting resource {}", input.resource_id);
    delete_resource(input.resource_id.clone(), &repo_ctx)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn handle_toggle_fav(
    input: ToggleFavInput,
    repo_ctx: State<'_, RepositoryContext>,
) -> Result<CryptoResponse, String> {
    toggle_fav(input.resource_id, &repo_ctx)
        .await
        .map_err(|e| e.to_string())?;
    Ok(CryptoResponse::Success)
}
#[tauri::command]
pub async fn handle_update_last_accessed(
    input: UpdateLastAccessedInput,
    repo_ctx: State<'_, RepositoryContext>,
) -> Result<CryptoResponse, String> {
    update_last_accessed(input.resource_id, &repo_ctx)
        .await
        .map_err(|e| e.to_string())?;
    Ok(CryptoResponse::Success)
}

#[tauri::command]
pub async fn handle_get_all_resources(
    crypto_utils: State<'_, Arc<Mutex<CryptoUtils>>>,
    repo_ctx: State<'_, RepositoryContext>,
    user_state: State<'_, UserState>,
) -> Result<CryptoResponse, String> {
    let user = user_state.get_user().await?;
    let resources = get_all_resources(&crypto_utils, &repo_ctx, &user.id)
        .await
        .map_err(|e| e.to_string())?;
    let resource_responses = resources
        .into_iter()
        .map(|cred| ResourceResponse {
            id: cred.id,
            data: cred.data,
            favourite: cred.favourite,
            last_accessed: cred.last_accessed,
            folder_id: cred.folder_id,
        })
        .collect();

    Ok(CryptoResponse::Resources(resource_responses))
}

#[tauri::command]
pub async fn handle_update_resource(
    input: UpdateResources,
    user_state: State<'_, UserState>,
    app_handle: AppHandle,
    crypto_utils: State<'_, Arc<Mutex<CryptoUtils>>>,
    repo_ctx: State<'_, RepositoryContext>,
) -> Result<CryptoResponse, String> {
    //TODO: migrate obsolete user records to another table.
    let current_device = user_state.get_device().await?;
    let user = user_state.get_user().await?;
    let decrypted_resource = update_resource(
        &input.id,
        input.data,
        &user.id,
        &current_device.id,
        &repo_ctx,
        &crypto_utils,
    )
    .await
    .map_err(|e| e.to_string())?;
    let response = ResourceResponse {
        id: decrypted_resource.id,
        data: decrypted_resource.data,
        favourite: decrypted_resource.favourite,
        last_accessed: decrypted_resource.last_accessed,
        folder_id: decrypted_resource.folder_id,
    };
    app_handle
        .emit("resource-update", response)
        .map_err(|e| e.to_string())?;
    Ok(CryptoResponse::UpdateResources)
}

#[tauri::command]
pub async fn handle_get_resource(
    input: GetResource,
    crypto_utils: State<'_, Arc<Mutex<CryptoUtils>>>,
    repo_ctx: State<'_, RepositoryContext>,
    user_state: State<'_, UserState>,
) -> Result<CryptoResponse, String> {
    let user = user_state.get_user().await?;
    let resource = get_resource(&input.resource_id, &repo_ctx, &user.id, &crypto_utils)
        .await
        .map_err(|e| e.to_string())?;
    Ok(CryptoResponse::GetResourceResponse(resource))
}
#[tauri::command]
pub async fn handle_share_resource(
    input: ShareResource,
    user_state: State<'_, UserState>,
    crypto_utils: State<'_, Arc<Mutex<CryptoUtils>>>,
    repo_ctx: State<'_, RepositoryContext>,
) -> Result<CryptoResponse, String> {
    // Get current user and device info
    let user = user_state.get_user().await?;

    share_resource(
        &input.user_id,
        &input.resource_id,
        &user.id,
        &repo_ctx,
        &crypto_utils,
    )
    .await
    .map_err(|e| e.to_string())?;
    Ok(CryptoResponse::Success)
}
