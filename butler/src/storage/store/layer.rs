//! Layer table operations (encrypted snapshot bytes)
//!
//! Key: {page_id}/{layer_name} (hierarchical)

use redb::{ReadableTable, ReadableDatabase};
use crate::error::Result;
use super::{RedbStore, LAYERS};

impl RedbStore {
    /// Store a layer with hierarchical key: {page_id}/{layer_name}
    pub fn put_layer(
        &self,
        page_id: &str,
        layer_name: &str,
        encrypted_bytes: &[u8],
    ) -> Result<()> {
        let key = format!("{}/{}", page_id, layer_name);
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(LAYERS)?;
            table.insert(key.as_str(), encrypted_bytes)?;
        }
        write_txn.commit()?;
        Ok(())
    }

    /// Get a layer by page_id and layer_name
    pub fn get_layer(
        &self,
        page_id: &str,
        layer_name: &str,
    ) -> Result<Option<Vec<u8>>> {
        let key = format!("{}/{}", page_id, layer_name);
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(LAYERS)?;

        match table.get(key.as_str())? {
            Some(guard) => Ok(Some(guard.value().to_vec())),
            None => Ok(None),
        }
    }

    /// Delete a specific layer
    pub fn delete_layer(&self, page_id: &str, layer_name: &str) -> Result<bool> {
        let key = format!("{}/{}", page_id, layer_name);
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
            if let Some(layer_name) = key_str.strip_prefix(&prefix) {
                layer_names.push(layer_name.to_string());
            }
        }
        Ok(layer_names)
    }

    /// Delete all layers for a page
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
    pub fn layer_exists(&self, page_id: &str, layer_name: &str) -> Result<bool> {
        let key = format!("{}/{}", page_id, layer_name);
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(LAYERS)?;
        Ok(table.get(key.as_str())?.is_some())
    }
}
