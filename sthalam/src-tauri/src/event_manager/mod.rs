//! EventManager - Bidirectional event communication between frontend and P2P network
//!
//! This module handles:
//! - Tauri events from frontend → Network function calls
//! - P2P events from network → Tauri events to frontend

mod p2p_listener;
mod tauri_listener;

use crypto_utils::CryptoUtils;
use network::p2p::emitter::P2PEvent;
use network::p2p::P2PService;
use persistance::database::RepositoryContext;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::runtime::Handle as RuntimeHandle;
use tokio::sync::{mpsc, RwLock};
use tracing::info;

/// EventManager coordinates bidirectional event flow
pub struct EventManager {
    app_handle: AppHandle,
    p2p_service: Arc<P2PService>,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: Arc<RwLock<CryptoUtils>>,
    _ucan_service: Arc<RwLock<gurkha::UcanService>>,
}

impl EventManager {
    /// Creates a new EventManager instance
    pub fn new(
        app_handle: AppHandle,
        p2p_service: Arc<P2PService>,
        repo_ctx: Arc<RepositoryContext>,
        crypto_utils: Arc<RwLock<CryptoUtils>>,
        ucan_service: Arc<RwLock<gurkha::UcanService>>,
    ) -> Self {
        info!("Creating EventManager");
        Self {
            app_handle,
            p2p_service,
            repo_ctx,
            crypto_utils,
            _ucan_service: ucan_service,
        }
    }

    /// Starts the EventManager by spawning listener tasks
    ///
    /// This spawns two async tasks:
    /// 1. Tauri event listener (frontend → network)
    /// 2. P2P event listener (network → frontend)
    pub fn start(self, p2p_receiver: mpsc::UnboundedReceiver<P2PEvent>, rt: &RuntimeHandle) {
        info!("Starting EventManager");

        // Clone for the tasks
        let app_handle_tauri = self.app_handle.clone();
        let app_handle_p2p = self.app_handle.clone();
        let p2p_service = self.p2p_service.clone();
        let repo_ctx = self.repo_ctx.clone();
        let crypto_utils = self.crypto_utils.clone();

        // Spawn Tauri listener task (frontend → network)
        rt.spawn(async move {
            tauri_listener::listen_to_tauri_events(
                app_handle_tauri,
                p2p_service,
                repo_ctx,
                crypto_utils,
            )
            .await;
        });

        // Spawn P2P listener task (network → frontend)
        rt.spawn(async move {
            p2p_listener::listen_to_p2p_events(app_handle_p2p, p2p_receiver).await;
        });

        info!("EventManager started successfully");
    }
}
