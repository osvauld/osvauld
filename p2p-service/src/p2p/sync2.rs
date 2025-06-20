use crate::p2p::peer_connection::PeerConnection;

use super::P2PEvent;
use osvauld_core::models::{
    DeviceManifestRequestPayload, DeviceNetworkSyncPayload, ManifestComparisonResult, Message,
    ResourceSyncData, ResourceUpdateMsg,
};
use osvauld_services::{
    add_resource_sync, add_share_records, apply_updates_and_get_peer_updates,
    create_network_sync_payload, generate_updates_for_peer, get_resource_for_remote_addition,
    get_resource_state_vector, get_share_records_for_resource, get_vector_clocks_for_resource,
    merge_share_records, merge_vector_clocks, process_device_manifest_request,
    process_network_sync, update_vector_clocks,
};

use tracing::{debug, error, info, instrument, Span};

// Helper method signatures to reduce repeated patterns
impl PeerConnection {
    pub async fn handle_manifest_request(
        &self,
        payload: &DeviceManifestRequestPayload,
    ) -> Result<(), String> {
        let result =
            process_device_manifest_request(payload, &self.repo_ctx, &self.user.id).await?;
        self.send_message(Message::DeviceManifestResponse(result.clone()))
            .await?;
        self.set_manifest_comparison_result(result).await;
        Ok(())
    }
    pub async fn handle_manifest_response(
        &self,
        payload: &ManifestComparisonResult,
    ) -> Result<(), String> {
        let manifest_result = payload.inverse();
        self.set_manifest_comparison_result(manifest_result).await;
        self.send_message(Message::DeviceManifestAck).await?;
        let manifest_result = self.manifest_result.lock().await;
        if let Some(ref manifest_diff) = *manifest_result {
            let device_network_payload =
                create_network_sync_payload(&manifest_diff.local_missing, &self.repo_ctx).await?;
            self.send_message(Message::DeviceNetworkSync(device_network_payload))
                .await?;
        }

        Ok(())
    }

    pub async fn handle_manifest_ack(&self) -> Result<(), String> {
        let manifest_result = self.manifest_result.lock().await;
        if let Some(ref manifest_diff) = *manifest_result {
            let device_network_payload =
                create_network_sync_payload(&manifest_diff.local_missing, &self.repo_ctx).await?;
            self.send_message(Message::DeviceNetworkSync(device_network_payload))
                .await?;
        }
        Ok(())
    }

    pub async fn handle_device_network_sync(
        &self,
        payload: &DeviceNetworkSyncPayload,
    ) -> Result<(), String> {
        process_network_sync(payload, &self.repo_ctx).await?;
        self.send_message(Message::DeviceNetworkSyncAck).await?;
        Ok(())
    }

    pub async fn send_resources(&self) -> Result<(), String> {
        let device = self.device.clone();
        let manifest_result = self.manifest_result.lock().await;

        if let Some(ref manifest_diff) = *manifest_result {
            // Collect resource IDs to avoid borrowing issues
            let resource_ids: Vec<_> = manifest_diff.remote_missing.unknown_resources.clone();
            if manifest_diff.local_missing.unknown_resources.is_empty() {
                self.send_message(Message::ResourceAddtionComplete).await?;
            }

            drop(manifest_result);

            for resource_id in resource_ids.iter() {
                let resource_payload =
                    get_resource_for_remote_addition(resource_id, &device, &self.repo_ctx).await?;
                self.send_message(Message::ResourceAddtionRequest(resource_payload))
                    .await?;
            }
        }
        Ok(())
    }
    pub async fn process_resource_addition_request(
        &self,
        payload: &ResourceSyncData,
    ) -> Result<(), String> {
        add_resource_sync(payload, &self.repo_ctx).await?;
        let mut manifest_result = self.manifest_result.lock().await;
        if let Some(ref mut manifest_comparison) = *manifest_result {
            // Remove the resource_id from remote_missing.unknown_resources
            manifest_comparison
                .local_missing
                .unknown_resources
                .retain(|id| id.to_string() != payload.resource.id);
            if manifest_comparison
                .local_missing
                .unknown_resources
                .is_empty()
            {
                self.send_message(Message::ResourceAddtionComplete).await?;
            }
        }
        Ok(())
    }
    pub async fn process_resource_addition_complete(&self) -> Result<(), String> {
        if self.is_initiator {
            let mut manifest_result = self.manifest_result.lock().await;
            if let Some(ref mut manifest_comparison) = *manifest_result {
                let resource_ids: Vec<_> = manifest_comparison.resources_requiring_sync.clone();
                if !resource_ids.is_empty() {
                    for resource_id in resource_ids {
                        let state_vector = get_resource_state_vector(
                            &resource_id,
                            &self.user.id,
                            &self.repo_ctx,
                            &self.crypto_utils,
                        )
                        .await
                        .map_err(|e| e.to_string())?;
                        let message = ResourceUpdateMsg::StateVectorRequest {
                            resource_id,
                            state_vector,
                        };
                        self.send_message(Message::MergeUpdate(message)).await?;
                    }
                }
            }
        }
        Ok(())
    }

    pub async fn process_resource_update_message(
        &self,
        payload: &ResourceUpdateMsg,
    ) -> Result<(), String> {
        match payload {
            ResourceUpdateMsg::StateVectorRequest {
                resource_id,
                state_vector,
            } => {
                let (updates, state_vector) = generate_updates_for_peer(
                    resource_id,
                    &self.user.id,
                    &self.repo_ctx,
                    &self.crypto_utils,
                    &state_vector,
                )
                .await
                .map_err(|e| e.to_string())?;
                let message = ResourceUpdateMsg::UpdatesResponse {
                    resource_id: resource_id.to_string(),
                    updates,
                    state_vector,
                };
                self.send_message(Message::MergeUpdate(message)).await?;
            }
            ResourceUpdateMsg::UpdatesResponse {
                resource_id,
                updates,
                state_vector,
            } => {
                let (remote_updates, _) = apply_updates_and_get_peer_updates(
                    resource_id,
                    &self.user.id,
                    &self.repo_ctx,
                    &self.crypto_utils,
                    updates,
                    state_vector,
                )
                .await
                .map_err(|e| e.to_string())?;
                let share_records = get_share_records_for_resource(resource_id, &self.repo_ctx)
                    .await
                    .map_err(|e| e.to_string())?;
                let vector_clocks = get_vector_clocks_for_resource(resource_id, &self.repo_ctx)
                    .await
                    .map_err(|e| e.to_string())?;
                let message = ResourceUpdateMsg::FinalUpdateMerge {
                    resource_id: resource_id.clone(),
                    updates: remote_updates,
                    vector_clocks,
                    share_records,
                };
                self.send_message(Message::MergeUpdate(message)).await?;

                self.event_emitter.emit(P2PEvent::UpdatesEvent {
                    resource_id: resource_id.clone(),
                    updates: updates.clone(),
                });
            }
            ResourceUpdateMsg::FinalUpdateMerge {
                resource_id,
                updates,
                vector_clocks,
                share_records,
            } => {
                self.event_emitter.emit(P2PEvent::UpdatesEvent {
                    resource_id: resource_id.clone(),
                    updates: updates.clone(),
                });
                let (add_clock, update_clock) =
                    merge_vector_clocks(resource_id, vector_clocks, &self.repo_ctx)
                        .await
                        .map_err(|e| e.to_string())?;
                let remote_share_records =
                    merge_share_records(resource_id, &share_records, &self.repo_ctx)
                        .await
                        .map_err(|e| e.to_string())?;
                let message = ResourceUpdateMsg::VectorClockResponse {
                    resource_id: resource_id.clone(),
                    update_clock,
                    add_clock,
                    share_records: remote_share_records,
                };
                self.send_message(Message::MergeUpdate(message)).await?;
            }
            ResourceUpdateMsg::VectorClockResponse {
                resource_id,
                update_clock,
                add_clock,
                share_records,
            } => {
                update_vector_clocks(add_clock, update_clock, &self.repo_ctx)
                    .await
                    .map_err(|e| e.to_string())?;
                add_share_records(share_records, &self.repo_ctx)
                    .await
                    .map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }
}
