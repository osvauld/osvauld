use crate::p2p::incoming::IncomingEvent;
use crate::p2p::P2PService;
use osvauld_core::models::p2p::Message;
use tokio::sync::mpsc;
use tracing::{error, info, instrument, warn};

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
                    IncomingEvent::SyncUpdate { payload } => {
                        service.handle_sync_update(payload).await;
                    }
                }
            }

            info!("Stopped processing incoming events");
        });
    }

    #[instrument(skip(self, payload), level = "info")]
    pub async fn handle_sync_update(&self, payload: String) {
        info!("Processing sync-update event");

        // Get the current state

        // let connection_id = format!("{}:{}", user_id, device_id);

        // Create a message for the update
        let message = Message::SyncEvent {
            event: "sync-update".to_string(),
            payload: payload.clone(),
        };
        // Remove the ? operator since this function returns ()
        if let Err(e) = self.send_sync_update(message).await {
            error!("Failed to send sync update: {}", e);
        }

        info!("Sync update processed and forwarded to all connections");
    }
}
