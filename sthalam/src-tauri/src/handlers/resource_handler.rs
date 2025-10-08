use crate::preview_generator::generate_preview_html;
use crate::types::{
    AddResourceInput, CryptoResponse, DeleteResourceInput, GetResource, GetResourceForFolderInput,
    ResourcePreview, ResourceResponse, ShareResource, ToggleFavInput, UpdateLastAccessedInput,
    UpdateResources,
};
use crate::user_state::UserState;
use crypto_utils::CryptoUtils;
use log::error;
use log::info;
use network::P2PService;
use persistance::database::RepositoryContext;
use search_indexer::SearchIndexManager;
use services::{
    create_resource, delete_resource, get_all_resources, get_resource_by_id_direct,
    get_resources_for_folder, share_resource, toggle_fav, update_last_accessed, update_resource,
};
use std::sync::Arc;
use std::time::Instant;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::{Mutex, RwLock};
#[tauri::command]
pub async fn handle_add_resource(
    input: AddResourceInput,
    user_state: State<'_, UserState>,
    app_handle: AppHandle,
    crypto_utils: State<'_, Arc<RwLock<CryptoUtils>>>,
    repo_ctx: State<'_, Arc<RepositoryContext>>,
    p2p_service: State<'_, Arc<P2PService>>,
) -> Result<CryptoResponse, String> {
    let user = user_state.get_user().await?;
    let device = user_state.get_device().await?;
    let resource_added = create_resource(
        input.resource_payload,
        input.resource_type,
        input.folder_id.clone(),
        &user,
        &device.id,
        &"sthalam".to_string(),
        repo_ctx.inner().clone(),
        &crypto_utils,
    )
    .await
    .map_err(|e| {
        error!("failed to decrypt resource {:?}", e);
        e.to_string()
    })?;

    let (preview, title) = match generate_preview_html(&resource_added.data, 3).await {
        Ok((preview, title)) => (preview, title),
        Err(e) => {
            eprintln!(
                "Failed to generate preview for resource {}: {}",
                resource_added.id, e
            );
            (String::new(), String::new()) // Use empty string as fallback
        }
    };

    let resource_preview = ResourcePreview {
        id: resource_added.id.clone(),
        title,
        preview,
        folder_id: resource_added.folder_id.clone(),
        favourite: resource_added.favourite,
        last_accessed: resource_added.last_accessed,
        last_modified: resource_added.last_accessed,
    };
    let response = ResourceResponse {
        id: resource_added.id.clone(),
        data: resource_added.data,
        favourite: resource_added.favourite,
        last_accessed: resource_added.last_accessed,
        folder_id: resource_added.folder_id.clone(),
    };

    app_handle
        .emit("resource-added", resource_preview)
        .map_err(|e| e.to_string())?;

    // Spawn background task to share with folder users and sync over P2P
    let resource_id = resource_added.id.clone();
    let folder_id = input.folder_id.clone();
    let user_clone = user.clone();
    let repo_ctx_clone = repo_ctx.inner().clone();
    let crypto_utils_clone = crypto_utils.inner().clone();
    let p2p_service_clone = p2p_service.inner().clone();
    
    tokio::spawn(async move {
        // First, auto-share the resource with all users who have access to the folder
        match services::auto_share_resource_with_folder_users(
            &resource_id,
            &folder_id,
            &user_clone,
            repo_ctx_clone,
            &crypto_utils_clone,
            "sthalam",
        )
        .await
        {
            Ok(_) => {
                info!("Successfully auto-shared resource {} with folder users", resource_id);
                
                // After successful sharing, sync over P2P
                if let Err(e) = p2p_service_clone.sync_resource(&resource_id).await {
                    error!("Failed to sync resource {} over P2P: {}", resource_id, e);
                } else {
                    info!("Successfully synced resource {} over P2P", resource_id);
                }
            }
            Err(e) => {
                error!("Failed to auto-share resource {} with folder users: {}", resource_id, e);
            }
        }
    });

    Ok(CryptoResponse::SelectedResourceResponse(response))
}

#[tauri::command]
pub async fn handle_get_resources_for_folder(
    input: GetResourceForFolderInput,
    crypto_utils: State<'_, Arc<RwLock<CryptoUtils>>>,
    repo_ctx: State<'_, Arc<RepositoryContext>>,
    user_state: State<'_, UserState>,
) -> Result<CryptoResponse, String> {
    let user = user_state.get_user().await?;
    let resources = get_resources_for_folder(
        &input.folder_id,
        &crypto_utils,
        &user.id,
        repo_ctx.inner().clone(),
    )
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
    repo_ctx: State<'_, Arc<RepositoryContext>>,
) -> Result<(), String> {
    info!("deleting resource {}", input.resource_id);
    delete_resource(input.resource_id.clone(), repo_ctx.inner().clone())
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn handle_toggle_fav(
    input: ToggleFavInput,
    repo_ctx: State<'_, Arc<RepositoryContext>>,
) -> Result<CryptoResponse, String> {
    toggle_fav(input.resource_id, repo_ctx.inner().clone())
        .await
        .map_err(|e| e.to_string())?;
    Ok(CryptoResponse::Success)
}
#[tauri::command]
pub async fn handle_update_last_accessed(
    input: UpdateLastAccessedInput,
    repo_ctx: State<'_, Arc<RepositoryContext>>,
) -> Result<CryptoResponse, String> {
    update_last_accessed(input.resource_id, repo_ctx.inner().clone())
        .await
        .map_err(|e| e.to_string())?;
    Ok(CryptoResponse::Success)
}

#[tauri::command]
pub async fn handle_get_all_resources(
    crypto_utils: State<'_, Arc<RwLock<CryptoUtils>>>,
    repo_ctx: State<'_, Arc<RepositoryContext>>,
    user_state: State<'_, UserState>,
) -> Result<CryptoResponse, String> {
    let user = user_state.get_user().await?;
    let resources = get_all_resources(&crypto_utils, repo_ctx.inner().clone(), &user.id)
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
    crypto_utils: State<'_, Arc<RwLock<CryptoUtils>>>,
    repo_ctx: State<'_, Arc<RepositoryContext>>,
    search_manager: State<'_, Arc<Mutex<SearchIndexManager>>>,
) -> Result<CryptoResponse, String> {
    let current_device = user_state.get_device().await?;
    let user = user_state.get_user().await?;
    let decrypted_resource = update_resource(
        &input.id,
        input.data,
        &user.id,
        &current_device.id,
        repo_ctx.inner().clone(),
        &crypto_utils,
    )
    .await
    .map_err(|e| e.to_string())?;

    let (preview, title) = match generate_preview_html(&decrypted_resource.data, 3).await {
        Ok((preview, title)) => (preview, title),
        Err(e) => {
            eprintln!(
                "Failed to generate preview for resource {}: {}",
                decrypted_resource.id, e
            );
            (String::new(), String::new()) // Use empty string as fallback
        }
    };

    let resource_preview = ResourcePreview {
        id: decrypted_resource.id.clone(),
        title,
        preview,
        folder_id: decrypted_resource.folder_id.clone(),
        favourite: decrypted_resource.favourite,
        last_accessed: decrypted_resource.last_accessed,
        last_modified: decrypted_resource.last_accessed,
    };

    app_handle
        .emit("resource-update", resource_preview)
        .map_err(|e| e.to_string())?;
    let search_manager_clone = search_manager.inner().clone();
    let resource_id = decrypted_resource.id.clone();
    let resource_data = decrypted_resource.data.clone();
    let folder_id = decrypted_resource.folder_id.clone();

    tokio::spawn(async move {
        info!(
            "Spawning background task to update search index for resource {}",
            resource_id
        );
        if let Err(e) = search_manager_clone
            .lock()
            .await
            .update_resource(&resource_id, &resource_data, &folder_id)
            .await
        {
            error!(
                "Failed to update search index for resource {}: {}",
                resource_id, e
            );
        } else {
            info!(
                "Successfully updated search index for resource {}",
                resource_id
            );
        }
    });
    Ok(CryptoResponse::UpdateResources)
}
#[derive(serde::Deserialize)]
pub struct SearchResourcesInput {
    pub query: String,
    pub limit: Option<usize>,
}

#[tauri::command]
pub async fn handle_search_resources(
    input: SearchResourcesInput,
    search_manager: State<'_, Arc<Mutex<SearchIndexManager>>>,
) -> Result<CryptoResponse, String> {
    let limit = input.limit.unwrap_or(50); // Default to 50 results

    info!(
        "Searching for query: '{}' with limit: {}",
        input.query, limit
    );

    // Perform the search
    let search_results = search_manager
        .lock()
        .await
        .search(&input.query, limit)
        .await
        .map_err(|e| {
            error!("Search failed: {}", e);
            e.to_string()
        })?;

    info!("Found {} search results", search_results.len());

    // Extract resource IDs from search results
    let resource_ids: Vec<String> = search_results
        .into_iter()
        .map(|result| result.resource_id)
        .collect();

    Ok(CryptoResponse::SearchedResourceIds(resource_ids))
}
#[tauri::command]
pub async fn handle_get_resource(
    input: GetResource,
    crypto_utils: State<'_, Arc<RwLock<CryptoUtils>>>,
    repo_ctx: State<'_, Arc<RepositoryContext>>,
    user_state: State<'_, UserState>,
) -> Result<CryptoResponse, String> {
    let start = Instant::now();
    let user = user_state.get_user().await?;

    let decrypt_start = Instant::now();
    let resource = get_resource_by_id_direct(
        &input.resource_id,
        &user.id,
        repo_ctx.inner().clone(),
        &crypto_utils,
    )
    .await
    .map_err(|e| e.to_string())?;
    info!("Decryption took: {:?}", decrypt_start.elapsed());

    let json_parse_start = Instant::now();
    let response = ResourceResponse {
        id: resource.id,
        data: resource.data,
        favourite: resource.favourite,
        last_accessed: resource.last_accessed,
        folder_id: resource.folder_id,
    };
    info!("JSON parsing took: {:?}", json_parse_start.elapsed());
    info!("Total time: {:?}", start.elapsed());

    Ok(CryptoResponse::SelectedResourceResponse(response))
}
#[tauri::command]
pub async fn handle_share_resource(
    input: ShareResource,
    user_state: State<'_, UserState>,
    crypto_utils: State<'_, Arc<RwLock<CryptoUtils>>>,
    repo_ctx: State<'_, Arc<RepositoryContext>>,
    p2p_service: State<'_, Arc<P2PService>>,
) -> Result<CryptoResponse, String> {
    // Get current user and device info
    let user = user_state.get_user().await?;
    share_resource(
        &input.user_id,
        &input.resource_id,
        input.permissions.clone(),
        &user,
        repo_ctx.inner().clone(),
        &crypto_utils,
    )
    .await
    .map_err(|e| e.to_string())?;
    p2p_service
        .sync_resource(&input.resource_id)
        .await
        .map_err(|e| e.to_string())?;
    Ok(CryptoResponse::Success)
}

#[tauri::command]
pub async fn emit_all_resources(
    selected_resource_id: Option<String>,
    user_state: State<'_, UserState>,
    app_handle: AppHandle,
    crypto_utils: State<'_, Arc<RwLock<CryptoUtils>>>,
    repo_ctx: State<'_, Arc<RepositoryContext>>,
) -> Result<CryptoResponse, String> {
    let overall_start = Instant::now();
    info!("Starting emit_all_resources");

    let user = user_state.get_user().await?;
    let user_id = user.id.clone();

    // Get all resource IDs
    let fetch_start = Instant::now();
    let mut all_resource_ids = repo_ctx
        .resource_repo
        .get_all_resource_ids()
        .await
        .map_err(|e| e.to_string())?;
    info!(
        "Fetched {} resource IDs in {:?}",
        all_resource_ids.len(),
        fetch_start.elapsed()
    );

    // Handle selected resource - return it immediately
    let selected_resource_response = if let Some(selected_resource) = selected_resource_id {
        let selected_start = Instant::now();

        // Remove selected resource from the list to avoid duplication
        if let Some(pos) = all_resource_ids
            .iter()
            .position(|id| id == &selected_resource)
        {
            all_resource_ids.remove(pos);
        }

        // Decrypt selected resource and return it
        match get_resource_by_id_direct(
            &selected_resource,
            &user_id,
            repo_ctx.inner().clone(),
            &crypto_utils,
        )
        .await
        {
            Ok(decrypted_resource) => {
                let (preview, title) =
                    match generate_preview_html(&decrypted_resource.data, 3).await {
                        Ok((preview, title)) => (preview, title),
                        Err(e) => {
                            eprintln!(
                                "Failed to generate preview for resource {}: {}",
                                decrypted_resource.id.clone(),
                                e
                            );
                            (String::new(), String::new()) // Use empty string as fallback
                        }
                    };

                info!("title {}", &title);
                let resource_preview = ResourcePreview {
                    id: decrypted_resource.id.clone(),
                    title,
                    preview,
                    favourite: decrypted_resource.favourite,
                    last_accessed: decrypted_resource.last_accessed,
                    folder_id: decrypted_resource.folder_id.clone(),
                    last_modified: decrypted_resource.last_accessed,
                };
                let response = ResourceResponse {
                    id: decrypted_resource.id.clone(),
                    data: decrypted_resource.data,
                    favourite: decrypted_resource.favourite,
                    last_accessed: decrypted_resource.last_accessed,
                    folder_id: decrypted_resource.folder_id,
                };
                let _ = app_handle.emit("resource-added", resource_preview);
                info!(
                    "Selected resource processed in {:?}",
                    selected_start.elapsed()
                );
                Some(response)
            }
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

    let remaining_count = all_resource_ids.len();
    info!(
        "Starting background processing of {} remaining resources",
        remaining_count
    );

    // Spawn background task to emit remaining resources
    let app_handle_clone = app_handle.clone();
    let crypto_utils_clone = crypto_utils.inner().clone();
    let repo_ctx_clone = repo_ctx.inner().clone();

    tokio::spawn(async move {
        let background_start = Instant::now();
        
        // Concurrency limit - adjust based on your system
        const MAX_CONCURRENT_TASKS: usize = 10;
        
        let mut tasks = tokio::task::JoinSet::new();
        let mut processed = 0;
        
        let mut resource_iter = all_resource_ids.into_iter();
        
        // Initial batch of tasks
        for _ in 0..MAX_CONCURRENT_TASKS {
            if let Some(resource_id) = resource_iter.next() {
                let user_id_clone = user_id.clone();
                let repo_ctx_task = repo_ctx_clone.clone();
                let crypto_utils_task = crypto_utils_clone.clone();
                let app_handle_task = app_handle_clone.clone();
                
                tasks.spawn(async move {
                    let resource_start = Instant::now();
                    let get_resource_start = Instant::now();
                    
                    match get_resource_by_id_direct(
                        &resource_id,
                        &user_id_clone,
                        repo_ctx_task,
                        &crypto_utils_task,
                    )
                    .await
                    {
                        Ok(decrypted_resource) => {
                            let get_resource_time = get_resource_start.elapsed();
                            let (preview, title) =
                                match generate_preview_html(&decrypted_resource.data, 3).await {
                                    Ok((preview, title)) => (preview, title),
                                    Err(e) => {
                                        eprintln!(
                                            "Failed to generate preview for resource {}: {}",
                                            decrypted_resource.id, e
                                        );
                                        (String::new(), String::new())
                                    }
                                };

                            info!("title {}", &title);
                            let emit_start = Instant::now();
                            let response = ResourcePreview {
                                id: decrypted_resource.id.clone(),
                                preview,
                                title,
                                favourite: decrypted_resource.favourite,
                                last_accessed: decrypted_resource.last_accessed,
                                folder_id: decrypted_resource.folder_id,
                                last_modified: decrypted_resource.last_accessed,
                            };

                            if let Err(e) = app_handle_task.emit("resource-added", response) {
                                eprintln!("Failed to emit resource-added for {}: {}", resource_id, e);
                            }
                            let emit_time = emit_start.elapsed();
                            
                            Ok((resource_id, resource_start.elapsed(), get_resource_time, emit_time))
                        }
                        Err(e) => {
                            eprintln!("Failed to decrypt resource {}: {}", resource_id, e);
                            Err(resource_id)
                        }
                    }
                });
            }
        }
        
        // Process tasks as they complete and spawn new ones
        while let Some(result) = tasks.join_next().await {
            match result {
                Ok(Ok((_resource_id, resource_time, get_resource_time, emit_time))) => {
                    processed += 1;
                    
                    // Log every 10th resource or if it takes longer than 100ms
                    if processed % 10 == 0 || resource_time.as_millis() > 100 {
                        info!(
                            "Resource {}/{} processed in {:?} (get_resource: {:?}, emit: {:?})",
                            processed, remaining_count, resource_time, get_resource_time, emit_time
                        );
                    }
                }
                Ok(Err(_)) => {
                    processed += 1;
                }
                Err(e) => {
                    eprintln!("Task panicked: {}", e);
                    processed += 1;
                }
            }
            
            // Spawn a new task if there are more resources
            if let Some(resource_id) = resource_iter.next() {
                let user_id_clone = user_id.clone();
                let repo_ctx_task = repo_ctx_clone.clone();
                let crypto_utils_task = crypto_utils_clone.clone();
                let app_handle_task = app_handle_clone.clone();
                
                tasks.spawn(async move {
                    let resource_start = Instant::now();
                    let get_resource_start = Instant::now();
                    
                    match get_resource_by_id_direct(
                        &resource_id,
                        &user_id_clone,
                        repo_ctx_task,
                        &crypto_utils_task,
                    )
                    .await
                    {
                        Ok(decrypted_resource) => {
                            let get_resource_time = get_resource_start.elapsed();
                            let (preview, title) =
                                match generate_preview_html(&decrypted_resource.data, 3).await {
                                    Ok((preview, title)) => (preview, title),
                                    Err(e) => {
                                        eprintln!(
                                            "Failed to generate preview for resource {}: {}",
                                            decrypted_resource.id, e
                                        );
                                        (String::new(), String::new())
                                    }
                                };

                            info!("title {}", &title);
                            let emit_start = Instant::now();
                            let response = ResourcePreview {
                                id: decrypted_resource.id.clone(),
                                preview,
                                title,
                                favourite: decrypted_resource.favourite,
                                last_accessed: decrypted_resource.last_accessed,
                                folder_id: decrypted_resource.folder_id,
                                last_modified: decrypted_resource.last_accessed,
                            };

                            if let Err(e) = app_handle_task.emit("resource-added", response) {
                                eprintln!("Failed to emit resource-added for {}: {}", resource_id, e);
                            }
                            let emit_time = emit_start.elapsed();
                            
                            Ok((resource_id, resource_start.elapsed(), get_resource_time, emit_time))
                        }
                        Err(e) => {
                            eprintln!("Failed to decrypt resource {}: {}", resource_id, e);
                            Err(resource_id)
                        }
                    }
                });
            }
        }

        let total_background_time = background_start.elapsed();
        info!(
            "Background processing completed: {}/{} resources in {:?} (avg: {:?} per resource)",
            processed,
            remaining_count,
            total_background_time,
            total_background_time / processed.max(1) as u32
        );

        // Emit completion event
        if let Err(e) = app_handle_clone.emit("resources-loading-complete", ()) {
            eprintln!("Failed to emit resources-loading-complete: {}", e);
        }
    });

    info!(
        "emit_all_resources completed initial phase in {:?}",
        overall_start.elapsed()
    );

    // Return immediately with selected resource (if any)
    match selected_resource_response {
        Some(resource) => Ok(CryptoResponse::SelectedResourceResponse(resource)),
        None => Ok(CryptoResponse::Success),
    }
}
