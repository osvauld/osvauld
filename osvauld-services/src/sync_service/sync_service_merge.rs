use osvauld_core::models::document::{generate_updates_for_peer, get_state_vector};
use osvauld_core::models::p2p::{ ResourceUpdateMsg, SyncPayload};
use osvauld_core::repositories::RepositoryError;
use tracing::{debug, error, info, instrument, warn, Span};

use super::sync_service_core::SyncService;

// Implement methods related to resource merge updates on SyncService
impl SyncService {
    /// Central method to process resource merge messages
    /// Dispatches to specific handlers based on the message type
    /// 
    /// # Arguments
    /// * `message` - The ResourceUpdateMsg to process
    /// * `current_span` - The current tracing span
    /// 
    /// # Returns
    /// * `Result<Message, RepositoryError>` - Response message or error
    #[instrument(
        skip(self, message, current_span), 
        fields(
            message_type = ?std::mem::discriminant(message)
        ),
        level = "info"
    )]
    pub async fn process_resource_merge_message(
        &self,
        message: &ResourceUpdateMsg,
        current_span: Span,
    ) -> Result<ResourceUpdateMsg, RepositoryError> {
        let _guard = current_span.enter();
        
        info!("Processing resource merge message");
        
        match message {
            ResourceUpdateMsg::StateVectorRequest{resource_id, state_vector} => {
                debug!("Handling state vector request");
                self.handle_state_vector_request(resource_id, state_vector).await
            },
            ResourceUpdateMsg::UpdatesResponse {resource_id,updates, state_vector } => {
                debug!("Handling updates response");
                self.handle_update_and_return_update(resource_id, updates, state_vector).await
            },
            ResourceUpdateMsg::FinalUpdates { .. } => {
                debug!("Handling final updates");
                todo!("Implementation for final updates pending")
            },
            ResourceUpdateMsg::SyncComplete { .. } => {
                debug!("Handling sync complete");
                todo!("Implementation for sync complete pending")
            }
        }
    }

    /// Handle an incoming state vector request from a remote peer
    /// 
    /// # Arguments
    /// * `request` - The state vector request containing resource ID and remote state vector
    /// 
    /// # Returns
    /// * `Result<Message, RepositoryError>` - Response message with updates or error
    #[instrument(
        skip(self), 
        fields(
            state_vector = %state_vector.len()
        ),
        level = "info"
    )]
    async fn handle_state_vector_request(
        &self,
        resource_id: &str,
        state_vector: &Vec<u8>
    ) -> Result<ResourceUpdateMsg, RepositoryError> {
            let updates = match self.resource_service.generate_updates_for_peer(
                resource_id, 
                state_vector
            ).await {
                Ok(updates) => {
                    debug!(
                        updates_size = updates.len(),
                        "Successfully generated updates for peer"
                    );
                    updates
                },
                Err(e) => {
                    error!(
                        error = %e,
                        "Failed to generate updates for peer"
                    );
                    return Err(e);
                }
            };
            
            // Get our current state vector to send back
            debug!("Getting our current state vector");
            let our_state_vector = match self.resource_service.get_resource_state_vector(
                resource_id
            ).await {
                Ok(sv) => {
                    debug!(
                        state_vector_size = sv.len(),
                        "Successfully retrieved our state vector"
                    );
                    sv
                },
                Err(e) => {
                    error!(
                        error = %e,
                        "Failed to get our state vector"
                    );
                    return Err(e);
                }
            };
            
            // Construct the response with updates and our state vector
            let response = ResourceUpdateMsg::UpdatesResponse {
                resource_id: resource_id.to_string(),
                updates: updates.clone(),
                state_vector: our_state_vector,
            };
            
            info!(
                updates_size = updates.len(),
                "Sending updates response"
            );
            
            Ok(response)
    }

    #[instrument(
        skip(self), 
        fields(
            resource_id = %resource_id
        ),
        level = "debug"
    )]
    pub async fn get_resource_for_update(
        &self,
        resource_id: &str,
    ) -> Result<SyncPayload, RepositoryError> {
        debug!("Getting resource for update using state vector approach");
        
        // Get our current state vector for the resource
        let resource_state_vector = match self.resource_service.get_resource_state_vector(resource_id).await {
            Ok(sv) => {
                info!(
                    state_vector_size = sv.len(),
                    "Retrieved state vector for resource"
                );
                sv
            },
            Err(e) => {
                error!(
                    error = %e,
                    resource_id = %resource_id,
                    "Failed to retrieve resource state vector"
                );
                return Err(e);
            }
        };
        
        debug!("Prepared resource update payload with state vector");
        
        // Create the state vector request message
        let payload = ResourceUpdateMsg::StateVectorRequest { 
            resource_id: resource_id.to_string(), 
            state_vector: resource_state_vector 
        };
        
        Ok(SyncPayload::ResourceMerge(payload))
    }

/// Handle updates received from a peer and generate any additional updates to return
/// 
/// # Arguments
/// * `resource_id` - The ID of the resource being updated
/// * `updates` - The updates received from the peer
/// * `state_vector` - The state vector from the peer
/// 
/// # Returns
/// * `Result<ResourceUpdateMsg, RepositoryError>` - Response message with additional updates or completion
#[instrument(
    skip(self, updates, state_vector), 
    fields(
        resource_id = %resource_id,
        updates_size = updates.len(),
        state_vector_size = state_vector.len()
    ),
    level = "info"
)]
async fn handle_update_and_return_update(
    &self,
    resource_id: &str,
    updates: &Vec<u8>,
    state_vector: &Vec<u8>,
) -> Result<ResourceUpdateMsg, RepositoryError> {
    info!("Processing updates from peer and generating response");
    
    // Apply the received updates and get any updates to send back
    match self.resource_service.apply_updates_and_get_peer_updates(
        resource_id, 
        updates, 
        state_vector
    ).await {
        Ok(peer_updates) => {
            debug!(
                updates_size = peer_updates.len(),
                "Generated updates for peer"
            );
            
            // Get vector clocks for the resource to include in the final update
            let vector_clocks = match self.vector_clock_repository.get_vector_clocks_for_resource(resource_id).await {
                Ok(clocks) => {
                    debug!(
                        clock_count = clocks.len(),
                        "Retrieved vector clocks for resource"
                    );
                    clocks
                },
                Err(e) => {
                    error!(
                        error = %e,
                        "Failed to retrieve vector clocks for resource"
                    );
                    return Err(e);
                }
            };
            
            // Always send FinalUpdates, even if the updates are empty
            // This ensures the peer gets the latest vector clocks
            Ok(ResourceUpdateMsg::FinalUpdates { 
                resource_id: resource_id.to_string(),
                updates: peer_updates,
                vector_clocks,
            })
        },
        Err(e) => {
            error!(
                error = %e,
                resource_id = %resource_id,
                "Failed to apply updates and generate response"
            );
            Err(e)
        }
    }
}
}
