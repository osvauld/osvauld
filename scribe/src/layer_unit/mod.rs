//! Layer Unit — Per-layer compute unit
//!
//! Consolidates all per-layer state (LoroDoc, dirty flag, local-only flag,
//! observer subscription) into a single struct. Makes Scribe a thin router
//! over LayerUnit instances.
//!
//! Also contains dynamic layer management (schema validation, permit preparation).

mod dynamic;

pub use dynamic::{
    handle_create_dynamic_layer, prepare_layer_permits, find_matching_dynamic_schema,
    handle_add_layer_access, handle_remove_layer_access,
};

use std::collections::HashSet;
use std::sync::{Arc, RwLock};

use domains::Layer;

/// Per-layer compute unit — owns all state for a single layer
pub struct LayerUnit {
    /// The CRDT document
    layer: Layer,
    /// Needs flush to storage?
    dirty: bool,
    /// sync: false in permit — never broadcast
    is_local_only: bool,
    /// Created via dynamic_layer_schema (not static template)
    pub is_dynamic: bool,
    /// Loro observer subscription (dropped = unsubscribed)
    loro_sub: Option<loro::Subscription>,
    /// Which subscriber DIDs are authorized to receive updates for this layer.
    /// Arc<RwLock> because the observer async task needs shared read access.
    /// Populated at subscription time (page permit) and permit issuance (layer permits).
    authorized_dids: Arc<RwLock<HashSet<String>>>,
}

impl LayerUnit {
    /// Create a LayerUnit wrapping an existing layer
    pub fn new(layer: Layer) -> Self {
        Self {
            layer,
            dirty: false,
            is_local_only: false,
            is_dynamic: false,
            loro_sub: None,
            authorized_dids: Arc::new(RwLock::new(HashSet::new())),
        }
    }

    /// Create a LayerUnit with an empty LoroDoc
    pub fn new_empty() -> Self {
        Self::new(Layer::new())
    }

    // Layer access

    /// Get a reference to the inner Layer
    pub fn layer(&self) -> &Layer {
        &self.layer
    }

    /// Get a mutable reference to the inner Layer
    pub fn layer_mut(&mut self) -> &mut Layer {
        &mut self.layer
    }

    /// Replace the inner layer (e.g. snapshot restore)
    pub fn replace_layer(&mut self, layer: Layer) {
        self.layer = layer;
    }

    // Dirty tracking

    /// Mark this layer as needing persistence
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Check if this layer needs persistence
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Clear the dirty flag without exporting
    pub fn clear_dirty(&mut self) {
        self.dirty = false;
    }

    /// Export snapshot if dirty, clear flag. Returns None if clean.
    pub fn take_dirty_snapshot(&mut self) -> Option<Vec<u8>> {
        if self.dirty {
            self.dirty = false;
            Some(self.layer.export_snapshot())
        } else {
            None
        }
    }

    // Metadata

    /// Check if this layer is local-only (sync: false)
    pub fn is_local_only(&self) -> bool {
        self.is_local_only
    }

    /// Set the local-only flag
    pub fn set_local_only(&mut self, val: bool) {
        self.is_local_only = val;
    }

    /// Check if this layer was created dynamically
    pub fn is_dynamic(&self) -> bool {
        self.is_dynamic
    }

    // Observer lifecycle

    /// Check if this layer has an active Loro observer
    pub fn has_observer(&self) -> bool {
        self.loro_sub.is_some()
    }

    /// Set the Loro observer subscription
    pub fn set_observer(&mut self, sub: loro::Subscription) {
        self.loro_sub = Some(sub);
    }

    /// Drop the observer (unsubscribes from Loro)
    pub fn clear_observer(&mut self) {
        self.loro_sub = None;
    }

    // Authorization

    /// Get shared reference to authorized_dids (for observer to clone the Arc)
    pub fn authorized_dids(&self) -> &Arc<RwLock<HashSet<String>>> {
        &self.authorized_dids
    }

    /// Authorize a DID to receive updates for this layer
    pub fn authorize_did(&self, did: &str) {
        if let Ok(mut set) = self.authorized_dids.write() {
            set.insert(did.to_string());
        }
    }

    /// Revoke a DID's authorization for this layer
    pub fn revoke_did(&self, did: &str) {
        if let Ok(mut set) = self.authorized_dids.write() {
            set.remove(did);
        }
    }

    /// Check if a DID is authorized for this layer
    pub fn is_authorized(&self, did: &str) -> bool {
        self.authorized_dids.read()
            .map(|set| set.contains(did))
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domains::Layer;

    #[test]
    fn test_new_empty() {
        let unit = LayerUnit::new_empty();
        assert!(!unit.is_dirty());
        assert!(!unit.is_local_only());
        assert!(!unit.is_dynamic());
        assert!(!unit.has_observer());
    }

    #[test]
    fn test_new_with_layer() {
        let layer = Layer::new();
        let map = layer.loro().get_map("test");
        map.insert("key", "value").unwrap();
        layer.commit();

        let unit = LayerUnit::new(layer);
        let content = unit.layer().to_json();
        assert!(content.get("test").is_some());
    }

    #[test]
    fn test_dirty_tracking() {
        let mut unit = LayerUnit::new_empty();
        assert!(!unit.is_dirty());
        assert!(unit.take_dirty_snapshot().is_none());

        unit.mark_dirty();
        assert!(unit.is_dirty());

        let snapshot = unit.take_dirty_snapshot();
        assert!(snapshot.is_some());
        // Flag cleared after take
        assert!(!unit.is_dirty());
        assert!(unit.take_dirty_snapshot().is_none());
    }

    #[test]
    fn test_dirty_snapshot_contains_data() {
        let mut unit = LayerUnit::new_empty();
        let list = unit.layer().loro().get_list("items");
        list.push("hello").unwrap();
        unit.layer().commit();
        unit.mark_dirty();

        let snapshot = unit.take_dirty_snapshot().unwrap();
        let restored = Layer::from_snapshot(&snapshot).unwrap();
        let content = restored.to_json();
        assert!(content.get("items").is_some());
    }

    #[test]
    fn test_local_only_flag() {
        let mut unit = LayerUnit::new_empty();
        assert!(!unit.is_local_only());

        unit.set_local_only(true);
        assert!(unit.is_local_only());

        unit.set_local_only(false);
        assert!(!unit.is_local_only());
    }

    #[test]
    fn test_dynamic_flag() {
        let mut unit = LayerUnit::new_empty();
        assert!(!unit.is_dynamic());

        unit.is_dynamic = true;
        assert!(unit.is_dynamic());
    }

    #[test]
    fn test_observer_lifecycle() {
        let mut unit = LayerUnit::new_empty();
        assert!(!unit.has_observer());

        let sub = unit.layer().subscribe_root(|_| {});
        unit.set_observer(sub);
        assert!(unit.has_observer());

        unit.clear_observer();
        assert!(!unit.has_observer());
    }

    #[test]
    fn test_clear_dirty() {
        let mut unit = LayerUnit::new_empty();
        unit.mark_dirty();
        assert!(unit.is_dirty());

        unit.clear_dirty();
        assert!(!unit.is_dirty());
    }

    #[test]
    fn test_replace_layer() {
        let mut unit = LayerUnit::new_empty();
        let map = unit.layer().loro().get_map("old");
        map.insert("key", "val").unwrap();
        unit.layer().commit();

        let new_layer = Layer::new();
        let map2 = new_layer.loro().get_map("new");
        map2.insert("k", "v").unwrap();
        new_layer.commit();

        unit.replace_layer(new_layer);
        let content = unit.layer().to_json();
        assert!(content.get("new").is_some());
        assert!(content.get("old").is_none());
    }
}
