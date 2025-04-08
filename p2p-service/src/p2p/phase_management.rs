use crate::p2p::emitter::P2PEvent;
use crate::p2p::peer_connection::PeerConnection;
use osvauld_core::models::p2p::{Message, Phases, SyncPhase}; // Import from p2p.rs
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, warn};

// PhaseState struct to encapsulate phase state
#[derive(Clone)]
pub struct PhaseState {
    pub current_phase: Arc<Mutex<SyncPhase>>,
    pub local_phase_complete: Arc<Mutex<bool>>,
    pub remote_phase_complete: Arc<Mutex<bool>>,
}

impl PhaseState {
    pub fn new() -> Self {
        Self {
            current_phase: Arc::new(Mutex::new(SyncPhase::DeviceSync)),
            local_phase_complete: Arc::new(Mutex::new(false)),
            remote_phase_complete: Arc::new(Mutex::new(false)),
        }
    }

    pub async fn set_remote_complete(&self, value: bool) {
        let mut remote_complete = self.remote_phase_complete.lock().await;
        *remote_complete = value;
    }

    pub async fn set_local_complete(&self, value: bool) {
        let mut local_complete = self.local_phase_complete.lock().await;
        *local_complete = value;
    }

    pub async fn is_complete(&self) -> bool {
        let local_complete = *self.local_phase_complete.lock().await;
        let remote_complete = *self.remote_phase_complete.lock().await;

        local_complete && remote_complete
    }

    pub async fn get_current_phase(&self) -> SyncPhase {
        let phase = self.current_phase.lock().await;
        phase.clone()
    }

    pub async fn get_next_phase(&self) -> SyncPhase {
        let current = self.current_phase.lock().await;
        match *current {
            SyncPhase::DeviceSync => SyncPhase::UserSync,
            SyncPhase::UserSync => SyncPhase::FolderSync,
            SyncPhase::FolderSync => SyncPhase::ResourceSync,
            SyncPhase::ResourceSync => SyncPhase::ShareSync,
            SyncPhase::ShareSync => SyncPhase::UpdateSync,
            SyncPhase::UpdateSync => SyncPhase::Complete,
            SyncPhase::Complete => SyncPhase::Complete,
        }
    }

    pub async fn is_valid_next_phase(&self, next_phase: &SyncPhase) -> bool {
        let next = self.get_next_phase().await;
        *next_phase == next
    }

    pub async fn reset_for_new_phase(&self, new_phase: SyncPhase) {
        let mut phase = self.current_phase.lock().await;
        *phase = new_phase;

        self.set_local_complete(false).await;
        self.set_remote_complete(false).await;
    }
}

// Implement phase-related methods for PeerConnection
impl PeerConnection {
    // Handler for phase-related messages
    pub async fn handle_phase_message(&self, phase_message: &Phases) -> Result<(), String> {
        match phase_message {
            Phases::PhaseComplete(phase) => self.handle_phase_complete(phase).await,
            Phases::PhaseAcknowledge(phase) => self.handle_phase_acknowledge(phase).await,
            Phases::InitiatePhase(phase) => self.handle_initiate_phase(phase).await,
        }
    }

    async fn handle_phase_complete(&self, phase: &SyncPhase) -> Result<(), String> {
        info!("Received PhaseComplete for phase: {:?}", phase);

        let current_phase = self.phase.get_current_phase().await;
        if *phase == current_phase {
            self.phase.set_remote_complete(true).await;

            // Send acknowledgment
            self.send_message(Message::Phases(Phases::PhaseAcknowledge(phase.clone())))
                .await?;

            // Check if we can transition to next phase
            self.check_phase_transition().await?;
        } else {
            warn!(
                "Received PhaseComplete for wrong phase. Current: {:?}, Received: {:?}",
                current_phase, phase
            );
        }

        Ok(())
    }

    async fn handle_phase_acknowledge(&self, phase: &SyncPhase) -> Result<(), String> {
        info!("Received PhaseAcknowledge for phase: {:?}", phase);

        let current_phase = self.phase.get_current_phase().await;
        if *phase == current_phase {
            // Check if we can transition to next phase
            self.check_phase_transition().await?;
        } else {
            warn!(
                "Received PhaseAcknowledge for wrong phase. Current: {:?}, Received: {:?}",
                current_phase, phase
            );
        }

        Ok(())
    }

    async fn handle_initiate_phase(&self, phase: &SyncPhase) -> Result<(), String> {
        info!("Received InitiatePhase for phase: {:?}", phase);

        // Validate phase transition
        if self.phase.is_valid_next_phase(phase).await {
            // Transition to the new phase
            self.transition_to_phase(phase.clone()).await?;
        } else {
            let current_phase = self.phase.get_current_phase().await;
            warn!(
                "Received invalid phase transition request. Current: {:?}, Requested: {:?}",
                current_phase, phase
            );
        }

        Ok(())
    }

    // Helper methods for phase management
    async fn check_phase_transition(&self) -> Result<(), String> {
        if self.phase.is_complete().await {
            if self.should_initiate_next_phase() {
                let next_phase = self.phase.get_next_phase().await;
                self.send_message(Message::Phases(Phases::InitiatePhase(next_phase.clone())))
                    .await?;
                self.transition_to_phase(next_phase).await?;
            }
        }

        Ok(())
    }

    fn should_initiate_next_phase(&self) -> bool {
        // Simple algorithm: device with lower ID initiates
        self.device.id < self.user.id
    }

    async fn transition_to_phase(&self, new_phase: SyncPhase) -> Result<(), String> {
        let current_phase = self.phase.get_current_phase().await;
        info!(
            "Transitioning from phase {:?} to {:?}",
            current_phase, new_phase
        );

        // Reset phase state for the new phase
        self.phase.reset_for_new_phase(new_phase.clone()).await;

        // If this is the Complete phase, emit a SyncComplete event
        if new_phase == SyncPhase::Complete {
            self.event_emitter.emit(P2PEvent::SyncComplete);
        } else {
            // Initiate synchronization for the new phase
            self.start_phase_sync().await?;
        }

        Ok(())
    }

    async fn start_phase_sync(&self) -> Result<(), String> {
        let current_phase = self.phase.get_current_phase().await;
        match current_phase {
            SyncPhase::DeviceSync => self.start_device_sync().await,
            SyncPhase::UserSync => self.start_user_sync().await,
            SyncPhase::FolderSync => self.start_folder_sync().await,
            SyncPhase::ResourceSync => self.start_resource_sync().await,
            SyncPhase::ShareSync => self.start_share_sync().await,
            SyncPhase::UpdateSync => self.start_update_sync().await,
            SyncPhase::Complete => Ok(()),
        }
    }

    // Mark current phase as complete
    pub async fn complete_current_phase(&self) -> Result<(), String> {
        let current_phase = self.phase.get_current_phase().await;
        info!("Marking current phase as complete: {:?}", current_phase);

        self.phase.set_local_complete(true).await;

        // Notify peer about phase completion
        self.send_message(Message::Phases(Phases::PhaseComplete(current_phase)))
            .await?;

        // Check if we can transition to next phase
        self.check_phase_transition().await?;

        Ok(())
    }

    // Phase-specific sync methods (placeholders)
    async fn start_user_sync(&self) -> Result<(), String> {
        info!("Starting user sync phase");
        // Implementation pending
        Ok(())
    }

    async fn start_resource_sync(&self) -> Result<(), String> {
        info!("Starting resource sync phase");
        // Implementation pending
        Ok(())
    }

    async fn start_folder_sync(&self) -> Result<(), String> {
        info!("Starting folder sync phase");
        // Implementation pending
        Ok(())
    }

    async fn start_share_sync(&self) -> Result<(), String> {
        info!("Starting share sync phase");
        // Implementation pending
        Ok(())
    }

    async fn start_update_sync(&self) -> Result<(), String> {
        info!("Starting update sync phase");
        // Implementation pending
        Ok(())
    }
}
