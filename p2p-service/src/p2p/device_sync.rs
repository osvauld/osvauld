use crate::p2p::peer_connection::PeerConnection;

use osvauld_core::models::{
    DeviceManifestComparisonResult, DeviceManifestRequestPayload, DeviceNetworkSyncPayload, Message,
};
use osvauld_services::{
    create_device_network_sync_payload, get_device_manifest, process_device_manifest_request,
    process_device_network_sync,
};

use tracing::{debug, error, info, instrument};

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
        payload: &mut DeviceNetworkSyncPayload,
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
}
