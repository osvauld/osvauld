//! Layer Unit — Per-layer compute unit
//!
//! Consolidates all per-layer state (LoroDoc, dirty flag, authorization config,
//! per-subscriber state, observer subscription) into a single struct. Makes Scribe
//! a thin router over LayerUnit instances.
//!
//! ## Authorization model
//!
//! Each LayerUnit owns its subscribers and their write permissions. Authorization is
//! checked at broadcast time using per-subscriber state. The `subscribers` map is
//! `Arc<RwLock<...>>` so the observer async task can read it for broadcast decisions.
//!
//! ## Subscriber keying
//!
//! Subscribers are keyed by `(user_did, device_id)` tuple, allowing the same user
//! to connect from multiple devices with independent version vectors and broadcast
//! channels.
//!
//! Also contains dynamic layer management (schema validation, permit preparation).

mod dynamic;

pub use dynamic::{
    find_matching_dynamic_schema, handle_add_layer_access, handle_create_dynamic_layer,
};

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use tokio::sync::mpsc;
use tracing::instrument;

use domains::Layer;

use crate::BroadcastPayload;

/// Layer-level configuration derived from permit
///
/// **Context**: Set once at layer creation, describes our own write permission
/// for this layer (viewer side) or default permission (node side).
#[derive(Debug, Clone)]
pub struct LayerConfig {
    /// Can we write to this layer locally?
    pub can_write: bool,
    /// sync: false — never broadcast to peers
    pub is_local_only: bool,
}

impl Default for LayerConfig {
    fn default() -> Self {
        Self {
            can_write: true,
            is_local_only: false,
        }
    }
}

/// Per-subscriber state within a LayerUnit
///
/// **Context**: Tracks a single (user, device) connection to this layer.
/// Created when a peer subscribes and presents a layer permit.
pub struct LayerSubscriber {
    /// Whether this subscriber can write to the layer
    pub can_write: bool,
    /// Their last known version vector for incremental sync
    pub version_vector: Vec<u8>,
    /// Channel to send broadcasts to this subscriber
    pub broadcast_tx: mpsc::Sender<BroadcastPayload>,
}

/// Sync temperature tier for per-layer sync policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SyncTier {
    /// Actively synchronized and expected in-memory.
    #[default]
    Hot,
    /// Historical layer with lower sync priority.
    Cold,
}

/// Per-layer compute unit — owns all state for a single layer
///
/// **Design**: Self-contained authorization and broadcast. Scribe routes
/// messages to LayerUnits; each unit knows its subscribers and their permissions.
pub struct LayerUnit {
    // === CRDT ===
    /// The CRDT document
    layer: Layer,
    /// Needs flush to storage?
    dirty: bool,
    /// Loro observer subscription (dropped = unsubscribed)
    loro_sub: Option<loro::Subscription>,

    // === Authorization (from permit) ===
    /// Cached layer configuration (our write permission, local-only flag)
    config: LayerConfig,
    /// Created via dynamic_layer_schema (not static template)
    pub is_dynamic: bool,
    /// Sync temperature tier for time-sharded strategies.
    sync_tier: SyncTier,

    // === Subscriber state ===
    /// Per-subscriber state ((user_did, device_id) → subscriber)
    /// Arc<RwLock> because the observer async task needs shared read access
    subscribers: Arc<RwLock<HashMap<(String, String), LayerSubscriber>>>,
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
            sync_tier: SyncTier::Hot,
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

    /// Get current sync tier.
    pub fn sync_tier(&self) -> SyncTier {
        self.sync_tier
    }

    /// Update sync tier.
    pub fn set_sync_tier(&mut self, tier: SyncTier) {
        self.sync_tier = tier;
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
    pub fn subscribers(&self) -> &Arc<RwLock<HashMap<(String, String), LayerSubscriber>>> {
        &self.subscribers
    }

    /// Add a subscriber with their write permission and broadcast channel
    ///
    /// **Context**: Called when a peer subscribes to this layer after permit validation.
    /// Key is (user_did, device_id) so the same user on multiple devices gets
    /// independent version vectors and broadcast channels.
    #[instrument(skip(self, broadcast_tx), fields(user_did = %user_did, device_id = %device_id, can_write = %can_write))]
    pub fn add_subscriber(
        &self,
        user_did: String,
        device_id: String,
        can_write: bool,
        broadcast_tx: mpsc::Sender<BroadcastPayload>,
    ) {
        if let Ok(mut subs) = self.subscribers.write() {
            subs.insert(
                (user_did, device_id),
                LayerSubscriber {
                    can_write,
                    version_vector: Vec::new(),
                    broadcast_tx,
                },
            );
        }
    }

    /// Remove a subscriber, returning their state if present
    #[instrument(skip(self), fields(user_did = %user_did, device_id = %device_id))]
    pub fn remove_subscriber(&self, user_did: &str, device_id: &str) -> Option<LayerSubscriber> {
        if let Ok(mut subs) = self.subscribers.write() {
            subs.remove(&(user_did.to_string(), device_id.to_string()))
        } else {
            None
        }
    }

    /// Update a subscriber's version vector after successful broadcast
    #[instrument(skip(self, vector), fields(user_did = %user_did, device_id = %device_id))]
    pub fn update_subscriber_vector(&self, user_did: &str, device_id: &str, vector: Vec<u8>) {
        if let Ok(mut subs) = self.subscribers.write() {
            let key = (user_did.to_string(), device_id.to_string());
            if let Some(sub) = subs.get_mut(&key) {
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

    /// Can we push updates to this (user, device)?
    ///
    /// **Context**: Checked at broadcast time. Subscriber must be present in the map.
    #[instrument(skip(self), fields(user_did = %user_did, device_id = %device_id))]
    pub fn can_push_to(&self, user_did: &str, device_id: &str) -> bool {
        self.subscribers
            .read()
            .map(|subs| {
                let key = (user_did.to_string(), device_id.to_string());
                subs.contains_key(&key)
            })
            .unwrap_or(false)
    }

    /// Can WE write to this layer locally? (viewer side check)
    #[instrument(skip(self))]
    pub fn can_local_write(&self) -> bool {
        self.config.can_write
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

        unit.add_subscriber(
            "did:key:alice".to_string(),
            "device-1".to_string(),
            true,
            make_tx(),
        );
        assert!(unit.has_subscribers());
        assert!(unit.can_push_to("did:key:alice", "device-1"));

        let removed = unit.remove_subscriber("did:key:alice", "device-1");
        assert!(removed.is_some());
        assert!(!unit.has_subscribers());
        assert!(!unit.can_push_to("did:key:alice", "device-1"));
    }

    #[test]
    fn test_can_push_to() {
        let unit = LayerUnit::new_empty();

        // Not a subscriber
        assert!(!unit.can_push_to("did:key:alice", "device-1"));

        // Subscriber present — can_push_to just checks presence
        unit.add_subscriber(
            "did:key:alice".to_string(),
            "device-1".to_string(),
            false,
            make_tx(),
        );
        assert!(unit.can_push_to("did:key:alice", "device-1"));

        // Different device not subscribed
        assert!(!unit.can_push_to("did:key:alice", "device-2"));

        // Different user not subscribed
        assert!(!unit.can_push_to("did:key:bob", "device-1"));
    }

    #[test]
    fn test_update_subscriber_vector() {
        let unit = LayerUnit::new_empty();
        unit.add_subscriber(
            "did:key:alice".to_string(),
            "device-1".to_string(),
            true,
            make_tx(),
        );

        // Initially empty
        {
            let subs = unit.subscribers().read().unwrap();
            let key = ("did:key:alice".to_string(), "device-1".to_string());
            assert!(subs.get(&key).unwrap().version_vector.is_empty());
        }

        let vv = vec![1, 2, 3, 4];
        unit.update_subscriber_vector("did:key:alice", "device-1", vv.clone());
        {
            let subs = unit.subscribers().read().unwrap();
            let key = ("did:key:alice".to_string(), "device-1".to_string());
            assert_eq!(subs.get(&key).unwrap().version_vector, vv);
        }
    }

    #[test]
    fn test_can_local_write() {
        let mut unit = LayerUnit::new_empty();
        // Default config has can_write: true
        assert!(unit.can_local_write());

        // Override via config for read-only
        unit.config = LayerConfig {
            can_write: false,
            is_local_only: false,
        };
        assert!(!unit.can_local_write());
    }

    #[test]
    fn test_multi_device_subscriber() {
        let unit = LayerUnit::new_empty();

        // Same user, two devices
        unit.add_subscriber(
            "did:key:alice".to_string(),
            "laptop".to_string(),
            true,
            make_tx(),
        );
        unit.add_subscriber(
            "did:key:alice".to_string(),
            "phone".to_string(),
            false,
            make_tx(),
        );

        // Both present
        assert!(unit.can_push_to("did:key:alice", "laptop"));
        assert!(unit.can_push_to("did:key:alice", "phone"));
        assert!(unit.has_subscribers());

        // Check independent can_write flags
        {
            let subs = unit.subscribers().read().unwrap();
            let laptop_key = ("did:key:alice".to_string(), "laptop".to_string());
            let phone_key = ("did:key:alice".to_string(), "phone".to_string());
            assert!(subs.get(&laptop_key).unwrap().can_write);
            assert!(!subs.get(&phone_key).unwrap().can_write);
        }

        // Independent version vectors
        unit.update_subscriber_vector("did:key:alice", "laptop", vec![1, 2]);
        unit.update_subscriber_vector("did:key:alice", "phone", vec![3, 4]);
        {
            let subs = unit.subscribers().read().unwrap();
            let laptop_key = ("did:key:alice".to_string(), "laptop".to_string());
            let phone_key = ("did:key:alice".to_string(), "phone".to_string());
            assert_eq!(subs.get(&laptop_key).unwrap().version_vector, vec![1, 2]);
            assert_eq!(subs.get(&phone_key).unwrap().version_vector, vec![3, 4]);
        }

        // Remove one device, other stays
        let removed = unit.remove_subscriber("did:key:alice", "laptop");
        assert!(removed.is_some());
        assert!(!unit.can_push_to("did:key:alice", "laptop"));
        assert!(unit.can_push_to("did:key:alice", "phone"));
        assert!(unit.has_subscribers());

        // Remove second device
        unit.remove_subscriber("did:key:alice", "phone");
        assert!(!unit.has_subscribers());
    }

    #[test]
    fn test_remove_nonexistent_subscriber() {
        let unit = LayerUnit::new_empty();
        let removed = unit.remove_subscriber("did:key:nobody", "device-x");
        assert!(removed.is_none());
    }

    #[test]
    fn test_update_vector_nonexistent_subscriber() {
        let unit = LayerUnit::new_empty();
        // Should not panic — silently no-ops
        unit.update_subscriber_vector("did:key:nobody", "device-x", vec![1, 2, 3]);
    }
}
