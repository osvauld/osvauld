//! Storage trait implementations for Scribe actor
//!
//! Butler's implementations of the scribe storage traits using RedbStore.

use std::collections::HashMap;
use std::sync::Arc;

use scribe::{LayerStorage, PeerVectorStorage, PeerResolver, Result, ScribeError};

use tracing::{info, instrument};
use crate::storage::RedbStore;

// Layer Storage Implementation

/// Butler's implementation of LayerStorage
///
/// Saves layer snapshots to RedbStore with AES encryption.
/// Scoped to a single page (page_id passed at construction).
pub struct ButlerLayerStorage {
    store: Arc<RedbStore>,
    page_id: String,
    aes_key: [u8; 32],
}

impl ButlerLayerStorage {
    pub fn new(store: Arc<RedbStore>, page_id: String, aes_key: [u8; 32]) -> Self {
        Self { store, page_id, aes_key }
    }
}

impl LayerStorage for ButlerLayerStorage {
    #[instrument(skip_all)]
    fn save_layer(&self, layer_name: &str, data: &[u8]) -> Result<()> {
        info!(page_id = %self.page_id, layer_name = %layer_name, data_len = data.len(), "ButlerLayerStorage::save_layer");

        // Encrypt layer data
        let encrypted = herald::encrypt_symmetric(&self.aes_key, data)
            .map_err(|e| ScribeError::Other(format!("Encryption failed: {}", e)))?;

        // Save to store
        self.store
            .put_layer(&self.page_id, layer_name, &encrypted)
            .map_err(|e| ScribeError::Other(format!("Store error: {}", e)))
    }
}

// Peer Vector Storage Implementation

/// Butler's implementation of PeerVectorStorage
///
/// Saves/loads peer state vectors to RedbStore.
/// Scoped to a single page (page_id passed at construction).
pub struct ButlerPeerVectorStorage {
    store: Arc<RedbStore>,
    page_id: String,
}

impl ButlerPeerVectorStorage {
    pub fn new(store: Arc<RedbStore>, page_id: String) -> Self {
        Self { store, page_id }
    }
}

impl PeerVectorStorage for ButlerPeerVectorStorage {
    #[instrument(skip_all)]
    fn load_vectors(
        &self,
        user_did: &str,
        device_id: &str,
    ) -> Result<Option<HashMap<String, Vec<u8>>>> {
        self.store
            .get_peer_vectors(&self.page_id, user_did, device_id)
            .map_err(|e| ScribeError::Other(format!("Store error: {}", e)))
    }

    #[instrument(skip_all)]
    fn save_vectors(
        &self,
        user_did: &str,
        device_id: &str,
        vectors: &HashMap<String, Vec<u8>>,
    ) -> Result<()> {
        self.store
            .put_peer_vectors(&self.page_id, user_did, device_id, vectors)
            .map_err(|e| ScribeError::Other(format!("Store error: {}", e)))
    }
}

// Peer Resolver Implementation

/// Butler's implementation of PeerResolver (node mode only)
///
/// Lists authorized users and loads their permits from RedbStore.
/// Scoped to a single page (page_id passed at construction).
pub struct ButlerPeerResolver {
    store: Arc<RedbStore>,
    page_id: String,
}

impl ButlerPeerResolver {
    pub fn new(store: Arc<RedbStore>, page_id: String) -> Self {
        Self { store, page_id }
    }
}

impl PeerResolver for ButlerPeerResolver {
    #[instrument(skip_all)]
    fn list_authorized_users(&self) -> Vec<String> {
        self.store
            .list_authorized_users_for_page(&self.page_id)
            .unwrap_or_default()
    }

    #[instrument(skip_all)]
    fn load_user_permit(&self, user_did: &str) -> Option<String> {
        self.store
            .get_user_page_permit(&self.page_id, user_did)
            .ok()
            .flatten()
    }
}
