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
            WebsiteMessage::FolderResourceInfo {
                folder_id,
                resource_ids,
                folder_ucan,
            } => {
                self.process_folder_resource_info(
                    folder_id.clone(),
                    resource_ids.clone(),
                    folder_ucan.clone(),
                )
                .await
            }
            WebsiteMessage::IncrementalSyncRequest {
                resource_id,
                resource_ucan,
                sync_data,
            } => {
                self.process_incremental_sync_request(
                    resource_id.clone(),
                    resource_ucan.clone(),
                    sync_data.clone(),
                )
                .await
            }
            WebsiteMessage::IncrementalSyncResponse {
                resource_id,
                sync_data,
            } => {
                self.process_incremental_sync_response(resource_id.clone(), sync_data.clone())
                    .await
            }
            WebsiteMessage::ViewerCommentsUpdate {
                resource_id,
                resource_ucan,
                sync_data,
            } => {
                self.process_viewer_comments_update(
                    resource_id.clone(),
                    resource_ucan.clone(),
                    sync_data.clone(),
                )
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
        info!("Found {} resources in folder {}", resource_count, folder_id);

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
                error!(
                    "Failed to prepare resource {} for viewer: {}",
                    resource_id, e
                );
                crate::p2p::errors::P2PError::Custom(format!("Failed to prepare resource: {}", e))
            })?;

            // Send as ResourceAdditionRequest
            self.send_message(Message::ResourceAdditionRequest(resource_sync_data))
                .await?;

            info!(
                "Sent resource {} with resource-specific permissions",
                resource_id
            );
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
    pub async fn start_website_sync(&self, first_sync: bool) -> P2PResult<()> {
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
        if !first_sync {
            // Get the UCAN token from peer user
            let ucan_token = peer_user.ucan_token.clone();

            // For now, just send the request with the ucan_token
            let message = Message::Website(WebsiteMessage::ResourceRequest { ucan_token });

            self.send_message(message).await?;

            info!("Sent website resource request");
            Ok(())
        } else {
            self.perform_incremental_sync().await?;
            Ok(())
        }
    }

    /// Perform incremental sync for reconnection
    /// Sends folder info and resource state vectors for sync
    #[instrument(skip(self), fields(connection_id = %self.get_id()), level = "info")]
    pub async fn perform_incremental_sync(&self) -> P2PResult<()> {
        info!("Starting incremental sync");

        // Get peer user and local user
        let peer_user = self.get_peer_user().await;
        let local_user = self.get_local_user().await?;
        info!(
            "Performing incremental sync for peer user: {}",
            peer_user.id
        );

        // Get folder manifest (folders with their resources and UCAN tokens)
        // TODO: this should be based on who sent it.
        let folder_manifest =
            services::get_viewer_folder_manifest(&local_user.id, self.repo_ctx.clone())
                .await
                .map_err(|e| {
                    error!("Failed to get viewer folder manifest: {}", e);
                    crate::p2p::errors::P2PError::Custom(format!(
                        "Failed to get folder manifest: {}",
                        e
                    ))
                })?;

        info!("Found {} folders to sync", folder_manifest.len());

        // For each folder, send folder info and resource sync requests
        for folder_info in folder_manifest {
            info!(
                "Processing folder {} with {} resources",
                folder_info.folder_id,
                folder_info.resource_ids.len()
            );

            // Send folder resource info message
            let message = Message::Website(WebsiteMessage::FolderResourceInfo {
                folder_id: folder_info.folder_id.clone(),
                resource_ids: folder_info.resource_ids.clone(),
                folder_ucan: folder_info.folder_ucan.clone(),
            });

            self.send_message(message).await?;
            info!(
                "Sent FolderResourceInfo for folder {} with {} resources",
                folder_info.folder_id,
                folder_info.resource_ids.len()
            );

            // For each resource, get state vectors and send incremental sync request
            for resource_id in &folder_info.resource_ids {
                info!("Getting sync info for resource {}", resource_id);

                let sync_info = services::get_resource_sync_info(
                    resource_id,
                    &local_user.id,
                    self.repo_ctx.clone(),
                    &self.crypto_utils,
                )
                .await
                .map_err(|e| {
                    error!(
                        "Failed to get sync info for resource {}: {}",
                        resource_id, e
                    );
                    crate::p2p::errors::P2PError::Custom(format!(
                        "Failed to get resource sync info: {}",
                        e
                    ))
                })?;

                // Send incremental sync request with sync_data string
                let sync_message = Message::Website(WebsiteMessage::IncrementalSyncRequest {
                    resource_id: sync_info.resource_id,
                    resource_ucan: sync_info.resource_ucan,
                    sync_data: sync_info.sync_data,
                });

                self.send_message(sync_message).await?;
                info!("Sent incremental sync request for resource {}", resource_id);
            }

            info!("Folder {} processing complete", folder_info.folder_id);
        }

        info!("Incremental sync complete");
        Ok(())
    }

    /// Process folder resource info from viewer during incremental sync
    /// This is called on the node side to detect and send new resources
    #[instrument(skip(self, viewer_resource_ids, folder_ucan), fields(connection_id = %self.get_id(), folder_id = %folder_id), level = "info")]
    async fn process_folder_resource_info(
        &self,
        folder_id: String,
        viewer_resource_ids: Vec<String>,
        folder_ucan: String,
    ) -> P2PResult<()> {
        info!(
            "Processing folder resource info for folder {} with {} viewer resources",
            folder_id,
            viewer_resource_ids.len()
        );

        // 1. Validate folder UCAN token
        let ucan = match crypto_utils::ucan_utils::validate_structure(&folder_ucan).await {
            Ok(ucan) => ucan,
            Err(e) => {
                error!("Failed to validate folder UCAN token: {}", e);
                return Ok(()); // Log and skip
            }
        };

        // 2. Extract folder_id from UCAN and verify it matches
        let ucan_folder_id = match crypto_utils::ucan_utils::extract_folder_id_from_ucan(&ucan) {
            Ok(id) => id,
            Err(e) => {
                error!("Failed to extract folder_id from UCAN: {}", e);
                return Ok(()); // Log and skip
            }
        };

        if ucan_folder_id != folder_id {
            error!(
                "Folder ID mismatch: message={}, ucan={}",
                folder_id, ucan_folder_id
            );
            return Ok(()); // Log and skip
        }

        info!("Folder UCAN validated for folder {}", folder_id);

        // 3. Get local user and peer user
        let local_user = match self.get_local_user().await {
            Ok(user) => user,
            Err(e) => {
                error!("Failed to get local user: {}", e);
                return Ok(()); // Log and skip
            }
        };
        let peer_user = self.get_peer_user().await;

        // 4. Get node's current resource list for this folder
        let node_resource_ids = match self
            .repo_ctx
            .resource_repo
            .get_resource_ids_by_folder_id(&folder_id)
            .await
        {
            Ok(ids) => ids,
            Err(e) => {
                error!("Failed to get resource IDs for folder {}: {}", folder_id, e);
                return Ok(()); // Log and skip
            }
        };

        info!(
            "Node has {} resources in folder {}",
            node_resource_ids.len(),
            folder_id
        );

        // 5. Find new resources (resources on node but not on viewer)
        let new_resource_ids: Vec<String> = node_resource_ids
            .into_iter()
            .filter(|id| !viewer_resource_ids.contains(id))
            .collect();

        if new_resource_ids.is_empty() {
            info!("No new resources to send for folder {}", folder_id);
            return Ok(());
        }

        info!(
            "Found {} new resources to send for folder {}",
            new_resource_ids.len(),
            folder_id
        );

        // 6. Send each new resource individually
        for resource_id in new_resource_ids {
            info!("Preparing new resource {} for viewer", resource_id);

            // Get resource to determine its type (for prepare_resource_for_viewer)
            let resource = match self
                .repo_ctx
                .resource_repo
                .find_by_id(&resource_id, &local_user.id)
                .await
            {
                Ok(res) => res,
                Err(e) => {
                    error!("Failed to get resource {} details: {}", resource_id, e);
                    continue; // Log and skip this resource
                }
            };

            let resource_type = resource.resource.resource_type.to_string();
            info!(
                "New resource {} is of type '{}'",
                resource_id, resource_type
            );

            // Prepare resource for viewer
            let resource_sync_data = match services::prepare_resource_for_viewer(
                &resource_id,
                &resource_type,
                &peer_user,
                &local_user,
                self.repo_ctx.clone(),
                &self.crypto_utils,
            )
            .await
            {
                Ok(data) => data,
                Err(e) => {
                    error!(
                        "Failed to prepare resource {} for viewer: {}",
                        resource_id, e
                    );
                    continue; // Log and skip this resource
                }
            };

            // Send resource as ResourceAdditionRequest
            if let Err(e) = self
                .send_message(Message::ResourceAdditionRequest(resource_sync_data))
                .await
            {
                error!("Failed to send resource {}: {}", resource_id, e);
                continue; // Log and skip this resource
            }

            info!(
                "Sent new resource {} to viewer with resource-specific permissions",
                resource_id
            );
        }

        info!(
            "Completed processing folder resource info for folder {}",
            folder_id
        );
        Ok(())
    }

    /// Process incremental sync request from viewer
    /// This is called on the node side to sync existing resources
    #[instrument(skip(self, resource_ucan, sync_data), fields(connection_id = %self.get_id(), resource_id = %resource_id), level = "info")]
    async fn process_incremental_sync_request(
        &self,
        resource_id: String,
        resource_ucan: String,
        sync_data: String,
    ) -> P2PResult<()> {
        info!(
            "Processing incremental sync request for resource {}",
            resource_id
        );

        // 1. Validate resource UCAN token
        let ucan = match crypto_utils::ucan_utils::validate_structure(&resource_ucan).await {
            Ok(ucan) => ucan,
            Err(e) => {
                error!("Failed to validate resource UCAN token: {}", e);
                return Ok(()); // Log and skip
            }
        };

        // 2. Extract resource_id from UCAN and verify it matches
        let ucan_resource_id = match crypto_utils::ucan_utils::extract_resource_id_from_ucan(&ucan)
        {
            Ok(id) => id,
            Err(e) => {
                error!("Failed to extract resource_id from UCAN: {}", e);
                return Ok(()); // Log and skip
            }
        };

        if ucan_resource_id != resource_id {
            error!(
                "Resource ID mismatch: message={}, ucan={}",
                resource_id, ucan_resource_id
            );
            return Ok(()); // Log and skip
        }

        info!("Resource UCAN validated for resource {}", resource_id);

        // 3. Get local user
        let local_user = match self.get_local_user().await {
            Ok(user) => user,
            Err(e) => {
                error!("Failed to get local user: {}", e);
                return Ok(()); // Log and skip
            }
        };

        // Call service function to process sync and get updates
        let response_sync_data = match services::process_incremental_resource_sync(
            &resource_id,
            &local_user.id,
            &sync_data,
            self.repo_ctx.clone(),
            &self.crypto_utils,
        )
        .await
        {
            Ok(data) => data,
            Err(e) => {
                error!(
                    "Failed to process incremental sync for resource {}: {}",
                    resource_id, e
                );
                return Ok(()); // Log and skip
            }
        };

        let local_device = match self.get_local_device().await {
            Some(device) => device,
            None => {
                return Ok(()); // Log and skip
            }
        };
        self.repo_ctx
            .vector_clock_repo
            .increment_vector_clock(&resource_id, &local_device.id)
            .await?;
        // Send response back to viewer
        if let Err(e) = self
            .send_message(Message::Website(WebsiteMessage::IncrementalSyncResponse {
                resource_id: resource_id.clone(),
                sync_data: response_sync_data,
            }))
            .await
        {
            error!("Failed to send incremental sync response: {}", e);
            return Ok(()); // Log and skip
        }

        info!(
            "Completed processing incremental sync request for resource {}",
            resource_id
        );
        Ok(())
    }

    /// Process incremental sync response from node
    /// This is called on the viewer side to apply updates
    #[instrument(skip(self, sync_data), fields(connection_id = %self.get_id(), resource_id = %resource_id), level = "info")]
    async fn process_incremental_sync_response(
        &self,
        resource_id: String,
        sync_data: String,
    ) -> P2PResult<()> {
        info!(
            "Processing incremental sync response for resource {}",
            resource_id
        );

        // Get local user
        let local_user = match self.get_local_user().await {
            Ok(user) => user,
            Err(e) => {
                error!("Failed to get local user: {}", e);
                return Ok(()); // Log and skip
            }
        };

        // Apply updates from node and generate viewer updates (only comments)
        let viewer_updates = match services::apply_and_generate_viewer_updates(
            &resource_id,
            &local_user.id,
            &sync_data,
            self.repo_ctx.clone(),
            &self.crypto_utils,
        )
        .await
        {
            Ok(data) => data,
            Err(e) => {
                error!(
                    "Failed to apply and generate viewer updates for resource {}: {}",
                    resource_id, e
                );
                return Ok(()); // Log and skip
            }
        };

        // Emit updates event to frontend (for viewer side)
        self.event_emitter.emit(P2PEvent::UpdatesEvent {
            resource_id: resource_id.clone(),
            updates: sync_data.clone(),
            client_id: 0, // Website sync uses default client_id
        });

        info!(
            "Updates event emitted to frontend for resource {}",
            resource_id
        );

        // Get resource UCAN token
        let resource_ucan = match services::get_resource_ucan_key(
            &resource_id,
            &local_user.id,
            self.repo_ctx.clone(),
        )
        .await
        {
            Ok(ucan) => ucan,
            Err(e) => {
                error!(
                    "Failed to get resource UCAN for resource {}: {}",
                    resource_id, e
                );
                return Ok(()); // Log and skip
            }
        };

        // Send viewer comments updates back to node
        if let Err(e) = self
            .send_message(Message::Website(WebsiteMessage::ViewerCommentsUpdate {
                resource_id: resource_id.clone(),
                resource_ucan,
                sync_data: viewer_updates,
            }))
            .await
        {
            error!("Failed to send viewer comments update back to node: {}", e);
            return Ok(()); // Log and skip
        }

        info!(
            "Successfully processed incremental sync response and sent viewer comments for resource {}",
            resource_id
        );
        Ok(())
    }

    /// Process viewer comments update on node side
    /// This is called on the node when viewer sends back comment updates
    #[instrument(skip(self, resource_ucan, sync_data), fields(connection_id = %self.get_id(), resource_id = %resource_id), level = "info")]
    async fn process_viewer_comments_update(
        &self,
        resource_id: String,
        resource_ucan: String,
        sync_data: String,
    ) -> P2PResult<()> {
        info!(
            "Processing viewer comments update for resource {}",
            resource_id
        );

        // 1. Validate resource UCAN token
        let ucan = match crypto_utils::ucan_utils::validate_structure(&resource_ucan).await {
            Ok(ucan) => ucan,
            Err(e) => {
                error!("Failed to validate resource UCAN token: {}", e);
                return Ok(()); // Log and skip
            }
        };

        // 2. Extract resource_id from UCAN and verify it matches
        let ucan_resource_id = match crypto_utils::ucan_utils::extract_resource_id_from_ucan(&ucan)
        {
            Ok(id) => id,
            Err(e) => {
                error!("Failed to extract resource_id from UCAN: {}", e);
                return Ok(()); // Log and skip
            }
        };

        if ucan_resource_id != resource_id {
            error!(
                "Resource ID mismatch: message={}, ucan={}",
                resource_id, ucan_resource_id
            );
            return Ok(()); // Log and skip
        }

        info!(
            "Resource UCAN validated for viewer comments update {}",
            resource_id
        );

        // 3. Get local user
        let local_user = match self.get_local_user().await {
            Ok(user) => user,
            Err(e) => {
                error!("Failed to get local user: {}", e);
                return Ok(()); // Log and skip
            }
        };

        // 4. Apply viewer's comment updates
        if let Err(e) = services::apply_updates(
            &resource_id,
            &sync_data,
            &local_user.id,
            self.repo_ctx.clone(),
            &self.crypto_utils,
        )
        .await
        {
            error!(
                "Failed to apply viewer comments for resource {}: {}",
                resource_id, e
            );
            return Ok(()); // Log and skip
        }

        let local_device = match self.get_local_device().await {
            Some(device) => device,
            None => {
                return Ok(()); // Log and skip
            }
        };
        self.repo_ctx
            .vector_clock_repo
            .increment_vector_clock(&resource_id, &local_device.id)
            .await?;

        // Emit updates event to frontend (for node side)
        self.event_emitter.emit(P2PEvent::UpdatesEvent {
            resource_id: resource_id.clone(),
            updates: sync_data.clone(),
            client_id: 0, // Website sync uses default client_id
        });

        info!(
            "Updates event emitted to frontend for resource {} (viewer comments)",
            resource_id
        );

        info!(
            "Successfully processed viewer comments update for resource {}",
            resource_id
        );
        Ok(())
    }
}
