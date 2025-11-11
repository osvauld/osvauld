//! Website Viewer Connection Handler
//!
//! Central router for all Website message types.
//! Delegates to specific handlers based on message variant.

use crate::p2p::{errors::P2PResult, PeerConnection};
use osvauld_core::models::{WebsiteMessage, WebsiteRequest};
use std::sync::Arc;
use tracing::{error, info};

/// Initiate website connection request from viewer to node
///
/// Called by sync_handler after P2P connection is established.
/// Validates folder existence and first_sync status before sending WebsiteRequest.
///
/// # Arguments
/// * `peer_conn` - The established peer connection with the node
/// * `ucan_token` - UCAN token for folder access (from connection string)
/// * `device_id` - Node's device ID (to look up node user)
/// * `p2p_service` - P2P service for accessing viewer user/device and repo
///
/// # Returns
/// * `Ok(())` - Request sent successfully
/// * `Err` - If validation fails or failed to send request
pub async fn initiate_website_request(
    peer_conn: Arc<PeerConnection>,
    ucan_token: String,
    device_id: String,
    p2p_service: Arc<crate::p2p::P2PService>,
) -> P2PResult<()> {
    info!("🌐 Initiating WebsiteRequest - validating connection");

    // 1. Get viewer's local user and device from P2PService
    let viewer_user = p2p_service
        .current_user
        .read()
        .await
        .clone()
        .ok_or_else(|| {
            crate::p2p::errors::P2PError::InvalidState("No viewer user found".to_string())
        })?;

    let viewer_device = p2p_service
        .current_device
        .read()
        .await
        .clone()
        .ok_or_else(|| {
            crate::p2p::errors::P2PError::InvalidState("No viewer device found".to_string())
        })?;

    info!(
        "Viewer: {} (device: {}, first_sync: {})",
        viewer_user.username, viewer_device.id, viewer_user.first_sync
    );

    // 2. Get node device to find user_id
    let node_device = p2p_service
        .repo_ctx
        .device_repo
        .find_by_id(&device_id)
        .await
        .map_err(|e| {
            crate::p2p::errors::P2PError::InvalidState(format!("Failed to get node device: {}", e))
        })?;

    let node_user_id = &node_device.user_id;

    // 3. Validate folder existence and first_sync status via website_service
    let domain = p2p_service.domain.as_str();
    let (first_sync_done, folder_exists) = services::check_user_and_folder_status(
        &ucan_token,
        node_user_id,
        domain,
        p2p_service.repo_ctx.clone(),
    )
    .await
    .map_err(|e| {
        error!("❌ Failed to validate folder/user status: {}", e);
        crate::p2p::errors::P2PError::InvalidState(format!("Validation failed: {}", e))
    })?;

    info!(
        "✓ Validation complete: first_sync={}, folder_exists={}",
        first_sync_done, folder_exists
    );

    // 4. TODO: Handle different flows based on status
    // - If !folder_exists: Folder not found - error or different flow?

    // 5. Create and send WebsiteRequest
    let request = WebsiteRequest {
        ucan_token,
        viewer_user: viewer_user.clone(),
        viewer_device: viewer_device.clone(),
        first_sync: first_sync_done,
    };

    info!(
        "📤 Sending WebsiteRequest to node (viewer first_sync={})",
        request.first_sync
    );

    peer_conn
        .send_message(osvauld_core::models::Message::Website(
            WebsiteMessage::WebsiteRequest(request),
        ))
        .await?;

    info!("✅ WebsiteRequest sent successfully");
    Ok(())
}

/// Process all Website messages - central routing function
///
/// This function receives all Website message variants and delegates
/// to the appropriate handler based on message type.
///
/// # Arguments
/// * `peer_conn` - The peer connection
/// * `message` - The WebsiteMessage variant to process
///
/// # Returns
/// * `Ok(())` - Message processed successfully
/// * `Err` - If processing fails
pub async fn process_message(
    peer_conn: Arc<PeerConnection>,
    message: WebsiteMessage,
) -> P2PResult<()> {
    match message {
        WebsiteMessage::WebsiteRequest(payload) => {
            process_website_request(peer_conn, payload).await
        }
        WebsiteMessage::UpdateUcan(payload) => process_update_ucan(peer_conn, payload).await,
    }
}

/// Process incoming WebsiteRequest from viewer
///
/// This function is called when a viewer sends a WebsiteRequest message.
/// It validates the viewer's connection by:
/// 1. Getting the node's local user/device from peer connection
/// 2. Extracting repo_ctx and domain from peer connection
/// 3. Calling website_service to check first_sync and folder existence
/// 4. Processing based on the status flags
///
/// # Arguments
/// * `peer_conn` - The peer connection with the viewer
/// * `request` - WebsiteRequest payload containing ucan_token, viewer_user, viewer_device, first_sync
///
/// # Returns
/// * `Ok(())` - Request processed successfully
/// * `Err` - If validation fails or status check fails
async fn process_website_request(
    peer_conn: Arc<PeerConnection>,
    request: WebsiteRequest,
) -> P2PResult<()> {
    info!(
        "Processing WebsiteRequest from viewer: {} (first_sync={})",
        request.viewer_user.username, request.first_sync
    );

    // 1. Get local node user and device from peer connection
    let node_user = peer_conn.get_local_user().await?;
    let node_device = peer_conn.get_local_device().await.ok_or_else(|| {
        crate::p2p::errors::P2PError::InvalidState("No local device found".to_string())
    })?;

    info!(
        "Node user: {}, device: {}",
        node_user.username, node_device.id
    );

    // 2. Get repo_ctx, domain, and crypto_utils from peer connection
    let repo_ctx = peer_conn.repo_ctx.clone();
    let domain = peer_conn.domain.as_str();
    let crypto_utils = peer_conn.crypto_utils.clone();

    // 3. Extract folder_id from viewer's UCAN token
    let folder_id =
        services::ucan_service::extract_folder_id_from_viewer_token(&request.ucan_token, domain)
            .await
            .map_err(|e| {
                crate::p2p::errors::P2PError::InvalidState(format!(
                    "Failed to extract folder_id: {}",
                    e
                ))
            })?;

    info!("Extracted folder_id: {}", folder_id);

    // 4. Extract role from viewer's UCAN token
    let viewer_role = services::ucan_service::get_role(&request.ucan_token)
        .await
        .map_err(|e| {
            error!("❌ Failed to extract role from viewer token: {}", e);
            crate::p2p::errors::P2PError::InvalidState(format!("Failed to extract role: {}", e))
        })?;

    info!("Extracted viewer role from token: {}", viewer_role);

    // 5. If first_sync = false, generate new viewer connection token
    if !request.first_sync {
        info!("First sync = false, generating new viewer connection token");

        let (new_ucan_token, new_ucan_cid) = services::ucan_service::create_viewer_connect_token(
            &request.viewer_user.ucan_pub_key,
            &folder_id,
            domain,
            crypto_utils.clone(),
            &repo_ctx,
        )
        .await
        .map_err(|e| {
            crate::p2p::errors::P2PError::InvalidState(format!(
                "Failed to create viewer connect token: {}",
                e
            ))
        })?;

        info!("Generated new viewer connect token (CID: {})", new_ucan_cid);

        // 6. Send UpdateUcan message to viewer
        let update_msg = osvauld_core::models::UpdateUcanMessage {
            new_ucan_token,
            new_ucan_cid,
        };

        peer_conn
            .send_message(osvauld_core::models::Message::Website(
                osvauld_core::models::WebsiteMessage::UpdateUcan(update_msg),
            ))
            .await?;

        info!("✓ Sent UpdateUcan message to viewer");
    }

    // 7. Get folder prepared for viewer
    let (folder, folder_share_record) = services::get_folder_to_send(
        &folder_id,
        &node_user.id,
        &viewer_role,
        &request.viewer_user.ucan_pub_key,
        domain,
        repo_ctx.clone(),
        &crypto_utils,
    )
    .await?;

    info!("✓ Prepared folder for viewer");

    // 8. Send FolderDataSync message to viewer
    peer_conn
        .send_message(osvauld_core::models::Message::Folder(
            osvauld_core::models::FolderMessage::FolderDataSync(
                osvauld_core::models::FolderDataSync {
                    folder,
                    folder_share_record,
                },
            ),
        ))
        .await?;

    info!("✅ Sent FolderDataSync to viewer");

    // 9. Get folder again to extract owner_folder_ucan (node's folder UCAN)
    let node_folder = repo_ctx
        .folder_repo
        .find_by_id(&folder_id)
        .await
        .map_err(|e| {
            error!("❌ Failed to get folder: {}", e);
            crate::p2p::errors::P2PError::InvalidState(format!("Failed to get folder: {}", e))
        })?;

    let owner_folder_ucan = node_folder.ucan.clone();
    info!("Retrieved node's folder UCAN for resource validation");

    // 10. Get all resource IDs for this folder
    let resource_ids = repo_ctx
        .resource_repo
        .get_resource_ids_by_folder_id(&folder_id)
        .await
        .map_err(|e| {
            error!("❌ Failed to get resource IDs: {}", e);
            crate::p2p::errors::P2PError::InvalidState(format!("Failed to get resource IDs: {}", e))
        })?;

    info!("Found {} resources in folder", resource_ids.len());

    // 11. Loop through resources and send each one
    for resource_id in &resource_ids {
        // Prepare resource for viewer
        let (viewer_encrypted_resource, share_record) = services::prepare_resource_for_viewer(
            &resource_id,
            &node_user.id,
            &request.viewer_user,
            domain,
            repo_ctx.clone(),
            &crypto_utils,
        )
        .await?;

        // Send ResourceDataSync message
        let resource_data = osvauld_core::models::ResourceDataSync {
            resource: viewer_encrypted_resource,
            share_records: vec![share_record], // Single share record for viewer
            owner_folder_ucan: owner_folder_ucan.clone(),
        };

        peer_conn
            .send_message(osvauld_core::models::Message::Resource(
                osvauld_core::models::ResourceMessage::ResourceDataSync(resource_data),
            ))
            .await?;

        info!("✅ Sent resource {} to viewer", resource_id);
    }

    info!("✅ Sent all {} resources to viewer", resource_ids.len());

    Ok(())
}

/// Process UpdateUcan message from node (received by viewer)
///
/// When viewer connects with first_sync=false, node generates a new
/// viewer-specific connection token and sends it via this message.
/// Viewer updates their stored UCAN token for future connections.
///
/// # Arguments
/// * `peer_conn` - The peer connection with the node
/// * `update_msg` - UpdateUcanMessage containing new token and CID
///
/// # Returns
/// * `Ok(())` - UCAN token updated successfully
/// * `Err` - If update fails
async fn process_update_ucan(
    peer_conn: Arc<PeerConnection>,
    update_msg: osvauld_core::models::UpdateUcanMessage,
) -> P2PResult<()> {
    info!("📥 Processing UpdateUcan message from node");

    // 1. Get viewer's local user from peer connection
    let viewer_user = peer_conn.get_local_user().await?;
    let repo_ctx = peer_conn.repo_ctx.clone();

    info!(
        "Updating UCAN token for viewer user: {} (CID: {})",
        viewer_user.username, update_msg.new_ucan_cid
    );

    // 2. Update viewer's UCAN token via user_service
    services::update_ucan(
        &viewer_user.id,
        update_msg.new_ucan_token,
        update_msg.new_ucan_cid,
        repo_ctx,
    )
    .await
    .map_err(|e| {
        crate::p2p::errors::P2PError::InvalidState(format!("Failed to update viewer UCAN: {}", e))
    })?;

    info!("✅ Viewer UCAN token updated successfully");
    Ok(())
}
