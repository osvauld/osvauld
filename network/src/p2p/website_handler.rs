use crate::p2p::{errors::P2PResult, peer_connection::PeerConnection};
use base64::{engine::general_purpose, Engine as _};
use osvauld_core::models::{p2p::FolderTokenResponse, Message};
use serde_json::json;
use services::generate_folder_token;
use tracing::{error, info, instrument};

impl PeerConnection {
    /// Handle a folder token request from another peer
    /// This is called on the sovereign node side when a request is received
    #[instrument(skip(self), fields(connection_id = %self.get_id(), folder_id = %folder_id), level = "info")]
    pub async fn handle_folder_token_request(
        &self,
        folder_id: String,
        domain: String,
    ) -> P2PResult<()> {
        info!(
            "Processing folder token request for folder {} from connection {}",
            folder_id,
            self.get_id()
        );

        // Generate the folder token
        match generate_folder_token(
            &folder_id,
            &domain,
            &self.crypto_utils,
            self.repo_ctx.clone(),
        )
        .await
        {
            Ok((ucan_token, ucan_public_key)) => {
                info!(
                    "Successfully generated folder token for folder {}",
                    folder_id
                );

                // Get user and device information
                let user = self.get_local_user().await?;
                let device = self.get_local_device().await.unwrap();

                // Create connection details JSON
                let connection_details = json!({
                    "user_public_key": user.public_key,
                    "device_public_key": device.device_key,
                    "username": user.username,
                    "ucan_token": ucan_token,
                    "ucan_pub_key": ucan_public_key,
                });

                // Convert to string and base64 encode
                let connection_json = connection_details.to_string();
                let connection_string =
                    general_purpose::STANDARD.encode(connection_json.as_bytes());

                info!("Generated connection string for folder {}", folder_id);

                // Create response message
                let response = Message::FolderTokenResponse(FolderTokenResponse {
                    folder_id: folder_id.clone(),
                    connection_string,
                });

                // Send response back to requester
                match self.send_message(response).await {
                    Ok(_) => {
                        info!(
                            "Successfully sent folder token response for folder {}",
                            folder_id
                        );
                        Ok(())
                    }
                    Err(e) => {
                        error!(
                            "Failed to send folder token response for folder {}: {}",
                            folder_id, e
                        );
                        Err(e)
                    }
                }
            }
            Err(e) => {
                error!(
                    "Failed to generate folder token for folder {}: {}",
                    folder_id, e
                );
                // TODO: Send error response back to requester
                Err(e.into())
            }
        }
    }
}
