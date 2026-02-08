//! Scribe actor state and configuration types
//!
//! Contains all state structs, type aliases, and configuration for the Scribe actor.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, RwLock};

use tokio::sync::mpsc;

use domains::{Layer, QuerySpec, QueryResult, QueryDelta};

use crate::{
    BroadcastPayload, EphemeralOutbound, PageUpdate, SyncEvent,
    LayerStorageRef, PeerVectorStorageRef, PeerResolverRef,
};

// Sync Configuration

/// Sync mode derived from permit capabilities
///
/// **Design**: Capability-based, not role-based
/// - Mode is derived from permit's sync configuration
/// - No hardcoded role names in decision logic
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncMode {
    /// Sync to a specific source node (users sync to their node)
    ToSource,
    /// Broadcast to all authorized peers (node mode)
    Broadcast,
}

impl Default for SyncMode {
    fn default() -> Self {
        SyncMode::Broadcast
    }
}

/// Sync configuration derived from permit
///
/// **Context**: Determines how this Scribe syncs based on permit capabilities
/// - ToSource: Sync to a specific source node (owner/viewer/customer)
/// - Broadcast: Broadcast to all authorized peers (node)
#[derive(Debug, Clone)]
pub struct SyncConfig {
    /// Sync mode derived from permit (capability-based)
    pub mode: SyncMode,
    /// For ToSource: node_id to sync to
    /// For Broadcast: None (uses list_authorized_users callback)
    pub sync_target: Option<String>,
}

// Subscriber Types

/// Information about a subscribed peer
///
/// **Design**: Stores the parsed permit directly instead of extracted fields.
/// All access checks delegate to gurkha::Permit methods, ensuring consistency.
pub struct SubscriberInfo {
    /// Parsed permit for all access checks
    /// Stores the full permit so we can use its methods directly
    pub permit: gurkha::Permit,
    /// Subscriber's DID (for pattern expansion with {aud})
    /// Extracted from permit audience and normalized to DID format
    pub subscriber_did: String,
    /// Whether this subscriber is visible (from permit presence config)
    /// Visible peers have their entry written to the presence layer
    pub is_visible: bool,
    /// Whether this subscriber can see others' presence
    /// Gates whether the presence layer is synced to this peer
    pub can_see_others: bool,
    /// Display name for this subscriber (from permit presence config)
    pub display_name: Option<String>,
    /// Channel to send CRDT broadcasts
    pub broadcast_tx: mpsc::Sender<BroadcastPayload>,
    /// Channel to send ephemeral broadcasts (cursor, typing)
    /// PeerActor creates this, spawns listener that sends datagrams
    pub ephemeral_tx: Option<mpsc::Sender<EphemeralOutbound>>,
    /// Their last known state vectors (layer_name → version_vector)
    pub vectors: HashMap<String, Vec<u8>>,
}

impl SubscriberInfo {
    /// Check if subscriber can receive updates for layer
    ///
    /// **Context**: Used during broadcast/reconciliation to check read permission
    /// **Delegates**: Uses gurkha::Permit::can_read_layer() which checks both
    ///              fixed layers and pattern-based permissions
    pub fn can_receive_layer(&self, layer_name: &str, page_id: &str) -> bool {
        // Block presence layer for peers who can't see others
        if layer_name.ends_with("/presence") && !self.can_see_others {
            return false;
        }
        // Check if layer is in no_incoming_updates (sync facts)
        if self.permit.sync_facts().no_incoming_updates.contains(&layer_name.to_string()) {
            return false;
        }
        // Delegate to permit's can_read_layer which handles both fixed and pattern-based
        self.permit.can_read_layer(layer_name, page_id, &self.subscriber_did)
    }

    /// Check if subscriber can write to layer
    ///
    /// **Context**: Used during incoming update validation
    /// **Delegates**: Uses gurkha::Permit::can_write_layer()
    pub fn can_write_layer(&self, layer_name: &str, page_id: &str) -> bool {
        self.permit.can_write_layer(layer_name, page_id, &self.subscriber_did)
    }
}

/// Query subscription info
pub struct QuerySubscriberInfo {
    /// Query specification
    pub spec: QuerySpec,
    /// Channel to send deltas
    pub delta_tx: mpsc::Sender<QueryDelta>,
    /// Current version (monotonically increasing)
    pub version: u64,
    /// Last known result (for computing deltas)
    pub last_result: Option<QueryResult>,
}

// Actor State

/// Scribe actor state
pub struct ScribeState {
    /// Page identifier
    pub page_id: String,
    /// LoroDoc layers (layer_name → Layer)
    pub layers: HashMap<String, Layer>,
    /// Peer subscribers (user_did, device_id) → SubscriberInfo
    /// Arc<RwLock> allows Loro observer callback to send directly to peers
    pub subscribers: Arc<RwLock<HashMap<(String, String), SubscriberInfo>>>,
    /// Layers that should NOT be broadcast (sync: false in permit)
    /// Arc<RwLock> allows observer tasks to check this without holding state lock
    pub local_only_layers: Arc<RwLock<HashSet<String>>>,
    /// Query subscribers (query_id → QuerySubscriberInfo)
    pub query_subscribers: HashMap<String, QuerySubscriberInfo>,
    /// Layers with unsaved changes
    pub dirty_layers: HashSet<String>,

    /// Layer persistence (saves layer snapshots)
    pub layer_storage: LayerStorageRef,
    /// Peer vector persistence (saves/loads peer state vectors)
    pub vector_storage: PeerVectorStorageRef,
    /// Peer resolver (node mode only) - lists authorized users, loads permits
    pub peer_resolver: Option<PeerResolverRef>,

    /// Sync configuration derived from permit (mode + sync_target)
    pub sync_config: Option<SyncConfig>,
    /// Channel to emit sync events (EnsureSync) - consumed by Coordinator
    pub sync_event_tx: Option<mpsc::Sender<SyncEvent>>,
    /// Loro subscriptions (layer_name → Subscription)
    /// Must keep these alive or subscriptions are cancelled
    pub loro_subscriptions: HashMap<String, loro::Subscription>,

    /// Unified page update channel - all events flow through here
    /// **Consumers**: UI/Lua callbacks, Broadcast to peers, Derivation engine
    /// **Design**: Single channel for layer changes, ephemeral data, peer presence
    /// **Arc<RwLock>**: Shared with observer tasks so new subscribers are visible
    pub page_update_subscribers: Arc<RwLock<Vec<mpsc::Sender<PageUpdate>>>>,

    /// Pending update source for observer context
    /// **Context**: Set before import() so observer knows who caused the change
    /// **Usage**: Observer reads this to populate `from_peer` in PageEvent
    /// **Cleared**: After import() completes
    pub pending_update_source: Arc<Mutex<Option<(String, String)>>>,

    // Note: ephemeral events now go through page_update_subscribers (PageUpdate::Ephemeral)
    // outbound ephemeral goes directly via SubscriberInfo.ephemeral_tx (Scribe → PeerActor channels)

    /// Validation handle (for kunki node mode validation)
    /// If provided, Scribe delegates validation to external ValidationService
    /// This enables validation.lua to access presence_lib from LuaRuntime
    /// Falls back to local validation if not available (though we're removing ScribeLuaRuntime)
    pub validation_handle: Option<crate::validation_handle::ValidationHandle>,

    /// Parsed permit - provides should_sync_layer, can_write_layer etc.
    /// Uses gurkha::Permit directly with page_id/did passed to methods
    pub our_permit: Option<gurkha::Permit>,
    /// Our DID (for pattern expansion with {aud})
    pub our_did: String,

    /// Shutdown sender for node script tick loop (if running)
    /// **Context**: Set when is_node=true and entry_node found in manifest
    /// **Usage**: Send () to shutdown the tick loop
    pub node_script_shutdown: Option<mpsc::Sender<()>>,
}

/// Arguments for spawning Scribe
pub struct ScribeArgs {
    pub page_id: String,
    pub layers: HashMap<String, Layer>,

    /// Layer persistence (saves layer snapshots)
    pub layer_storage: LayerStorageRef,
    /// Peer vector persistence (saves/loads peer state vectors)
    pub vector_storage: PeerVectorStorageRef,
    /// Peer resolver (node mode only) - lists authorized users, loads permits
    pub peer_resolver: Option<PeerResolverRef>,

    /// Optional: Sync configuration derived from permit (relationship + sync_target)
    /// - Owner: sync to sovereign node
    /// - Node: broadcast only (no outbound sync)
    /// - Viewer: sync to source node
    pub sync_config: Option<SyncConfig>,
    /// Channel to emit sync events (EnsureSync) - consumed by Coordinator
    pub sync_event_tx: Option<mpsc::Sender<SyncEvent>>,
    /// Validation handle (for kunki node mode validation)
    /// If provided, Scribe delegates validation to external ValidationService
    pub validation_handle: Option<crate::validation_handle::ValidationHandle>,
    /// Our permit for this page (local write authorization)
    /// Used to check if we can write to a layer before committing
    pub our_permit: Option<String>,
    /// Our DID (for pattern expansion and validation)
    pub our_did: String,
    /// Our username (for node script permit bindings)
    pub our_username: String,
    /// Whether this Scribe runs on a node (enables derivation engine and node scripts)
    pub is_node: bool,
    // Note: ephemeral_broadcast_tx removed - outbound ephemeral now goes directly via
    // SubscriberInfo.ephemeral_tx (Scribe → PeerActor channels)
}

impl ScribeState {
    /// Check if a layer should sync based on permit configuration
    ///
    /// **Context**: Used to filter out local-only layers (sync: false) before broadcasting
    /// **Returns**: true if layer should sync, false if local-only
    pub fn should_sync_layer(&self, layer_name: &str) -> bool {
        self.our_permit
            .as_ref()
            .map(|permit| permit.should_sync_layer(layer_name, &self.page_id, &self.our_did))
            .unwrap_or(true)  // No permit means sync everything
    }

    /// Check if we can write to a layer
    pub fn can_write_layer(&self, layer_name: &str) -> bool {
        self.our_permit
            .as_ref()
            .map(|permit| permit.can_write_layer(layer_name, &self.page_id, &self.our_did))
            .unwrap_or(true)  // No permit means full access
    }

    /// Update the local_only_layers set based on current permit
    ///
    /// **Context**: Called when permit is set/updated to pre-compute which layers are local-only
    /// **Why**: Observer tasks can check this without holding state lock
    pub fn update_local_only_layers(&self) {
        let Some(permit) = &self.our_permit else {
            return;
        };

        let mut local_only = HashSet::new();

        for layer_name in self.layers.keys() {
            if !permit.should_sync_layer(layer_name, &self.page_id, &self.our_did) {
                local_only.insert(layer_name.clone());
            }
        }

        if let Ok(mut set) = self.local_only_layers.write() {
            *set = local_only;
        }
    }

    /// Mark a specific layer as local-only (sync: false)
    pub fn mark_layer_local_only_if_needed(&self, layer_name: &str) {
        if !self.should_sync_layer(layer_name) {
            if let Ok(mut set) = self.local_only_layers.write() {
                set.insert(layer_name.to_string());
            }
        }
    }
}
