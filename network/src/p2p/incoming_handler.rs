use crate::p2p::incoming::IncomingEvent;
use crate::p2p::P2PService;
use osvauld_core::models::Message;
use tokio::sync::mpsc;
use tracing::{error, info, instrument};

/// Implementation of P2PService methods for handling incoming events and event processing
impl P2PService {
    /// Starts processing incoming events in a background task
    #[instrument(skip_all, level = "debug")]
    pub fn start_processing_incoming_events(
        service: Self,
        mut receiver: mpsc::UnboundedReceiver<IncomingEvent>,
    ) {
        // Spawn a task to process events
        tokio::spawn(async move {
            info!("Started processing incoming events");

            while let Some(event) = receiver.recv().await {
                match event {
                    IncomingEvent::RequestFolderToken {
                        folder_id,
                        device_id,
                        domain,
                    } => {
                        service
                            .handle_request_folder_token(folder_id, device_id, domain)
                            .await;
                    }
                }
            }

            info!("Stopped processing incoming events");
        });
    }
    // TODO: FolderTokenRequest message type doesn't exist yet - re-enable after implementation
    #[allow(dead_code)]
    pub async fn handle_request_folder_token(
        &self,
        _folder_id: String,
        _device_id: String,
        _domain: String,
    ) {
        // info!(
        //     "Handling folder token request for folder {} to device {}",
        //     folder_id, device_id
        // );

        // // Create the FolderTokenRequest message
        // let message = Message::FolderTokenRequest(osvauld_core::models::p2p::FolderTokenRequest {
        //     folder_id: folder_id.clone(),
        //     domain,
        // });

        // // TODO: Update send_or_reconnect to not use ConnectionAction
        // // Send the message to the peer connection
        // match self.send_or_reconnect(&device_id, message).await {
        //     Ok(_) => {
        //         info!(
        //             "Successfully sent folder token request for folder {}",
        //             folder_id
        //         );
        //     }
        //     Err(e) => {
        //         error!(
        //             "Failed to send folder token request for folder {}: {}",
        //             folder_id, e
        //         );
        //     }
        // }
    }
}
