//! Peer Registry - Authenticated peer state
//!
//! After Transport establishes a connection, Courier performs handshake.
//! Once authenticated, peer info is stored here.

use transport::NodeId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::RwLock;
use transport::ConnectionHandle;
use tracing::{debug, info};

/// Type of peer connection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PeerType {
    /// This node's owner (user who owns the node)
    Owner,
    /// User's own node (from user's perspective)
    MyNode,
    /// Another user (peer)
    PeerUser,
    /// Another user's node
    PeerNode,
}

/// Authenticated peer information
///
/// Created after successful handshake. Contains identity and connection info.
#[derive(Debug, Clone)]
pub struct PeerInfo {
    /// Peer's iroh NodeId
    pub node_id: NodeId,
    /// Peer type (Owner, MyNode, etc.)
    pub peer_type: PeerType,
    /// Peer's DID (did:key:...)
    pub did: String,
    /// Peer's username
    pub username: String,
    /// Ed25519 public key for identity verification
    pub public_key: Vec<u8>,
    /// The permit they presented (verified)
    pub permit: String,
    /// Long-lived permit we issued to them
    pub issued_permit: Option<String>,
    /// Connection handle for sending messages
    pub conn: ConnectionHandle,
    /// When the peer connected
    pub connected_at: chrono::DateTime<chrono::Utc>,
}

impl PeerInfo {
    pub fn new(
        node_id: NodeId,
        peer_type: PeerType,
        did: String,
        username: String,
        public_key: Vec<u8>,
        permit: String,
        conn: ConnectionHandle,
    ) -> Self {
        Self {
            node_id,
            peer_type,
            did,
            username,
            public_key,
            permit,
            issued_permit: None,
            conn,
            connected_at: chrono::Utc::now(),
        }
    }

    /// Set the permit we issued to this peer
    pub fn with_issued_permit(mut self, permit: String) -> Self {
        self.issued_permit = Some(permit);
        self
    }

    /// Check if this is the owner
    pub fn is_owner(&self) -> bool {
        self.peer_type == PeerType::Owner
    }

    /// Check if this is our own node
    pub fn is_my_node(&self) -> bool {
        self.peer_type == PeerType::MyNode
    }
}

/// Registry of authenticated peers
///
/// Stores PeerInfo after successful handshake.
/// Keyed by NodeId for quick lookup.
pub struct PeerRegistry {
    peers: RwLock<HashMap<NodeId, PeerInfo>>,
}

impl PeerRegistry {
    pub fn new() -> Self {
        Self {
            peers: RwLock::new(HashMap::new()),
        }
    }

    /// Register a new authenticated peer
    pub async fn insert(&self, info: PeerInfo) {
        let node_id = info.node_id;
        let peer_type = info.peer_type;
        let username = info.username.clone();

        let mut peers = self.peers.write().await;
        peers.insert(node_id, info);

        info!(
            "Registered peer: {} ({}) as {:?}",
            username, node_id, peer_type
        );
    }

    /// Remove a peer
    pub async fn remove(&self, node_id: &NodeId) -> Option<PeerInfo> {
        let mut peers = self.peers.write().await;
        let removed = peers.remove(node_id);

        if let Some(ref info) = removed {
            info!(
                "Unregistered peer: {} ({}) - {:?}",
                info.username, node_id, info.peer_type
            );
        }

        removed
    }

    /// Get peer info by NodeId
    pub async fn get(&self, node_id: &NodeId) -> Option<PeerInfo> {
        let peers = self.peers.read().await;
        peers.get(node_id).cloned()
    }

    /// Check if we have an authenticated peer
    pub async fn contains(&self, node_id: &NodeId) -> bool {
        let peers = self.peers.read().await;
        peers.contains_key(node_id)
    }

    /// Get the owner peer (if connected)
    pub async fn get_owner(&self) -> Option<PeerInfo> {
        let peers = self.peers.read().await;
        peers
            .values()
            .find(|p| p.peer_type == PeerType::Owner)
            .cloned()
    }

    /// Get all connected nodes (MyNode type)
    pub async fn get_my_nodes(&self) -> Vec<PeerInfo> {
        let peers = self.peers.read().await;
        peers
            .values()
            .filter(|p| p.peer_type == PeerType::MyNode)
            .cloned()
            .collect()
    }

    /// Get all peers of a specific type
    pub async fn get_by_type(&self, peer_type: PeerType) -> Vec<PeerInfo> {
        let peers = self.peers.read().await;
        peers
            .values()
            .filter(|p| p.peer_type == peer_type)
            .cloned()
            .collect()
    }

    /// Get all authenticated peers
    pub async fn all(&self) -> Vec<PeerInfo> {
        let peers = self.peers.read().await;
        peers.values().cloned().collect()
    }

    /// Count of authenticated peers
    pub async fn len(&self) -> usize {
        let peers = self.peers.read().await;
        peers.len()
    }

    /// Check if empty
    pub async fn is_empty(&self) -> bool {
        let peers = self.peers.read().await;
        peers.is_empty()
    }

    /// Get NodeIds by DID (a user may have multiple devices)
    pub async fn get_by_did(&self, did: &str) -> Vec<PeerInfo> {
        let peers = self.peers.read().await;
        peers
            .values()
            .filter(|p| p.did == did)
            .cloned()
            .collect()
    }
}

impl Default for PeerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_peer_type_equality() {
        assert_eq!(PeerType::Owner, PeerType::Owner);
        assert_ne!(PeerType::Owner, PeerType::MyNode);
    }

    #[tokio::test]
    async fn test_registry_operations() {
        let registry = PeerRegistry::new();
        assert!(registry.is_empty().await);
        assert_eq!(registry.len().await, 0);
    }
}
