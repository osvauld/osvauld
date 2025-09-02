use crate::p2p::{
    errors::{P2PError, P2PResult, SyncError},
    peer_connection::PeerConnection,
};

use osvauld_core::models::{Message, UserManifestPayload, UserNetworkSyncPayload};
use services::{
    create_user_network_sync_payload, get_user_manifest, process_user_manifest_request,
    process_user_network_sync_payload,
};
use tracing::{debug, error, info, instrument};

impl PeerConnection {
    /// Initiates user network synchronization process
    #[instrument(skip(self), fields(
        connection_id = %self.get_id(),
        is_initiator = self.is_initiator
    ), level = "info")]
    pub async fn start_user_network_sync(&self) -> P2PResult<()> {
        if !self.is_initiator {
            debug!("Not initiator, skipping user network sync initiation");
            return Ok(());
        }

        info!("Starting user network sync as initiator");

        let peer_user = self.get_peer_user().await;
        let current_user = self.get_local_user().await?;

        debug!(
            "Getting user manifest for peer_user: {} and current_user: {}",
            peer_user.id, current_user.id
        );

        let user_manifest =
            get_user_manifest(self.repo_ctx.clone(), &peer_user.id, &current_user.id)
                .await
                .map_err(|e| {
                    error!("Failed to get user manifest: {}", e);
                    P2PError::Sync(SyncError::ManifestGenerationFailed {
                        reason: e.to_string(),
                    })
                })?;

        debug!("User manifest generated successfully, sending request");

        self.send_message(Message::UserManifestPayload(UserManifestPayload::Request(
            user_manifest,
        )))
        .await?;

        info!("User manifest request sent successfully");
        Ok(())
    }

    /// Processes incoming user manifest payloads
    #[instrument(skip(self, payload), fields(
        connection_id = %self.get_id(),
        payload_type = ?std::mem::discriminant(payload)
    ), level = "info")]
    pub async fn process_user_manifest_payload(
        &self,
        payload: &UserManifestPayload,
    ) -> P2PResult<()> {
        match payload {
            UserManifestPayload::Request(request_payload) => {
                info!("Processing user manifest request");

                let peer_user = self.get_peer_user().await;
                let current_user = self.get_local_user().await?;

                debug!(
                    "Processing manifest request from peer: {} for current user: {}",
                    peer_user.id, current_user.id
                );

                let manifest_result = process_user_manifest_request(
                    request_payload,
                    self.repo_ctx.clone(),
                    &peer_user.id,
                    &current_user.id,
                )
                .await
                .map_err(|e| {
                    error!("Failed to process user manifest request: {}", e);
                    P2PError::Sync(SyncError::ManifestComparisonFailed {
                        reason: e.to_string(),
                    })
                })?;

                debug!("Manifest comparison completed, storing result");
                self.set_user_manifest_comparison_result(manifest_result.clone())
                    .await;

                debug!("Sending manifest response");
                self.send_message(Message::UserManifestPayload(UserManifestPayload::Response(
                    manifest_result,
                )))
                .await?;

                info!("User manifest response sent successfully");
            }

            UserManifestPayload::Response(manifest) => {
                info!("Processing user manifest response");

                // Inverse the manifest to get our perspective
                let manifest_result = manifest.inverse();

                debug!("Storing inversed manifest result");
                self.set_user_manifest_comparison_result(manifest_result)
                    .await;

                debug!("Sending manifest acknowledgment");
                self.send_message(Message::UserManifestPayload(UserManifestPayload::Ack))
                    .await?;

                info!("User manifest acknowledgment sent");
            }

            UserManifestPayload::Ack => {
                info!("Processing user manifest acknowledgment");

                let manifest = self.get_user_manifest_result().await?;
                let peer_user = self.get_peer_user().await;

                debug!(
                    "Creating network sync payload for {} remote missing items",
                    manifest.remote_missing.unknown_resources.len()
                );

                let payload = create_user_network_sync_payload(
                    &manifest.remote_missing,
                    &peer_user,
                    self.repo_ctx.clone(),
                    &self.crypto_utils,
                    &self.domain,
                )
                .await
                .map_err(|e| {
                    error!("Failed to create user network sync payload: {}", e);
                    P2PError::Sync(SyncError::UserSyncFailed {
                        reason: e.to_string(),
                    })
                })?;

                debug!("Sending user network sync payload");
                self.send_message(Message::UserNetworkSync(payload)).await?;

                info!("User network sync initiated");
            }
        }

        Ok(())
    }

    /// Processes user network sync payload
    #[instrument(skip(self, payload), fields(
        connection_id = %self.get_id(),
        is_initiator = self.is_initiator
    ), level = "info")]
    pub async fn process_user_network_sync(
        &self,
        payload: &mut UserNetworkSyncPayload,
    ) -> P2PResult<()> {
        info!("Processing user network sync payload");

        // If we're the initiator, we need to send our own sync payload
        if self.is_initiator {
            debug!("As initiator, preparing our sync payload");

            let manifest = self.get_user_manifest_result().await?;
            let peer_user = self.get_peer_user().await;

            debug!(
                "Creating local sync payload for {} remote missing items",
                manifest.remote_missing.unknown_resources.len()
            );

            let local_payload = create_user_network_sync_payload(
                &manifest.remote_missing,
                &peer_user,
                self.repo_ctx.clone(),
                &self.crypto_utils,
                &self.domain,
            )
            .await
            .map_err(|e| {
                error!("Failed to create local user network sync payload: {}", e);
                P2PError::Sync(SyncError::UserSyncFailed {
                    reason: e.to_string(),
                })
            })?;

            debug!("Sending our user network sync payload");
            self.send_message(Message::UserNetworkSync(local_payload))
                .await?;
        }

        // Process the received payload
        debug!("Processing received user network sync payload");

        process_user_network_sync_payload(payload, self.repo_ctx.clone())
            .await
            .map_err(|e| {
                error!("Failed to process user network sync payload: {}", e);
                P2PError::Sync(SyncError::UserSyncFailed {
                    reason: e.to_string(),
                })
            })?;

        info!("User network sync payload processed successfully");

        // Send acknowledgment
        debug!("Sending user network sync acknowledgment");
        self.send_message(Message::UserNetworkSyncAck).await?;

        info!("User network sync completed");
        Ok(())
    }
}
