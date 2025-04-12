use crate::types::{
    AddResourceInput, CryptoResponse, DeleteResourceInput, GetResource, GetResourceForFolderInput,
    ResourceResponse, ShareResource, ToggleFavInput, UpdateLastAccessedInput, UpdateResources,
};
use crate::user_state::UserState;
use log::info;
use osvauld_services::{ResourceService, TransactionService};
use std::sync::Arc;

use tauri::{AppHandle, Emitter, State};
#[tauri::command]
pub async fn handle_add_resource(
    input: AddResourceInput,
    resource_service: State<'_, Arc<ResourceService>>,
    transaction_service: State<'_, Arc<TransactionService>>,
    user_state: State<'_, UserState>,
    app_handle: AppHandle,
) -> Result<CryptoResponse, String> {
    let user = user_state.get_user().await?;
    let device = user_state.get_device().await?;
    let (resource, resource_key, sync_record_set, share_record_set, vector_clocks, share_record) =
        resource_service
            .add_resource(
                input.resource_payload,
                input.resource_type,
                input.folder_id,
                &user,
                &device.id,
            )
            .await
            .map_err(|e| e.to_string())?;
    let _ = transaction_service
        .create_resource_with_sync(
            resource.clone(),
            resource_key,
            &sync_record_set,
            &share_record_set,
            &share_record,
            &vector_clocks,
        )
        .await
        .map_err(|e| e.to_string());
    let resource_added = resource_service
        .get_resource_by_id_direct(&resource.id, &user.id)
        .await
        .map_err(|e| e.to_string())?;
    let response = ResourceResponse {
        id: resource_added.id,
        data: resource_added.data,
        favourite: resource_added.favourite,
        last_accessed: resource_added.last_accessed,
        folder_id: resource_added.folder_id,
    };
    app_handle
        .emit("resource-added", response)
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
    // sync_service: State<'_, Arc<SyncService>>,
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
) -> Result<CryptoResponse, String> {
    let resources = resource_service
        .get_all_resources()
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
    transaction_service: State<'_, Arc<TransactionService>>,
    input: UpdateResources,
    user_state: State<'_, UserState>,
    app_handle: AppHandle,
) -> Result<CryptoResponse, String> {
    //TODO: migrate obsolete user records to another table.
    let current_device = user_state.get_device().await?;
    let (encrypted_data, _current_user) = resource_service
        .update_resources(input.id.clone(), input.data)
        .await
        .map_err(|e| e.to_string())?;
    transaction_service
        .update_resource_with_sync_and_share(&input.id, &encrypted_data, &current_device)
        .await
        .map_err(|e| e.to_string())?;

    let resource_added = resource_service
        .get_resource_by_id_direct(&input.id, &current_device.user_id)
        .await
        .map_err(|e| e.to_string())?;
    let response = ResourceResponse {
        id: resource_added.id,
        data: resource_added.data,
        favourite: resource_added.favourite,
        last_accessed: resource_added.last_accessed,
        folder_id: resource_added.folder_id,
    };
    app_handle
        .emit("resource-update", response)
        .map_err(|e| e.to_string())?;
    Ok(CryptoResponse::UpdateResources)
}

#[tauri::command]
pub async fn get_resource(
    input: GetResource,
    resource_service: State<'_, Arc<ResourceService>>,
) -> Result<CryptoResponse, String> {
    let resource = resource_service
        .get_resource(input.resource_id)
        .await
        .map_err(|e| e.to_string())?;
    Ok(CryptoResponse::GetResourceResponse(resource))
}
#[tauri::command]
pub async fn share_resource(
    input: ShareResource,
    resource_service: State<'_, Arc<ResourceService>>,
    transaction_service: State<'_, Arc<TransactionService>>,
    user_state: State<'_, UserState>,
) -> Result<CryptoResponse, String> {
    // Get current user and device info
    let user = user_state.get_user().await?;
    let device = user_state.get_device().await?;
    let (
        new_resource_key,
        share_record,
        recipient_vector_clocks,
        sync_record_set,
        device_record_set,
        resource_record_set,
    ) = resource_service
        .share_resource(input.user_id, input.resource_id, &user.id, &device.id)
        .await
        .map_err(|e| e.to_string())?;

    transaction_service
        .share_resource_transaction(
            new_resource_key,
            share_record,
            recipient_vector_clocks,
            sync_record_set,
            device_record_set,
            resource_record_set,
        )
        .await
        .map_err(|e| e.to_string())?;

    Ok(CryptoResponse::Success)
}
