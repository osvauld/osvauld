//! Coordinator state management and helpers
//!
//! Provides unified state management for peer connections in a single registry.
//!
//! Generic over `C: Connection` to support different transport implementations.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use ractor::ActorRef;
use tokio::sync::mpsc;
use tracing::warn;
use transport::{Connection, NodeId};

use crate::handle::CourierEvent;
use crate::peer_actor::{BlobStore, PeerMessage};
use crate::state::PeerType;
use crate::trace::MessageTrace;
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
///
/// Generic over `C: Connection` to support different transport implementations.
pub struct PeerEntry<C: Connection> {
    /// Reference to the PeerActor handling this connection
    pub actor: ActorRef<PeerMessage>,
    /// Connection handle for sending messages
    pub conn: C,
    /// None until authenticated, Some after successful handshake
    pub auth: Option<PeerInfo>,
}

/// Coordinator state with unified peer registry
///
/// Generic over `C: Connection` to support different transport implementations.
pub struct CoordinatorState<C: Connection> {
    /// Our node ID
    pub our_node_id: NodeId,

    /// Running mode (shell or node)
    pub mode: CourierMode,

    /// Butler reference for storage/crypto
    pub butler: Arc<Butler>,

    /// Blob store for assets
    pub blob_store: BlobStore,

    /// All connected peers (unified registry)
    pub peers: HashMap<NodeId, PeerEntry<C>>,

    /// Outbound connections we initiated (waiting for handshake)
    pub pending_connections: HashSet<NodeId>,

    /// Permits for pending outbound connections
    pub pending_permits: HashMap<NodeId, String>,

    /// Event subscribers
    pub event_tx: Option<mpsc::Sender<CourierEvent>>,

    /// Channel to request connections (handled by CourierRunner)
    pub connect_tx: Option<mpsc::Sender<ConnectRequest>>,

    /// Optional trace channel for protocol message capture (tests only)
    pub message_tx: Option<mpsc::UnboundedSender<MessageTrace>>,
}

impl<C: Connection> CoordinatorState<C> {
    /// Create new coordinator state
    pub fn new(
        our_node_id: NodeId,
        mode: CourierMode,
        butler: Arc<Butler>,
        blob_store: BlobStore,
        connect_tx: Option<mpsc::Sender<ConnectRequest>>,
        event_tx: Option<mpsc::Sender<CourierEvent>>,
        message_tx: Option<mpsc::UnboundedSender<MessageTrace>>,
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
            message_tx,
        }
    }

    // Peer Lifecycle

    /// Add a new peer (on connection, before authentication)
    pub fn add_peer(&mut self, node_id: NodeId, actor: ActorRef<PeerMessage>, conn: C) {
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

    // Peer Accessors

    /// Get peer actor if exists
    pub fn get_actor(&self, node_id: &NodeId) -> Option<&ActorRef<PeerMessage>> {
        self.peers.get(node_id).map(|e| &e.actor)
    }

    /// Get connection handle if exists
    pub fn get_conn(&self, node_id: &NodeId) -> Option<&C> {
        self.peers.get(node_id).map(|e| &e.conn)
    }

    /// Get peer info if authenticated
    pub fn get_auth(&self, node_id: &NodeId) -> Option<&PeerInfo> {
        self.peers.get(node_id)?.auth.as_ref()
    }

    /// Get full peer entry
    pub fn get_peer(&self, node_id: &NodeId) -> Option<&PeerEntry<C>> {
        self.peers.get(node_id)
    }

    // Auth Guards

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

    // Iterators

    /// Iterate over authenticated peers
    pub fn authenticated_peers(&self) -> impl Iterator<Item = (NodeId, &PeerEntry<C>)> {
        self.peers.iter()
            .filter(|(_, e)| e.auth.is_some())
            .map(|(id, e)| (*id, e))
    }

    /// Get all peer node IDs
    pub fn peer_node_ids(&self) -> Vec<NodeId> {
        self.peers.keys().copied().collect()
    }

    // State Queries

    /// Check if peer is connected (may or may not be authenticated)
    pub fn is_connected(&self, node_id: &NodeId) -> bool {
        self.peers.contains_key(node_id)
    }

    /// Check if peer is authenticated
    pub fn is_authenticated(&self, node_id: &NodeId) -> bool {
        self.get_auth(node_id).is_some()
    }

    // Scribe Integration (for ScribeConnect pattern)

    /// Get PeerActor by user DID
    ///
    /// **Context**: Used by Scribe to send ScribeConnect to the appropriate PeerActor
    /// **Returns**: PeerActor ref if user is authenticated, None otherwise
    pub fn get_peer_actor_for_did(&self, user_did: &str) -> Option<&ActorRef<PeerMessage>> {
        self.authenticated_peers()
            .find(|(_, entry)| entry.auth.as_ref().map(|a| a.did.as_str()) == Some(user_did))
            .map(|(_, entry)| &entry.actor)
    }

    /// Get the node's PeerActor (for User mode connecting to their node)
    ///
    /// **Context**: In User mode, Scribe needs to connect to the node's PeerActor
    /// **Returns**: PeerActor ref for the node if connected, None otherwise
    pub fn get_node_peer_actor(&self) -> Option<&ActorRef<PeerMessage>> {
        self.authenticated_peers()
            .find(|(_, entry)| entry.auth.as_ref().map(|a| a.peer_type) == Some(PeerType::MyNode))
            .map(|(_, entry)| &entry.actor)
    }

    /// Get all authenticated PeerActor refs for pages with shares
    ///
    /// **Context**: In Node mode, Scribe needs to connect to all PeerActors for peers with shares
    /// **Returns**: Iterator over (DID, PeerActor ref) for authenticated peers
    pub fn get_all_authenticated_peer_actors(&self) -> impl Iterator<Item = (&str, &ActorRef<PeerMessage>)> {
        self.authenticated_peers()
            .filter_map(|(_, entry)| {
                entry.auth.as_ref().map(|a| (a.did.as_str(), &entry.actor))
            })
    }
}
