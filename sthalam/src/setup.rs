//! Initialization and setup functions
//!
//! Handles Butler, transport, and P2P initialization.

use butler::{Butler, SyncEvent};
use courier::{
    Courier, CourierEvent, CourierHandle, CourierMode, HandshakeServices, Transport,
    TransportConfig,
};
use std::sync::Arc;

/// Initialize P2P after login.
///
/// **sync_rx**: receiver for Scribe->Butler sync events, forwarded to Coordinator.
/// **capture_tx**: optional broadcast channel for event capture.
pub async fn init_p2p(
    butler: Arc<Butler>,
    mut sync_rx: tokio::sync::mpsc::Receiver<SyncEvent>,
    capture_tx: Option<tokio::sync::broadcast::Sender<String>>,
) -> Result<(CourierHandle, tokio::sync::mpsc::Receiver<CourierEvent>), String> {
    let device_key = butler
        .device_key()
        .await
        .map_err(|e| format!("Failed to get device key: {}", e))?;

    tracing::info!("Initializing transport with device key...");

    let config = TransportConfig::new(device_key);
    let (transport, transport_rx) = Transport::init(config)
        .await
        .map_err(|e| format!("Failed to initialize transport: {}", e))?;

    let transport = Arc::new(transport);
    let node_id = transport.node_id();
    tracing::info!(node_id = %node_id, "Transport initialized");

    let handshake_services = Arc::new(HandshakeServices::new(butler.clone()));

    let (handle, event_rx, courier) = Courier::init_with_services_and_capture(
        CourierMode::User,
        transport,
        Some(handshake_services),
        capture_tx,
    );

    tokio::spawn(async move {
        courier.run(transport_rx).await;
    });

    // Forward Scribe -> Butler sync events to Coordinator so EnsureSync triggers
    // connections/subscriptions.
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
                SyncEvent::SubscribeLayers {
                    page_id,
                    creator_did,
                    layers,
                } => {
                    tracing::info!(
                        page_id = %page_id,
                        creator_did = %creator_did,
                        count = layers.len(),
                        "Received SubscribeLayers from Scribe, forwarding to Coordinator"
                    );
                    if let Err(e) = handle_for_sync.subscribe_layers(&page_id, &creator_did, layers)
                    {
                        tracing::warn!(error = %e, "Failed to forward SubscribeLayers");
                    }
                }
            }
        }
    });

    Ok((handle, event_rx))
}
