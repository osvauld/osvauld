//! Layer Unit — Per-layer compute unit
//!
//! Consolidates all per-layer state (LoroDoc, dirty flag, authorization config,
//! per-subscriber state, observer subscription) into a single struct. Makes Scribe
//! a thin router over LayerUnit instances.
//!
//! ## Authorization model
//!
//! Each LayerUnit owns its subscribers and their capabilities. Authorization is
//! checked at broadcast time using per-subscriber state, not an eagerly-populated
//! HashSet. The `subscribers` map is `Arc<RwLock<...>>` so the observer async task
//! can read it for broadcast decisions.
//!
//! Also contains dynamic layer management (schema validation, permit preparation).

mod dynamic;

pub use dynamic::{
    find_matching_dynamic_schema, handle_add_layer_access, handle_create_dynamic_layer,
};

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use tokio::sync::mpsc;

use domains::Layer;

use crate::BroadcastPayload;

// === Capability types ===

/// Cached capabilities from a layer permit
///
/// **Context**: Parsed from layer permit at subscription time.
/// Each field maps to a specific authorization check.
#[derive(Debug, Clone, Default)]
pub struct Capabilities {
    /// Can read layer data (receive broadcasts)
    pub read: bool,
    /// Can write to layer (send updates)
    pub write: bool,
    /// Can sync (bidirectional replication)
    pub sync: bool,
}

/// Layer-level configuration derived from permit
///
/// **Context**: Set once at layer creation, describes our own capabilities
/// for this layer (viewer side) or default capabilities (node side).
#[derive(Debug, Clone)]
pub struct LayerConfig {
    /// Our capabilities for this layer
    pub capabilities: Capabilities,
    /// sync: false — never broadcast to peers
    pub is_local_only: bool,
}

impl Default for LayerConfig {
    fn default() -> Self {
        Self {
            capabilities: Capabilities {
                read: true,
                write: true,
                sync: true,
            },
            is_local_only: false,
        }
    }
}

/// Per-subscriber state within a LayerUnit
///
/// **Context**: Tracks what each subscriber can do with this specific layer.
/// Created when a peer subscribes and presents a layer permit.
pub struct LayerSubscriber {
    /// What this subscriber can do (from the layer permit we issued)
    pub capabilities: Capabilities,
    /// Their last known version vector for incremental sync
    pub version_vector: Vec<u8>,
    /// Channel to send broadcasts to this subscriber
    pub broadcast_tx: mpsc::Sender<BroadcastPayload>,
}

/// Per-layer compute unit — owns all state for a single layer
///
/// **Design**: Self-contained authorization and broadcast. Scribe routes
/// messages to LayerUnits; each unit knows its subscribers and their capabilities.
pub struct LayerUnit {
    // === CRDT ===
    /// The CRDT document
    layer: Layer,
    /// Needs flush to storage?
    dirty: bool,
    /// Loro observer subscription (dropped = unsubscribed)
    loro_sub: Option<loro::Subscription>,

    // === Authorization (from permit) ===
    /// Cached layer configuration (our capabilities, local-only flag)
    config: LayerConfig,
    /// Created via dynamic_layer_schema (not static template)
    pub is_dynamic: bool,

    // === Subscriber state ===
    /// Per-subscriber state (did → subscriber)
    /// Arc<RwLock> because the observer async task needs shared read access
    subscribers: Arc<RwLock<HashMap<String, LayerSubscriber>>>,
}

impl LayerUnit {
    /// Create a LayerUnit wrapping an existing layer
    pub fn new(layer: Layer) -> Self {
        Self {
            layer,
            dirty: false,
            loro_sub: None,
            config: LayerConfig::default(),
            is_dynamic: false,
            subscribers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create a LayerUnit with an empty LoroDoc
    pub fn new_empty() -> Self {
        Self::new(Layer::new())
    }

    // === Layer access ===

    /// Get a reference to the inner Layer
    pub fn layer(&self) -> &Layer {
        &self.layer
    }

    /// Replace the inner layer (e.g. snapshot restore)
    pub fn replace_layer(&mut self, layer: Layer) {
        self.layer = layer;
    }

    // === Config ===

    /// Get the layer configuration
    pub fn config(&self) -> &LayerConfig {
        &self.config
    }

    // === Dirty tracking ===

    /// Mark this layer as needing persistence
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Check if this layer needs persistence
    pub fn is_dirty(&self) -> bool {
        self.dirty
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

    // === Metadata ===

    /// Check if this layer is local-only (sync: false)
    pub fn is_local_only(&self) -> bool {
        self.config.is_local_only
    }

    /// Set the local-only flag
    pub fn set_local_only(&mut self, val: bool) {
        self.config.is_local_only = val;
    }

    /// Check if this layer was created dynamically
    pub fn is_dynamic(&self) -> bool {
        self.is_dynamic
    }

    // === Observer lifecycle ===

    /// Check if this layer has an active Loro observer
    pub fn has_observer(&self) -> bool {
        self.loro_sub.is_some()
    }

    /// Set the Loro observer subscription
    pub fn set_observer(&mut self, sub: loro::Subscription) {
        self.loro_sub = Some(sub);
    }

    // === Subscriber management ===

    /// Get shared reference to subscribers (for observer to clone the Arc)
    pub fn subscribers(&self) -> &Arc<RwLock<HashMap<String, LayerSubscriber>>> {
        &self.subscribers
    }

    /// Add a subscriber with their capabilities and broadcast channel
    pub fn add_subscriber(
        &self,
        did: String,
        capabilities: Capabilities,
        broadcast_tx: mpsc::Sender<BroadcastPayload>,
    ) {
        if let Ok(mut subs) = self.subscribers.write() {
            subs.insert(
                did,
                LayerSubscriber {
                    capabilities,
                    version_vector: Vec::new(),
                    broadcast_tx,
                },
            );
        }
    }

    /// Remove a subscriber, returning their state if present
    pub fn remove_subscriber(&self, did: &str) -> Option<LayerSubscriber> {
        if let Ok(mut subs) = self.subscribers.write() {
            subs.remove(did)
        } else {
            None
        }
    }

    /// Update a subscriber's version vector after successful broadcast
    pub fn update_subscriber_vector(&self, did: &str, vector: Vec<u8>) {
        if let Ok(mut subs) = self.subscribers.write() {
            if let Some(sub) = subs.get_mut(did) {
                sub.version_vector = vector;
            }
        }
    }

    /// Check if any subscribers are connected
    pub fn has_subscribers(&self) -> bool {
        self.subscribers
            .read()
            .map(|subs| !subs.is_empty())
            .unwrap_or(false)
    }

    // === Authorization checks ===

    /// Can we push updates to this DID?
    ///
    /// **Context**: Checked at broadcast time. Subscriber must have read
    /// capability and have given consent.
    pub fn can_push_to(&self, did: &str) -> bool {
        self.subscribers
            .read()
            .map(|subs| subs.get(did).map(|s| s.capabilities.read).unwrap_or(false))
            .unwrap_or(false)
    }

    /// Can WE write to this layer locally? (viewer side check)
    pub fn can_local_write(&self) -> bool {
        self.config.capabilities.write
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
        assert!(!unit.has_subscribers());
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

    // === Subscriber tests ===

    fn make_tx() -> mpsc::Sender<BroadcastPayload> {
        let (tx, _rx) = mpsc::channel(16);
        tx
    }

    #[test]
    fn test_add_and_remove_subscriber() {
        let unit = LayerUnit::new_empty();
        assert!(!unit.has_subscribers());

        let caps = Capabilities {
            read: true,
            write: true,
            sync: true,
        };
        unit.add_subscriber("did:key:alice".to_string(), caps, make_tx());
        assert!(unit.has_subscribers());
        assert!(unit.can_push_to("did:key:alice"));

        let removed = unit.remove_subscriber("did:key:alice");
        assert!(removed.is_some());
        assert!(!unit.has_subscribers());
        assert!(!unit.can_push_to("did:key:alice"));
    }

    #[test]
    fn test_can_push_to() {
        let unit = LayerUnit::new_empty();

        // Not a subscriber
        assert!(!unit.can_push_to("did:key:alice"));

        // Subscriber with read
        let caps = Capabilities {
            read: true,
            write: false,
            sync: true,
        };
        unit.add_subscriber("did:key:alice".to_string(), caps, make_tx());
        assert!(unit.can_push_to("did:key:alice"));

        // Subscriber without read
        let no_read = Capabilities {
            read: false,
            write: true,
            sync: false,
        };
        unit.add_subscriber("did:key:bob".to_string(), no_read, make_tx());
        assert!(!unit.can_push_to("did:key:bob"));
    }

    #[test]
    fn test_update_subscriber_vector() {
        let unit = LayerUnit::new_empty();
        let caps = Capabilities {
            read: true,
            write: true,
            sync: true,
        };
        unit.add_subscriber("did:key:alice".to_string(), caps, make_tx());

        // Initially empty
        {
            let subs = unit.subscribers().read().unwrap();
            assert!(subs.get("did:key:alice").unwrap().version_vector.is_empty());
        }

        let vv = vec![1, 2, 3, 4];
        unit.update_subscriber_vector("did:key:alice", vv.clone());
        {
            let subs = unit.subscribers().read().unwrap();
            assert_eq!(subs.get("did:key:alice").unwrap().version_vector, vv);
        }
    }

    #[test]
    fn test_can_local_write() {
        let mut unit = LayerUnit::new_empty();
        // Default config has write: true
        assert!(unit.can_local_write());

        // Override via config for read-only
        unit.config = LayerConfig {
            capabilities: Capabilities {
                read: true,
                write: false,
                sync: true,
            },
            is_local_only: false,
        };
        assert!(!unit.can_local_write());
    }
}
