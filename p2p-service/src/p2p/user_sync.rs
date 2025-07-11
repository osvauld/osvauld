use crate::p2p::peer_connection::PeerConnection;

use osvauld_core::models::{
    FirstUserExchange, Message, UserManifestPayload, UserNetworkSyncPayload, UserWithDevices,
};
use osvauld_services::{
    create_user_network_sync_payload, get_my_user_devices, get_user_manifest,
    process_user_manifest_request, process_user_network_sync_payload,
};

// Helper method signatures to reduce repeated patterns
impl PeerConnection {
    pub async fn send_first_user_connection_payload(&self, is_request: bool) -> Result<(), String> {
        // Only the initiator sends the UserConnection message
        let user = self.get_local_user().await?;
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
        payload: &mut FirstUserExchange,
    ) -> Result<(), String> {
        match payload {
            FirstUserExchange::Request(user_with_devices) => {
                user_with_devices.user.owner = false;
                user_with_devices.user.first_sync = true;
                self.repo_ctx
                    .user_repo
                    .add_users_with_devices_bulk(&[user_with_devices.clone()])
                    .await
                    .map_err(|e| e.to_string())?;
                self.send_first_user_connection_payload(false).await?;
            }
            FirstUserExchange::Response(user_with_devices) => {
                user_with_devices.user.owner = false;
                user_with_devices.user.first_sync = true;
                self.repo_ctx
                    .user_repo
                    .add_users_with_devices_bulk(&[user_with_devices.clone()])
                    .await
                    .map_err(|e| e.to_string())?;
                let user_manifest = get_user_manifest(&self.repo_ctx, &self.user.id).await?;

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
        match payload {
            UserManifestPayload::Request(request_payload) => {
                let manifest_result =
                    process_user_manifest_request(request_payload, &self.repo_ctx, &self.user.id)
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
        payload: &mut UserNetworkSyncPayload,
    ) -> Result<(), String> {
        if self.is_initiator {
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
