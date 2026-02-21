//! Scribe actor state and configuration types
//!
//! Contains all state structs, type aliases, and configuration for the Scribe actor.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};

use tokio::sync::{broadcast, mpsc};

use domains::{Layer, QueryDelta, QueryResult, QuerySpec};

use crate::layer_unit::LayerUnit;
use crate::policy_compat;
use crate::{
    BroadcastPayload, DynamicLayerMeta, EphemeralOutbound, LayerStorageRef, PageUpdate,
    PeerResolverRef, PeerVectorStorageRef, PermitIssuerRef, SyncEvent,
};

// Layer Name Normalization

/// Normalize a layer name by stripping `{page_id}/` prefix if present
///
/// **Context**: Lua apps and wire protocol use prefixed names (`{page_id}/messages`)
/// but Scribe stores layers with bare names (`messages`). This strips the prefix
/// at the Scribe boundary so all internal lookups use consistent bare names.
pub fn normalize_layer_name(name: &str, page_id: &str) -> String {
    let prefix = format!("{}/", page_id);
    if let Some(bare) = name.strip_prefix(&prefix) {
        bare.to_string()
    } else {
        name.to_string()
    }
}

// Capture Helpers

/// Shorten a DID to its last 12 characters for compact logging
pub fn short_did(did: &str) -> &str {
    let start = did.len().saturating_sub(12);
    &did[start..]
}

/// Shorten a layer name by collapsing `did:key:...` segments to `...`
pub fn layer_short(name: &str) -> String {
    name.split('/')
        .map(|seg| {
            if seg.starts_with("did:key:") {
                "..."
            } else {
                seg
            }
        })
        .collect::<Vec<_>>()
        .join("/")
}

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

/// Page-level connection state for a peer
///
/// **Design**: Stores the parsed permit directly instead of extracted fields.
/// All access checks delegate to policy decision helpers.
/// **Note**: Version vectors are NOT stored here — they live on LayerSubscriber per-layer.
pub struct PeerConnection {
    /// Parsed page permit for all access checks (static layers)
    /// Stores the full permit so we can use its methods directly
    pub permit: gurkha::PolicyPermit,
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
}

impl PeerConnection {
    /// Check if subscriber can receive updates for layer via page permit
    pub fn can_receive_layer(&self, layer_name: &str, page_id: &str) -> bool {
        // Block presence layer for peers who can't see others
        if (layer_name == "presence" || layer_name.ends_with("/presence")) && !self.can_see_others {
            return false;
        }
        // Check if layer is in no_incoming_updates (sync facts)
        if policy_compat::no_incoming_updates(&self.permit).contains(&layer_name.to_string()) {
            return false;
        }
        let _ = page_id;
        policy_compat::can_read_layer(&self.permit, layer_name, &self.subscriber_did)
    }

    /// Check if subscriber can write to layer
    ///
    /// **Context**: Used during incoming update validation
    /// **Note**: Write auth stays subscriber-centric for now. For dynamic layer writes,
    /// the node trusts sync_target (path 2 in Permissions::can_write).
    pub fn can_write_layer(&self, layer_name: &str, page_id: &str) -> bool {
        let _ = page_id;
        policy_compat::can_write_layer(&self.permit, layer_name, &self.subscriber_did)
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
    /// Per-layer compute units (layer_name → LayerUnit)
    pub units: HashMap<String, LayerUnit>,
    /// Peer subscribers (user_did, device_id) → PeerConnection
    /// Arc<RwLock> allows Loro observer callback to send directly to peers
    pub subscribers: Arc<RwLock<HashMap<(String, String), PeerConnection>>>,
    /// Query subscribers (query_id → QuerySubscriberInfo)
    pub query_subscribers: HashMap<String, QuerySubscriberInfo>,

    /// Layer persistence (saves layer snapshots)
    pub layer_storage: LayerStorageRef,
    /// Peer vector persistence (saves/loads peer state vectors)
    pub vector_storage: PeerVectorStorageRef,
    /// Peer resolver (node mode only) - lists authorized users, loads permits
    pub peer_resolver: Option<PeerResolverRef>,
    /// Permit issuer (node mode only) - issues layer permits for dynamic layers
    pub permit_issuer: Option<PermitIssuerRef>,

    /// Sync configuration derived from permit (mode + sync_target)
    pub sync_config: Option<SyncConfig>,
    /// Channel to emit sync events (EnsureSync) - consumed by Coordinator
    pub sync_event_tx: Option<mpsc::Sender<SyncEvent>>,

    /// Broadcast channel for capture system (pre-serialized JSON lines)
    pub capture_tx: Option<broadcast::Sender<String>>,
    /// Monotonic counter for capture event ordering (disambiguates same-millisecond events)
    pub capture_seq: AtomicU64,

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
    pub our_permit: Option<gurkha::PolicyPermit>,
    /// Our DID (for pattern expansion with {aud})
    pub our_did: String,

    /// Shutdown sender for node script tick loop (if running)
    /// **Context**: Set when is_node=true and entry_node found in manifest
    /// **Usage**: Send () to shutdown the tick loop
    pub node_script_shutdown: Option<mpsc::Sender<()>>,

    /// Cached peer roles (peer_did → role string)
    /// **Context**: `get_peer_role` does a redb read + UCAN parse per call.
    /// Role is static for a peer's session, so caching avoids 12% CPU overhead.
    pub peer_role_cache: HashMap<String, String>,
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
    /// Permit issuer (node mode only) - issues layer permits for dynamic layers
    pub permit_issuer: Option<PermitIssuerRef>,

    /// Optional: Sync configuration derived from permit (relationship + sync_target)
    /// - Owner: sync to sovereign node
    /// - Node: broadcast only (no outbound sync)
    /// - Viewer: sync to source node
    pub sync_config: Option<SyncConfig>,
    /// Channel to emit sync events (EnsureSync) - consumed by Coordinator
    pub sync_event_tx: Option<mpsc::Sender<SyncEvent>>,
    /// Broadcast channel for capture system (pre-serialized JSON lines)
    pub capture_tx: Option<broadcast::Sender<String>>,
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
    /// Parse dynamic-layer metadata for a bare layer name using our permit schemas.
    pub fn dynamic_ref_for_layer(&self, layer_name: &str) -> Option<DynamicLayerMeta> {
        let permit = self.our_permit.as_ref()?;
        let parsed = policy_compat::parse_dynamic_layer(permit, layer_name)?;

        Some(DynamicLayerMeta {
            schema_key: parsed.schema_key,
            creator_did: parsed.creator_did,
            placeholders: parsed.placeholders,
        })
    }

    /// Get the next capture sequence number (monotonically increasing)
    fn next_capture_seq(&self) -> u64 {
        self.capture_seq.fetch_add(1, Ordering::Relaxed)
    }

    fn emit_capture(&self, event_type: &str, extra_fields: serde_json::Value) {
        if let Some(ref tx) = self.capture_tx {
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0);

            let mut payload = serde_json::Map::new();
            payload.insert(
                "type".to_string(),
                serde_json::Value::String(event_type.to_string()),
            );
            payload.insert(
                "ts".to_string(),
                serde_json::Value::Number(serde_json::Number::from(ts as u64)),
            );
            payload.insert(
                "seq".to_string(),
                serde_json::Value::Number(serde_json::Number::from(self.next_capture_seq())),
            );
            payload.insert(
                "page_id".to_string(),
                serde_json::Value::String(self.page_id.clone()),
            );

            if let serde_json::Value::Object(extra) = extra_fields {
                payload.extend(extra);
            }

            if let Ok(json) = serde_json::to_string(&serde_json::Value::Object(payload)) {
                let _ = tx.send(json);
            }
        }
    }

    /// Check if a layer should sync based on permit configuration
    ///
    /// **Context**: Used to filter out local-only layers (sync: false) before broadcasting
    /// **Returns**: true if layer should sync, false if local-only
    pub fn should_sync_layer(&self, layer_name: &str) -> bool {
        self.our_permit
            .as_ref()
            .map(|permit| policy_compat::should_sync_layer(permit, layer_name, &self.our_did))
            .unwrap_or(true) // No permit means sync everything
    }

    /// Check if we can write to a layer
    pub fn can_write_layer(&self, layer_name: &str) -> bool {
        self.our_permit
            .as_ref()
            .map(|permit| policy_compat::can_write_layer(permit, layer_name, &self.our_did))
            .unwrap_or(true) // No permit means full access
    }

    /// Emit a sync event to the capture channel (pre-serialized JSON line)
    ///
    /// **Context**: Called before sync_event_tx.send() to capture SyncEvent for observability
    pub fn emit_sync_event_capture(&self, event: &SyncEvent) {
        self.emit_capture(
            "sync_event",
            serde_json::json!({
                "data": event,
            }),
        );

        match event {
            SyncEvent::EnsureSync { .. } | SyncEvent::SubscribeLayers { .. } => {}
        }
    }

    /// Emit a layer authorization event to the capture channel
    ///
    /// **Context**: Called when a DID is authorized/pending for a layer, for observability
    pub fn emit_layer_auth_capture(&self, layer: &str, did: &str, action: &str) {
        self.emit_capture(
            "layer_auth",
            serde_json::json!({
                "layer": layer,
                "layer_short": layer_short(layer),
                "did": did,
                "short_did": short_did(did),
                "action": action,
            }),
        );
    }

    /// Emit a write permission check result to the capture channel
    ///
    /// **Context**: Called from Permissions::can_write to trace authorization decisions
    pub fn emit_permission_check_capture(
        &self,
        layer: &str,
        peer_did: &str,
        result: &str,
        granted_by: &str,
    ) {
        self.emit_capture(
            "permission_check",
            serde_json::json!({
                "layer": layer,
                "layer_short": layer_short(layer),
                "peer_did": peer_did,
                "short_did": short_did(peer_did),
                "operation": "write",
                "result": result,
                "granted_by": granted_by,
            }),
        );
    }

    /// Emit an apply update outcome to the capture channel
    ///
    /// **Context**: Called from handle_apply_update to trace update application results
    pub fn emit_apply_update_capture(
        &self,
        layer: &str,
        from_peer: Option<&str>,
        result: &str,
        reason: Option<&str>,
    ) {
        self.emit_capture(
            "apply_update",
            serde_json::json!({
                "layer": layer,
                "layer_short": layer_short(layer),
                "from_peer": from_peer,
                "short_did": from_peer.map(short_did),
                "result": result,
                "reason": reason,
            }),
        );
    }

    /// Emit a page update to the capture channel (pre-serialized JSON line)
    ///
    /// **Context**: Called alongside page_update_subscribers fanout for observability
    pub fn emit_page_update_capture(&self, update: &PageUpdate) {
        self.emit_capture(
            "page_update",
            serde_json::json!({
                "data": update,
            }),
        );
    }

    /// Emit a subscriber state snapshot to the capture channel
    ///
    /// **Context**: Called after handle_subscribe() completes to show what layers a peer can access
    pub fn emit_subscriber_state_capture(
        &self,
        peer_did: &str,
        authorized_layers: &[String],
        total_layers: usize,
        can_see_others: bool,
        is_visible: bool,
    ) {
        let short_layers: Vec<String> = authorized_layers.iter().map(|l| layer_short(l)).collect();
        self.emit_capture(
            "subscriber_state",
            serde_json::json!({
                "peer_did": peer_did,
                "short_did": short_did(peer_did),
                "authorized_layers": authorized_layers,
                "authorized_layers_short": short_layers,
                "total_layers": total_layers,
                "can_see_others": can_see_others,
                "is_visible": is_visible,
            }),
        );
    }

    /// Emit a layer subscribe result to the capture channel
    ///
    /// **Context**: Called after HandleLayerSubscribe completes, for observability
    pub fn emit_layer_subscribe_result_capture(
        &self,
        layer: &str,
        peer_did: &str,
        result: &str,
        error: Option<&str>,
        subscriber_added: bool,
    ) {
        self.emit_capture(
            "layer_subscribe_result",
            serde_json::json!({
                "layer": layer,
                "layer_short": layer_short(layer),
                "peer_did": peer_did,
                "short_did": short_did(peer_did),
                "result": result,
                "error": error,
                "subscriber_added": subscriber_added,
            }),
        );
    }

    /// Emit a permit issue decision to the capture channel
    ///
    /// **Context**: Called from issue_layer_permit_for_subscribe at each decision point
    pub fn emit_permit_issue_capture(
        &self,
        layer: &str,
        peer_did: &str,
        path: &str,
        result: &str,
        reason: Option<&str>,
    ) {
        self.emit_capture(
            "permit_issue_decision",
            serde_json::json!({
                "layer": layer,
                "layer_short": layer_short(layer),
                "peer_did": peer_did,
                "short_did": short_did(peer_did),
                "path": path,
                "result": result,
                "reason": reason,
            }),
        );
    }

    /// Emit a broadcast decision skip event to the capture channel
    ///
    /// **Context**: Called when a subscriber is skipped during broadcast, for observability
    pub fn emit_broadcast_decision_capture(&self, layer: &str, peer_did: &str, reason: &str) {
        self.emit_capture(
            "broadcast_decision",
            serde_json::json!({
                "layer": layer,
                "layer_short": layer_short(layer),
                "peer_did": peer_did,
                "short_did": short_did(peer_did),
                "decision": "skip",
                "reason": reason,
            }),
        );
    }
}
