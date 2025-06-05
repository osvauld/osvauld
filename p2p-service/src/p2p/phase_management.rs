use crate::p2p::emitter::P2PEvent;
use crate::p2p::peer_connection::PeerConnection;
use osvauld_core::models::p2p::{DisconnectStatus, Message, Phase, PhaseAction, PhaseType};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{Level, debug, error, info, instrument, span, trace, warn};

// PhaseState struct to encapsulate phase state
#[derive(Clone)]
pub struct PhaseState {
    pub current_phase: Arc<Mutex<PhaseType>>,
    pub local_phase_complete: Arc<Mutex<bool>>,
    pub remote_phase_complete: Arc<Mutex<bool>>,
}

impl PhaseState {
    #[instrument(level = "debug")]
    pub fn new() -> Self {
        debug!("Creating new PhaseState with initial phase DeviceSync");
        Self {
            current_phase: Arc::new(Mutex::new(PhaseType::DeviceSync)),
            local_phase_complete: Arc::new(Mutex::new(false)),
            remote_phase_complete: Arc::new(Mutex::new(false)),
        }
    }

    #[instrument(skip(self), level = "debug")]
    pub async fn set_remote_complete(&self, value: bool) {
        let mut remote_complete = self.remote_phase_complete.lock().await;
        let current_phase = self.get_current_phase().await;
        debug!(phase = ?current_phase, previous_value = *remote_complete, new_value = value, "Setting remote phase completion state");
        *remote_complete = value;
    }

    #[instrument(skip(self), level = "debug")]
    pub async fn set_local_complete(&self, value: bool) {
        let mut local_complete = self.local_phase_complete.lock().await;
        let current_phase = self.get_current_phase().await;
        debug!(phase = ?current_phase, previous_value = *local_complete, new_value = value, "Setting local phase completion state");
        *local_complete = value;
    }

    #[instrument(skip(self), level = "trace")]
    pub async fn get_current_phase(&self) -> PhaseType {
        let phase = self.current_phase.lock().await;
        trace!(current_phase = ?*phase, "Retrieved current phase");
        phase.clone()
    }

    #[instrument(skip(self), level = "debug")]
    pub async fn get_next_phase(&self) -> PhaseType {
        let current = self.current_phase.lock().await;
        let next_phase = match *current {
            PhaseType::AddDevice => PhaseType::DeviceSync,
            PhaseType::UserSync => PhaseType::DeviceSync,
            PhaseType::FirstUserConnection => PhaseType::DeviceSync,
            PhaseType::DeviceSync => PhaseType::FolderSync,
            // PhaseType::UserSync => PhaseType::FolderSync,
            PhaseType::FolderSync => PhaseType::ResourceSync,
            PhaseType::ResourceSync => PhaseType::ShareSync,
            PhaseType::ShareSync => PhaseType::UpdateSync,
            PhaseType::UpdateSync => PhaseType::DeviceRecordSync,
            PhaseType::DeviceRecordSync => PhaseType::Complete,
            PhaseType::Complete => PhaseType::Complete,
        };

        debug!(current_phase = ?*current, next_phase = ?next_phase, "Determined next phase");
        next_phase
    }

    #[instrument(skip(self), fields(new_phase = ?new_phase), level = "info")]
    pub async fn reset_for_new_phase(&self, new_phase: PhaseType) {
        let mut phase = self.current_phase.lock().await;
        // Use clone() since PhaseType doesn't implement Copy
        let old_phase = phase.clone();

        info!(
            old_phase = ?old_phase,
            new_phase = ?new_phase,
            "Resetting phase state for new phase"
        );

        *phase = new_phase;

        // Drop the lock before calling other async methods
        drop(phase);

        debug!("Setting local and remote completion to false for new phase");
        self.set_local_complete(false).await;
        self.set_remote_complete(false).await;
    }
}

// Implement phase-related methods for PeerConnection
impl PeerConnection {
    // Handler for phase-related messages
    #[instrument(
        skip(self, phase), 
        fields(
            phase_action = ?phase.action, 
            phase_type = ?phase.phase_type
        ), 
        level = "info"
    )]
    pub async fn handle_phase_message(&self, phase: &Phase) -> Result<(), String> {
        match phase.action {
            PhaseAction::Init => {
                info!(
                    "Received sync initialization for phase: {:?}",
                    phase.phase_type
                );

                // Reset our phase state to match
                debug!("Resetting phase state to match received phase");
                self.phase
                    .reset_for_new_phase(phase.phase_type.clone())
                    .await;
                self.send_message(Message::Phase(Phase {
                    action: PhaseAction::Ack,
                    phase_type: phase.phase_type.clone(),
                }))
                .await?;

                info!("Phase initialization handled successfully");
                Ok(())
            }

            PhaseAction::Complete => {
                info!("Received PhaseComplete for phase: {:?}", phase.phase_type);

                let current_phase = self.phase.get_current_phase().await;
                if phase.phase_type == current_phase {
                    debug!("Setting remote phase as complete");
                    self.phase.set_remote_complete(true).await;

                    // Send acknowledgment
                    debug!("Sending Phase Ack message");
                    self.send_message(Message::Phase(Phase {
                        action: PhaseAction::CompleteAck,
                        phase_type: phase.phase_type.clone(),
                    }))
                    .await?;

                    if self.is_initiator {
                        let next_phase = self.phase.get_next_phase().await;
                        self.phase.reset_for_new_phase(next_phase.clone()).await;
                        let message = Message::Phase(Phase {
                            action: PhaseAction::Init,
                            phase_type: next_phase,
                        });
                        self.send_message(message).await?;
                    } else {
                        if let Err(e) = self.start_phase_sync().await {
                            error!("Failed to start phase sync after PhaseComplete: {}", e);
                            return Err(e);
                        }
                    }
                } else {
                    warn!(
                        "Received PhaseComplete for wrong phase. Current: {:?}, Received: {:?}",
                        current_phase, phase.phase_type
                    );
                }

                Ok(())
            }

            PhaseAction::CompleteAck => {
                info!("remote acknowledgment received");
                Ok(())
            }

            PhaseAction::Ack => {
                let current_phase = self.phase.get_current_phase().await;
                info!(
                    "Received PhaseAck for phase: {:?}, current phase: {:?}",
                    phase.phase_type, current_phase
                );

                if phase.phase_type == current_phase {
                    // Initiator received Ack for their Init message
                    // Now they should start their side of the sync process
                    debug!("Initiator received Ack, starting sync process");
                    self.start_phase_sync().await?;

                    info!("PhaseAck handled successfully");
                } else {
                    warn!(
                        "Received PhaseAck for wrong phase. Current: {:?}, Received: {:?}",
                        current_phase, phase.phase_type
                    );
                }

                Ok(())
            }
        }
    }

    #[instrument(skip_all, level = "debug")]
    async fn start_phase_sync(&self) -> Result<(), String> {
        let current_phase = self.phase.get_current_phase().await;
        debug!(current_phase = ?current_phase, "Starting sync for current phase");

        let span = span!(Level::INFO, "phase_sync", phase = ?current_phase);
        let _enter = span.enter();

        let result = match current_phase {
            PhaseType::AddDevice => self.start_add_device_sync().await,
            PhaseType::FirstUserConnection => self.start_first_user_connection().await,
            PhaseType::DeviceSync => self.start_device_sync().await,
            PhaseType::UserSync => self.start_user_sync().await,
            PhaseType::FolderSync => self.start_folder_sync().await,
            PhaseType::ResourceSync => self.start_resource_sync().await,
            PhaseType::ShareSync => self.start_share_sync().await,
            PhaseType::UpdateSync => self.start_update_sync().await,
            PhaseType::DeviceRecordSync => self.start_device_record_sync().await,
            PhaseType::Complete => self.complete_sync().await,
        };

        match &result {
            Ok(_) => info!(current_phase = ?current_phase, "Phase sync started successfully"),
            Err(e) => {
                error!(current_phase = ?current_phase, error = %e, "Failed to start phase sync")
            }
        }

        result
    }

    // Mark current phase as complete
    #[instrument(skip(self), level = "info")]
    pub async fn complete_current_phase(&self) -> Result<(), String> {
        let current_phase = self.phase.get_current_phase().await;
        info!("Marking current phase as complete: {:?}", current_phase);

        debug!("Setting local phase completion state to true");
        self.phase.set_local_complete(true).await;

        // Notify peer about phase completion using new message structure
        debug!("Sending Phase Complete message");
        self.send_message(Message::Phase(Phase {
            action: PhaseAction::Complete,
            phase_type: current_phase,
        }))
        .await?;

        info!("Current phase marked as complete successfully");
        Ok(())
    }

    // Phase-specific sync methods (placeholders)
    #[instrument(skip(self), level = "info")]
    async fn start_user_sync(&self) -> Result<(), String> {
        info!("Starting user sync phase");
        self.get_and_send_next_sync().await
    }

pub async fn start_device_sync(&self) -> Result<(), String> {
    info!("Starting multi-phase device sync");
    
    let current_span = tracing::Span::current();
    
    // Get Phase 1 payload from sync service for the peer device we're connected to
    match self
        .context
        .sync_service
        .get_device_sync_request_payload(&self.device.id, current_span)
        .await
    {
        Ok(payload) => {
            info!("Sending device sync request (Phase 1) for peer device: {}", self.device.id);
            let message = Message::NetworkSync(payload);
            self.send_message(message).await
        }
        Err(e) => {
            error!(error = %e, "Failed to get device sync request payload");
            Err(format!("Failed to get device sync request payload: {}", e))
        }
    }
}
    #[instrument(skip(self), level = "info")]
    async fn start_resource_sync(&self) -> Result<(), String> {
        info!("Starting resource sync phase");
        self.get_and_send_next_sync().await
    }
    #[instrument(skip(self), level = "info")]
    async fn complete_sync(&self) -> Result<(), String> {
        info!("completed everything......");
        self.context
            .user_service
            .update_device_last_synced(&self.device.id)
            .await
            .map_err(|e| e.to_string())?;
        self.event_emitter.emit(P2PEvent::SyncComplete);
        if self.is_initiator {
            let _ = self.check_for_possible_disconnection().await?;
        }
        Ok(())
    }

    #[instrument(skip(self), level = "info")]
    async fn start_folder_sync(&self) -> Result<(), String> {
        info!("Starting folder sync phase");
        self.get_and_send_next_sync().await
    }

    #[instrument(skip(self), level = "info")]
    async fn start_share_sync(&self) -> Result<(), String> {
        info!("Starting share sync phase");
        self.get_and_send_next_sync().await
    }

    #[instrument(skip(self), level = "info")]
    async fn start_update_sync(&self) -> Result<(), String> {
        info!("Starting update sync phase");
        self.get_and_send_next_sync().await
    }
    #[instrument(skip(self), level = "info")]
    async fn start_device_record_sync(&self) -> Result<(), String> {
        info!("Starting device record sync phase");
        self.get_and_send_next_sync().await
    }

    #[instrument(skip(self), level = "info")]
    pub async fn request_disconnection(&self) -> Result<(), String> {
        info!("Requesting disconnection from peer: {}", self.get_id());
        if !self.can_close_connection().await {
            debug!("Connection not ready to be closed yet");
            return Ok(());
        }

        // Send disconnect request message
        let disconnect_request = Message::Disconnect(DisconnectStatus::Request);
        self.send_message(disconnect_request).await?;

        info!("Disconnection request sent");
        Ok(())
    }
    #[instrument(skip(self), level = "debug")]
    pub async fn check_for_possible_disconnection(&self) -> Result<(), String> {
        // Only check for possible disconnection if sync is complete
        let is_editing = self.is_live_editing().await;

        if !is_editing {
            debug!(
                "Sync complete and no live editing active, scheduling disconnection after timeout"
            );

            // Cancel existing timer if there is one
            self.cancel_disconnection_timer().await;

            // Get current connection ID (for logging)
            let connection_id = self.get_id();

            // Clone what we need for the async task
            let self_clone = self.clone();

            // Spawn timeout task
            let task = tokio::spawn(async move {
                // Sleep for 2 minutes
                tokio::time::sleep(std::time::Duration::from_secs(120)).await;

                // After timeout, check if we should still disconnect
                let is_editing = self_clone.is_live_editing().await;
                let current_phase = self_clone.phase.get_current_phase().await;

                if !is_editing && current_phase == PhaseType::Complete {
                    info!(
                        "Disconnection timeout reached for {}, initiating disconnection",
                        connection_id
                    );

                    // Request disconnection after timeout
                    if let Err(e) = self_clone.request_disconnection().await {
                        error!("Failed to request disconnection: {}", e);
                    }
                } else {
                    info!(
                        "Disconnection cancelled for connection {}: live_editing={}, phase={:?}",
                        connection_id, is_editing, current_phase
                    );
                }
            });

            // Store the task handle
            let mut timer_guard = self.disconnection_timer.lock().await;
            *timer_guard = Some(task);
        } else {
            debug!("Live editing still active, cannot disconnect yet");
        }

        Ok(())
    }
    #[instrument(skip(self), level = "debug")]
    pub async fn cancel_disconnection_timer(&self) -> bool {
        let mut timer_guard = self.disconnection_timer.lock().await;

        if let Some(handle) = timer_guard.take() {
            debug!(
                "Cancelling existing disconnection timer for {}",
                self.get_id()
            );
            handle.abort();
            true
        } else {
            debug!(
                "No existing disconnection timer to cancel for {}",
                self.get_id()
            );
            false
        }
    }

    #[instrument(skip(self, status), level = "info")]
    pub async fn handle_disconnect_message(&self, status: &DisconnectStatus) -> Result<(), String> {
        match status {
            DisconnectStatus::Request => {
                info!("Received disconnection request");

                // Check if we can allow disconnection
                let can_disconnect = self.can_close_connection().await;

                if can_disconnect {
                    info!("Accepting disconnection request");
                    // Send acceptance
                    let response = Message::Disconnect(DisconnectStatus::Accepted);
                    self.send_message(response).await?;

                    // Close our side of the connection
                    info!("Closing connection after accepting disconnect request");
                    self.close_connection().await?;
                } else {
                    // Determine reason for rejection
                    let current_phase = self.phase.get_current_phase().await;
                    let is_editing = self.is_live_editing().await;

                    let reason = if current_phase != PhaseType::Complete {
                        format!("Sync not complete (current phase: {:?})", current_phase)
                    } else if is_editing {
                        "Live editing is still active".to_string()
                    } else {
                        "Cannot disconnect at this time".to_string()
                    };

                    info!("Rejecting disconnection request: {}", reason);
                    let response = Message::Disconnect(DisconnectStatus::Rejected(reason));
                    self.send_message(response).await?;
                }

                Ok(())
            }
            DisconnectStatus::Accepted => {
                info!("Peer accepted disconnection request");
                // Close our side of the connection
                self.close_connection().await
            }
            DisconnectStatus::Rejected(reason) => {
                info!("Peer rejected disconnection request: {}", reason);
                // Don't close the connection
                Ok(())
            }
        }
    }
    #[instrument(skip(self), level = "debug")]
    pub async fn can_close_connection(&self) -> bool {
        // Check if sync is complete
        let current_phase = self.phase.get_current_phase().await;
        let sync_complete = current_phase == PhaseType::Complete;

        // Check if live editing is inactive
        let is_editing = self.is_live_editing().await;

        let can_close = sync_complete && !is_editing;
        debug!(
            sync_complete = sync_complete,
            is_editing = is_editing,
            can_close = can_close,
            "Checking if connection can be closed"
        );

        can_close
    }
}
