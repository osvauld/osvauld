use crate::types::{
    AddResourceInput, CryptoResponse, DeleteResourceInput, GetAllResources, GetResource,
    GetResourceForFolderInput, ResourceResponse, ShareResource, ToggleFavInput,
    UpdateLastAccessedInput, UpdateResources,
};
use crate::user_state::UserState;
use log::info;
use osvauld_services::{ResourceService, ShareService, SyncService, TransactionService};
use rendezvous_client::rendezvous_service::RendezvousService;
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn handle_add_resource(
    input: AddResourceInput,
    resource_service: State<'_, Arc<ResourceService>>,
    sync_service: State<'_, Arc<SyncService>>,
    share_service: State<'_, Arc<ShareService>>,
    transaction_service: State<'_, Arc<TransactionService>>,
    user_state: State<'_, UserState>,
) -> Result<CryptoResponse, String> {
    let user = user_state.get_user().await?;
    let device = user_state.get_device().await?;
    let (resource, resource_key) = resource_service
        .add_resource(
            input.resource_payload,
            input.resource_type,
            input.folder_id,
            &user,
        )
        .await
        .map_err(|e| e.to_string())?;
    let (sync_record_set, vector_clocks) = sync_service
        .prepare_resource_to_sync(&resource, &user.id, &device)
        .await
        .map_err(|e| e.to_string())?;
    let share_record_set = share_service
        .prepare_owner_share_record(resource.id.clone())
        .await
        .map_err(|e| e.to_string())?;
    let _ = transaction_service
        .create_resource_with_sync(
            resource.clone(),
            resource_key,
            &sync_record_set,
            share_record_set,
            &vector_clocks,
        )
        .await
        .map_err(|e| e.to_string());

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
    transaction_service: State<'_, Arc<TransactionService>>,
    input: UpdateResources,
    user_state: State<'_, UserState>,
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
    share_service: State<'_, Arc<ShareService>>,
    transaction_service: State<'_, Arc<TransactionService>>,
    rendezvous_service: State<'_, Arc<RendezvousService>>,
) -> Result<CryptoResponse, String> {
    //TODO: change from public key to user_id?
    // let (resource_key, vector_clock) = resource_service
    //     .share_resource(input.resource_id.clone(), input.public_key.clone())
    //     .await
    //     .map_err(|e| e.to_string())?;
    // //TODO: add sync record for share
    // let share_service_set = share_service
    //     .prepare_share_records(input.resource_id.clone(), input.public_key.clone())
    //     .await
    //     .map_err(|e| e.to_string())?;
    // transaction_service
    //     .share_resource(
    //         resource_key,
    //         vector_clock,
    //         share_service_set,
    //         input.resource_id.clone(),
    //     )
    //     .await
    //     .map_err(|e| e.to_string())?;
    // let shared_by_user_id = get_key_id(&input.public_key).map_err(|e| e.to_string())?;
    // match rendezvous_service
    //     .get_connection_status(vec![shared_by_user_id.clone()])
    //     .await
    // {
    //     Ok(status_list) => {
    //         if let Some(status) = status_list.into_iter().next() {
    //             // Log the status but continue regardless
    //             log::info!(
    //                 "User {} connection status: {}",
    //                 shared_by_user_id,
    //                 status.connection_status
    //             );
    //         } else {
    //             log::warn!("No connection status found for user {}", shared_by_user_id);
    //         }
    //     }
    //     Err(e) => {
    //         // Log the error but continue anyway
    //         log::warn!("Failed to get connection status: {}", e);
    //     }
    // }
    //
    Ok(CryptoResponse::Success)
}
