//! Page table operations
//!
//! Key: {page_id} (v2 schema - page_id is globally unique)
//! Also maintains SPACE_PAGES index: {space_id}/{page_id} → ()

use redb::{ReadableTable, ReadableDatabase};
use crate::error::{ButlerError, Result};
use crate::models::PageData;
use tracing::instrument;
use super::{RedbStore, PAGES, SPACE_PAGES, LAYERS};

impl RedbStore {
    #[instrument(skip_all)]
    pub fn put_page(&self, page: &PageData) -> Result<()> {
        let value = bincode::serialize(page)
            .map_err(|e| ButlerError::Serialization(e.to_string()))?;

        let write_txn = self.db.begin_write()?;
        {
            // Store page by page_id
            let mut pages_table = write_txn.open_table(PAGES)?;
            pages_table.insert(page.meta.id.as_str(), value.as_slice())?;

            // Update SPACE_PAGES index
            let mut index_table = write_txn.open_table(SPACE_PAGES)?;
            let index_key = format!("{}/{}", page.meta.space_id, page.meta.id);
            index_table.insert(index_key.as_str(), &[] as &[u8])?;
        }
        write_txn.commit()?;
        Ok(())
    }

    /// Get page by page_id (v2 - no space_id needed)
    #[instrument(skip_all)]
    pub fn get_page_by_id(&self, page_id: &str) -> Result<Option<PageData>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(PAGES)?;

        match table.get(page_id)? {
            Some(guard) => {
                let bytes = guard.value();
                let page: PageData = bincode::deserialize(bytes)
                    .map_err(|e| ButlerError::Serialization(e.to_string()))?;
                Ok(Some(page))
            }
            None => Ok(None),
        }
    }

    /// Get page by ID.
    #[instrument(skip_all)]
    pub fn get_page(&self, _space_id: &str, page_id: &str) -> Result<Option<PageData>> {
        self.get_page_by_id(page_id)
    }

    #[instrument(skip_all)]
    pub fn delete_page(&self, space_id: &str, page_id: &str) -> Result<bool> {
        let write_txn = self.db.begin_write()?;
        let removed = {
            // Remove from PAGES table
            let mut pages_table = write_txn.open_table(PAGES)?;
            let result = pages_table.remove(page_id)?.is_some();

            // Remove from SPACE_PAGES index
            let mut index_table = write_txn.open_table(SPACE_PAGES)?;
            let index_key = format!("{}/{}", space_id, page_id);
            let _ = index_table.remove(index_key.as_str())?;

            result
        };
        write_txn.commit()?;
        Ok(removed)
    }

    /// List pages in space using SPACE_PAGES index
    #[instrument(skip_all)]
    pub fn list_pages_in_space(&self, space_id: &str) -> Result<Vec<PageData>> {
        let read_txn = self.db.begin_read()?;
        let index_table = read_txn.open_table(SPACE_PAGES)?;
        let pages_table = read_txn.open_table(PAGES)?;

        let prefix = format!("{}/", space_id);
        let mut pages = Vec::new();

        for result in index_table.range(prefix.as_str()..)? {
            let (key, _) = result?;
            let key_str = key.value();

            // Stop if we've passed the prefix
            if !key_str.starts_with(&prefix) {
                break;
            }

            // Extract page_id from index key
            if let Some(page_id) = key_str.strip_prefix(&prefix) {
                if let Some(page_guard) = pages_table.get(page_id)? {
                    let page: PageData = bincode::deserialize(page_guard.value())
                        .map_err(|e| ButlerError::Serialization(e.to_string()))?;
                    pages.push(page);
                }
            }
        }
        Ok(pages)
    }

    /// List page IDs in space (without loading full PageData)
    #[instrument(skip_all)]
    pub fn list_page_ids_in_space(&self, space_id: &str) -> Result<Vec<String>> {
        let read_txn = self.db.begin_read()?;
        let index_table = read_txn.open_table(SPACE_PAGES)?;

        let prefix = format!("{}/", space_id);
        let mut page_ids = Vec::new();

        for result in index_table.range(prefix.as_str()..)? {
            let (key, _) = result?;
            let key_str = key.value();

            if !key_str.starts_with(&prefix) {
                break;
            }

            if let Some(page_id) = key_str.strip_prefix(&prefix) {
                page_ids.push(page_id.to_string());
            }
        }
        Ok(page_ids)
    }

    #[instrument(skip_all)]
    pub fn list_all_pages(&self) -> Result<Vec<PageData>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(PAGES)?;

        let mut pages = Vec::new();
        for result in table.iter()? {
            let (_, value) = result?;
            let page: PageData = bincode::deserialize(value.value())
                .map_err(|e| ButlerError::Serialization(e.to_string()))?;
            pages.push(page);
        }
        Ok(pages)
    }

    /// Find a page by ID (v2 - direct lookup, no scan needed)
    #[instrument(skip_all)]
    pub fn find_page_by_id(&self, page_id: &str) -> Result<Option<PageData>> {
        self.get_page_by_id(page_id)
    }

    /// Get all layers for a page (returns encrypted bytes mapped by layer name)
    #[instrument(skip_all)]
    pub fn get_all_layers(&self, page_id: &str) -> Result<Vec<(String, Vec<u8>)>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(LAYERS)?;

        let prefix = format!("{}/", page_id);
        let mut layers = Vec::new();

        for result in table.range(prefix.as_str()..)? {
            let (key, value) = result?;
            let key_str = key.value();

            // Stop if we've passed the prefix
            if !key_str.starts_with(&prefix) {
                break;
            }

            // Extract layer_name from key
            if let Some(layer_name) = key_str.strip_prefix(&prefix) {
                layers.push((layer_name.to_string(), value.value().to_vec()));
            }
        }
        Ok(layers)
    }
}
