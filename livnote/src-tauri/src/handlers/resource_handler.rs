use crate::types::{
    AddResourceInput, CryptoResponse, DeleteResourceInput, GetResource, GetResourceForFolderInput,
    ResourceResponse, ShareResource, ToggleFavInput, UpdateLastAccessedInput, UpdateResources,
};
use crate::user_state::UserState;
use crypto_utils::CryptoUtils;
use log::info;
use osvauld_core::models::{ConnectionAction, ConnectionType};
use osvauld_db::database::RepositoryContext;
use osvauld_services::{
    create_resource, delete_resource, get_all_resources, get_resource, get_resource_by_id_direct,
    get_resources_for_folder, get_shared_user_devices_for_note, share_resource, toggle_fav,
    update_last_accessed, update_resource,
};
use p2p_service::P2PService;
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
        .emit("resource-added", response.clone())
        .map_err(|e| e.to_string())?;

    Ok(CryptoResponse::SelectedResourceResponse(response))
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

    let response = ResourceResponse {
        id: resource.id,
        data: resource.data,
        favourite: resource.favourite,
        last_accessed: resource.last_accessed,
        folder_id: resource.folder_id,
    };
    Ok(CryptoResponse::SelectedResourceResponse(response))
}
#[tauri::command]
pub async fn handle_share_resource(
    input: ShareResource,
    user_state: State<'_, UserState>,
    crypto_utils: State<'_, Arc<Mutex<CryptoUtils>>>,
    repo_ctx: State<'_, RepositoryContext>,
    p2p_service: State<'_, Arc<P2PService>>,
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
    let devices = repo_ctx
        .device_repo
        .get_devices_by_user_id(&input.user_id)
        .await
        .map_err(|e| e.to_string())?;
    for device in devices {
        let p2p_service_clone = p2p_service.inner().clone();
        tokio::spawn(async move {
            if let Err(e) = p2p_service_clone
                .connect_with_ticket(
                    &device.id,
                    ConnectionType::User,
                    Some(ConnectionAction::UserSync),
                )
                .await
            {
                // Log the error or handle it appropriately
                eprintln!("Failed to connect to device {}: {}", &device.id, e);
            }
        });
    }
    Ok(CryptoResponse::Success)
}
#[tauri::command]
pub async fn emit_all_resources(
    selected_resource_id: Option<String>,
    user_state: State<'_, UserState>,
    app_handle: AppHandle,
    crypto_utils: State<'_, Arc<Mutex<CryptoUtils>>>,
    repo_ctx: State<'_, RepositoryContext>,
) -> Result<CryptoResponse, String> {
    let user = user_state.get_user().await?;
    let user_id = user.id.clone();
    // Get all resource IDs
    let mut all_resource_ids = repo_ctx
        .resource_repo
        .get_all_resource_ids()
        .await
        .map_err(|e| e.to_string())?;

    // Handle selected resource - return it immediately
    let selected_resource_response = if let Some(selected_resource) = selected_resource_id {
        // Remove selected resource from the list to avoid duplication
        if let Some(pos) = all_resource_ids
            .iter()
            .position(|id| id == &selected_resource)
        {
            all_resource_ids.remove(pos);
        }

        // Decrypt selected resource and return it
        match get_resource_by_id_direct(&selected_resource, &user_id, &repo_ctx, &crypto_utils)
            .await
        {
            Ok(decrypted_resource) => Some(ResourceResponse {
                id: decrypted_resource.id,
                data: decrypted_resource.data,
                favourite: decrypted_resource.favourite,
                last_accessed: decrypted_resource.last_accessed,
                folder_id: decrypted_resource.folder_id,
            }),
            Err(e) => {
                eprintln!(
                    "Failed to decrypt selected resource {}: {}",
                    selected_resource, e
                );
                None
            }
        }
    } else {
        None
    };

    // Spawn background task to emit remaining resources
    let app_handle_clone = app_handle.clone();
    let crypto_utils_clone = crypto_utils.inner().clone();
    let repo_ctx_clone = repo_ctx.inner().clone();

    tokio::spawn(async move {
        for resource_id in all_resource_ids {
            match get_resource_by_id_direct(
                &resource_id,
                &user_id,
                &repo_ctx_clone,
                &crypto_utils_clone,
            )
            .await
            {
                Ok(decrypted_resource) => {
                    let response = ResourceResponse {
                        id: decrypted_resource.id,
                        data: decrypted_resource.data,
                        favourite: decrypted_resource.favourite,
                        last_accessed: decrypted_resource.last_accessed,
                        folder_id: decrypted_resource.folder_id,
                    };

                    if let Err(e) = app_handle_clone.emit("resource-added", response) {
                        eprintln!("Failed to emit resource-added for {}: {}", resource_id, e);
                    }
                }
                Err(e) => {
                    eprintln!("Failed to decrypt resource {}: {}", resource_id, e);
                    // Continue with next resource instead of stopping
                }
            }
        }

        // Emit completion event
        if let Err(e) = app_handle_clone.emit("resources-loading-complete", ()) {
            eprintln!("Failed to emit resources-loading-complete: {}", e);
        }
    });

    // Return immediately with selected resource (if any)
    match selected_resource_response {
        Some(resource) => Ok(CryptoResponse::SelectedResourceResponse(resource)),
        None => Ok(CryptoResponse::Success),
    }
}
