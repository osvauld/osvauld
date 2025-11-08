//! P2P Synchronization Service
//!
//! Lightweight orchestration layer for folder sync after share_folder().
//! Gets connection and delegates to folder_sync for heavy lifting.

use crate::p2p::{errors::P2PResult, folder_sync, P2PService};
use crypto_utils::CryptoUtils;
use osvauld_core::models::User;
use persistance::database::RepositoryContext;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::error;

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
    folder_sync::send_folder_with_resources(
        &folder_id,
        &recipient_user_id,
        &current_user,
        peer_conn,
        repo_ctx,
        crypto_utils,
    )
    .await
    .map_err(|e| e.into())
}
