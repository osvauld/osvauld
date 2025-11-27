//! LayerService - Service for Layer lifecycle management
//!
//! Terminology:
//! - Space: Container for Pages
//! - Page: A sub-application instance
//! - Layer: CRDT data containers (the actual collaborative state)

use crate::error::{ButlerError, Result};
use crate::storage::{LayerCache, LayerCacheStats};
use crate::models::Layer;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time;

/// Service for Layer lifecycle management
///
/// - Load layers on demand (from cache or redb)
/// - Apply peer updates (merge)
/// - Periodic flush to persistent storage
/// - Background flush loop
///
/// ## Key Format
/// All methods use `page_id` + `layer_name` to identify layers.
/// Layer names come from permit templates (e.g., "content", "comments").
pub struct LayerService {
    cache: Arc<RwLock<LayerCache>>,
}

impl LayerService {
    pub fn new(cache: Arc<RwLock<LayerCache>>) -> Self {
        Self { cache }
    }

    /// Get a layer for reading
    ///
    /// Loads from redb if not in cache.
    pub async fn get_layer<F>(
        &self,
        page_id: &str,
        layer_name: &str,
        decrypt_fn: F,
    ) -> Result<Option<Layer>>
    where
        F: FnOnce(&[u8]) -> Result<Vec<u8>>,
    {
        let mut cache = self.cache.write().await;

        // Try to get from cache (loading from redb if needed)
        if let Some(layer) = cache.get(page_id, layer_name, decrypt_fn)? {
            // Clone the layer for the caller
            Ok(Some(Layer::from_snapshot(&layer.export_snapshot())
                .map_err(|e| ButlerError::Loro(e.to_string()))?))
        } else {
            Ok(None)
        }
    }

    /// Get a layer mutably for editing
    ///
    /// Marks the layer as dirty automatically.
    pub async fn get_layer_mut<F>(
        &self,
        page_id: &str,
        layer_name: &str,
        decrypt_fn: F,
    ) -> Result<bool>
    where
        F: FnOnce(&[u8]) -> Result<Vec<u8>>,
    {
        let mut cache = self.cache.write().await;
        Ok(cache.get_mut(page_id, layer_name, decrypt_fn)?.is_some())
    }

    /// Create a new layer and add to cache
    pub async fn create_layer(&self, page_id: String, layer_name: String) -> Result<()> {
        let layer = Layer::new();
        let mut cache = self.cache.write().await;
        cache.insert(page_id, layer_name, layer);
        Ok(())
    }

    /// Apply updates from a peer (merge operation)
    ///
    /// Loads the layer if not in cache, applies the update, marks dirty.
    pub async fn apply_update<F>(
        &self,
        page_id: &str,
        layer_name: &str,
        update: &[u8],
        decrypt_fn: F,
    ) -> Result<()>
    where
        F: FnOnce(&[u8]) -> Result<Vec<u8>>,
    {
        let mut cache = self.cache.write().await;
        cache.apply_update(page_id, layer_name, update, decrypt_fn)
    }

    /// Get updates to send to a peer (since their version)
    pub async fn get_updates_for_peer<F>(
        &self,
        page_id: &str,
        layer_name: &str,
        peer_version: &[u8],
        decrypt_fn: F,
    ) -> Result<Option<Vec<u8>>>
    where
        F: FnOnce(&[u8]) -> Result<Vec<u8>>,
    {
        let mut cache = self.cache.write().await;

        if let Some(layer) = cache.get(page_id, layer_name, decrypt_fn)? {
            let updates = layer.export_updates(peer_version)
                .map_err(|e| ButlerError::Loro(e.to_string()))?;
            Ok(Some(updates))
        } else {
            Ok(None)
        }
    }

    /// Get the version vector for a layer
    pub async fn get_version_vector<F>(
        &self,
        page_id: &str,
        layer_name: &str,
        decrypt_fn: F,
    ) -> Result<Option<Vec<u8>>>
    where
        F: FnOnce(&[u8]) -> Result<Vec<u8>>,
    {
        let mut cache = self.cache.write().await;

        if let Some(layer) = cache.get(page_id, layer_name, decrypt_fn)? {
            Ok(Some(layer.version_vector()))
        } else {
            Ok(None)
        }
    }

    /// Flush all dirty layers to redb
    pub async fn flush_all<F>(&self, encrypt_fn: F) -> Result<usize>
    where
        F: Fn(&[u8]) -> Result<Vec<u8>>,
    {
        let mut cache = self.cache.write().await;
        cache.flush_all(encrypt_fn)
    }

    /// Flush a single layer to redb
    pub async fn flush_layer<F>(
        &self,
        page_id: &str,
        layer_name: &str,
        encrypt_fn: F,
    ) -> Result<bool>
    where
        F: FnOnce(&[u8]) -> Result<Vec<u8>>,
    {
        let mut cache = self.cache.write().await;
        cache.flush_layer(page_id, layer_name, encrypt_fn)
    }

    /// Mark a layer as dirty (needs flush)
    pub async fn mark_dirty(&self, page_id: &str, layer_name: &str) {
        let mut cache = self.cache.write().await;
        cache.mark_dirty(page_id, layer_name);
    }

    /// Get list of dirty layers as (page_id, layer_name) tuples
    pub async fn dirty_layers(&self) -> Vec<(String, String)> {
        let cache = self.cache.read().await;
        cache.dirty_layers()
    }

    /// Check if a layer is in cache
    pub async fn is_cached(&self, page_id: &str, layer_name: &str) -> bool {
        let cache = self.cache.read().await;
        cache.contains(page_id, layer_name)
    }

    /// Get cache statistics
    pub async fn cache_stats(&self) -> LayerCacheStats {
        let cache = self.cache.read().await;
        cache.stats()
    }

    /// Evict a layer from cache (does not delete from redb)
    pub async fn evict(&self, page_id: &str, layer_name: &str) -> Option<Layer> {
        let mut cache = self.cache.write().await;
        cache.evict(page_id, layer_name)
    }

    /// Start a background flush loop
    ///
    /// Periodically flushes dirty layers to redb.
    pub fn start_flush_loop<F>(
        self: Arc<Self>,
        interval: Duration,
        encrypt_fn: F,
    ) -> tokio::task::JoinHandle<()>
    where
        F: Fn(&[u8]) -> Result<Vec<u8>> + Send + Sync + 'static,
    {
        let encrypt_fn = Arc::new(encrypt_fn);

        tokio::spawn(async move {
            let mut interval_timer = time::interval(interval);

            loop {
                interval_timer.tick().await;

                let encrypt = encrypt_fn.clone();
                match self.flush_all(move |data| encrypt(data)).await {
                    Ok(count) => {
                        if count > 0 {
                            log::debug!("Flushed {} dirty layers to redb", count);
                        }
                    }
                    Err(e) => {
                        log::error!("Error flushing layers: {}", e);
                    }
                }
            }
        })
    }
}
