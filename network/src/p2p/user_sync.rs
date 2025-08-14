use crate::p2p::peer_connection::PeerConnection;

use osvauld_core::models::{Message, UserManifestPayload, UserNetworkSyncPayload, UserWithDevices};
use services::{
    create_user_network_sync_payload, get_user_manifest, process_user_manifest_request,
    process_user_network_sync_payload,
};

// Helper method signatures to reduce repeated patterns
impl PeerConnection {
    pub async fn start_user_network_sync(&self) -> Result<(), String> {
        if self.is_initiator {
            let peer_user = self.get_peer_user().await;
            let current_user = self.get_local_user().await?;
            let user_manifest =
                get_user_manifest(self.repo_ctx.clone(), &peer_user.id, &current_user.id).await?;
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
        match payload {
            UserManifestPayload::Request(request_payload) => {
                let peer_user = self.get_peer_user().await;
                let current_user = self.get_local_user().await?;
                let manifest_result = process_user_manifest_request(
                    request_payload,
                    self.repo_ctx.clone(),
                    &peer_user.id,
                    &current_user.id,
                )
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
                let peer_user = self.get_peer_user().await;
                let payload = create_user_network_sync_payload(
                    &manifest.remote_missing,
                    &peer_user,
                    self.repo_ctx.clone(),
                    &self.crypto_utils,
                    &self.domain,
                )
                .await?;
                let message = Message::UserNetworkSync(payload);
                self.send_message(message).await?;
            }
        }
        Ok(())
    }

    pub async fn process_user_network_sync(
        &self,
        payload: &mut UserNetworkSyncPayload,
    ) -> Result<(), String> {
        if self.is_initiator {
            let manifest = self.get_user_manifest_result().await?;

            let peer_user = self.get_peer_user().await;
            let local_payload = create_user_network_sync_payload(
                &manifest.remote_missing,
                &peer_user,
                self.repo_ctx.clone(),
                &self.crypto_utils,
                &self.domain,
            )
            .await?;
            let message = Message::UserNetworkSync(local_payload);
            self.send_message(message).await?;
        }

        process_user_network_sync_payload(payload, self.repo_ctx.clone()).await?;
        self.send_message(Message::UserNetworkSyncAck).await?;
        Ok(())
    }
}
