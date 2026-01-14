//! LayerCache - In-memory cache for Layer instances
//!
//! Terminology:
//! - Space: Container for Pages
//! - Page: A sub-application instance
//! - Layer: CRDT data containers (the actual collaborative state)

use crate::error::{ButlerError, Result};
use crate::storage::RedbStore;
use crate::models::Layer;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

/// Cached Layer entry with metadata
pub struct CachedLayer {
    /// The layer (decrypted, in memory) - uses core's Layer wrapper
    pub layer: Layer,
    /// Whether this layer has unsaved changes
    pub dirty: bool,
    /// Last access time for LRU eviction
    pub last_accessed: Instant,
    /// Page ID (needed for flush)
    pub page_id: String,
    /// Layer name (needed for flush)
    pub layer_name: String,
}

impl CachedLayer {
    fn new(layer: Layer, page_id: String, layer_name: String) -> Self {
        Self {
            layer,
            dirty: false,
            last_accessed: Instant::now(),
            page_id,
            layer_name,
        }
    }

    fn touch(&mut self) {
        self.last_accessed = Instant::now();
    }
}

/// In-memory cache for Layer instances
///
/// - Source of truth during runtime (decrypted layers)
/// - Required for merge operations (can't merge encrypted bytes)
/// - Uses core's Layer wrapper (never loro directly)
/// - LRU eviction for memory management
/// - Periodic flush to redb
///
/// ## Key Format
/// Cache uses composite keys: `{page_id}/{layer_name}` internally.
/// All layers for a page use the same AES key (from PageMeta.encrypted_key).
pub struct LayerCache {
    layers: HashMap<String, CachedLayer>,
    max_size: usize,
    store: Arc<RedbStore>,
}

/// Create composite cache key from page_id and layer_name
fn cache_key(page_id: &str, layer_name: &str) -> String {
    format!("{}/{}", page_id, layer_name)
}

impl LayerCache {
    pub fn new(store: Arc<RedbStore>, max_size: usize) -> Self {
        Self {
            layers: HashMap::new(),
            max_size,
            store,
        }
    }

    /// Check if layer is in cache
    pub fn contains(&self, page_id: &str, layer_name: &str) -> bool {
        self.layers.contains_key(&cache_key(page_id, layer_name))
    }

    /// Get number of cached layers
    pub fn len(&self) -> usize {
        self.layers.len()
    }

    /// Check if cache is empty
    pub fn is_empty(&self) -> bool {
        self.layers.is_empty()
    }

    /// Get a layer from cache (loading from redb if needed)
    ///
    /// # Arguments
    /// * `page_id` - Page ID
    /// * `layer_name` - Layer name (from permit template)
    /// * `decrypt_fn` - Function to decrypt bytes from redb
    ///
    /// Returns None if layer doesn't exist in cache or redb
    pub fn get<F>(
        &mut self,
        page_id: &str,
        layer_name: &str,
        decrypt_fn: F,
    ) -> Result<Option<&Layer>>
    where
        F: FnOnce(&[u8]) -> Result<Vec<u8>>,
    {
        let key = cache_key(page_id, layer_name);

        // If in cache, touch and return
        if self.layers.contains_key(&key) {
            if let Some(entry) = self.layers.get_mut(&key) {
                entry.touch();
            }
            return Ok(self.layers.get(&key).map(|e| &e.layer));
        }

        // Try to load from redb
        if let Some(encrypted_bytes) = self.store.get_layer(page_id, layer_name)? {
            let decrypted = decrypt_fn(&encrypted_bytes)?;
            let layer = Layer::from_snapshot(&decrypted)
                .map_err(|e| ButlerError::Loro(e.to_string()))?;

            // Evict if needed before inserting
            self.evict_if_needed();

            self.layers.insert(
                key.clone(),
                CachedLayer::new(layer, page_id.to_string(), layer_name.to_string()),
            );
            return Ok(self.layers.get(&key).map(|e| &e.layer));
        }

        Ok(None)
    }

    /// Get a layer mutably for editing
    ///
    /// Automatically marks the layer as dirty.
    pub fn get_mut<F>(
        &mut self,
        page_id: &str,
        layer_name: &str,
        decrypt_fn: F,
    ) -> Result<Option<&mut Layer>>
    where
        F: FnOnce(&[u8]) -> Result<Vec<u8>>,
    {
        let key = cache_key(page_id, layer_name);

        // First ensure it's loaded
        let _ = self.get(page_id, layer_name, decrypt_fn)?;

        // Now get mutable reference and mark dirty
        if let Some(entry) = self.layers.get_mut(&key) {
            entry.touch();
            entry.dirty = true;
            return Ok(Some(&mut entry.layer));
        }

        Ok(None)
    }

    /// Insert a new layer into the cache
    ///
    /// The layer is marked dirty so it will be flushed to redb.
    pub fn insert(&mut self, page_id: String, layer_name: String, layer: Layer) {
        self.evict_if_needed();
        let key = cache_key(&page_id, &layer_name);
        let mut entry = CachedLayer::new(layer, page_id, layer_name);
        entry.dirty = true; // New layers need to be saved
        self.layers.insert(key, entry);
    }

    /// Mark a layer as dirty (needs flush)
    pub fn mark_dirty(&mut self, page_id: &str, layer_name: &str) {
        let key = cache_key(page_id, layer_name);
        if let Some(entry) = self.layers.get_mut(&key) {
            entry.dirty = true;
        }
    }

    /// Mark a layer as clean (after flush)
    pub fn mark_clean(&mut self, page_id: &str, layer_name: &str) {
        let key = cache_key(page_id, layer_name);
        if let Some(entry) = self.layers.get_mut(&key) {
            entry.dirty = false;
        }
    }

    /// Get list of dirty layer keys (page_id, layer_name)
    pub fn dirty_layers(&self) -> Vec<(String, String)> {
        self.layers
            .values()
            .filter(|entry| entry.dirty)
            .map(|entry| (entry.page_id.clone(), entry.layer_name.clone()))
            .collect()
    }

    /// Apply update bytes to a layer (merge operation)
    ///
    /// Loads the layer if not in cache, applies the update, marks dirty.
    pub fn apply_update<F>(
        &mut self,
        page_id: &str,
        layer_name: &str,
        update: &[u8],
        decrypt_fn: F,
    ) -> Result<()>
    where
        F: FnOnce(&[u8]) -> Result<Vec<u8>>,
    {
        let key = cache_key(page_id, layer_name);

        // Ensure layer is loaded
        let _ = self.get(page_id, layer_name, decrypt_fn)?;

        if let Some(entry) = self.layers.get_mut(&key) {
            entry.layer.apply(update)
                .map_err(|e| ButlerError::Loro(e.to_string()))?;
            entry.dirty = true;
            entry.touch();
            Ok(())
        } else {
            Err(ButlerError::LayerNotFound(key))
        }
    }

    /// Flush all dirty layers to redb
    ///
    /// # Arguments
    /// * `encrypt_fn` - Function to encrypt snapshot bytes before storing
    pub fn flush_all<F>(&mut self, encrypt_fn: F) -> Result<usize>
    where
        F: Fn(&[u8]) -> Result<Vec<u8>>,
    {
        let dirty_layers = self.dirty_layers();
        let mut flushed = 0;

        for (page_id, layer_name) in dirty_layers {
            let key = cache_key(&page_id, &layer_name);
            if let Some(entry) = self.layers.get_mut(&key) {
                let snapshot = entry.layer.export_snapshot();
                let encrypted = encrypt_fn(&snapshot)?;
                self.store.put_layer(&page_id, &layer_name, &encrypted)?;
                entry.dirty = false;
                flushed += 1;
            }
        }

        // Also flush redb to disk
        self.store.flush()?;

        Ok(flushed)
    }

    /// Flush a single layer to redb
    pub fn flush_layer<F>(
        &mut self,
        page_id: &str,
        layer_name: &str,
        encrypt_fn: F,
    ) -> Result<bool>
    where
        F: FnOnce(&[u8]) -> Result<Vec<u8>>,
    {
        let key = cache_key(page_id, layer_name);
        if let Some(entry) = self.layers.get_mut(&key) {
            if entry.dirty {
                let snapshot = entry.layer.export_snapshot();
                let encrypted = encrypt_fn(&snapshot)?;
                self.store.put_layer(page_id, layer_name, &encrypted)?;
                entry.dirty = false;
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Remove a layer from cache (does not delete from redb)
    pub fn evict(&mut self, page_id: &str, layer_name: &str) -> Option<Layer> {
        let key = cache_key(page_id, layer_name);
        self.layers.remove(&key).map(|e| e.layer)
    }

    /// LRU eviction - remove least recently accessed layers until under max_size
    fn evict_if_needed(&mut self) {
        while self.layers.len() >= self.max_size {
            // Find the oldest non-dirty entry
            let oldest = self.layers
                .iter()
                .filter(|(_, e)| !e.dirty) // Don't evict dirty layers
                .min_by_key(|(_, e)| e.last_accessed)
                .map(|(id, _)| id.clone());

            if let Some(id) = oldest {
                self.layers.remove(&id);
            } else {
                // All layers are dirty, can't evict safely
                break;
            }
        }
    }

    /// Get cache stats
    pub fn stats(&self) -> LayerCacheStats {
        let dirty_count = self.layers.iter().filter(|(_, e)| e.dirty).count();
        LayerCacheStats {
            total: self.layers.len(),
            dirty: dirty_count,
            max_size: self.max_size,
        }
    }
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct LayerCacheStats {
    pub total: usize,
    pub dirty: usize,
    pub max_size: usize,
}
