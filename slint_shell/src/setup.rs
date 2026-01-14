//! Initialization and setup functions
//!
//! Handles Butler, transport, and P2P initialization.

use std::sync::Arc;
use butler::{Butler, SyncEvent};
use courier::{Courier, CourierEvent, CourierHandle, CourierMode, HandshakeServices, Transport, TransportConfig};

/// Initialize P2P after login (matches Tauri pattern)
///
/// **Context**: Called after Butler identity is set
/// **Returns**: (CourierHandle, event receiver) for P2P operations
pub async fn init_p2p(
    butler: Arc<Butler>,
) -> Result<(CourierHandle, tokio::sync::mpsc::Receiver<CourierEvent>), String> {
    // Get device key from Butler (requires identity)
    let device_key = butler
        .device_key()
        .await
        .map_err(|e| format!("Failed to get device key: {}", e))?;

    println!("Initializing transport with device key...");

    // Initialize Transport
    let config = TransportConfig::new(device_key);
    let (transport, transport_rx) = Transport::init(config)
        .await
        .map_err(|e| format!("Failed to initialize transport: {}", e))?;

    let transport = Arc::new(transport);
    let node_id = transport.node_id();
    println!("Transport initialized with node ID: {}", node_id);

    // Create HandshakeServices with Butler
    let handshake_services = Arc::new(HandshakeServices::new(butler.clone()));

    // Initialize Courier with services
    let (handle, event_rx, courier) =
        Courier::init_with_services(CourierMode::User, transport, Some(handshake_services));

    // Spawn courier event loop
    tokio::spawn(async move {
        courier.run(transport_rx).await;
    });

    // Wire up sync events: Scribe → Butler → Coordinator
    // This enables EnsureSync to trigger connections/subscriptions
    let (sync_tx, mut sync_rx) = tokio::sync::mpsc::channel::<SyncEvent>(32);
    butler.set_sync_event_tx(sync_tx).await;

    // Forward sync events to Coordinator
    let handle_for_sync = handle.clone();
    tokio::spawn(async move {
        while let Some(event) = sync_rx.recv().await {
            match event {
                SyncEvent::EnsureSync { user_did } => {
                    if let Err(e) = handle_for_sync.ensure_sync(&user_did) {
                        tracing::warn!(user_did = %user_did, error = %e, "Failed to forward EnsureSync");
                    } else {
                        tracing::debug!(user_did = %user_did, "Forwarded EnsureSync to Coordinator");
                    }
                }
            }
        }
    });

    Ok((handle, event_rx))
}
