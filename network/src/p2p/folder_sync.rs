//! Folder sync protocol - simple push after share_folder()
//!
//! This module handles folder sync operations.

use crate::p2p::{errors::P2PResult, peer_connection::PeerConnection, resource_sync};
use crypto_utils::CryptoUtils;
use osvauld_core::models::{FolderDataSync, Message, User};
use persistance::database::RepositoryContext;
use services::{get_folder_by_id, get_folder_share_record};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info};

/// Send folder data then all resources
///
/// Entry point called by sync_service.
/// Orchestrates: folder send + all resources send.
pub async fn send_folder_with_resources(
    folder_id: &str,
    recipient_user_id: &str,
    current_user: &User,
    peer_conn: Arc<PeerConnection>,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: Arc<RwLock<CryptoUtils>>,
) -> P2PResult<()> {
    // 1. Get owner's folder to extract UCAN
    let owner_folder = get_folder_by_id(folder_id, repo_ctx.clone())
        .await
        .map_err(|e| {
            error!("Failed to get folder {}: {}", folder_id, e);
            crate::p2p::errors::P2PError::InvalidState(format!("Failed to get folder: {}", e))
        })?;
    let owner_folder_ucan = owner_folder.ucan.clone();

    // 2. Send folder first
    send_folder_data(
        folder_id,
        recipient_user_id,
        peer_conn.clone(),
        repo_ctx.clone(),
    )
    .await?;

    // 3. Send all resources with owner's folder UCAN (delegate to resource_sync)
    resource_sync::send_all_resources_for_folder(
        folder_id,
        recipient_user_id,
        current_user,
        owner_folder_ucan,
        peer_conn,
        repo_ctx,
        crypto_utils,
    )
    .await
}

/// Send just folder data
async fn send_folder_data(
    folder_id: &str,
    recipient_user_id: &str,
    peer_conn: Arc<PeerConnection>,
    repo_ctx: Arc<RepositoryContext>,
) -> P2PResult<()> {
    info!("Sending folder {} to node", folder_id);

    // Get folder by ID
    let mut folder = get_folder_by_id(folder_id, repo_ctx.clone())
        .await
        .map_err(|e| {
            error!("Failed to get folder {}: {}", folder_id, e);
            crate::p2p::errors::P2PError::InvalidState(format!("Failed to get folder: {}", e))
        })?;

    // Get share record
    let folder_share_record = get_folder_share_record(folder_id, recipient_user_id, repo_ctx)
        .await
        .map_err(|e| {
            error!("Failed to get folder share record: {}", e);
            crate::p2p::errors::P2PError::InvalidState(format!(
                "Failed to get folder share record: {}",
                e
            ))
        })?;

    // Update folder's UCAN with the recipient's folder UCAN token
    folder.ucan = folder_share_record.ucan_token.clone();

    // Send message
    let data = FolderDataSync {
        folder,
        folder_share_record,
    };

    peer_conn.send_message(Message::FolderDataSync(data)).await
}

/// Handle folder data sync from owner (node side)
///
/// Orchestrates folder acceptance by delegating to folder_service
pub async fn handle_folder_data_sync(
    payload: FolderDataSync,
    peer_conn: Arc<PeerConnection>,
    repo_ctx: Arc<RepositoryContext>,
    _crypto_utils: Arc<RwLock<CryptoUtils>>,
) -> P2PResult<()> {
    info!("📥 Received folder {} from peer", payload.folder.id);

    // Get the peer user to access their connection token
    let peer_user_guard = peer_conn.user.read().await;
    let peer_user_id = &peer_user_guard.id;
    let peer_connection_token = &peer_user_guard.ucan_token;
    let domain = &peer_conn.domain;
    services::accept_folder_from_peer(
        &payload.folder,
        &payload.folder_share_record,
        peer_connection_token,
        domain,
        repo_ctx,
    )
    .await
    .map_err(|e| {
        error!("❌ Failed to accept folder from peer: {}", e);
        error!("   Folder ID: {}", payload.folder.id);
        error!("   Peer user ID: {}", peer_user_id);
        crate::p2p::errors::P2PError::InvalidState(format!("Failed to accept folder: {}", e))
    })?;

    info!("✅ Accepted and saved folder {}", payload.folder.id);
    Ok(())
}
