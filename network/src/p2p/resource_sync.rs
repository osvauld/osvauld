use crate::p2p::peer_connection::PeerConnection;

use super::P2PEvent;
use osvauld_core::models::{
    ConnectionType, LiveEditMessage, Message, ResourceKey, ResourceSyncData, ResourceUpdateMsg,
};
use services::{
    add_resource_sync, add_share_records, apply_updates, apply_updates_and_get_peer_updates,
    find_missing_resource_keys, generate_updates_for_peer, get_resource_for_remote_addition,
    get_resource_keys_for_resource, get_resource_state_vector, get_resource_ucan_key,
    get_share_records_for_resource, get_vector_clocks_for_resource, merge_share_records,
    merge_vector_clocks, update_vector_clocks, validate_authority_for_update,
};

use tracing::{debug, error, info, instrument};

// Helper method signatures to reduce repeated patterns
impl PeerConnection {
    #[instrument(skip(self), fields(
    connection_id = %self.get_id(),
), level = "info")]
    pub async fn send_resources(&self) -> Result<(), String> {
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

            let peer_device = self.get_peer_device().await;
            let resource_payload = match get_resource_for_remote_addition(
                resource_id,
                &peer_device,
                self.repo_ctx.clone(),
            )
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
                .send_message(Message::ResourceAdditionRequest(resource_payload))
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
        payload: &mut ResourceSyncData,
    ) -> Result<(), String> {
        info!("Processing resource addition request");
        debug!("Adding resource to local repository");
        let connection_type = self.get_connection_type().await;
        match add_resource_sync(payload, self.repo_ctx.clone(), &connection_type).await {
            Ok(_) => {
                info!(
                    resource_id = %payload.resource.id,
                    "Resource added successfully to local repository"
                );
                self.event_emitter.emit(P2PEvent::ResourceAdded {
                    resource_id: payload.resource.id.clone(),
                });
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
        let connection_type = self.get_connection_type().await;
        let is_empty = match connection_type {
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
            self.send_message(Message::ResourceAdditionComplete).await?;
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
                    let user = self.get_local_user().await?;

                    let state_vectors = match get_resource_state_vector(
                        resource_id,
                        &user.id,
                        self.repo_ctx.clone(),
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
                    let ucan_token =
                        get_resource_ucan_key(resource_id, &user.id, self.repo_ctx.clone())
                            .await
                            .map_err(|e| e.to_string())?;

                    let message = ResourceUpdateMsg::StateVectorRequest {
                        resource_id: resource_id.to_string(),
                        state_vectors,
                        ucan_token,
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
                let is_token_valid = validate_authority_for_update(
                    resource_id,
                    ucan_token,
                    &peer_user.id,
                    self.repo_ctx.clone(),
                    &self.domain,
                )
                .await
                .map_err(|e| e.to_string())?;
                if !is_token_valid {
                    error!("token is invalid");
                    return Err("update permission is missing".to_string());
                }
                info!("state vecotrs {:?}", state_vectors);

                let updates = match generate_updates_for_peer(
                    resource_id,
                    &user.id,
                    self.repo_ctx.clone(),
                    &self.crypto_utils,
                    &state_vectors,
                )
                .await
                {
                    Ok(updates) => {
                        debug!(
                            resource_id = %resource_id,
                            udpates = updates,
                            "Generated updates for peer successfully"
                        );
                        updates
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

                let ucan_token =
                    get_resource_ucan_key(resource_id, &user.id, self.repo_ctx.clone())
                        .await
                        .map_err(|e| e.to_string())?;
                let message = ResourceUpdateMsg::UpdatesResponse {
                    resource_id: resource_id.to_string(),
                    updates,
                    ucan_token,
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
                ucan_token,
            } => {
                debug!(
                    resource_id = %resource_id,
                    update_count = updates.len(),
                    "Processing updates response"
                );
                let user = self.get_local_user().await?;

                let peer_user = self.get_peer_user().await;
                let is_token_valid = validate_authority_for_update(
                    resource_id,
                    ucan_token,
                    &peer_user.id,
                    self.repo_ctx.clone(),
                    &self.domain,
                )
                .await
                .map_err(|e| e.to_string())?;
                if !is_token_valid {
                    error!("token is invalid");
                    return Err("update permission is missing".to_string());
                }
                let remote_updates = match apply_updates_and_get_peer_updates(
                    resource_id,
                    &user.id,
                    updates,
                    self.repo_ctx.clone(),
                    &self.crypto_utils,
                )
                .await
                {
                    Ok(updates) => {
                        debug!(
                            resource_id = %resource_id,
                            remote_update_count = updates.len(),
                            "Applied updates and generated peer updates"
                        );
                        updates
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

                let share_records = match get_share_records_for_resource(
                    resource_id,
                    self.repo_ctx.clone(),
                )
                .await
                {
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

                let vector_clocks = match get_vector_clocks_for_resource(
                    resource_id,
                    self.repo_ctx.clone(),
                )
                .await
                {
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

                let resource_keys = match get_resource_keys_for_resource(
                    resource_id,
                    self.repo_ctx.clone(),
                )
                .await
                {
                    Ok(keys) => {
                        debug!(
                            resource_id = %resource_id,
                            "Retrieved resource keys for resource"
                        );
                        keys
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
                    resource_keys,
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
                let peer_device = self.get_peer_device().await;
                let client_id = peer_device.get_client_id().map_err(|e| e.to_string())?;

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

                let user = self.get_local_user().await?;
                apply_updates(
                    resource_id,
                    updates,
                    &user.id,
                    self.repo_ctx.clone(),
                    &self.crypto_utils,
                )
                .await?;
                let peer_device = self.get_peer_device().await;
                let client_id = peer_device.get_client_id().map_err(|e| e.to_string())?;
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
                let local_resource_keys =
                    get_resource_keys_for_resource(resource_id, self.repo_ctx.clone()).await?;
                let (local_missing, remote_missing) =
                    find_missing_resource_keys(&local_resource_keys, resource_keys);
                self.repo_ctx
                    .resource_key_repo
                    .add_resource_keys(&local_missing)
                    .await
                    .map_err(|e| e.to_string())?;

                let (add_clock, update_clock) =
                    match merge_vector_clocks(resource_id, vector_clocks, self.repo_ctx.clone())
                        .await
                    {
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
                    match merge_share_records(resource_id, &share_records, self.repo_ctx.clone())
                        .await
                    {
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
                    resource_keys: remote_missing,
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
                resource_keys,
            } => {
                debug!(
                    resource_id = %resource_id,
                    share_record_count = share_records.len(),
                    "Processing vector clock response"
                );

                match update_vector_clocks(add_clock, update_clock, self.repo_ctx.clone()).await {
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

                match add_share_records(share_records, self.repo_ctx.clone()).await {
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
                self.repo_ctx
                    .resource_key_repo
                    .add_resource_keys(resource_keys)
                    .await
                    .map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }
    #[instrument(skip(self, message), fields(message_type = ?std::mem::discriminant(message)), level = "debug")]
    pub async fn handle_live_edit_flow(&self, message: &LiveEditMessage) -> Result<(), String> {
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
                let client_id = peer_device.get_client_id().map_err(|e| e.to_string())?;
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
                let client_id = peer_device.get_client_id().map_err(|e| e.to_string())?;
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
