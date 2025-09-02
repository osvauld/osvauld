use crate::p2p::{errors::P2PResult, peer_connection::PeerConnection};

use osvauld_core::models::{
    DeviceManifestComparisonResult, DeviceManifestRequestPayload, DeviceNetworkSyncPayload, Message,
};
use services::{
    create_device_network_sync_payload, get_device_manifest, process_device_manifest_request,
    process_device_network_sync,
};

use tracing::{debug, info, instrument};

impl PeerConnection {
    #[instrument(skip(self), fields(
        connection_id = %self.get_id(),
    ), level = "info")]
    pub async fn start_add_device_process(&self) -> P2PResult<()> {
        info!("Starting add device process");
        debug!("Retrieving device manifest for user");

        let peer_user = self.get_peer_user().await;
        let manifest = get_device_manifest(self.repo_ctx.clone(), &peer_user.id).await?;

        debug!(
            manifest_resource_count = manifest.resources.len(),
            "Device manifest retrieved successfully"
        );

        self.send_message(Message::DeviceManifestRequest(manifest))
            .await?;

        info!("Device manifest request sent successfully");
        Ok(())
    }

    #[instrument(skip(self, payload), fields(
        connection_id = %self.get_id(),
        known_device_count = payload.known_device_ids.len(),
        other_users_count = payload.other_users.len(),
        folder_count = payload.folder_ids.len(),
        resource_count = payload.resources.len()
    ), level = "info")]
    pub async fn handle_manifest_request(
        &self,
        payload: &DeviceManifestRequestPayload,
    ) -> P2PResult<()> {
        info!("Processing device manifest request");
        debug!(
            "Processing manifest with {} known devices, {} other users, {} folders, {} resources",
            payload.known_device_ids.len(),
            payload.other_users.len(),
            payload.folder_ids.len(),
            payload.resources.len()
        );

        let peer_user = self.get_peer_user().await;
        let result =
            process_device_manifest_request(payload, self.repo_ctx.clone(), &peer_user.id).await?;

        debug!(
            local_missing_unknown_users = result.local_missing.unknown_users.len(),
            // ... other debug fields ...
            "Manifest comparison completed"
        );

        self.send_message(Message::DeviceManifestResponse(result.clone()))
            .await?;

        info!("Device manifest response sent successfully");
        debug!("Setting manifest comparison result");
        self.set_device_manifest_comparison_result(result).await;
        Ok(())
    }

    #[instrument(skip(self, payload), fields(
        connection_id = %self.get_id(),
    ), level = "info")]
    pub async fn handle_manifest_response(
        &self,
        payload: &DeviceManifestComparisonResult,
    ) -> P2PResult<()> {
        info!("Processing device manifest response");
        debug!(
            "Received manifest comparison with {} sync-required resources",
            payload.resources_requiring_sync.len()
        );

        let manifest_result = payload.inverse();
        self.set_device_manifest_comparison_result(manifest_result)
            .await;

        self.send_message(Message::DeviceManifestAck).await?;

        info!("Device manifest acknowledgment sent successfully");

        let manifest = self.get_device_manifest_result().await?;
        let device_network_payload =
            create_device_network_sync_payload(&manifest.remote_missing, self.repo_ctx.clone())
                .await?;

        debug!("Network sync payload created successfully");

        self.send_message(Message::DeviceNetworkSync(device_network_payload))
            .await?;

        info!("Device network sync message sent successfully");
        Ok(())
    }

    #[instrument(skip(self), fields(connection_id = %self.get_id()), level = "info")]
    pub async fn handle_manifest_ack(&self) -> P2PResult<()> {
        info!("Processing device manifest acknowledgment");

        let manifest = self.get_device_manifest_result().await?;

        let device_network_payload =
            create_device_network_sync_payload(&manifest.remote_missing, self.repo_ctx.clone())
                .await?;

        debug!("Network sync payload created successfully for ack response");

        self.send_message(Message::DeviceNetworkSync(device_network_payload))
            .await?;

        Ok(())
    }

    #[instrument(skip(self, payload), fields(
        connection_id = %self.get_id(),
    ), level = "info")]
    pub async fn handle_device_network_sync(
        &self,
        payload: &mut DeviceNetworkSyncPayload,
    ) -> P2PResult<()> {
        info!("Processing device network sync payload");
        debug!(
            "Processing network sync with {} users with devices, {} devices from common users, {} devices from current user, {} folders",
            payload.unknown_users_with_devices.len(),
            payload.unknown_devices_from_common_users.len(),
            payload.unknown_devices_from_current_user.len(),
            payload.unknown_folders.len()
        );

        process_device_network_sync(payload, self.repo_ctx.clone()).await?;

        info!("Network sync processed successfully");
        debug!("Sending network sync acknowledgment");

        self.send_message(Message::DeviceNetworkSyncAck).await?;

        info!("Device network sync acknowledgment sent successfully");
        Ok(())
    }
}
