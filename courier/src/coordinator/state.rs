//! Coordinator state management and helpers
//!
//! Provides unified state management for peer connections, combining
//! what was previously three separate HashMaps into a single registry.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use ractor::ActorRef;
use tokio::sync::mpsc;
use tracing::warn;
use transport::{ConnectionHandle, NodeId};

use crate::handle::CourierEvent;
use crate::peer_actor::{BlobStore, PeerMessage};
use crate::state::PeerType;
use crate::ConnectRequest;
use butler::Butler;

use super::CourierMode;

/// Information about an authenticated peer
#[derive(Debug, Clone)]
pub struct PeerInfo {
    pub node_id: NodeId,
    pub peer_type: PeerType,
    pub did: String,
    pub username: String,
}

/// Unified peer entry - combines actor, connection, and auth state
#[derive(Debug)]
pub struct PeerEntry {
    /// Reference to the PeerActor handling this connection
    pub actor: ActorRef<PeerMessage>,
    /// Connection handle for sending messages
    pub conn: ConnectionHandle,
    /// None until authenticated, Some after successful handshake
    pub auth: Option<PeerInfo>,
}

/// Coordinator state with unified peer registry
pub struct CoordinatorState {
    /// Our node ID
    pub our_node_id: NodeId,

    /// Running mode (shell or node)
    pub mode: CourierMode,

    /// Butler reference for storage/crypto
    pub butler: Arc<Butler>,

    /// Blob store for assets
    pub blob_store: BlobStore,

    /// All connected peers (unified registry)
    pub peers: HashMap<NodeId, PeerEntry>,

    /// Outbound connections we initiated (waiting for handshake)
    pub pending_connections: HashSet<NodeId>,

    /// Permits for pending outbound connections
    pub pending_permits: HashMap<NodeId, String>,

    /// Event subscribers
    pub event_tx: Option<mpsc::Sender<CourierEvent>>,

    /// Channel to request connections (handled by CourierRunner)
    pub connect_tx: Option<mpsc::Sender<ConnectRequest>>,
}

impl CoordinatorState {
    /// Create new coordinator state
    pub fn new(
        our_node_id: NodeId,
        mode: CourierMode,
        butler: Arc<Butler>,
        blob_store: BlobStore,
        connect_tx: Option<mpsc::Sender<ConnectRequest>>,
        event_tx: Option<mpsc::Sender<CourierEvent>>,
    ) -> Self {
        Self {
            our_node_id,
            mode,
            butler,
            blob_store,
            peers: HashMap::new(),
            pending_connections: HashSet::new(),
            pending_permits: HashMap::new(),
            event_tx,
            connect_tx,
        }
    }

    // =========================================================================
    // Peer Lifecycle
    // =========================================================================

    /// Add a new peer (on connection, before authentication)
    pub fn add_peer(&mut self, node_id: NodeId, actor: ActorRef<PeerMessage>, conn: ConnectionHandle) {
        self.peers.insert(node_id, PeerEntry {
            actor,
            conn,
            auth: None,
        });
    }

    /// Mark peer as authenticated
    pub fn authenticate_peer(
        &mut self,
        node_id: NodeId,
        peer_type: PeerType,
        did: String,
        username: String,
    ) -> bool {
        if let Some(entry) = self.peers.get_mut(&node_id) {
            entry.auth = Some(PeerInfo {
                node_id,
                peer_type,
                did,
                username,
            });
            true
        } else {
            false
        }
    }

    /// Remove peer and all associated state (cleanup on disconnect)
    pub fn remove_peer(&mut self, node_id: NodeId) {
        self.peers.remove(&node_id);
        self.pending_connections.remove(&node_id);
        self.pending_permits.remove(&node_id);
    }

    // =========================================================================
    // Peer Accessors
    // =========================================================================

    /// Get peer actor if exists
    pub fn get_actor(&self, node_id: &NodeId) -> Option<&ActorRef<PeerMessage>> {
        self.peers.get(node_id).map(|e| &e.actor)
    }

    /// Get connection handle if exists
    pub fn get_conn(&self, node_id: &NodeId) -> Option<&ConnectionHandle> {
        self.peers.get(node_id).map(|e| &e.conn)
    }

    /// Get peer info if authenticated
    pub fn get_auth(&self, node_id: &NodeId) -> Option<&PeerInfo> {
        self.peers.get(node_id)?.auth.as_ref()
    }

    /// Get full peer entry
    pub fn get_peer(&self, node_id: &NodeId) -> Option<&PeerEntry> {
        self.peers.get(node_id)
    }

    // =========================================================================
    // Auth Guards
    // =========================================================================

    /// Require authenticated peer, returns None with warning if not
    pub fn require_auth(&self, node_id: NodeId, action: &str) -> Option<&PeerInfo> {
        match self.get_auth(&node_id) {
            Some(info) => Some(info),
            None => {
                warn!("Cannot {} for unauthenticated peer: {}", action, node_id);
                None
            }
        }
    }

    // =========================================================================
    // Iterators
    // =========================================================================

    /// Iterate over authenticated peers
    pub fn authenticated_peers(&self) -> impl Iterator<Item = (NodeId, &PeerEntry)> {
        self.peers.iter()
            .filter(|(_, e)| e.auth.is_some())
            .map(|(id, e)| (*id, e))
    }

    /// Get all peer node IDs
    pub fn peer_node_ids(&self) -> Vec<NodeId> {
        self.peers.keys().copied().collect()
    }

    // =========================================================================
    // State Queries
    // =========================================================================

    /// Check if peer is connected (may or may not be authenticated)
    pub fn is_connected(&self, node_id: &NodeId) -> bool {
        self.peers.contains_key(node_id)
    }

    /// Check if peer is authenticated
    pub fn is_authenticated(&self, node_id: &NodeId) -> bool {
        self.get_auth(node_id).is_some()
    }
}
