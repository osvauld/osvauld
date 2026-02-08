//! Initialization and setup functions
//!
//! Handles Butler, transport, and P2P initialization.

use std::sync::Arc;
use butler::{Butler, SyncEvent};
use courier::{Courier, CourierEvent, CourierHandle, CourierMode, HandshakeServices, Transport, TransportConfig};

/// Initialize P2P after login (matches Tauri pattern)
///
/// **Context**: Called after Butler identity is set
/// **sync_rx**: Receiver for sync events (EnsureSync) - created in main.rs, sender passed to Butler
/// **Returns**: (CourierHandle, event receiver) for P2P operations
pub async fn init_p2p(
    butler: Arc<Butler>,
    mut sync_rx: tokio::sync::mpsc::Receiver<SyncEvent>,
) -> Result<(CourierHandle, tokio::sync::mpsc::Receiver<CourierEvent>), String> {
    // Get device key from Butler (requires identity)
    let device_key = butler
        .device_key()
        .await
        .map_err(|e| format!("Failed to get device key: {}", e))?;

    tracing::info!("Initializing transport with device key...");

    // Initialize Transport
    let config = TransportConfig::new(device_key);
    let (transport, transport_rx) = Transport::init(config)
        .await
        .map_err(|e| format!("Failed to initialize transport: {}", e))?;

    let transport = Arc::new(transport);
    let node_id = transport.node_id();
    tracing::info!(node_id = %node_id, "Transport initialized");

    // Create HandshakeServices with Butler
    let handshake_services = Arc::new(HandshakeServices::new(butler.clone()));

    // Initialize Courier with services
    let (handle, event_rx, courier) =
        Courier::init_with_services(CourierMode::User, transport, Some(handshake_services));

    // Spawn courier event loop
    tokio::spawn(async move {
        courier.run(transport_rx).await;
    });

    // Forward sync events to Coordinator (Scribe -> Butler -> Coordinator)
    // This enables EnsureSync to trigger connections/subscriptions
    let handle_for_sync = handle.clone();
    tokio::spawn(async move {
        while let Some(event) = sync_rx.recv().await {
            match event {
                SyncEvent::EnsureSync { user_did } => {
                    tracing::info!(user_did = %user_did, "Received EnsureSync from Scribe, forwarding to Coordinator");
                    if let Err(e) = handle_for_sync.ensure_sync(&user_did) {
                        tracing::warn!(user_did = %user_did, error = %e, "Failed to forward EnsureSync");
                    } else {
                        tracing::info!(user_did = %user_did, "Forwarded EnsureSync to Coordinator");
                    }
                }
            }
        }
    });

    Ok((handle, event_rx))
}
