use crate::p2p::{
    errors::{P2PError, P2PResult, ResourceSyncError},
    peer_connection::PeerConnection,
    P2PEvent,
};

use osvauld_core::models::{
    ConnectionType, LiveEditMessage, Message, ResourceSyncData, ResourceUpdateMsg,
};
use services::{
    add_resource_sync, add_share_records, apply_updates, apply_updates_and_get_peer_updates,
    find_missing_resource_keys, generate_updates_for_peer, get_resource_for_remote_addition,
    get_resource_keys_for_resource, get_resource_state_vector, get_resource_ucan_key,
    get_share_records_for_resource, get_vector_clocks_for_resource, merge_share_records,
    merge_vector_clocks, update_vector_clocks, validate_authority_for_update,
};

use tracing::{debug, error, info, instrument};

impl PeerConnection {
    /// Sync a single resource by sending StateVectorRequest
    /// This initiates the resource update flow for one specific resource
    #[instrument(skip(self), fields(
        connection_id = %self.get_id(),
        resource_id = %resource_id
    ), level = "info")]
    pub async fn sync_single_resource(&self, resource_id: &str, user_id: &str) -> P2PResult<()> {
        info!("Starting sync for resource {}", resource_id);

        // Get state vectors for this resource
        let state_vectors = get_resource_state_vector(
            resource_id,
            user_id,
            self.repo_ctx.clone(),
            &self.crypto_utils,
        )
        .await?;

        debug!(
            resource_id = %resource_id,
            "State vector retrieved successfully"
        );

        // Get UCAN token for this resource
        let ucan_token = get_resource_ucan_key(resource_id, user_id, self.repo_ctx.clone()).await?;

        // Send StateVectorRequest message
        let message = ResourceUpdateMsg::StateVectorRequest {
            resource_id: resource_id.to_string(),
            state_vectors,
            ucan_token,
        };

        self.send_message(Message::MergeUpdate(message)).await?;

        info!(
            resource_id = %resource_id,
            "State vector request sent successfully"
        );

        Ok(())
    }

    /// Send a single resource to peer by preparing payload and sending ResourceAdditionRequest
    /// This is used for initial resource transmission (not sync)
    #[instrument(skip(self), fields(
        connection_id = %self.get_id(),
        resource_id = %resource_id
    ), level = "info")]
    pub async fn send_single_resource(&self, resource_id: &str) -> P2PResult<()> {
        info!("Preparing to send resource {}", resource_id);

        let peer_device = self.get_peer_device().await;

        // Get resource payload for remote addition
        let resource_payload =
            get_resource_for_remote_addition(resource_id, &peer_device, self.repo_ctx.clone())
                .await?;

        debug!(
            resource_id = %resource_id,
            "Resource payload prepared for transmission"
        );

        // Send ResourceAdditionRequest message
        self.send_message(Message::ResourceAdditionRequest(resource_payload))
            .await?;

        info!(
            resource_id = %resource_id,
            "Resource addition request sent successfully"
        );

        Ok(())
    }

    #[instrument(skip(self), fields(
        connection_id = %self.get_id(),
    ), level = "info")]
    pub async fn send_resources(&self) -> P2PResult<()> {
        info!("Starting resource transmission process");
        let connection_type = self.get_connection_type().await;

        // Collect resource IDs to avoid borrowing issues
        let resource_ids: Vec<String> = match connection_type {
            ConnectionType::Device => {
                let manifest = self.get_device_manifest_result().await?;
                debug!(
                    local_missing = manifest.local_missing.unknown_resources.len(),
                    remote_missing = manifest.remote_missing.unknown_resources.len(),
                    "Device manifest retrieved"
                );

                if manifest.remote_missing.unknown_resources.is_empty() {
                    self.send_message(Message::ResourceAdditionComplete).await?;
                }
                manifest
                    .remote_missing
                    .unknown_resources
                    .iter()
                    .cloned()
                    .collect()
            }
            ConnectionType::User => {
                let manifest = self.get_user_manifest_result().await?;
                debug!(
                    local_missing = manifest.local_missing.unknown_resources.len(),
                    remote_missing = manifest.remote_missing.unknown_resources.len(),
                    "User manifest retrieved"
                );

                if manifest.remote_missing.unknown_resources.is_empty() {
                    self.send_message(Message::ResourceAdditionComplete).await?;
                }
                manifest
                    .remote_missing
                    .unknown_resources
                    .iter()
                    .cloned()
                    .collect()
            }
            ConnectionType::Website => {
                vec![]
            }
        };

        info!(
            resources_to_send = resource_ids.len(),
            "Resource collection completed"
        );

        for (index, resource_id) in resource_ids.iter().enumerate() {
            debug!(
                resource_index = index + 1,
                total_resources = resource_ids.len(),
                resource_id = %resource_id,
                "Processing resource for transmission"
            );

            // Use the new send_single_resource method
            self.send_single_resource(resource_id).await?;

            info!(
                resource_id = %resource_id,
                progress = format!("{}/{}", index + 1, resource_ids.len()),
                "Resource sent successfully"
            );
        }

        info!("All resource addition requests sent successfully");
        Ok(())
    }

    #[instrument(skip(self, payload), fields(
        connection_id = %self.get_id(),
        resource_id = %payload.resource.id,
        resource_type = %payload.resource.resource_type,
        resource_keys_count = payload.resource_keys.len(),
        share_records_count = payload.share_records.len(),
        vector_clocks_count = payload.vector_clocks.len()
    ), level = "info")]
    pub async fn process_resource_addition_request(
        &self,
        payload: &mut ResourceSyncData,
    ) -> P2PResult<()> {
        info!("Processing resource addition request");
        debug!("Adding resource to local repository");

        let connection_type = self.get_connection_type().await;
        add_resource_sync(payload, self.repo_ctx.clone(), &connection_type).await?;

        info!(
            resource_id = %payload.resource.id,
            "Resource added successfully to local repository"
        );
        let peer_user = self.get_peer_user().await;
        self.event_emitter.emit(P2PEvent::ResourceAdded {
            resource_id: payload.resource.id.clone(),
            username: peer_user.username,
        });

        let is_empty = match connection_type {
            ConnectionType::Device => {
                self.remove_device_local_missing_resource(&payload.resource.id)
                    .await
            }
            ConnectionType::User => {
                self.remove_user_local_missing_resource(&payload.resource.id)
                    .await
            }
            ConnectionType::Website => true,
        };

        if is_empty {
            self.send_message(Message::ResourceAdditionComplete).await?;
        }

        Ok(())
    }

    #[instrument(skip(self), fields(
        connection_id = %self.get_id(),
        is_initiator = self.is_initiator
    ), level = "info")]
    pub async fn process_resource_addition_complete(&self) -> P2PResult<()> {
        info!("Processing resource addition completion");

        if self.is_initiator {
            debug!("Processing as initiator - checking for resources requiring sync");

            let connection_type = self.get_connection_type().await;
            let resource_ids: Vec<String> = match connection_type {
                ConnectionType::Device => {
                    let manifest = self.get_device_manifest_result().await?;
                    manifest.resources_requiring_sync.iter().cloned().collect()
                }
                ConnectionType::User => {
                    let manifest = self.get_user_manifest_result().await?;
                    manifest.resources_requiring_sync.iter().cloned().collect()
                }
                ConnectionType::Website => {
                    vec![]
                }
            };

            info!(
                sync_required_count = resource_ids.len(),
                "Found resources requiring synchronization"
            );

            if !resource_ids.is_empty() {
                debug!("Starting state vector exchange for sync-required resources");

                let local_user = self.get_local_user().await?;
                for (index, resource_id) in resource_ids.iter().enumerate() {
                    debug!(
                        resource_index = index + 1,
                        total_resources = resource_ids.len(),
                        resource_id = %resource_id,
                        "Requesting state vector for resource"
                    );

                    // Use the new sync_single_resource method
                    self.sync_single_resource(resource_id, &local_user.id)
                        .await?;

                    info!(
                        resource_id = %resource_id,
                        progress = format!("{}/{}", index + 1, resource_ids.len()),
                        "State vector request sent successfully"
                    );
                }

                info!("All state vector requests sent successfully");
            } else {
                debug!("No resources require synchronization");
            }
        } else {
            debug!("Not initiator - no action required for resource addition complete");
        }

        Ok(())
    }

    #[instrument(skip(self, payload), fields(
        connection_id = %self.get_id(),
        message_type = ?std::mem::discriminant(payload)
    ), level = "info")]
    pub async fn process_resource_update_message(
        &self,
        payload: &ResourceUpdateMsg,
    ) -> P2PResult<()> {
        info!("Processing resource update message");

        let user = self.get_local_user().await?;

        match payload {
            ResourceUpdateMsg::StateVectorRequest {
                resource_id,
                state_vectors,
                ucan_token,
            } => {
                debug!(
                    resource_id = %resource_id,
                    "Processing state vector request"
                );

                let peer_user = self.get_peer_user().await;

                // Service error automatically propagates
                let is_token_valid = validate_authority_for_update(
                    resource_id,
                    ucan_token,
                    &peer_user.id,
                    self.repo_ctx.clone(),
                    &self.domain,
                )
                .await?;

                if !is_token_valid {
                    error!("Token is invalid for resource {}", resource_id);
                    return Err(ResourceSyncError::InvalidUpdateAuthority {
                        resource_id: resource_id.clone(),
                    }
                    .into());
                }

                // Service error automatically propagates
                let updates = generate_updates_for_peer(
                    resource_id,
                    &user.id,
                    self.repo_ctx.clone(),
                    &self.crypto_utils,
                    &state_vectors,
                )
                .await?;

                debug!(
                    resource_id = %resource_id,
                    updates = updates,
                    "Generated updates for peer successfully"
                );

                let ucan_token =
                    get_resource_ucan_key(resource_id, &user.id, self.repo_ctx.clone()).await?;

                let message = ResourceUpdateMsg::UpdatesResponse {
                    resource_id: resource_id.to_string(),
                    updates,
                    ucan_token,
                };

                self.send_message(Message::MergeUpdate(message)).await?;

                info!(
                    resource_id = %resource_id,
                    "Updates response sent successfully"
                );
            }

            ResourceUpdateMsg::UpdatesResponse {
                resource_id,
                updates,
                ucan_token,
            } => {
                debug!(
                    resource_id = %resource_id,
                    update_count = updates.len(),
                    "Processing updates response"
                );

                let peer_user = self.get_peer_user().await;

                // Service error automatically propagates
                let is_token_valid = validate_authority_for_update(
                    resource_id,
                    ucan_token,
                    &peer_user.id,
                    self.repo_ctx.clone(),
                    &self.domain,
                )
                .await?;

                if !is_token_valid {
                    error!("Token is invalid for resource {}", resource_id);
                    return Err(ResourceSyncError::InvalidUpdateAuthority {
                        resource_id: resource_id.clone(),
                    }
                    .into());
                }

                // Service error automatically propagates
                let remote_updates = apply_updates_and_get_peer_updates(
                    resource_id,
                    &user.id,
                    updates,
                    self.repo_ctx.clone(),
                    &self.crypto_utils,
                )
                .await?;

                debug!(
                    resource_id = %resource_id,
                    remote_update_count = remote_updates.len(),
                    "Applied updates and generated peer updates"
                );

                // Service errors automatically propagate
                let share_records =
                    get_share_records_for_resource(resource_id, self.repo_ctx.clone()).await?;

                debug!(
                    resource_id = %resource_id,
                    share_record_count = share_records.len(),
                    "Retrieved share records for resource"
                );

                let vector_clocks =
                    get_vector_clocks_for_resource(resource_id, self.repo_ctx.clone()).await?;

                debug!(
                    resource_id = %resource_id,
                    "Retrieved vector clocks for resource"
                );

                let resource_keys =
                    get_resource_keys_for_resource(resource_id, self.repo_ctx.clone()).await?;

                debug!(
                    resource_id = %resource_id,
                    "Retrieved resource keys for resource"
                );

                let message = ResourceUpdateMsg::FinalUpdateMerge {
                    resource_id: resource_id.clone(),
                    updates: remote_updates,
                    vector_clocks,
                    share_records,
                    resource_keys,
                };

                self.send_message(Message::MergeUpdate(message)).await?;

                info!(
                    resource_id = %resource_id,
                    "Final update merge sent successfully"
                );

                let peer_device = self.get_peer_device().await;
                let client_id = peer_device.get_client_id().map_err(|e| {
                    P2PError::Custom(format!("failed to get client_id {}", e.to_string()))
                })?;

                // Emit updates event to frontend
                self.event_emitter.emit(P2PEvent::UpdatesEvent {
                    resource_id: resource_id.clone(),
                    updates: updates.clone(),
                    client_id,
                });

                debug!(
                    resource_id = %resource_id,
                    "Updates event emitted to frontend"
                );
            }

            ResourceUpdateMsg::FinalUpdateMerge {
                resource_id,
                updates,
                vector_clocks,
                share_records,
                resource_keys,
            } => {
                debug!(
                    resource_id = %resource_id,
                    update_count = updates.len(),
                    share_record_count = share_records.len(),
                    "Processing final update merge"
                );

                // Service error automatically propagates
                apply_updates(
                    resource_id,
                    updates,
                    &user.id,
                    self.repo_ctx.clone(),
                    &self.crypto_utils,
                )
                .await?;

                let peer_device = self.get_peer_device().await;

                let client_id = peer_device.get_client_id().map_err(|e| {
                    P2PError::Custom(format!("failed to get client_id {}", e.to_string()))
                })?;

                // Emit updates event to frontend
                self.event_emitter.emit(P2PEvent::UpdatesEvent {
                    resource_id: resource_id.clone(),
                    updates: updates.clone(),
                    client_id,
                });

                debug!(
                    resource_id = %resource_id,
                    "Final updates event emitted to frontend"
                );

                // Service error automatically propagates
                let local_resource_keys =
                    get_resource_keys_for_resource(resource_id, self.repo_ctx.clone()).await?;

                let (local_missing, remote_missing) =
                    find_missing_resource_keys(&local_resource_keys, resource_keys);

                // Repository error automatically propagates
                self.repo_ctx
                    .resource_key_repo
                    .add_resource_keys(&local_missing)
                    .await?;

                // Service error automatically propagates
                let (add_clock, update_clock) =
                    merge_vector_clocks(resource_id, vector_clocks, self.repo_ctx.clone()).await?;

                debug!(
                    resource_id = %resource_id,
                    "Vector clocks merged successfully"
                );

                // Service error automatically propagates
                let remote_share_records =
                    merge_share_records(resource_id, &share_records, self.repo_ctx.clone()).await?;

                debug!(
                    resource_id = %resource_id,
                    merged_record_count = remote_share_records.len(),
                    "Share records merged successfully"
                );

                let message = ResourceUpdateMsg::VectorClockResponse {
                    resource_id: resource_id.clone(),
                    update_clock,
                    add_clock,
                    share_records: remote_share_records,
                    resource_keys: remote_missing,
                };

                self.send_message(Message::MergeUpdate(message)).await?;

                info!(
                    resource_id = %resource_id,
                    "Vector clock response sent successfully"
                );
            }

            ResourceUpdateMsg::VectorClockResponse {
                resource_id,
                update_clock,
                add_clock,
                share_records,
                resource_keys,
            } => {
                debug!(
                    resource_id = %resource_id,
                    share_record_count = share_records.len(),
                    "Processing vector clock response"
                );

                // Service error automatically propagates
                update_vector_clocks(add_clock, update_clock, self.repo_ctx.clone()).await?;

                debug!(
                    resource_id = %resource_id,
                    "Vector clocks updated successfully"
                );

                // Service error automatically propagates
                add_share_records(share_records, self.repo_ctx.clone()).await?;

                info!(
                    resource_id = %resource_id,
                    "Share records added successfully - sync complete"
                );

                // Repository error automatically propagates
                self.repo_ctx
                    .resource_key_repo
                    .add_resource_keys(resource_keys)
                    .await?;
            }
        }

        Ok(())
    }

    #[instrument(skip(self, message), fields(
        connection_id = %self.get_id(),
        message_type = ?std::mem::discriminant(message)
    ), level = "debug")]
    pub async fn handle_live_edit_flow(&self, message: &LiveEditMessage) -> P2PResult<()> {
        let user = self.get_local_user().await?;

        match message {
            LiveEditMessage::DocumentCheck { resource_id } => {
                self.event_emitter.emit(P2PEvent::DocumentCheck {
                    resource_id: resource_id.clone(),
                    connection_id: self.get_id(),
                });
                Ok(())
            }

            LiveEditMessage::StateVectorExchange {
                resource_id,
                state_vectors,
            } => {
                self.event_emitter.emit(P2PEvent::UpdateRequest {
                    resource_id: resource_id.clone(),
                    connection_id: self.get_id(),
                    state_vectors: state_vectors.clone(),
                    current_user_id: user.id.clone(),
                });
                Ok(())
            }

            LiveEditMessage::UpdateExchange {
                resource_id,
                updates,
            } => {
                let connection_id = self.get_id();
                let peer_device = self.get_peer_device().await;

                let client_id = peer_device.get_client_id().map_err(|e| {
                    P2PError::Custom(format!("failed to get client_id {}", e.to_string()))
                })?;
                self.event_emitter.emit(P2PEvent::ProcessUpdate {
                    resource_id: resource_id.clone(),
                    connection_id,
                    updates: updates.clone(),
                    client_id,
                });
                Ok(())
            }

            LiveEditMessage::UpdateExchangeResponse {
                resource_id,
                updates,
            } => {
                let connection_id = self.get_id();
                let peer_device = self.get_peer_device().await;

                let client_id = peer_device.get_client_id().map_err(|e| {
                    P2PError::Custom(format!("failed to get client_id {}", e.to_string()))
                })?;

                self.event_emitter.emit(P2PEvent::ProcessUpdateResponse {
                    resource_id: resource_id.clone(),
                    connection_id,
                    updates: updates.clone(),
                    client_id,
                });
                Ok(())
            }

            LiveEditMessage::NotSameDocument => {
                self.event_emitter.emit(P2PEvent::DocumentMismatch {
                    connection_id: self.get_id(),
                });
                Ok(())
            }

            LiveEditMessage::DocumentChange { resource_id } => {
                info!("Peer changed document: {}", resource_id);

                // Emit an event so the listener can remove this connection from active connections
                self.event_emitter.emit(P2PEvent::DocumentChanged {
                    resource_id: resource_id.clone(),
                    connection_id: self.get_id(),
                });
                Ok(())
            }

            LiveEditMessage::DocumentUpdate {
                updates,
                resource_id,
                client_id,
                doc_type,
            } => {
                self.event_emitter.emit(P2PEvent::EditingEvent {
                    updates: updates.to_vec(),
                    resource_id: resource_id.to_string(),
                    client_id: client_id.clone(),
                    doc_type: doc_type.to_string(),
                });
                Ok(())
            }

            LiveEditMessage::AwarenessUpdate {
                resource_id,
                client_id,
                awareness_data,
            } => {
                self.event_emitter.emit(P2PEvent::AwarenessEvent {
                    awareness_data: awareness_data.to_vec(),
                    resource_id: resource_id.to_string(),
                    client_id: client_id.clone(),
                });
                Ok(())
            }
        }
    }
}
