use crate::application::services::{ResourceService, SyncService};
use crate::domains::models::sync_types::ResourceType;
use crate::types::{
    AddResourceInput, CryptoResponse, DeleteResourceInput, GetAllResources, GetResource,
    GetResourceForFolderInput, ResourceResponse, ToggleFavInput, UpdateLastAccessedInput,
    UpdateResources,
};
use log::info;
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn handle_add_resource(
    input: AddResourceInput,
    resource_service: State<'_, Arc<ResourceService>>,
    sync_service: State<'_, Arc<SyncService>>,
) -> Result<CryptoResponse, String> {
    let resource = resource_service
        .add_resource(input.resource_payload, input.resource_type, input.folder_id)
        .await
        .map_err(|e| e.to_string())?;
    sync_service
        .add_resource_to_sync(resource.clone())
        .await
        .map_err(|e| e.to_string())?;
    Ok(CryptoResponse::ResourceCreateted(resource.id))
}

#[tauri::command]
pub async fn handle_get_resources_for_folder(
    input: GetResourceForFolderInput,
    resource_service: State<'_, Arc<ResourceService>>,
) -> Result<CryptoResponse, String> {
    let resources = resource_service
        .get_resources_for_folder(input.folder_id)
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
    resource_service: State<'_, Arc<ResourceService>>,
    sync_service: State<'_, Arc<SyncService>>,
) -> Result<(), String> {
    info!("deleting resource {}", input.resource_id);
    resource_service
        .delete_resource(input.resource_id.clone())
        .await
        .map_err(|e| e.to_string())?;
    // sync_service
    //     .add_soft_deletion_sync_record(input.resource_id, ResourceType::Resource)
    //     .await
    //     .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn toggle_fav(
    input: ToggleFavInput,
    resource_service: State<'_, Arc<ResourceService>>,
) -> Result<CryptoResponse, String> {
    resource_service
        .toggle_fav(input.resource_id)
        .await
        .map_err(|e| e.to_string())?;
    Ok(CryptoResponse::Success)
}
#[tauri::command]
pub async fn update_last_accessed(
    input: UpdateLastAccessedInput,
    resource_service: State<'_, Arc<ResourceService>>,
) -> Result<CryptoResponse, String> {
    resource_service
        .update_last_accessed(input.resource_id)
        .await
        .map_err(|e| e.to_string())?;
    Ok(CryptoResponse::Success)
}

#[tauri::command]
pub async fn get_all_resources(
    resource_service: State<'_, Arc<ResourceService>>,
    input: GetAllResources,
) -> Result<CryptoResponse, String> {
    let resources = resource_service
        .get_all_resources(input.favourite)
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
pub async fn update_resource(
    resource_service: State<'_, Arc<ResourceService>>,
    input: UpdateResources,
) -> Result<CryptoResponse, String> {
    resource_service
        .update_resources(input)
        .await
        .map_err(|e| e.to_string())?;
    Ok(CryptoResponse::UpdateResources)
}

#[tauri::command]
pub async fn get_resource(
    input: GetResource,
    resource_service: State<'_, Arc<ResourceService>>,
) -> Result<CryptoResponse, String> {
    log::info!("input{:?}", input);
    let resource = resource_service
        .get_resource(input.resource_id)
        .await
        .map_err(|e| e.to_string())?;
    Ok(CryptoResponse::GetResourceResponse(resource))
}
