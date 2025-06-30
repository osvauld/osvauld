use crate::p2p::peer_connection::PeerConnection;

use super::P2PEvent;
use osvauld_core::models::{
    ConnectionType, DeviceManifestComparisonResult, DeviceManifestRequestPayload,
    DeviceNetworkSyncPayload, FirstUserExchange, Message, ResourceSyncData, ResourceUpdateMsg,
    User, UserManifestPayload, UserNetworkSyncPayload, UserWithDevices,
};
use osvauld_services::{
    add_resource_sync, add_share_records, apply_updates_and_get_peer_updates,
    create_device_network_sync_payload, create_user_network_sync_payload,
    generate_updates_for_peer, get_device_manifest, get_my_user_devices,
    get_resource_for_remote_addition, get_resource_state_vector, get_share_records_for_resource,
    get_user_manifest, get_vector_clocks_for_resource, merge_share_records, merge_vector_clocks,
    process_device_manifest_request, process_device_network_sync, process_user_manifest_request,
    process_user_network_sync_payload, update_vector_clocks,
};

use tracing::{debug, error, info, instrument, Span};

// Helper method signatures to reduce repeated patterns
impl PeerConnection {
    #[instrument(skip(self), fields(
        connection_id = %self.get_id(),
        user_id = %self.user.id
    ), level = "info")]
    pub async fn start_add_device_process(&self) -> Result<(), String> {
        info!("Starting add device process");
        debug!("Retrieving device manifest for user");

        let manifest = match get_device_manifest(&self.repo_ctx, &self.user.id).await {
            Ok(manifest) => {
                debug!(
                    manifest_resource_count = manifest.resources.len(),
                    "Device manifest retrieved successfully"
                );
                manifest
            }
            Err(e) => {
                error!(error = %e, "Failed to get device manifest");
                return Err(format!("Failed to get device manifest: {}", e));
            }
        };

        match self
            .send_message(Message::DeviceManifestRequest(manifest))
            .await
        {
            Ok(_) => {
                info!("Device manifest request sent successfully");
                Ok(())
            }
            Err(e) => {
                error!(error = %e, "Failed to send device manifest request");
                Err(format!("Failed to send device manifest request: {}", e))
            }
        }
    }

    #[instrument(skip(self, payload), fields(
        connection_id = %self.get_id(),
        user_id = %self.user.id,
        known_device_count = payload.known_device_ids.len(),
        other_users_count = payload.other_users.len(),
        folder_count = payload.folder_ids.len(),
        resource_count = payload.resources.len()
    ), level = "info")]
    pub async fn handle_manifest_request(
        &self,
        payload: &DeviceManifestRequestPayload,
    ) -> Result<(), String> {
        info!("Processing device manifest request");
        debug!(
            "Processing manifest with {} known devices, {} other users, {} folders, {} resources",
            payload.known_device_ids.len(),
            payload.other_users.len(),
            payload.folder_ids.len(),
            payload.resources.len()
        );

        let result = match process_device_manifest_request(payload, &self.repo_ctx, &self.user.id)
            .await
        {
            Ok(result) => {
                debug!(
                    local_missing_unknown_users = result.local_missing.unknown_users.len(),
                    local_missing_unknown_devices_common =
                        result.local_missing.unknown_devices_from_common_users.len(),
                    local_missing_unknown_devices_current =
                        result.local_missing.unknown_devices_from_current_user.len(),
                    local_missing_unknown_resources = result.local_missing.unknown_resources.len(),
                    local_missing_unknown_folders = result.local_missing.unknown_folders.len(),
                    remote_missing_unknown_users = result.remote_missing.unknown_users.len(),
                    remote_missing_unknown_devices_common = result
                        .remote_missing
                        .unknown_devices_from_common_users
                        .len(),
                    remote_missing_unknown_devices_current = result
                        .remote_missing
                        .unknown_devices_from_current_user
                        .len(),
                    remote_missing_unknown_resources =
                        result.remote_missing.unknown_resources.len(),
                    remote_missing_unknown_folders = result.remote_missing.unknown_folders.len(),
                    resources_requiring_sync = result.resources_requiring_sync.len(),
                    "Manifest comparison completed"
                );
                result
            }
            Err(e) => {
                error!(error = %e, "Failed to process device manifest request");
                return Err(format!("Failed to process device manifest request: {}", e));
            }
        };

        match self
            .send_message(Message::DeviceManifestResponse(result.clone()))
            .await
        {
            Ok(_) => {
                info!("Device manifest response sent successfully");
                debug!("Setting manifest comparison result");
                self.set_device_manifest_comparison_result(result).await;
                Ok(())
            }
            Err(e) => {
                error!(error = %e, "Failed to send device manifest response");
                Err(format!("Failed to send device manifest response: {}", e))
            }
        }
    }

    #[instrument(skip(self, payload), fields(
        connection_id = %self.get_id(),
        local_missing_unknown_users = payload.local_missing.unknown_users.len(),
        local_missing_unknown_devices_common = payload.local_missing.unknown_devices_from_common_users.len(),
        local_missing_unknown_devices_current = payload.local_missing.unknown_devices_from_current_user.len(),
        local_missing_unknown_resources = payload.local_missing.unknown_resources.len(),
        local_missing_unknown_folders = payload.local_missing.unknown_folders.len(),
        remote_missing_unknown_users = payload.remote_missing.unknown_users.len(),
        remote_missing_unknown_devices_common = payload.remote_missing.unknown_devices_from_common_users.len(),
        remote_missing_unknown_devices_current = payload.remote_missing.unknown_devices_from_current_user.len(),
        remote_missing_unknown_resources = payload.remote_missing.unknown_resources.len(),
        remote_missing_unknown_folders = payload.remote_missing.unknown_folders.len(),
        resources_requiring_sync = payload.resources_requiring_sync.len()
    ), level = "info")]
    pub async fn handle_manifest_response(
        &self,
        payload: &DeviceManifestComparisonResult,
    ) -> Result<(), String> {
        info!("Processing device manifest response");
        debug!(
            "Received manifest comparison with {} sync-required resources",
            payload.resources_requiring_sync.len()
        );

        let manifest_result = payload.inverse();
        debug!(
            inverted_local_missing_unknown_users =
                manifest_result.local_missing.unknown_users.len(),
            inverted_local_missing_unknown_devices_common = manifest_result
                .local_missing
                .unknown_devices_from_common_users
                .len(),
            inverted_local_missing_unknown_devices_current = manifest_result
                .local_missing
                .unknown_devices_from_current_user
                .len(),
            inverted_local_missing_unknown_resources =
                manifest_result.local_missing.unknown_resources.len(),
            inverted_local_missing_unknown_folders =
                manifest_result.local_missing.unknown_folders.len(),
            inverted_remote_missing_unknown_users =
                manifest_result.remote_missing.unknown_users.len(),
            inverted_remote_missing_unknown_devices_common = manifest_result
                .remote_missing
                .unknown_devices_from_common_users
                .len(),
            inverted_remote_missing_unknown_devices_current = manifest_result
                .remote_missing
                .unknown_devices_from_current_user
                .len(),
            inverted_remote_missing_unknown_resources =
                manifest_result.remote_missing.unknown_resources.len(),
            inverted_remote_missing_unknown_folders =
                manifest_result.remote_missing.unknown_folders.len(),
            "Manifest result inverted for local perspective"
        );

        self.set_device_manifest_comparison_result(manifest_result)
            .await;

        match self.send_message(Message::DeviceManifestAck).await {
            Ok(_) => {
                info!("Device manifest acknowledgment sent successfully");
            }
            Err(e) => {
                error!(error = %e, "Failed to send device manifest ack");
                return Err(format!("Failed to send device manifest ack: {}", e));
            }
        }

        let manifest = self.get_device_manifest_result().await?;
        let device_network_payload = match create_device_network_sync_payload(
            &manifest.remote_missing,
            &self.repo_ctx,
        )
        .await
        {
            Ok(payload) => {
                debug!("Network sync payload created successfully");
                payload
            }
            Err(e) => {
                error!(error = %e, "Failed to create network sync payload");
                return Err(format!("Failed to create network sync payload: {}", e));
            }
        };

        match self
            .send_message(Message::DeviceNetworkSync(device_network_payload))
            .await
        {
            Ok(_) => {
                info!("Device network sync message sent successfully");
                Ok(())
            }
            Err(e) => {
                error!(error = %e, "Failed to send device network sync");
                Err(format!("Failed to send device network sync: {}", e))
            }
        }
    }

    #[instrument(skip(self), fields(connection_id = %self.get_id()), level = "info")]
    pub async fn handle_manifest_ack(&self) -> Result<(), String> {
        info!("Processing device manifest acknowledgment");

        let manifest = self.get_device_manifest_result().await?;

        let device_network_payload = match create_device_network_sync_payload(
            &manifest.remote_missing,
            &self.repo_ctx,
        )
        .await
        {
            Ok(payload) => {
                debug!("Network sync payload created successfully for ack response");
                payload
            }
            Err(e) => {
                error!(error = %e, "Failed to create network sync payload for ack");
                return Err(format!("Failed to create network sync payload: {}", e));
            }
        };

        self.send_message(Message::DeviceNetworkSync(device_network_payload))
            .await?;
        Ok(())
    }

    #[instrument(skip(self, payload), fields(
        connection_id = %self.get_id(),
        unknown_users_with_devices_count = payload.unknown_users_with_devices.len(),
        unknown_devices_common_users_count = payload.unknown_devices_from_common_users.len(),
        unknown_devices_current_user_count = payload.unknown_devices_from_current_user.len(),
        unknown_folders_count = payload.unknown_folders.len()
    ), level = "info")]
    pub async fn handle_device_network_sync(
        &self,
        payload: &DeviceNetworkSyncPayload,
    ) -> Result<(), String> {
        info!("Processing device network sync payload");
        debug!(
            "Processing network sync with {} users with devices, {} devices from common users, {} devices from current user, {} folders",
            payload.unknown_users_with_devices.len(),
            payload.unknown_devices_from_common_users.len(),
            payload.unknown_devices_from_current_user.len(),
            payload.unknown_folders.len()
        );

        match process_device_network_sync(payload, &self.repo_ctx).await {
            Ok(_) => {
                info!("Network sync processed successfully");
                debug!("Sending network sync acknowledgment");
            }
            Err(e) => {
                error!(error = %e, "Failed to process network sync");
                return Err(format!("Failed to process network sync: {}", e));
            }
        }

        match self.send_message(Message::DeviceNetworkSyncAck).await {
            Ok(_) => {
                info!("Device network sync acknowledgment sent successfully");
                Ok(())
            }
            Err(e) => {
                error!(error = %e, "Failed to send device network sync ack");
                Err(format!("Failed to send device network sync ack: {}", e))
            }
        }
    }

    #[instrument(skip(self), fields(
        connection_id = %self.get_id(),
        device_id = %self.device.id
    ), level = "info")]
    pub async fn send_resources(&self) -> Result<(), String> {
        info!("Starting resource transmission process");

        // Collect resource IDs to avoid borrowing issues
        let resource_ids: Vec<String> = match self.connection_type {
            ConnectionType::Device => {
                let manifest = self.get_device_manifest_result().await?;
                if manifest.local_missing.unknown_resources.is_empty() {
                    self.send_message(Message::ResourceAddtionComplete).await?;
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
                if manifest.local_missing.unknown_resources.is_empty() {
                    self.send_message(Message::ResourceAddtionComplete).await?;
                }
                manifest
                    .remote_missing
                    .unknown_resources
                    .iter()
                    .cloned()
                    .collect()
            }
        };

        for (index, resource_id) in resource_ids.iter().enumerate() {
            debug!(
                resource_index = index + 1,
                total_resources = resource_ids.len(),
                resource_id = %resource_id,
                "Processing resource for transmission"
            );

            let resource_payload =
                match get_resource_for_remote_addition(resource_id, &self.device, &self.repo_ctx)
                    .await
                {
                    Ok(payload) => {
                        debug!(
                            resource_id = %resource_id,
                            "Resource payload prepared for transmission"
                        );
                        payload
                    }
                    Err(e) => {
                        error!(
                            error = %e,
                            resource_id = %resource_id,
                            "Failed to get resource for remote addition"
                        );
                        return Err(format!("Failed to get resource for remote addition: {}", e));
                    }
                };

            match self
                .send_message(Message::ResourceAddtionRequest(resource_payload))
                .await
            {
                Ok(_) => {
                    info!(
                        resource_id = %resource_id,
                        progress = format!("{}/{}", index + 1, resource_ids.len()),
                        "Resource addition request sent successfully"
                    );
                }
                Err(e) => {
                    error!(
                        error = %e,
                        resource_id = %resource_id,
                        "Failed to send resource addition request"
                    );
                    return Err(format!("Failed to send resource addition request: {}", e));
                }
            }
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
        payload: &ResourceSyncData,
    ) -> Result<(), String> {
        info!("Processing resource addition request");
        debug!("Adding resource to local repository");

        match add_resource_sync(payload, &self.repo_ctx).await {
            Ok(_) => {
                info!(
                    resource_id = %payload.resource.id,
                    "Resource added successfully to local repository"
                );
            }
            Err(e) => {
                error!(
                    error = %e,
                    resource_id = %payload.resource.id,
                    "Failed to add resource sync"
                );
                return Err(format!("Failed to add resource sync: {}", e));
            }
        }
        let is_empty = match self.connection_type {
            ConnectionType::Device => {
                self.remove_device_local_missing_resource(&payload.resource.id)
                    .await
            }
            ConnectionType::User => {
                self.remove_user_local_missing_resource(&payload.resource.id)
                    .await
            }
        };
        if is_empty {
            self.send_message(Message::ResourceAddtionComplete).await?;
        }
        Ok(())
    }

    #[instrument(skip(self), fields(
        connection_id = %self.get_id(),
        is_initiator = self.is_initiator
    ), level = "info")]
    pub async fn process_resource_addition_complete(&self) -> Result<(), String> {
        info!("Processing resource addition completion");

        if self.is_initiator {
            debug!("Processing as initiator - checking for resources requiring sync");

            let resource_ids: Vec<String> = match self.connection_type {
                ConnectionType::Device => {
                    let manifest = self.get_device_manifest_result().await?;
                    manifest.resources_requiring_sync.iter().cloned().collect()
                }
                ConnectionType::User => {
                    let manifest = self.get_user_manifest_result().await?;
                    manifest.resources_requiring_sync.iter().cloned().collect()
                }
            };

            info!(
                sync_required_count = resource_ids.len(),
                "Found resources requiring synchronization"
            );

            if !resource_ids.is_empty() {
                debug!("Starting state vector exchange for sync-required resources");

                for (index, resource_id) in resource_ids.iter().enumerate() {
                    debug!(
                        resource_index = index + 1,
                        total_resources = resource_ids.len(),
                        resource_id = %resource_id,
                        "Requesting state vector for resource"
                    );

                    let state_vector = match get_resource_state_vector(
                        resource_id,
                        &self.user.id,
                        &self.repo_ctx,
                        &self.crypto_utils,
                    )
                    .await
                    {
                        Ok(vector) => {
                            debug!(
                                resource_id = %resource_id,
                                "State vector retrieved successfully"
                            );
                            vector
                        }
                        Err(e) => {
                            error!(
                                error = %e,
                                resource_id = %resource_id,
                                "Failed to get resource state vector"
                            );
                            return Err(format!("Failed to get resource state vector: {}", e));
                        }
                    };

                    let message = ResourceUpdateMsg::StateVectorRequest {
                        resource_id: resource_id.to_string(),
                        state_vector,
                    };

                    match self.send_message(Message::MergeUpdate(message)).await {
                        Ok(_) => {
                            info!(
                                resource_id = %resource_id,
                                progress = format!("{}/{}", index + 1, resource_ids.len()),
                                "State vector request sent successfully"
                            );
                        }
                        Err(e) => {
                            error!(
                                error = %e,
                                resource_id = %resource_id,
                                "Failed to send state vector request"
                            );
                            return Err(format!("Failed to send state vector request: {}", e));
                        }
                    }
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
    ) -> Result<(), String> {
        info!("Processing resource update message");

        match payload {
            ResourceUpdateMsg::StateVectorRequest {
                resource_id,
                state_vector,
            } => {
                debug!(
                    resource_id = %resource_id,
                    "Processing state vector request"
                );

                let (updates, state_vector) = match generate_updates_for_peer(
                    resource_id,
                    &self.user.id,
                    &self.repo_ctx,
                    &self.crypto_utils,
                    &state_vector,
                )
                .await
                {
                    Ok((updates, vector)) => {
                        debug!(
                            resource_id = %resource_id,
                            update_count = updates.len(),
                            "Generated updates for peer successfully"
                        );
                        (updates, vector)
                    }
                    Err(e) => {
                        error!(
                            error = %e,
                            resource_id = %resource_id,
                            "Failed to generate updates for peer"
                        );
                        return Err(format!("Failed to generate updates for peer: {}", e));
                    }
                };

                let message = ResourceUpdateMsg::UpdatesResponse {
                    resource_id: resource_id.to_string(),
                    updates,
                    state_vector,
                };

                match self.send_message(Message::MergeUpdate(message)).await {
                    Ok(_) => {
                        info!(
                            resource_id = %resource_id,
                            "Updates response sent successfully"
                        );
                    }
                    Err(e) => {
                        error!(
                            error = %e,
                            resource_id = %resource_id,
                            "Failed to send updates response"
                        );
                        return Err(format!("Failed to send updates response: {}", e));
                    }
                }
            }
            ResourceUpdateMsg::UpdatesResponse {
                resource_id,
                updates,
                state_vector,
            } => {
                debug!(
                    resource_id = %resource_id,
                    update_count = updates.len(),
                    "Processing updates response"
                );

                let (remote_updates, _) = match apply_updates_and_get_peer_updates(
                    resource_id,
                    &self.user.id,
                    &self.repo_ctx,
                    &self.crypto_utils,
                    updates,
                    state_vector,
                )
                .await
                {
                    Ok((updates, result)) => {
                        debug!(
                            resource_id = %resource_id,
                            remote_update_count = updates.len(),
                            "Applied updates and generated peer updates"
                        );
                        (updates, result)
                    }
                    Err(e) => {
                        error!(
                            error = %e,
                            resource_id = %resource_id,
                            "Failed to apply updates and get peer updates"
                        );
                        return Err(format!(
                            "Failed to apply updates and get peer updates: {}",
                            e
                        ));
                    }
                };

                let share_records =
                    match get_share_records_for_resource(resource_id, &self.repo_ctx).await {
                        Ok(records) => {
                            debug!(
                                resource_id = %resource_id,
                                share_record_count = records.len(),
                                "Retrieved share records for resource"
                            );
                            records
                        }
                        Err(e) => {
                            error!(
                                error = %e,
                                resource_id = %resource_id,
                                "Failed to get share records for resource"
                            );
                            return Err(format!("Failed to get share records: {}", e));
                        }
                    };

                let vector_clocks =
                    match get_vector_clocks_for_resource(resource_id, &self.repo_ctx).await {
                        Ok(clocks) => {
                            debug!(
                                resource_id = %resource_id,
                                "Retrieved vector clocks for resource"
                            );
                            clocks
                        }
                        Err(e) => {
                            error!(
                                error = %e,
                                resource_id = %resource_id,
                                "Failed to get vector clocks for resource"
                            );
                            return Err(format!("Failed to get vector clocks: {}", e));
                        }
                    };

                let message = ResourceUpdateMsg::FinalUpdateMerge {
                    resource_id: resource_id.clone(),
                    updates: remote_updates,
                    vector_clocks,
                    share_records,
                };

                match self.send_message(Message::MergeUpdate(message)).await {
                    Ok(_) => {
                        info!(
                            resource_id = %resource_id,
                            "Final update merge sent successfully"
                        );
                    }
                    Err(e) => {
                        error!(
                            error = %e,
                            resource_id = %resource_id,
                            "Failed to send final update merge"
                        );
                        return Err(format!("Failed to send final update merge: {}", e));
                    }
                }

                // Emit updates event to frontend
                self.event_emitter.emit(P2PEvent::UpdatesEvent {
                    resource_id: resource_id.clone(),
                    updates: updates.clone(),
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
            } => {
                debug!(
                    resource_id = %resource_id,
                    update_count = updates.len(),
                    share_record_count = share_records.len(),
                    "Processing final update merge"
                );

                // Emit updates event to frontend
                self.event_emitter.emit(P2PEvent::UpdatesEvent {
                    resource_id: resource_id.clone(),
                    updates: updates.clone(),
                });
                debug!(
                    resource_id = %resource_id,
                    "Final updates event emitted to frontend"
                );

                let (add_clock, update_clock) =
                    match merge_vector_clocks(resource_id, vector_clocks, &self.repo_ctx).await {
                        Ok((add, update)) => {
                            debug!(
                                resource_id = %resource_id,
                                "Vector clocks merged successfully"
                            );
                            (add, update)
                        }
                        Err(e) => {
                            error!(
                                error = %e,
                                resource_id = %resource_id,
                                "Failed to merge vector clocks"
                            );
                            return Err(format!("Failed to merge vector clocks: {}", e));
                        }
                    };

                let remote_share_records =
                    match merge_share_records(resource_id, &share_records, &self.repo_ctx).await {
                        Ok(records) => {
                            debug!(
                                resource_id = %resource_id,
                                merged_record_count = records.len(),
                                "Share records merged successfully"
                            );
                            records
                        }
                        Err(e) => {
                            error!(
                                error = %e,
                                resource_id = %resource_id,
                                "Failed to merge share records"
                            );
                            return Err(format!("Failed to merge share records: {}", e));
                        }
                    };

                let message = ResourceUpdateMsg::VectorClockResponse {
                    resource_id: resource_id.clone(),
                    update_clock,
                    add_clock,
                    share_records: remote_share_records,
                };

                match self.send_message(Message::MergeUpdate(message)).await {
                    Ok(_) => {
                        info!(
                            resource_id = %resource_id,
                            "Vector clock response sent successfully"
                        );
                    }
                    Err(e) => {
                        error!(
                            error = %e,
                            resource_id = %resource_id,
                            "Failed to send vector clock response"
                        );
                        return Err(format!("Failed to send vector clock response: {}", e));
                    }
                }
            }
            ResourceUpdateMsg::VectorClockResponse {
                resource_id,
                update_clock,
                add_clock,
                share_records,
            } => {
                debug!(
                    resource_id = %resource_id,
                    share_record_count = share_records.len(),
                    "Processing vector clock response"
                );

                match update_vector_clocks(add_clock, update_clock, &self.repo_ctx).await {
                    Ok(_) => {
                        debug!(
                            resource_id = %resource_id,
                            "Vector clocks updated successfully"
                        );
                    }
                    Err(e) => {
                        error!(
                            error = %e,
                            resource_id = %resource_id,
                            "Failed to update vector clocks"
                        );
                        return Err(format!("Failed to update vector clocks: {}", e));
                    }
                }

                match add_share_records(share_records, &self.repo_ctx).await {
                    Ok(_) => {
                        info!(
                            resource_id = %resource_id,
                            "Share records added successfully - sync complete"
                        );
                    }
                    Err(e) => {
                        error!(
                            error = %e,
                            resource_id = %resource_id,
                            "Failed to add share records"
                        );
                        return Err(format!("Failed to add share records: {}", e));
                    }
                }
            }
        }
        Ok(())
    }

    pub async fn send_first_user_connection_payload(&self, is_request: bool) -> Result<(), String> {
        // Only the initiator sends the UserConnection message
        let user = match self.get_local_user().await {
            Some(user) => user,
            None => return Err("Local user not found".into()),
        };
        let devices = get_my_user_devices(&user.id, &self.repo_ctx).await?;
        let message = if is_request {
            FirstUserExchange::Request(UserWithDevices { user, devices })
        } else {
            FirstUserExchange::Response(UserWithDevices { user, devices })
        };
        self.send_message(Message::FirstUserConnection(message))
            .await?;
        Ok(())
    }

    pub async fn process_first_connection_exchange(
        &self,
        payload: &FirstUserExchange,
    ) -> Result<(), String> {
        match payload {
            FirstUserExchange::Request(user_with_devices) => {
                self.repo_ctx
                    .user_repo
                    .add_users_with_devices_bulk(&[user_with_devices.clone()])
                    .await
                    .map_err(|e| e.to_string())?;
                self.send_first_user_connection_payload(false).await?;
            }
            FirstUserExchange::Response(user_with_devices) => {
                self.repo_ctx
                    .user_repo
                    .add_users_with_devices_bulk(&[user_with_devices.clone()])
                    .await
                    .map_err(|e| e.to_string())?;
                let user_manifest = get_user_manifest(&self.repo_ctx, &self.device.id).await?;

                self.send_message(Message::UserManifestPayload(UserManifestPayload::Request(
                    user_manifest,
                )))
                .await?;
            }
        }
        Ok(())
    }

    pub async fn start_user_network_sync(&self) -> Result<(), String> {
        if self.is_initiator {
            let user_manifest = get_user_manifest(&self.repo_ctx, &self.user.id).await?;
            self.send_message(Message::UserManifestPayload(UserManifestPayload::Request(
                user_manifest,
            )))
            .await?;
        }
        Ok(())
    }

    pub async fn process_user_manifest_payload(
        &self,
        payload: &UserManifestPayload,
    ) -> Result<(), String> {
        let user = match self.get_local_user().await {
            Some(user) => user,
            None => return Err("Local user not found".into()),
        };
        match payload {
            UserManifestPayload::Request(request_payload) => {
                let manifest_result =
                    process_user_manifest_request(request_payload, &self.repo_ctx, &user.id)
                        .await?;
                self.set_user_manifest_comparison_result(manifest_result.clone())
                    .await;
                self.send_message(Message::UserManifestPayload(UserManifestPayload::Response(
                    manifest_result,
                )))
                .await?;
            }
            UserManifestPayload::Response(manifest) => {
                let manifest_result = manifest.inverse();
                self.set_user_manifest_comparison_result(manifest_result)
                    .await;
                self.send_message(Message::UserManifestPayload(UserManifestPayload::Ack))
                    .await?;
            }
            UserManifestPayload::Ack => {
                let manifest = self.get_user_manifest_result().await?;
                let payload =
                    create_user_network_sync_payload(&manifest.remote_missing, &self.repo_ctx)
                        .await?;
                let message = Message::UserNetworkSync(payload);
                self.send_message(message).await?;
            }
        }
        Ok(())
    }

    pub async fn process_user_network_sync(
        &self,
        payload: &UserNetworkSyncPayload,
    ) -> Result<(), String> {
        if !self.is_initiator {
            let manifest = self.get_user_manifest_result().await?;
            let local_payload =
                create_user_network_sync_payload(&manifest.remote_missing, &self.repo_ctx).await?;
            let message = Message::UserNetworkSync(local_payload);
            self.send_message(message).await?;
        }

        process_user_network_sync_payload(payload, &self.repo_ctx).await?;
        self.send_message(Message::UserNetworkSyncAck).await?;
        Ok(())
    }
}
