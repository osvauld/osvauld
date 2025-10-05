use std::sync::Arc;

use crate::chat_state::ChatState;
use crate::current_note_state::CurrentNoteState;
use crypto_utils::CryptoUtils;
use network::p2p::{P2PEvent, incoming::P2PSender};
use persistance::database::RepositoryContext;
use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tokio::sync::{mpsc, RwLock};

mod p2p_handlers;
mod p2p_reciever;
mod p2p_sender;
mod tauri_events;

/// Initializes all listeners for the application
/// This connects the Tauri event system with the P2P event system
pub struct EventManager {
    app_handle: AppHandle,
    p2p_receiver: mpsc::UnboundedReceiver<P2PEvent>,
    p2p_sender: P2PSender,
    current_note_state: CurrentNoteState,
    chat_state: ChatState,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: Arc<RwLock<CryptoUtils>>,
}

#[derive(Debug, Clone)]
enum UpdateType {
    SyncUpdate,
    AwarenessUpdate,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NoteChangePayload {
    pub note_id: Option<String>,
    pub state_vectors: serde_json::Value,
}

impl UpdateType {
    fn event_name(&self) -> &'static str {
        match self {
            UpdateType::SyncUpdate => "sync-update",
            UpdateType::AwarenessUpdate => "awareness-update",
        }
    }

    fn description(&self) -> &'static str {
        match self {
            UpdateType::SyncUpdate => "sync update",
            UpdateType::AwarenessUpdate => "awareness update",
        }
    }
}

impl EventManager {
    /// Create a new EventManager that handles bidirectional events
    pub fn new(
        app_handle: AppHandle,
        p2p_receiver: mpsc::UnboundedReceiver<P2PEvent>,
        p2p_sender: P2PSender,
        repo_ctx: Arc<RepositoryContext>,
        crypto_utils: Arc<RwLock<CryptoUtils>>,
    ) -> Self {
        Self {
            app_handle,
            p2p_receiver,
            p2p_sender,
            current_note_state: CurrentNoteState::new(),
            chat_state: ChatState::new(repo_ctx.clone()),
            repo_ctx,
            crypto_utils,
        }
    }

    /// Start listening for all events
    pub async fn start_listening(mut self) {
        // Set up Tauri event listeners
        self.setup_tauri_listeners();

        self.start_reconciliation_timer().await;
        // Start P2P event listener in background
        tokio::spawn(async move {
            self.listen_for_p2p_events().await;
        });
    }

    /// Add a method to access the current note state
    pub fn get_current_note_state(&self) -> CurrentNoteState {
        self.current_note_state.clone()
    }
}
