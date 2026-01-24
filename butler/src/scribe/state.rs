//! Scribe actor state and configuration types
//!
//! Contains all state structs, type aliases, and configuration for the Scribe actor.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, RwLock};

use tokio::sync::mpsc;

use crate::models::{Layer, QuerySpec, QueryResult};
use crate::error::Result;

use super::message::{BroadcastPayload, EphemeralOutbound, PageEvent, SyncEvent, EphemeralEvent};
use super::lua_runtime::ScribeLuaRuntime;

// =============================================================================
// Type Aliases
// =============================================================================

/// Layer write permission - whether a peer can write to a layer
///
/// This is a simple bool where:
/// - `true` = can write (was Collaborator/Submitter)
/// - `false` = read only (was Viewer)
pub type LayerWritePermission = bool;

/// Function to save layer bytes (scoped to page_id)
pub type SaveLayerFn = Arc<dyn Fn(&str, &[u8]) -> Result<()> + Send + Sync>;

/// Function to load peer's state vectors
pub type LoadPeerVectorFn = Arc<dyn Fn(&str, &str) -> Result<Option<HashMap<String, Vec<u8>>>> + Send + Sync>;

/// Function to save peer's state vectors
pub type SavePeerVectorFn = Arc<dyn Fn(&str, &str, &HashMap<String, Vec<u8>>) -> Result<()> + Send + Sync>;

/// Function to list authorized users for a page (node mode)
/// Returns Vec<user_did> - Coordinator handles device resolution
pub type ListAuthorizedUsersFn = Arc<dyn Fn(&str) -> Vec<String> + Send + Sync>;

/// Function to load a user's permit for a page (node mode - for sync authorization)
/// Args: (page_id, user_did) -> Option<permit_token>
pub type LoadUserPermitFn = Arc<dyn Fn(&str, &str) -> Option<String> + Send + Sync>;

// =============================================================================
// Pattern-Based Access Control
// =============================================================================

/// Pattern rule for layer access control (role-agnostic)
///
/// Patterns support placeholders:
/// - `{page_id}` - Expands to the page ID
/// - `{aud}` - Expands to the subscriber's DID
/// - `*` - Wildcard matching any segment
///
/// Example: `{page_id}/*/{aud}` → `shop123/orders/did:key:customer_a`
#[derive(Debug, Clone)]
pub struct PatternRule {
    /// Original pattern from permit (e.g., "{page_id}/*/{aud}")
    pub pattern: String,
    /// Can sync this pattern (receive updates)
    pub sync: bool,
    /// Can create layers matching this pattern
    pub create: bool,
    /// Can write to existing layers matching this pattern
    pub write: bool,
}

impl PatternRule {
    /// Check if a layer name matches this pattern
    ///
    /// **Context**: Determining if peer can access a specific layer
    /// **Pattern**: e.g., "shop123/*/did:key:customer_a" (after expansion)
    /// **Layer**: e.g., "shop123/orders/did:key:customer_a"
    pub fn matches(&self, layer_name: &str, page_id: &str, subscriber_did: &str) -> bool {
        let expanded = self.expand(page_id, subscriber_did);
        matches_wildcard_pattern(layer_name, &expanded)
    }

    /// Expand placeholders in pattern
    fn expand(&self, page_id: &str, subscriber_did: &str) -> String {
        self.pattern
            .replace("{page_id}", page_id)
            .replace("{aud}", subscriber_did)
    }
}

/// Match layer name against pattern with wildcards
///
/// Pattern: "shop123/*/did:key:abc" matches "shop123/orders/did:key:abc"
/// Pattern: "shop123/*/*" matches "shop123/orders/did:key:abc"
pub fn matches_wildcard_pattern(layer_name: &str, pattern: &str) -> bool {
    let layer_parts: Vec<&str> = layer_name.split('/').collect();
    let pattern_parts: Vec<&str> = pattern.split('/').collect();

    if layer_parts.len() != pattern_parts.len() {
        return false;
    }

    layer_parts.iter().zip(pattern_parts.iter()).all(|(layer, pat)| {
        *pat == "*" || layer == pat
    })
}

// =============================================================================
// Sync Configuration
// =============================================================================

/// Sync policies parsed from permit
#[derive(Debug, Clone, Default)]
pub struct SyncPolicy {
    /// Layers that are never synced
    pub local_only: HashSet<String>,
    /// Layers where peer doesn't receive updates
    pub no_incoming_updates: HashSet<String>,
    /// Layers that send full snapshot instead of incremental
    pub send_full_snapshot: HashSet<String>,
}

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

// =============================================================================
// Subscriber Types
// =============================================================================

/// Information about a subscribed peer
pub struct SubscriberInfo {
    /// Channel to send CRDT broadcasts
    pub broadcast_tx: mpsc::Sender<BroadcastPayload>,
    /// Channel to send ephemeral broadcasts (cursor, typing)
    /// PeerActor creates this, spawns listener that sends datagrams
    pub ephemeral_tx: Option<mpsc::Sender<EphemeralOutbound>>,
    /// Their last known state vectors (layer_name → version_vector)
    pub vectors: HashMap<String, Vec<u8>>,
    /// Write permissions for each layer (fixed layers)
    /// true = can write, false = read only
    pub layer_permissions: HashMap<String, LayerWritePermission>,
    /// Sync policies
    pub sync_policy: SyncPolicy,

    // ==================== Pattern-Based Access (Role-Agnostic) ====================

    /// Subscriber's DID (for pattern expansion with {aud})
    pub subscriber_did: String,
    /// Patterns this peer can receive (read from)
    pub readable_patterns: Vec<PatternRule>,
    /// Patterns this peer can write to (create layers)
    pub writable_patterns: Vec<PatternRule>,
}

impl SubscriberInfo {
    /// Check if subscriber can receive updates for layer
    ///
    /// **Context**: Used during broadcast/reconciliation to check read permission
    /// **Checks**:
    /// 1. Fixed layer permission exists AND not in no_incoming_updates
    /// 2. OR pattern matches peer's readable_patterns
    pub fn can_receive_layer(&self, layer_name: &str, page_id: &str) -> bool {
        // 1. Check fixed layer permission
        if self.layer_permissions.contains_key(layer_name)
            && !self.sync_policy.no_incoming_updates.contains(layer_name)
        {
            return true;
        }

        // 2. Check pattern-based read permission
        for pattern in &self.readable_patterns {
            if pattern.sync && pattern.matches(layer_name, page_id, &self.subscriber_did) {
                return true;
            }
        }

        false
    }
}

/// Query subscription info
pub struct QuerySubscriberInfo {
    /// Query specification
    pub spec: QuerySpec,
    /// Channel to send deltas
    pub delta_tx: mpsc::Sender<crate::models::QueryDelta>,
    /// Current version (monotonically increasing)
    pub version: u64,
    /// Last known result (for computing deltas)
    pub last_result: Option<QueryResult>,
}

// =============================================================================
// Actor State
// =============================================================================

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
    /// Injected save function
    pub save_layer: SaveLayerFn,
    /// Injected load peer vector function
    pub load_peer_vector: LoadPeerVectorFn,
    /// Injected save peer vector function
    pub save_peer_vector: SavePeerVectorFn,
    /// Sync configuration derived from permit (mode + sync_target)
    pub sync_config: Option<SyncConfig>,
    /// Channel to emit sync events (EnsureSync) - consumed by Coordinator
    pub sync_event_tx: Option<mpsc::Sender<SyncEvent>>,
    /// Callback to list authorized user_dids for this page (node mode)
    pub list_authorized_users: Option<ListAuthorizedUsersFn>,
    /// Callback to load a user's permit for this page (node mode - permit-based sync auth)
    pub load_user_permit: Option<LoadUserPermitFn>,
    /// Loro subscriptions (layer_name → Subscription)
    /// Must keep these alive or subscriptions are cancelled
    pub loro_subscriptions: HashMap<String, loro::Subscription>,

    // ==================== Unified Page Event Channel ====================

    /// Unified page event channel - all layer changes flow through here
    /// **Consumers**: UI/Lua callbacks, Broadcast to peers, Derivation engine
    /// **Design**: One channel per page, all layer observers send to this
    /// **Arc<RwLock>**: Shared with observer tasks so new subscribers are visible
    pub page_event_subscribers: Arc<RwLock<Vec<mpsc::Sender<PageEvent>>>>,

    /// Pending update source for observer context
    /// **Context**: Set before import() so observer knows who caused the change
    /// **Usage**: Observer reads this to populate `from_peer` in PageEvent
    /// **Cleared**: After import() completes
    pub pending_update_source: Arc<Mutex<Option<(String, String)>>>,

    // ==================== Ephemeral Events (Live Data Streaming) ====================

    /// Subscribers to ephemeral events (cursor, typing, presence)
    /// **Context**: App runtime subscribes to receive real-time events
    /// **Arc<RwLock>**: Shared with PeerSession handlers
    pub ephemeral_subscribers: Arc<RwLock<Vec<mpsc::Sender<EphemeralEvent>>>>,

    // Note: ephemeral_broadcast_tx removed - outbound ephemeral now goes directly via
    // SubscriberInfo.ephemeral_tx (Scribe → PeerActor channels)

    // ==================== Lua Runtime (Validation + Derivation) ====================

    /// Unified Lua runtime for validation and derivation
    /// - All Scribes: Run validation (if validation_code provided)
    /// - Node Scribes: Also run derivation (if init_code provided)
    pub lua_runtime: Option<ScribeLuaRuntime>,

    // ==================== Our Permit (Local Write Authorization) ====================

    /// Our permit for this page (local writes)
    /// Used to check if we can write to a layer before committing
    pub our_permit: Option<String>,
    /// Our DID (for pattern expansion with {aud})
    pub our_did: String,
    /// Our role from permit (owner/viewer/node)
    pub our_role: String,
    /// Fixed layer permissions from our permit (layer_name → can_write)
    pub our_layer_permissions: HashMap<String, LayerWritePermission>,
    /// Patterns we can write to (parsed from our permit)
    pub our_writable_patterns: Vec<PatternRule>,

    // ==================== Node Script (is_node mode) ====================

    /// Shutdown sender for node script tick loop (if running)
    /// **Context**: Set when is_node=true and entry_node found in manifest
    /// **Usage**: Send () to shutdown the tick loop
    pub node_script_shutdown: Option<mpsc::Sender<()>>,
}

/// Arguments for spawning Scribe
pub struct ScribeArgs {
    pub page_id: String,
    pub layers: HashMap<String, Layer>,
    pub save_layer: SaveLayerFn,
    pub load_peer_vector: LoadPeerVectorFn,
    pub save_peer_vector: SavePeerVectorFn,
    /// Optional: Sync configuration derived from permit (relationship + sync_target)
    /// - Owner: sync to sovereign node
    /// - Node: broadcast only (no outbound sync)
    /// - Viewer: sync to source node
    pub sync_config: Option<SyncConfig>,
    /// Channel to emit sync events (EnsureSync) - consumed by Coordinator
    pub sync_event_tx: Option<mpsc::Sender<SyncEvent>>,
    /// Callback to list authorized user_dids for this page (node mode)
    pub list_authorized_users: Option<ListAuthorizedUsersFn>,
    /// Callback to load a user's permit for this page (node mode - permit-based sync auth)
    pub load_user_permit: Option<LoadUserPermitFn>,
    /// Validation Lua code (loaded from app:shared/validation.lua)
    /// If provided, creates ScribeLuaRuntime for incoming updates
    pub validation_code: Option<String>,
    /// Derivation init Lua code (loaded from app:shared/init.lua)
    /// If provided (and is_node=true), ScribeLuaRuntime also handles derivation
    pub init_code: Option<String>,
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
    ///
    /// Note: Only checks if we have a permit. If no permit, defaults to sync=true.
    pub fn should_sync_layer(&self, layer_name: &str) -> bool {
        match &self.our_permit {
            Some(permit) => {
                super::permit::should_sync_layer(permit, layer_name, &self.page_id, &self.our_did)
            }
            None => true, // No permit means sync everything (legacy mode)
        }
    }

    /// Update the local_only_layers set based on current permit
    ///
    /// **Context**: Called when permit is set/updated to pre-compute which layers are local-only
    /// **Why**: Observer tasks can check this without parsing permit each time
    pub fn update_local_only_layers(&self) {
        let Some(permit) = &self.our_permit else {
            return;
        };

        let mut local_only = HashSet::new();

        // Check all existing layers
        for layer_name in self.layers.keys() {
            if !super::permit::should_sync_layer(permit, layer_name, &self.page_id, &self.our_did) {
                local_only.insert(layer_name.clone());
            }
        }

        // Update the shared set
        if let Ok(mut set) = self.local_only_layers.write() {
            *set = local_only;
        }
    }

    /// Mark a specific layer as local-only (sync: false)
    ///
    /// **Context**: Called when a new layer is created to check if it should be local-only
    pub fn mark_layer_local_only_if_needed(&self, layer_name: &str) {
        if !self.should_sync_layer(layer_name) {
            if let Ok(mut set) = self.local_only_layers.write() {
                set.insert(layer_name.to_string());
            }
        }
    }
}
