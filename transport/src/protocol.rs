//! Protocol handler for Osvauld P2P connections
//!
//! Implements iroh's ProtocolHandler trait to handle incoming connections
//! for the "osvauld/p2p/1" ALPN protocol.
//!
//! The Router dispatches connections to this handler based on ALPN negotiation.

use crate::events::TransportEvent;
use crate::pool::{ConnectionHandle, ConnectionPool};
use crate::NodeId;

use iroh::endpoint::Connection;
use iroh::protocol::{AcceptError, ProtocolHandler};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{info, warn};

/// Protocol handler for osvauld/p2p/1 ALPN connections
///
/// **Role**: Handles incoming connections for our P2P protocol
/// **Router dispatches**: Connections with ALPN "osvauld/p2p/1" come here
/// **We do**: Store in pool, emit Connected event, spawn disconnect watcher
#[derive(Clone, Debug)]
pub struct OsvaualdProtocol {
    pool: Arc<ConnectionPool>,
    event_tx: mpsc::Sender<TransportEvent>,
}

impl OsvaualdProtocol {
    /// Create a new protocol handler
    pub fn new(pool: Arc<ConnectionPool>, event_tx: mpsc::Sender<TransportEvent>) -> Self {
        Self { pool, event_tx }
    }
}

impl ProtocolHandler for OsvaualdProtocol {
    /// Handle an accepted connection
    ///
    /// **Context**: Called by Router when a peer connects with our ALPN
    /// **We do**: Store handle in pool, emit Connected event, spawn disconnect watcher
    /// **Consumer does**: Spawn PeerSession with ConnectionHandle for reading/writing
    async fn accept(&self, conn: Connection) -> Result<(), AcceptError> {
        let node_id = conn.remote_id();
        info!("Accepted connection from: {}", node_id);

        let handle = ConnectionHandle::new(conn.clone(), node_id);
        self.pool.insert(handle.clone()).await;

        // Emit Connected - consumer (SessionManager) will spawn PeerSession
        if let Err(e) = self
            .event_tx
            .send(TransportEvent::Connected {
                node_id,
                conn: handle,
            })
            .await
        {
            warn!("Failed to send Connected event: {}", e);
        }

        // Spawn disconnect watcher
        let event_tx = self.event_tx.clone();
        let pool = self.pool.clone();
        tokio::spawn(connection_close_watcher(conn, node_id, event_tx, pool));

        Ok(())
    }
}

/// Watch for connection close and emit Disconnected event
///
/// **Context**: Monitors connection lifecycle
/// **We do**: Wait for close, then cleanup pool and emit event
async fn connection_close_watcher(
    conn: Connection,
    node_id: NodeId,
    event_tx: mpsc::Sender<TransportEvent>,
    pool: Arc<ConnectionPool>,
) {
    // Wait for connection to close (triggered by either side)
    conn.closed().await;

    // Connection ended - clean up
    pool.remove(&node_id).await;
    let _ = event_tx
        .send(TransportEvent::Disconnected { node_id })
        .await;
    info!("Disconnected from: {}", node_id);
}
