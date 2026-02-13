//! Scribe actor state and configuration types
//!
//! Contains all state structs, type aliases, and configuration for the Scribe actor.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};

use tokio::sync::{broadcast, mpsc};

use domains::{json_to_loro_value, Layer, QueryDelta, QueryResult, QuerySpec};

use crate::layer_unit::LayerUnit;
use crate::{
    BroadcastPayload, EphemeralOutbound, LayerStorageRef, PageUpdate, PeerResolverRef,
    PeerVectorStorageRef, PermitIssuerRef, SyncEvent,
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

fn decode_permit_summary(token: &str, event_layer_name: &str) -> serde_json::Value {
    let Ok(parsed) = gurkha::Permit::from_token(token) else {
        return serde_json::json!({
            "decode_ok": false,
            "error": "invalid_token",
        });
    };

    let audience = parsed.audience().unwrap_or_default();
    let issuer = parsed.issuer().unwrap_or_default();
    let token_type = parsed.token_type().unwrap_or_default();

    let page_id = parsed.page_id().unwrap_or_default();
    let page_prefix = format!("{}/", page_id);
    let layer_cfg = parsed.layers().get(event_layer_name).or_else(|| {
        let bare_layer = event_layer_name
            .strip_prefix(&page_prefix)
            .unwrap_or(event_layer_name);
        parsed.layers().get(bare_layer)
    });

    let (sync, write) = if let Some(cfg) = layer_cfg {
        (cfg.sync, cfg.write)
    } else {
        (false, false)
    };

    serde_json::json!({
        "decode_ok": true,
        "audience": audience,
        "issuer": issuer,
        "token_type": token_type,
        "page_id": page_id,
        "layer_name": event_layer_name,
        "permissions": {
            "sync": sync,
            "write": write,
        },
    })
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

/// Information about a subscribed peer
///
/// **Design**: Stores the parsed permit directly instead of extracted fields.
/// All access checks delegate to gurkha::Permit methods, ensuring consistency.
pub struct SubscriberInfo {
    /// Parsed page permit for all access checks (static layers)
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
    /// Check if subscriber can receive updates for layer (two-tier)
    ///
    /// **Context**: Used at subscribe time to populate LayerUnit.authorized_dids
    /// **Two-tier**: Checks page permit (static layers) then layer permits (dynamic layers)
    pub fn can_receive_layer(&self, layer_name: &str, page_id: &str) -> bool {
        // Block presence layer for peers who can't see others
        if (layer_name == "presence" || layer_name.ends_with("/presence")) && !self.can_see_others {
            return false;
        }
        // Check if layer is in no_incoming_updates (sync facts)
        if self
            .permit
            .sync_facts()
            .no_incoming_updates
            .contains(&layer_name.to_string())
        {
            return false;
        }
        // Check page permit only (layer permits are now handled via LayerUnit.authorized_dids)
        self.permit
            .can_read_layer(layer_name, page_id, &self.subscriber_did)
    }

    /// Check if subscriber can write to layer
    ///
    /// **Context**: Used during incoming update validation
    /// **Note**: Write auth stays subscriber-centric for now. For dynamic layer writes,
    /// the node trusts sync_target (path 2 in Permissions::can_write).
    pub fn can_write_layer(&self, layer_name: &str, page_id: &str) -> bool {
        self.permit
            .can_write_layer(layer_name, page_id, &self.subscriber_did)
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
    /// Peer subscribers (user_did, device_id) → SubscriberInfo
    /// Arc<RwLock> allows Loro observer callback to send directly to peers
    pub subscribers: Arc<RwLock<HashMap<(String, String), SubscriberInfo>>>,
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
    /// Uses gurkha::Permit directly with page_id/did passed to methods
    pub our_permit: Option<gurkha::Permit>,
    /// Our DID (for pattern expansion with {aud})
    pub our_did: String,

    /// Shutdown sender for node script tick loop (if running)
    /// **Context**: Set when is_node=true and entry_node found in manifest
    /// **Usage**: Send () to shutdown the tick loop
    pub node_script_shutdown: Option<mpsc::Sender<()>>,

    /// Pending layer authorizations for layers that don't exist yet
    /// **Context**: AuthorizeLayerSubscriber received before create_layer_from_peer
    /// **Usage**: Applied when the layer is created
    pub pending_layer_authorizations: HashMap<String, Vec<String>>,
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
    /// Protocol-reserved sync metadata layer name.
    ///
    /// These layers are pairwise and handled by protocol rules, not permit templates.
    fn is_protocol_sync_meta_layer(layer_name: &str) -> bool {
        layer_name.starts_with("__sync_meta/")
    }

    /// Get the next capture sequence number (monotonically increasing)
    fn next_capture_seq(&self) -> u64 {
        self.capture_seq.fetch_add(1, Ordering::Relaxed)
    }

    /// Check if a layer should sync based on permit configuration
    ///
    /// **Context**: Used to filter out local-only layers (sync: false) before broadcasting
    /// **Returns**: true if layer should sync, false if local-only
    pub fn should_sync_layer(&self, layer_name: &str) -> bool {
        if Self::is_protocol_sync_meta_layer(layer_name) {
            return true;
        }

        self.our_permit
            .as_ref()
            .map(|permit| permit.should_sync_layer(layer_name, &self.page_id, &self.our_did))
            .unwrap_or(true) // No permit means sync everything
    }

    /// Track a newly created sync-enabled layer in our protocol sync metadata map.
    ///
    /// **Context**: Local layer creation (dynamic layer or first-write implicit layer).
    /// **We store**: `__sync_meta/<our_did>[<layer_name>] = {synced: false, version: 1}`.
    /// **Why**: Keep pairwise sync metadata current so node receives layer bootstrap state.
    pub fn track_new_sync_layer_in_meta(&mut self, layer_name: &str) {
        if !self.should_sync_layer(layer_name) || Self::is_protocol_sync_meta_layer(layer_name) {
            return;
        }

        let sync_meta_layer = format!("__sync_meta/{}", self.our_did);
        let created_sync_meta_layer = if !self.units.contains_key(&sync_meta_layer) {
            let mut unit = LayerUnit::new_empty();
            unit.set_local_only(false);
            self.units.insert(sync_meta_layer.clone(), unit);
            true
        } else {
            false
        };

        if let Some(unit) = self.units.get_mut(&sync_meta_layer) {
            let map = unit.layer().loro().get_map(sync_meta_layer.as_str());
            if map
                .insert(
                    layer_name,
                    json_to_loro_value(&serde_json::json!({"synced": false, "version": 1})),
                )
                .is_ok()
            {
                unit.layer().commit();
                unit.mark_dirty();
            }
        }

        if created_sync_meta_layer {
            crate::loro_observer::setup_layer_observer(self, &sync_meta_layer);
        }
    }

    /// Check if we can write to a layer
    pub fn can_write_layer(&self, layer_name: &str) -> bool {
        self.our_permit
            .as_ref()
            .map(|permit| permit.can_write_layer(layer_name, &self.page_id, &self.our_did))
            .unwrap_or(true) // No permit means full access
    }

    /// Emit a sync event to the capture channel (pre-serialized JSON line)
    ///
    /// **Context**: Called before sync_event_tx.send() to capture SyncEvent for observability
    pub fn emit_sync_event_capture(&self, event: &SyncEvent) {
        if let Some(ref tx) = self.capture_tx {
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0);
            if let Ok(json) = serde_json::to_string(&serde_json::json!({
                "type": "sync_event",
                "ts": ts,
                "seq": self.next_capture_seq(),
                "page_id": &self.page_id,
                "data": event,
            })) {
                let _ = tx.send(json);
            }

            match event {
                SyncEvent::NewDynamicLayer {
                    page_id,
                    layer_name,
                    permits,
                }
                | SyncEvent::LayerAccessChanged {
                    page_id,
                    layer_name,
                    permits,
                } => {
                    let source = match event {
                        SyncEvent::NewDynamicLayer { .. } => "new_dynamic_layer",
                        SyncEvent::LayerAccessChanged { .. } => "layer_access_changed",
                        SyncEvent::EnsureSync { .. } => "ensure_sync",
                    };

                    for (audience_did, token) in permits {
                        let permit_summary = decode_permit_summary(token, layer_name);
                        if let Ok(json) = serde_json::to_string(&serde_json::json!({
                            "type": "permit_generated",
                            "ts": ts,
                            "seq": self.next_capture_seq(),
                            "page_id": page_id,
                            "layer": layer_name,
                            "layer_short": layer_short(layer_name),
                            "audience": audience_did,
                            "short_did": short_did(audience_did),
                            "source": source,
                            "permit": permit_summary,
                        })) {
                            let _ = tx.send(json);
                        }
                    }
                }
                SyncEvent::EnsureSync { .. } => {}
            }
        }
    }

    /// Emit a layer authorization event to the capture channel
    ///
    /// **Context**: Called when a DID is authorized/pending for a layer, for observability
    pub fn emit_layer_auth_capture(&self, layer: &str, did: &str, action: &str) {
        if let Some(ref tx) = self.capture_tx {
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0);
            if let Ok(json) = serde_json::to_string(&serde_json::json!({
                "type": "layer_auth",
                "ts": ts,
                "seq": self.next_capture_seq(),
                "page_id": &self.page_id,
                "layer": layer,
                "layer_short": layer_short(layer),
                "did": did,
                "short_did": short_did(did),
                "action": action,
            })) {
                let _ = tx.send(json);
            }
        }
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
        if let Some(ref tx) = self.capture_tx {
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0);
            if let Ok(json) = serde_json::to_string(&serde_json::json!({
                "type": "permission_check",
                "ts": ts,
                "seq": self.next_capture_seq(),
                "page_id": &self.page_id,
                "layer": layer,
                "layer_short": layer_short(layer),
                "peer_did": peer_did,
                "short_did": short_did(peer_did),
                "operation": "write",
                "result": result,
                "granted_by": granted_by,
            })) {
                let _ = tx.send(json);
            }
        }
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
        if let Some(ref tx) = self.capture_tx {
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0);
            if let Ok(json) = serde_json::to_string(&serde_json::json!({
                "type": "apply_update",
                "ts": ts,
                "seq": self.next_capture_seq(),
                "page_id": &self.page_id,
                "layer": layer,
                "layer_short": layer_short(layer),
                "from_peer": from_peer,
                "short_did": from_peer.map(short_did),
                "result": result,
                "reason": reason,
            })) {
                let _ = tx.send(json);
            }
        }
    }

    /// Emit a page update to the capture channel (pre-serialized JSON line)
    ///
    /// **Context**: Called alongside page_update_subscribers fanout for observability
    pub fn emit_page_update_capture(&self, update: &PageUpdate) {
        if let Some(ref tx) = self.capture_tx {
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0);
            if let Ok(json) = serde_json::to_string(&serde_json::json!({
                "type": "page_update",
                "ts": ts,
                "seq": self.next_capture_seq(),
                "page_id": &self.page_id,
                "data": update,
            })) {
                let _ = tx.send(json);
            }
        }
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
        if let Some(ref tx) = self.capture_tx {
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0);
            let short_layers: Vec<String> =
                authorized_layers.iter().map(|l| layer_short(l)).collect();
            if let Ok(json) = serde_json::to_string(&serde_json::json!({
                "type": "subscriber_state",
                "ts": ts,
                "seq": self.next_capture_seq(),
                "page_id": &self.page_id,
                "peer_did": peer_did,
                "short_did": short_did(peer_did),
                "authorized_layers": authorized_layers,
                "authorized_layers_short": short_layers,
                "total_layers": total_layers,
                "can_see_others": can_see_others,
                "is_visible": is_visible,
            })) {
                let _ = tx.send(json);
            }
        }
    }

    /// Emit a broadcast decision skip event to the capture channel
    ///
    /// **Context**: Called when a subscriber is skipped during broadcast, for observability
    pub fn emit_broadcast_decision_capture(&self, layer: &str, peer_did: &str, reason: &str) {
        if let Some(ref tx) = self.capture_tx {
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0);
            if let Ok(json) = serde_json::to_string(&serde_json::json!({
                "type": "broadcast_decision",
                "ts": ts,
                "seq": self.next_capture_seq(),
                "page_id": &self.page_id,
                "layer": layer,
                "layer_short": layer_short(layer),
                "peer_did": peer_did,
                "short_did": short_did(peer_did),
                "decision": "skip",
                "reason": reason,
            })) {
                let _ = tx.send(json);
            }
        }
    }
}
