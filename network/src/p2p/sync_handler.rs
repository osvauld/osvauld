//! P2P Synchronization Service
//!
//! Lightweight orchestration layer for folder sync and resource sync.
//! Gets connection and delegates to appropriate sync modules.

use crate::p2p::{errors::P2PResult, P2PService};
use crypto_utils::CryptoUtils;
use osvauld_core::models::{FolderTokenRequest, Message, User};
use persistance::database::RepositoryContext;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info};

/// Send folder and all its resources to a node
///
/// Lightweight public API - just orchestrates connection setup,
/// then delegates to folder_sync for actual work.
///
/// Spawns async task (fire-and-forget). Errors logged.
pub async fn send_folder(
    folder_id: String,
    recipient_user_id: String,
    current_user: User,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: Arc<RwLock<CryptoUtils>>,
    p2p_service: Arc<P2PService>,
) -> P2PResult<()> {
    // Spawn async task
    tokio::spawn(async move {
        if let Err(e) = send_folder_impl(
            folder_id,
            recipient_user_id,
            current_user,
            repo_ctx,
            crypto_utils,
            p2p_service,
        )
        .await
        {
            error!("Folder sync failed: {}", e);
        }
    });

    Ok(())
}

/// Internal implementation
async fn send_folder_impl(
    folder_id: String,
    recipient_user_id: String,
    current_user: User,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: Arc<RwLock<CryptoUtils>>,
    p2p_service: Arc<P2PService>,
) -> P2PResult<()> {
    // 1. Get recipient's devices
    let devices = repo_ctx
        .device_repo
        .get_devices_by_user_id(&recipient_user_id)
        .await?;

    if devices.is_empty() {
        error!("No devices found for recipient {}", recipient_user_id);
        return Ok(());
    }

    let device = &devices[0];

    // 2. Get or establish peer connection
    let peer_conn = match p2p_service.get_connection_by_id(&device.id).await {
        Ok(conn) => conn,
        Err(_) => {
            // No active connection, try to establish one
            error!(
                "No active connection to device {} for user {}, attempting to connect",
                device.id, recipient_user_id
            );

            match p2p_service.connect_with_ticket(&device.id).await? {
                Some(conn) => conn,
                None => {
                    error!(
                        "Failed to establish connection to device {} for user {}",
                        device.id, recipient_user_id
                    );
                    return Ok(());
                }
            }
        }
    };

    // 3. Delegate to folder_sync (does all the heavy lifting)
    crate::p2p::folder_sync::send_folder_with_resources(
        &folder_id,
        &recipient_user_id,
        &current_user,
        peer_conn,
        repo_ctx,
        crypto_utils,
    )
    .await
}

/// Sync a resource with peers who have access
///
/// Gets all users with access to resource, then syncs with each by:
/// 1. Getting their devices
/// 2. Establishing connection if needed
/// 3. Sending ResourceSyncRequest with UCANs
///
/// The peer will respond with either:
/// - ResourceNotFoundRequest (if they don't have it) → We send full resource
/// - StateVectorRequest (if they have it) → Start CRDT merge sync
///
/// # Arguments
/// * `resource_id` - ID of the resource to sync
/// * `repo_ctx` - Database repository context
/// * `crypto_utils` - Crypto utilities
/// * `p2p_service` - P2P service for connections and current user
///
/// # Returns
/// * `Ok(())` - Sync request sent successfully
/// * `Err` - If resource not found or failed to get UCANs
pub async fn sync_resource(
    resource_id: String,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: Arc<RwLock<CryptoUtils>>,
    p2p_service: Arc<P2PService>,
) -> P2PResult<()> {
    info!("Initiating resource sync for: {}", resource_id);

    // Get current user from P2PService
    let user_guard = p2p_service.current_user.read().await;
    let current_user = user_guard
        .as_ref()
        .ok_or_else(|| crate::p2p::errors::P2PError::InvalidState("No user logged in".to_string()))?
        .clone();
    drop(user_guard);

    // 1. Get all share records for this resource to find users with access
    let share_records =
        services::get_all_share_records_for_resource(&resource_id, repo_ctx.clone())
            .await
            .map_err(|e| {
                error!(
                    "Failed to get share records for resource {}: {}",
                    resource_id, e
                );
                crate::p2p::errors::P2PError::InvalidState(format!(
                    "Failed to get share records: {}",
                    e
                ))
            })?;

    if share_records.is_empty() {
        error!("No share records found for resource {}", resource_id);
        return Ok(());
    }

    info!(
        "Found {} users with access to resource",
        share_records.len()
    );

    // 2. For each user with access, get their devices and sync
    for share_record in share_records {
        // Skip syncing with ourselves
        if share_record.recipient_user_id == current_user.id {
            continue;
        }

        info!("Syncing with user: {}", share_record.recipient_user_id);

        // Get recipient's devices
        let devices = repo_ctx
            .device_repo
            .get_devices_by_user_id(&share_record.recipient_user_id)
            .await
            .map_err(|e| crate::p2p::errors::P2PError::InvalidState(e.to_string()))?;

        if devices.is_empty() {
            error!("No devices found for user {}", share_record.recipient_user_id);
            continue;
        }

        let device = &devices[0];

        // 4. Get or establish peer connection
        let peer_conn = match p2p_service.get_connection_by_id(&device.id).await {
            Ok(conn) => conn,
            Err(_) => {
                // No active connection, try to establish one
                info!(
                    "No active connection to device {} for user {}, attempting to connect",
                    device.id, share_record.recipient_user_id
                );

                match p2p_service.connect_with_ticket(&device.id).await? {
                    Some(conn) => conn,
                    None => {
                        error!(
                            "Failed to establish connection to device {} for user {}",
                            device.id, share_record.recipient_user_id
                        );
                        continue;
                    }
                }
            }
        };

        // 5. Call resource_sync to initiate sync
        info!("Calling resource_sync for peer: {}", device.id);

        match crate::p2p::resource_sync::sync_resource(
            &resource_id,
            &share_record.recipient_user_id,
            &current_user,
            peer_conn,
            repo_ctx.clone(),
            crypto_utils.clone(),
        )
        .await
        {
            Ok(_) => {
                info!("✓ Sync initiated with peer: {}", device.id);
            }
            Err(e) => {
                error!("Failed to sync with peer {}: {}", device.id, e);
                continue;
            }
        }
    }

    info!("✓ Resource sync initiated for: {}", resource_id);
    Ok(())
}

/// Sync folder with all recipients (CRDT merge sync)
///
/// Gets all folder_share_records, establishes connections with each recipient,
/// and delegates to folder_sync handler for state vector comparison.
///
/// This is used for:
/// - Owner → Node: Push new resources added to folder
/// - Viewer/Node → Node: Pull missing resources from node
///
/// # Arguments
/// * `folder_id` - ID of the folder to sync
/// * `repo_ctx` - Database repository context
/// * `crypto_utils` - Crypto utilities
/// * `p2p_service` - P2P service for connections and current user
///
/// # Returns
/// * `Ok(())` - Sync request sent successfully
/// * `Err` - If folder not found or no recipients
pub async fn sync_folder(
    folder_id: String,
    repo_ctx: Arc<RepositoryContext>,
    p2p_service: Arc<P2PService>,
) -> P2PResult<()> {
    info!("Initiating folder sync for: {}", folder_id);

    // Get current user from P2PService
    let user_guard = p2p_service.current_user.read().await;
    let current_user = user_guard
        .as_ref()
        .ok_or_else(|| crate::p2p::errors::P2PError::InvalidState("No user logged in".to_string()))?
        .clone();
    drop(user_guard);

    // 1. Get all folder_share_records
    let folder_share_records = services::get_all_folder_share_records(&folder_id, repo_ctx.clone())
        .await
        .map_err(|e| {
            error!(
                "Failed to get folder share records for {}: {}",
                folder_id, e
            );
            crate::p2p::errors::P2PError::InvalidState(format!(
                "Failed to get folder share records: {}",
                e
            ))
        })?;

    if folder_share_records.is_empty() {
        error!("No folder share records found for folder {}", folder_id);
        return Ok(());
    }

    info!("Found {} recipients for folder", folder_share_records.len());

    // 2. For each recipient (except current user), sync folder
    for share_record in folder_share_records {
        // Skip syncing with ourselves
        if share_record.recipient_user_id == current_user.id {
            continue;
        }

        let recipient_user_id = &share_record.recipient_user_id;
        info!("Syncing folder with user: {}", recipient_user_id);

        // Get recipient's devices
        let devices = repo_ctx
            .device_repo
            .get_devices_by_user_id(recipient_user_id)
            .await
            .map_err(|e| crate::p2p::errors::P2PError::InvalidState(e.to_string()))?;

        if devices.is_empty() {
            error!("No devices found for user {}", recipient_user_id);
            continue;
        }

        let device = &devices[0];

        // Get or establish peer connection
        let peer_conn = match p2p_service.get_connection_by_id(&device.id).await {
            Ok(conn) => conn,
            Err(_) => {
                // No active connection, try to establish one
                info!(
                    "No active connection to device {} for user {}, attempting to connect",
                    device.id, recipient_user_id
                );

                match p2p_service.connect_with_ticket(&device.id).await? {
                    Some(conn) => conn,
                    None => {
                        error!(
                            "Failed to establish connection to device {} for user {}",
                            device.id, recipient_user_id
                        );
                        continue;
                    }
                }
            }
        };

        // TODO: Delegate to folder_sync handler
        info!(
            "TODO: Folder sync not yet implemented for peer: {}",
            device.id
        );
        let _ = (&folder_id, &peer_conn);
        // match folder_sync::sync_folder_handler(
        //     &folder_id,
        //     peer_conn,
        //     repo_ctx.clone(),
        //     crypto_utils.clone(),
        // )
        // .await
        // {
        //     Ok(_) => {
        //         info!("✓ Folder sync sent to peer: {}", device.id);
        //     }
        //     Err(e) => {
        //         error!("Failed to sync folder with peer {}: {}", device.id, e);
        //         continue;
        //     }
        // }
    }

    info!("✓ Folder sync initiated for: {}", folder_id);
    Ok(())
}

/// Request folder token from a sovereign node
///
/// Gets/establishes connection with the node and sends FolderTokenRequest
/// with folder_id and folder_ucan. The node will generate a shareable link
/// and send it back via FolderTokenResponse.
///
/// # Arguments
/// * `folder_id` - ID of the folder to get token for
/// * `user_id` - User ID of the sovereign node to request from
/// * `repo_ctx` - Database repository context
/// * `p2p_service` - P2P service for connections
///
/// # Returns
/// * `Ok(())` - Request sent successfully
/// * `Err` - If connection fails or folder not found
pub async fn request_folder_token(
    folder_id: String,
    user_id: String,
    repo_ctx: Arc<RepositoryContext>,
    p2p_service: Arc<P2PService>,
) -> P2PResult<()> {
    info!(
        "Requesting folder token for folder: {} from user: {}",
        folder_id, user_id
    );

    // 1. Validate current user is logged in
    let user_guard = p2p_service.current_user.read().await;
    user_guard
        .as_ref()
        .ok_or_else(|| crate::p2p::errors::P2PError::InvalidState("No user logged in".to_string()))?;
    drop(user_guard);

    // 2. Get folder to extract UCAN
    let folder = services::get_folder_by_id(&folder_id, repo_ctx.clone())
        .await
        .map_err(|e| {
            error!("Failed to get folder {}: {}", folder_id, e);
            crate::p2p::errors::P2PError::InvalidState(format!("Failed to get folder: {}", e))
        })?;

    // 3. Get recipient's devices
    let devices = repo_ctx
        .device_repo
        .get_devices_by_user_id(&user_id)
        .await?;

    if devices.is_empty() {
        error!("No devices found for user {}", user_id);
        return Err(crate::p2p::errors::P2PError::InvalidState(format!(
            "No devices found for user {}",
            user_id
        )));
    }

    let device = &devices[0];

    // 4. Get or establish peer connection
    let peer_conn = match p2p_service.get_connection_by_id(&device.id).await {
        Ok(conn) => conn,
        Err(_) => {
            // No active connection, try to establish one
            info!(
                "No active connection to device {} for user {}, attempting to connect",
                device.id, user_id
            );

            match p2p_service.connect_with_ticket(&device.id).await? {
                Some(conn) => conn,
                None => {
                    error!(
                        "Failed to establish connection to device {} for user {}",
                        device.id, user_id
                    );
                    return Err(crate::p2p::errors::P2PError::InvalidState(format!(
                        "Failed to connect to device {}",
                        device.id
                    )));
                }
            }
        }
    };

    // 5. Create and send FolderTokenRequest
    let request = FolderTokenRequest {
        folder_id: folder.id.clone(),
        folder_permit: folder.ucan.clone(),
    };

    info!(
        "Sending FolderTokenRequest to peer: {} for folder: {}",
        device.id, folder.id
    );

    peer_conn
        .send_message(Message::Folder(
            osvauld_core::models::FolderMessage::FolderTokenRequest(request),
        ))
        .await?;

    info!("✓ Folder token request sent successfully");
    Ok(())
}

/// Request folder resources from a node by folder ID
///
/// This is for nodes/viewers who have database access and want to pull
/// missing resources from another node. Automatically determines the node
/// to sync from by looking up the folder share record.
///
/// # Arguments
/// * `folder_id` - Folder ID to request resources for
/// * `repo_ctx` - Database repository context
/// * `p2p_service` - P2P service for connections
///
/// # Returns
/// * `Ok(())` - Request sent successfully
/// * `Err` - If connection fails or folder not found
pub async fn request_folder_resources(
    folder_id: String,
    repo_ctx: Arc<RepositoryContext>,
    p2p_service: Arc<P2PService>,
) -> P2PResult<()> {
    info!("Requesting folder resources for folder: {}", folder_id);

    // 1. Get current user from P2PService
    let user_guard = p2p_service.current_user.read().await;
    let current_user = user_guard
        .as_ref()
        .ok_or_else(|| crate::p2p::errors::P2PError::InvalidState("No user logged in".to_string()))?
        .clone();
    drop(user_guard);

    // 2. Get all folder shares for this folder
    let all_folder_shares = repo_ctx
        .folder_share_repo
        .get_records_by_folder_id(&folder_id)
        .await
        .map_err(|e| crate::p2p::errors::P2PError::InvalidState(format!("Failed to find folder shares: {}", e)))?;

    // 3. Filter out current user and get unique user IDs
    let other_user_ids: Vec<String> = all_folder_shares
        .iter()
        .map(|share| share.recipient_user_id.clone())
        .filter(|user_id| user_id != &current_user.id)
        .collect::<std::collections::HashSet<_>>() // Remove duplicates
        .into_iter()
        .collect();

    if other_user_ids.is_empty() {
        info!("No other users have access to folder {}", folder_id);
        return Ok(());
    }

    info!("  Found {} other users with access to folder", other_user_ids.len());

    // 4. Request from each user who has access
    for user_id in &other_user_ids {
        info!("  Requesting from user: {}", user_id);

        // Get user's devices
        let devices = match repo_ctx.device_repo.get_devices_by_user_id(user_id).await {
            Ok(devices) => devices,
            Err(e) => {
                error!("Failed to get devices for user {}: {}", user_id, e);
                continue; // Skip this user and try the next one
            }
        };

        if devices.is_empty() {
            error!("No devices found for user {}", user_id);
            continue; // Skip this user
        }

        let device = &devices[0];

        // Get or establish peer connection
        let peer_conn = match p2p_service.get_connection_by_id(&device.id).await {
            Ok(conn) => conn,
            Err(_) => {
                info!(
                    "No active connection to device {} for user {}, attempting to connect",
                    device.id, user_id
                );

                match p2p_service.connect_with_ticket(&device.id).await {
                    Ok(Some(conn)) => conn,
                    Ok(None) => {
                        error!("Failed to establish connection to device {} for user {}", device.id, user_id);
                        continue; // Skip this user
                    }
                    Err(e) => {
                        error!("Error connecting to device {} for user {}: {}", device.id, user_id, e);
                        continue; // Skip this user
                    }
                }
            }
        };

        // Send folder resources request
        if let Err(e) = crate::p2p::folder_sync::send_folder_resources_request_by_id(
            peer_conn,
            folder_id.clone(),
            current_user.id.clone(),
            repo_ctx.clone(),
        )
        .await
        {
            error!("Failed to send folder resources request to user {}: {}", user_id, e);
            // Continue to next user even if this one fails
        } else {
            info!("✓ Folder resources request sent to user: {}", user_id);
        }
    }

    info!("✓ Folder resources requests completed for: {}", folder_id);
    Ok(())
}

/// Connect viewer to website/node
///
/// Fire-and-forget: spawns async task to establish P2P connection
/// and delegate to website_handler to send WebsiteRequest
///
/// # Arguments
/// * `device_id` - Node's device ID
/// * `ucan_token` - UCAN token for folder access (from connection string)
/// * `p2p_service` - P2P service
pub fn connect_to_website(device_id: String, ucan_token: String, p2p_service: Arc<P2PService>) {
    tokio::spawn(async move {
        info!("🌐 Connecting viewer to node device: {}", device_id);

        // Check if connection already exists, otherwise establish new connection
        let peer_conn = match p2p_service.get_connection_by_id(&device_id).await {
            Ok(conn) => {
                info!("✅ Using existing P2P connection");
                conn
            }
            Err(_) => {
                // Establish P2P connection (handshake happens automatically)
                match p2p_service.connect_with_ticket(&device_id).await {
                    Ok(Some(conn)) => {
                        info!("✅ P2P connection established with handshake");
                        conn
                    }
                    Ok(None) => {
                        error!("⚠️ Connection already in progress for device: {}", device_id);
                        return;
                    }
                    Err(e) => {
                        error!("❌ Failed to connect to node: {}", e);
                        return;
                    }
                }
            }
        };

        info!("🔄 Sending FolderResourcesRequest to pull folder data");

        // Send FolderResourcesRequest using ViewerAuth token
        if let Err(e) = crate::p2p::folder_sync::send_folder_resources_request(
            peer_conn,
            ucan_token,
        )
        .await
        {
            error!("❌ Failed to send FolderResourcesRequest: {}", e);
            return;
        }

        info!("✅ FolderResourcesRequest sent - waiting for folder data");
        let _ = (device_id, p2p_service); // Suppress warnings
    });
}
