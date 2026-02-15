//! Peer Vector Operations (for CRDT sync state tracking)
//!
//! Key: {page_id}/{user_did}/{device_id}
//! Value: HashMap<layer_name, state_vector_bytes>

use super::{RedbStore, VECTORS};
use crate::error::{ButlerError, Result};
use redb::ReadableDatabase;
use std::collections::HashMap;
use tracing::instrument;

impl RedbStore {
    /// Get peer vectors for a specific peer on a page.
    ///
    /// **Context**: Used during sync to determine what updates a peer needs.
    /// Returns None if we haven't synced with this peer before.
    #[instrument(skip_all)]
    pub fn get_peer_vectors(
        &self,
        page_id: &str,
        user_did: &str,
        device_id: &str,
    ) -> Result<Option<HashMap<String, Vec<u8>>>> {
        let key = Self::key3(page_id, user_did, device_id);
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(VECTORS)?;

        match table.get(key.as_str())? {
            Some(guard) => {
                let bytes = guard.value();
                let vectors: HashMap<String, Vec<u8>> = bincode::deserialize(bytes)
                    .map_err(|e| ButlerError::Serialization(e.to_string()))?;
                Ok(Some(vectors))
            }
            None => Ok(None),
        }
    }

    /// Store peer vectors for a specific peer on a page.
    ///
    /// **Context**: Called after successful sync to record the peer's state.
    /// Enables efficient delta sync on next connection.
    #[instrument(skip_all)]
    pub fn put_peer_vectors(
        &self,
        page_id: &str,
        user_did: &str,
        device_id: &str,
        vectors: &HashMap<String, Vec<u8>>,
    ) -> Result<()> {
        let key = Self::key3(page_id, user_did, device_id);
        let value =
            bincode::serialize(vectors).map_err(|e| ButlerError::Serialization(e.to_string()))?;

        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(VECTORS)?;
            table.insert(key.as_str(), value.as_slice())?;
        }
        write_txn.commit()?;
        Ok(())
    }

    /// Get a single layer's peer vector.
    ///
    /// **Context**: Convenience method for single-layer sync operations.
    #[instrument(skip_all)]
    pub fn get_peer_vector_for_layer(
        &self,
        page_id: &str,
        user_did: &str,
        device_id: &str,
        layer_name: &str,
    ) -> Result<Option<Vec<u8>>> {
        if let Some(vectors) = self.get_peer_vectors(page_id, user_did, device_id)? {
            Ok(vectors.get(layer_name).cloned())
        } else {
            Ok(None)
        }
    }

    /// Update a single layer's peer vector.
    ///
    /// **Context**: Called after syncing a specific layer.
    /// Preserves other layer vectors in the map.
    #[instrument(skip_all)]
    pub fn put_peer_vector_for_layer(
        &self,
        page_id: &str,
        user_did: &str,
        device_id: &str,
        layer_name: &str,
        vector: Vec<u8>,
    ) -> Result<()> {
        let mut vectors = self
            .get_peer_vectors(page_id, user_did, device_id)?
            .unwrap_or_default();
        vectors.insert(layer_name.to_string(), vector);
        self.put_peer_vectors(page_id, user_did, device_id, &vectors)
    }

    /// Delete all peer vectors for a specific peer on a page.
    ///
    /// **Context**: Called when a peer's access is revoked.
    #[instrument(skip_all)]
    pub fn delete_peer_vectors(
        &self,
        page_id: &str,
        user_did: &str,
        device_id: &str,
    ) -> Result<bool> {
        let key = Self::key3(page_id, user_did, device_id);
        let write_txn = self.db.begin_write()?;
        let removed = {
            let mut table = write_txn.open_table(VECTORS)?;
            let result = table.remove(key.as_str())?;
            result.is_some()
        };
        write_txn.commit()?;
        Ok(removed)
    }

    /// Delete all peer vectors for a page (all peers).
    ///
    /// **Context**: Called when a page is deleted.
    #[instrument(skip_all)]
    pub fn delete_all_peer_vectors_for_page(&self, page_id: &str) -> Result<usize> {
        let prefix = Self::prefix1(page_id);
        let keys_to_delete: Vec<String> = self
            .scan_prefix_bytes(VECTORS, prefix.as_str())?
            .into_iter()
            .map(|(rest, _)| Self::key2(page_id, rest.as_str()))
            .collect();

        let count = keys_to_delete.len();
        if count > 0 {
            let write_txn = self.db.begin_write()?;
            {
                let mut table = write_txn.open_table(VECTORS)?;
                for key in &keys_to_delete {
                    table.remove(key.as_str())?;
                }
            }
            write_txn.commit()?;
        }
        Ok(count)
    }

    /// List all peers with vectors for a given page.
    ///
    /// **Context**: Used to iterate over all known peers for a page.
    /// Returns tuples of (user_did, device_id).
    #[instrument(skip_all)]
    pub fn list_peers_for_page(&self, page_id: &str) -> Result<Vec<(String, String)>> {
        let prefix = Self::prefix1(page_id);
        let rows = self.scan_prefix_bytes(VECTORS, prefix.as_str())?;
        let mut peers = Vec::new();
        for (rest, _) in rows {
            // Parse {page_id}/{user_did}/{device_id}
            if let Some((user_did, device_id)) = rest.split_once('/') {
                peers.push((user_did.to_string(), device_id.to_string()));
            }
        }
        Ok(peers)
    }
}
