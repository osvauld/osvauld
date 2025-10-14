use crate::p2p::{errors::P2PResult, peer_connection::PeerConnection, P2PEvent};
use base64::{engine::general_purpose, Engine as _};
use osvauld_core::models::{p2p::FolderTokenResponse, ConnectionType, Message, WebsiteMessage};
use serde_json::json;
use services::{add_resource_sync, generate_folder_token};
use tracing::{error, info, instrument};

impl PeerConnection {
    /// Handle a folder token request from another peer
    /// This is called on the sovereign node side when a request is received
    #[instrument(skip(self), fields(connection_id = %self.get_id(), folder_id = %folder_id), level = "info")]
    pub async fn handle_folder_token_request(
        &self,
        folder_id: String,
        domain: String,
    ) -> P2PResult<()> {
        info!(
            "Processing folder token request for folder {} from connection {}",
            folder_id,
            self.get_id()
        );

        // Generate the folder token
        match generate_folder_token(
            &folder_id,
            &domain,
            &self.crypto_utils,
            self.repo_ctx.clone(),
        )
        .await
        {
            Ok((ucan_token, ucan_public_key)) => {
                info!(
                    "Successfully generated folder token for folder {}",
                    folder_id
                );

                // Get user and device information
                let user = self.get_local_user().await?;
                let device = self.get_local_device().await.unwrap();

                // Create connection details JSON
                let connection_details = json!({
                    "user_public_key": user.public_key,
                    "device_public_key": device.device_key,
                    "username": user.username,
                    "ucan_token": ucan_token,
                    "ucan_pub_key": ucan_public_key,
                });

                // Convert to string and base64 encode
                let connection_json = connection_details.to_string();
                let connection_string =
                    general_purpose::STANDARD.encode(connection_json.as_bytes());

                info!("Generated connection string for folder {}", folder_id);

                // Create response message
                let response = Message::FolderTokenResponse(FolderTokenResponse {
                    folder_id: folder_id.clone(),
                    connection_string,
                });

                // Send response back to requester
                match self.send_message(response).await {
                    Ok(_) => {
                        info!(
                            "Successfully sent folder token response for folder {}",
                            folder_id
                        );
                        Ok(())
                    }
                    Err(e) => {
                        error!(
                            "Failed to send folder token response for folder {}: {}",
                            folder_id, e
                        );
                        Err(e)
                    }
                }
            }
            Err(e) => {
                error!(
                    "Failed to generate folder token for folder {}: {}",
                    folder_id, e
                );
                // TODO: Send error response back to requester
                Err(e.into())
            }
        }
    }

    /// Handle website-specific messages
    #[instrument(skip(self, message), fields(connection_id = %self.get_id()), level = "info")]
    pub async fn handle_website_message(&self, message: &WebsiteMessage) -> P2PResult<()> {
        match message {
            WebsiteMessage::ResourceRequest { ucan_token } => {
                self.process_website_resource_request(ucan_token.clone())
                    .await
            }
            WebsiteMessage::ResourceResponse {
                resource_id,
                resource_data,
            } => {
                self.process_website_resource_response(resource_id.clone(), resource_data.clone())
                    .await
            }
            WebsiteMessage::InitialSyncComplete {
                folder_id,
                resource_count,
            } => {
                self.process_initial_sync_complete(folder_id.clone(), *resource_count)
                    .await
            }
        }
    }

    /// Process a resource request from a viewer (website connection)
    /// This is called on the sovereign node side
    #[instrument(skip(self, ucan_token), fields(connection_id = %self.get_id()), level = "info")]
    async fn process_website_resource_request(&self, ucan_token: String) -> P2PResult<()> {
        info!("Processing website resource request from viewer");

        // Parse and validate the UCAN token
        let ucan = crypto_utils::ucan_utils::validate_structure(&ucan_token)
            .await
            .map_err(|e| {
                error!("Failed to parse UCAN token: {}", e);
                crate::p2p::errors::ResourceSyncError::InvalidUpdateAuthority {
                    resource_id: "unknown".to_string(),
                }
            })?;

        // Extract folder_id from UCAN capabilities
        let folder_id =
            crypto_utils::ucan_utils::extract_folder_id_from_ucan(&ucan).map_err(|e| {
                error!("Failed to extract folder_id from UCAN: {}", e);
                crate::p2p::errors::ResourceSyncError::InvalidUpdateAuthority {
                    resource_id: "no_folder".to_string(),
                }
            })?;

        info!("Extracted folder_id from UCAN: {}", folder_id);

        // Get local and peer users
        let local_user = self.get_local_user().await?;
        let peer_user = self.get_peer_user().await;

        // 1. Prepare and send folder with folder-specific token
        let folder_data = services::prepare_folder_for_viewer(
            &folder_id,
            &peer_user,
            &local_user,
            self.repo_ctx.clone(),
            &self.crypto_utils,
        )
        .await
        .map_err(|e| {
            error!("Failed to prepare folder for viewer: {}", e);
            crate::p2p::errors::P2PError::Custom(format!("Failed to prepare folder: {}", e))
        })?;

        info!("Sending folder {} to viewer", folder_id);

        self.send_message(Message::FolderSync(
            osvauld_core::models::FolderSyncMessage::UnknownFoldersPayload(
                osvauld_core::models::UnknownFoldersPayload {
                    folder_data: vec![folder_data],
                },
            ),
        ))
        .await?;

        // 2. Get all resource IDs for this folder
        let resource_ids = self
            .repo_ctx
            .resource_repo
            .get_resource_ids_by_folder_id(&folder_id)
            .await
            .map_err(|e| {
                error!("Failed to get resource IDs for folder {}: {}", folder_id, e);
                crate::p2p::errors::P2PError::Custom(format!(
                    "Failed to get resources for folder: {}",
                    e
                ))
            })?;

        let resource_count = resource_ids.len();
        info!(
            "Found {} resources in folder {}",
            resource_count, folder_id
        );

        if resource_ids.is_empty() {
            info!("No resources found for folder {}", folder_id);
            return Ok(());
        }

        // 3. Prepare and send each resource
        for resource_id in resource_ids {
            info!("Preparing resource {} for viewer", resource_id);

            // Get resource to determine its type
            let resource = self
                .repo_ctx
                .resource_repo
                .find_by_id(&resource_id, &local_user.id)
                .await
                .map_err(|e| {
                    error!("Failed to get resource {} details: {}", resource_id, e);
                    crate::p2p::errors::P2PError::Custom(format!(
                        "Failed to get resource details: {}",
                        e
                    ))
                })?;

            let resource_type = resource.resource.resource_type.to_string();
            info!("Resource {} is of type '{}'", resource_id, resource_type);

            let resource_sync_data = services::prepare_resource_for_viewer(
                &resource_id,
                &resource_type,
                &peer_user,
                &local_user,
                self.repo_ctx.clone(),
                &self.crypto_utils,
            )
            .await
            .map_err(|e| {
                error!("Failed to prepare resource {} for viewer: {}", resource_id, e);
                crate::p2p::errors::P2PError::Custom(format!(
                    "Failed to prepare resource: {}",
                    e
                ))
            })?;

            // Send as ResourceAdditionRequest
            self.send_message(Message::ResourceAdditionRequest(resource_sync_data))
                .await?;

            info!("Sent resource {} with resource-specific permissions", resource_id);
        }

        info!(
            "Successfully sent folder and {} resources for folder {}",
            resource_count, folder_id
        );

        // 4. Send InitialSyncComplete message to signal viewer can update first_sync
        self.send_message(Message::Website(
            osvauld_core::models::WebsiteMessage::InitialSyncComplete {
                folder_id: folder_id.clone(),
                resource_count,
            },
        ))
        .await?;

        info!(
            "Sent InitialSyncComplete for folder {} with {} resources",
            folder_id, resource_count
        );

        Ok(())
    }

    /// Process a resource response from the sovereign node
    /// This is called on the viewer side
    #[instrument(skip(self, resource_data), fields(connection_id = %self.get_id(), resource_id = %resource_id), level = "info")]
    async fn process_website_resource_response(
        &self,
        resource_id: String,
        mut resource_data: osvauld_core::models::ResourceSyncData,
    ) -> P2PResult<()> {
        info!(
            "Processing website resource response for resource {}",
            resource_id
        );

        // Add resource to local repository with Website connection type
        let connection_type = ConnectionType::Website;
        add_resource_sync(&mut resource_data, self.repo_ctx.clone(), &connection_type).await?;

        info!(
            "Resource {} added successfully to local repository",
            resource_id
        );

        // Emit event to notify frontend
        let peer_user = self.get_peer_user().await;
        self.event_emitter.emit(P2PEvent::ResourceAdded {
            resource_id: resource_id.clone(),
            username: peer_user.username,
        });

        info!(
            "Resource response processing complete for resource {}",
            resource_id
        );

        Ok(())
    }

    /// Process initial sync complete message
    /// This is called on the viewer side when sovereign node finishes sending all resources
    #[instrument(skip(self), fields(connection_id = %self.get_id(), folder_id = %folder_id, resource_count = %resource_count), level = "info")]
    async fn process_initial_sync_complete(
        &self,
        folder_id: String,
        resource_count: usize,
    ) -> P2PResult<()> {
        info!(
            "Received InitialSyncComplete for folder {} with {} resources",
            folder_id, resource_count
        );

        // Get peer user
        let peer_user = self.get_peer_user().await;

        // Update first_sync to true in the database
        self.repo_ctx
            .user_repo
            .update_first_sync(&peer_user.id, true)
            .await
            .map_err(|e| {
                error!(
                    "Failed to update first_sync for user {}: {}",
                    peer_user.id, e
                );
                crate::p2p::errors::P2PError::Custom(format!("Failed to update first_sync: {}", e))
            })?;

        info!(
            "Updated first_sync to true for user {} after receiving {} resources",
            peer_user.id, resource_count
        );

        Ok(())
    }

    /// Start website sync - called by viewer after handshake
    /// This is called by the viewer (initiator) after WebsiteHandshakeResponse
    #[instrument(skip(self), fields(connection_id = %self.get_id()), level = "info")]
    pub async fn start_website_sync(&self) -> P2PResult<()> {
        info!("Starting website sync");

        // Only initiator should execute this
        if !self.is_initiator {
            info!("Not initiator, skipping website sync");
            return Ok(());
        }

        // Get peer user from database to check first_sync
        let peer_user = self.get_peer_user().await;
        info!(
            "Peer user: {}, first_sync: {}",
            peer_user.id, peer_user.first_sync
        );
        let peer_user = self
            .repo_ctx
            .user_repo
            .get_user_by_id(&peer_user.id)
            .await?;

        // Get the UCAN token from peer user
        let ucan_token = peer_user.ucan_token.clone();

        // TODO: Check if first_sync is false (meaning this IS first connection)
        // If !first_sync, need to get ALL resources
        // If first_sync, need to get folder_id from ucan, get resources, and state vectors

        // For now, just send the request with the ucan_token
        let message = Message::Website(WebsiteMessage::ResourceRequest { ucan_token });

        self.send_message(message).await?;

        info!("Sent website resource request");
        Ok(())
    }
}
