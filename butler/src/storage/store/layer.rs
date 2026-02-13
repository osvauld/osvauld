//! Layer table operations (encrypted snapshot bytes)
//!
//! Key: {page_id}/{layer_name} (hierarchical)

use super::{RedbStore, APP_HASHES, LAYERS};
use crate::error::Result;
use redb::{ReadableDatabase, ReadableTable};
use tracing::{info, instrument};

impl RedbStore {
    /// Store a layer with hierarchical key: {page_id}/{layer_name}
    ///
    /// Note: If layer_name already starts with page_id prefix, it's used as-is
    /// to avoid double-prefixing (Scribe stores layers with full names).
    #[instrument(skip_all)]
    pub fn put_layer(&self, page_id: &str, layer_name: &str, encrypted_bytes: &[u8]) -> Result<()> {
        let prefix = format!("{}/", page_id);
        let key = if layer_name.starts_with(&prefix) {
            // Layer name already includes page_id prefix - use as-is
            layer_name.to_string()
        } else {
            // Layer name without prefix - prepend page_id
            format!("{}/{}", page_id, layer_name)
        };
        info!(page_id = %page_id, layer_name = %layer_name, key = %key, bytes = encrypted_bytes.len(), "Writing layer to DB");
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(LAYERS)?;
            table.insert(key.as_str(), encrypted_bytes)?;
        }
        write_txn.commit()?;
        Ok(())
    }

    /// Get a layer by page_id and layer_name
    ///
    /// Note: If layer_name already starts with page_id prefix, it's used as-is.
    #[instrument(skip_all)]
    pub fn get_layer(&self, page_id: &str, layer_name: &str) -> Result<Option<Vec<u8>>> {
        let prefix = format!("{}/", page_id);
        let key = if layer_name.starts_with(&prefix) {
            layer_name.to_string()
        } else {
            format!("{}/{}", page_id, layer_name)
        };
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(LAYERS)?;

        match table.get(key.as_str())? {
            Some(guard) => Ok(Some(guard.value().to_vec())),
            None => Ok(None),
        }
    }

    /// Delete a specific layer
    ///
    /// Note: If layer_name already starts with page_id prefix, it's used as-is.
    #[instrument(skip_all)]
    pub fn delete_layer(&self, page_id: &str, layer_name: &str) -> Result<bool> {
        let prefix = format!("{}/", page_id);
        let key = if layer_name.starts_with(&prefix) {
            layer_name.to_string()
        } else {
            format!("{}/{}", page_id, layer_name)
        };
        let write_txn = self.db.begin_write()?;
        let removed = {
            let mut table = write_txn.open_table(LAYERS)?;
            let result = table.remove(key.as_str())?;
            result.is_some()
        };
        write_txn.commit()?;
        Ok(removed)
    }

    /// List all layer names for a page
    ///
    /// Returns layer names WITHOUT the page_id prefix.
    /// Handles legacy double-prefixed keys ({page_id}/{page_id}/layer) by
    /// stripping both prefixes.
    #[instrument(skip_all)]
    pub fn list_layer_names(&self, page_id: &str) -> Result<Vec<String>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(LAYERS)?;

        let prefix = format!("{}/", page_id);
        let mut layer_names = Vec::new();

        for result in table.range(prefix.as_str()..)? {
            let (key, _value) = result?;
            let key_str = key.value();

            // Stop if we've passed the prefix
            if !key_str.starts_with(&prefix) {
                break;
            }

            // Extract layer_name from key
            if let Some(mut layer_name) = key_str.strip_prefix(&prefix) {
                // Handle legacy double-prefix: if layer_name still starts with page_id/,
                // strip it again to get the actual layer name
                if let Some(fixed_name) = layer_name.strip_prefix(&prefix) {
                    layer_name = fixed_name;
                }
                layer_names.push(layer_name.to_string());
            }
        }
        Ok(layer_names)
    }

    /// Delete all layers for a page
    #[instrument(skip_all)]
    pub fn delete_all_layers(&self, page_id: &str) -> Result<usize> {
        let layer_names = self.list_layer_names(page_id)?;
        let count = layer_names.len();

        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(LAYERS)?;
            for layer_name in &layer_names {
                let key = format!("{}/{}", page_id, layer_name);
                table.remove(key.as_str())?;
            }
        }
        write_txn.commit()?;
        Ok(count)
    }

    /// Check if a layer exists
    #[instrument(skip_all)]
    pub fn layer_exists(&self, page_id: &str, layer_name: &str) -> Result<bool> {
        let key = format!("{}/{}", page_id, layer_name);
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(LAYERS)?;
        Ok(table.get(key.as_str())?.is_some())
    }

    // App Layer Content Hash Operations

    /// Store content hash for an app layer
    ///
    /// **Context**: After syncing an app layer, store its content hash
    /// **Key**: `{page_id}/{layer_name}`
    /// **Value**: SHA-256 hash bytes (32 bytes)
    #[instrument(skip_all)]
    pub fn put_app_hash(&self, page_id: &str, layer_name: &str, hash: &[u8; 32]) -> Result<()> {
        let key = format!("{}/{}", page_id, layer_name);
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(APP_HASHES)?;
            table.insert(key.as_str(), hash.as_slice())?;
        }
        write_txn.commit()?;
        Ok(())
    }

    /// Get content hash for an app layer
    ///
    /// **Returns**: SHA-256 hash if stored, None if layer hasn't been hashed
    #[instrument(skip_all)]
    pub fn get_app_hash(&self, page_id: &str, layer_name: &str) -> Result<Option<[u8; 32]>> {
        let key = format!("{}/{}", page_id, layer_name);
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(APP_HASHES)?;
        match table.get(key.as_str())? {
            Some(guard) => {
                let bytes = guard.value();
                if bytes.len() == 32 {
                    let mut hash = [0u8; 32];
                    hash.copy_from_slice(bytes);
                    Ok(Some(hash))
                } else {
                    Ok(None)
                }
            }
            None => Ok(None),
        }
    }

    /// Delete content hash for an app layer
    #[instrument(skip_all)]
    pub fn delete_app_hash(&self, page_id: &str, layer_name: &str) -> Result<bool> {
        let key = format!("{}/{}", page_id, layer_name);
        let write_txn = self.db.begin_write()?;
        let removed;
        {
            let mut table = write_txn.open_table(APP_HASHES)?;
            let result = table.remove(key.as_str())?;
            removed = result.is_some();
        }
        write_txn.commit()?;
        Ok(removed)
    }
}
