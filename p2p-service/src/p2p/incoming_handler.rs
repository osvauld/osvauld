use crate::p2p::P2PService;
use crate::p2p::incoming::IncomingEvent;
use osvauld_core::models::p2p::Message;
use osvauld_core::models::vector_clock::ResourceVectorClock;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn, instrument};

/// Implementation of P2PService methods for handling incoming events and event processing
impl P2PService {
    /// Starts processing incoming events in a background task
    #[instrument(skip_all, level = "debug")]
    pub fn start_processing_incoming_events(
        service: Self, 
        mut receiver: mpsc::UnboundedReceiver<IncomingEvent>
    ) {
        // Spawn a task to process events
        tokio::spawn(async move {
            info!("Started processing incoming events");
            
            while let Some(event) = receiver.recv().await {
                match event {
                    IncomingEvent::MergeComplete { 
                        encrypted_doc, 
                        vector_clock, 
                        resource_id,
                        user_id,
                        device_id,
                    } => {
                        service.handle_merge_complete(
                            encrypted_doc, 
                            vector_clock, 
                            &resource_id,
                            &user_id,
                            &device_id,
                        ).await;
                    },
                    IncomingEvent::SyncUpdate { payload } => {
                        service.handle_sync_update(payload).await;
                    },
                    
                }
            }
            
            info!("Stopped processing incoming events");
        });
    }
    
    /// Handles a merge complete event
    #[instrument(skip(self, vector_clock, encrypted_doc), 
                fields(resource_id = %resource_id, user_id = %user_id, device_id = %device_id),
                level = "info")]
    pub async fn handle_merge_complete(
        &self,
        encrypted_doc: String,
        vector_clock: Vec<ResourceVectorClock>,
        resource_id: &str,
        user_id: &str,
        device_id: &str,
    ) {
        info!("Processing merge-complete event for resource: {}", resource_id);
        
        let connection_id = format!("{}:{}", user_id, device_id);
        
        match self.get_connection_by_id(&connection_id).await {
            Ok(connection) => {
                info!("Found connection for {}", connection_id);
                // Use the connection to send an update
                // Implementation for sending updates will go here in the future
            }
            Err(e) => {
                error!("Connection not found for {}: {}", connection_id, e);
            }
        }
    }
    
    /// Handles a sync update event
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
        //  match self.get_connection_by_id(&connection_id).await {
        //     Ok(connection) => {
        //         info!("Found connection for {}", connection_id);
        //         // Use the connection to send an update
        //         // Implementation for sending updates will go here in the future
        //     }
        //     Err(e) => {
        //         error!("Connection not found for {}: {}", connection_id, e);
        //     }
        // }
        // Send the message to all connections
        
        info!("Sync update processed and forwarded to all connections");
    }
    
}
