use crate::p2p::peer_connection::PeerConnection;

use super::P2PEvent;
use osvauld_core::models::{
    ConnectionType, LiveEditMessage, Message, ResourceSyncData, ResourceUpdateMsg,
};
use osvauld_services::{
    add_resource_sync, add_share_records, apply_updates_and_get_peer_updates,
    generate_updates_for_peer, get_resource_for_remote_addition, get_resource_state_vector,
    get_share_records_for_resource, get_vector_clocks_for_resource, merge_share_records,
    merge_vector_clocks, update_vector_clocks,
};

use tracing::{debug, error, info, instrument};

// Helper method signatures to reduce repeated patterns
impl PeerConnection {
    #[instrument(skip(self), fields(
    connection_id = %self.get_id(),
    device_id = %self.device.id,
    connection_type = ?self.connection_type
), level = "info")]
    pub async fn send_resources(&self) -> Result<(), String> {
        info!("Starting resource transmission process");
        let connection_type = match self.connection_type.clone() {
            Some(ct) => ct,
            None => return Err("connection type is none".to_string()),
        };
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
        let connection_type = match self.connection_type.clone() {
            Some(ct) => ct,
            None => return Err("connection type is none".to_string()),
        };
        match add_resource_sync(payload, &self.repo_ctx, &connection_type).await {
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
        let connection_type = match self.connection_type.clone() {
            Some(ct) => ct,
            None => return Err("connection type is none".to_string()),
        };
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

            let connection_type = match self.connection_type.clone() {
                Some(ct) => ct,
                None => return Err("connection type is none".to_string()),
            };
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

                    let state_vector = match get_resource_state_vector(
                        resource_id,
                        &user.id,
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

        let user = self.get_local_user().await?;
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
                    &user.id,
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
                let user = self.get_local_user().await?;

                let (remote_updates, _) = match apply_updates_and_get_peer_updates(
                    resource_id,
                    &user.id,
                    updates,
                    state_vector,
                    &self.repo_ctx,
                    &self.crypto_utils,
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
                state_vector,
            } => {
                self.event_emitter.emit(P2PEvent::UpdateRequest {
                    resource_id: resource_id.clone(),
                    connection_id: self.get_id(),
                    state_vector: state_vector.clone(),
                    current_user_id: user.id.clone(),
                });
                Ok(())
            }
            LiveEditMessage::UpdateExchange {
                resource_id,
                updates,
                buffer,
                state_vector,
            } => {
                let connection_id = self.get_id();
                self.event_emitter.emit(P2PEvent::ProcessUpdate {
                    resource_id: resource_id.clone(),
                    connection_id,
                    state_vector: state_vector.clone(),
                    updates: updates.clone(),
                    buffer: buffer.clone(),
                });
                Ok(())
            }
            LiveEditMessage::UpdateExchangeResponse {
                resource_id,
                updates,
                state_vector: _,
            } => {
                let connection_id = self.get_id();
                self.event_emitter.emit(P2PEvent::ProcessUpdateResponse {
                    resource_id: resource_id.clone(),
                    connection_id,
                    updates: updates.clone(),
                });
                Ok(())
            }
            LiveEditMessage::CurrentBufferExchange {
                resource_id,
                buffer,
            } => {
                let connection_id = self.get_id();
                self.event_emitter.emit(P2PEvent::CurrentBufferExchange {
                    resource_id: resource_id.clone(),
                    connection_id,
                    updates: buffer.clone(),
                });
                Ok(())
            }
            LiveEditMessage::NotSameDocument => Ok(()),
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
            } => {
                self.event_emitter.emit(P2PEvent::EditingEvent {
                    updates: updates.to_vec(),
                    resource_id: resource_id.to_string(),
                    client_id: client_id.clone(),
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
