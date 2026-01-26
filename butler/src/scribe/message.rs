//! Scribe message types
//!
//! All messages received by the Scribe actor.

use std::collections::HashMap;
use std::path::PathBuf;
use ractor::RpcReplyPort;
use tokio::sync::mpsc;
use serde::{Serialize, Deserialize};

use crate::models::{QuerySpec, QueryDelta};
use crate::error::Result;

/// Messages received by Scribe actor
#[derive(Debug)]
pub enum ScribeMessage {
    /// Peer subscription - PeerActor subscribes to receive layer updates
    ///
    /// **Context**: PeerActor connects for a page and wants sync updates
    /// **We do**: Parse permit, extract layer permissions + sync policies, store subscriber
    Subscribe {
        user_did: String,
        device_id: String,
        /// Callback to send CRDT broadcasts to this peer
        broadcast_tx: mpsc::Sender<BroadcastPayload>,
        /// Callback to send ephemeral broadcasts to this peer (cursor, typing)
        /// PeerActor creates this channel, spawns listener that sends datagrams
        ephemeral_tx: Option<mpsc::Sender<EphemeralOutbound>>,
        /// Raw permit string - we parse to extract permissions
        permit: String,
    },

    /// Peer unsubscription
    Unsubscribe {
        user_did: String,
        device_id: String,
    },

    /// Apply layer update (from local UI or remote peer)
    ///
    /// **Context**: Local edit or remote sync push
    /// **We do**: Permission check, CRDT merge, broadcast to subscribers
    ApplyUpdate {
        layer_name: String,
        update: Vec<u8>,
        /// None = local edit, Some = remote peer
        from_peer: Option<(String, String)>,
        /// Permit from SyncOffer (for peers not yet subscribed)
        permit: Option<String>,
    },

    /// Apply layer update with result feedback (for SyncAccept control)
    ///
    /// **Context**: Remote sync push that needs success/failure feedback
    /// **We do**: Permission check, CRDT merge, broadcast to subscribers, reply with result
    ApplyUpdateWithResult {
        layer_name: String,
        update: Vec<u8>,
        from_peer: Option<(String, String)>,
        permit: Option<String>,
        reply: tokio::sync::oneshot::Sender<std::result::Result<(), String>>,
    },

    /// Sync request - peer wants updates since their version
    SyncRequest {
        layer_name: String,
        their_vector: Vec<u8>,
        reply: RpcReplyPort<Result<Vec<u8>>>,
    },

    /// Export full snapshot (for initial sync or send_full_snapshot policy)
    ExportSnapshot {
        layer_name: String,
        reply: RpcReplyPort<Result<Vec<u8>>>,
    },

    /// Get our state vector (for incremental sync)
    GetStateVector {
        layer_name: String,
        reply: RpcReplyPort<Result<Vec<u8>>>,
    },

    /// Get updates since a given state vector (for resync after divergence)
    ///
    /// **Context**: SyncAccept showed divergence, sender needs full diff
    /// **We do**: Export updates from our LoroDoc since their vector
    GetUpdatesSince {
        layer_name: String,
        state_vector: Vec<u8>,
        reply: tokio::sync::oneshot::Sender<Result<Vec<u8>>>,
    },

    /// Flush dirty layers to storage
    Flush,

    /// Update peer's cached state vector (from PeerActor after SyncAccept/SyncAck)
    ///
    /// **Context**: PeerActor received confirmation of peer's state
    /// **We do**: Update in-memory vector cache (persistence via periodic flush)
    UpdatePeerVector {
        user_did: String,
        device_id: String,
        layer_name: String,
        state_vector: Vec<u8>,
    },

    /// Periodic reconciliation - check all subscribers for divergence
    ///
    /// **Context**: Timer fires every 30 seconds
    /// **We do**: Compare vectors, re-broadcast if diverged
    ReconcileWithPeers,

    /// Get LoroList container reference for Lua FFI
    ///
    /// **Context**: Lua app wants direct access to a list container
    /// **We do**: Get layer, extract LoroList handle, return to Lua
    GetLoroList {
        layer_name: String,
        reply: tokio::sync::oneshot::Sender<Result<loro::LoroList>>,
    },

    /// Get LoroMap container reference for Lua FFI
    ///
    /// **Context**: Lua app wants direct access to a map container
    /// **We do**: Get layer, extract LoroMap handle, return to Lua
    GetLoroMap {
        layer_name: String,
        reply: tokio::sync::oneshot::Sender<Result<loro::LoroMap>>,
    },

    /// Get or create LoroList (creates layer if doesn't exist)
    ///
    /// **Context**: Lua calls loro:get_or_create_layer("name", "list")
    /// **We do**: Get layer or create new one, return LoroList handle
    GetOrCreateLoroList {
        layer_name: String,
        reply: tokio::sync::oneshot::Sender<Result<loro::LoroList>>,
    },

    /// Get or create LoroMap (creates layer if doesn't exist)
    ///
    /// **Context**: Lua calls loro:get_or_create_layer("name", "map")
    /// **We do**: Get layer or create new one, return LoroMap handle
    GetOrCreateLoroMap {
        layer_name: String,
        reply: tokio::sync::oneshot::Sender<Result<loro::LoroMap>>,
    },

    /// List layers matching a pattern
    ///
    /// **Context**: Lua calls loro:list_layers("*:order")
    /// **We do**: Return layer names matching glob pattern
    ListLayers {
        pattern: String,
        reply: tokio::sync::oneshot::Sender<Vec<String>>,
    },

    /// Subscribe to all page events (unified channel)
    ///
    /// **Context**: App runtime subscribes to receive ALL layer changes for a page
    /// **We do**: Return receiver end of the page event broadcast channel
    /// **Consumers**: UI/Lua callbacks, Broadcast to peers, Derivation engine - all filter as needed
    /// **Design**: Replaces per-layer SubscribeToLoroChanges - one subscription, all layers
    SubscribeToPageEvents {
        event_tx: mpsc::Sender<PageEvent>,
    },

    /// Notify that Lua modified a layer directly via FFI
    ///
    /// **Context**: Lua called push/set/etc on a Loro container (LoroList/LoroMap)
    /// **We do**: Save layer, notify observers, broadcast to peers
    LayerModifiedByLua {
        layer_name: String,
    },

    /// Commit changes to a layer (for Lua bindings)
    ///
    /// **Context**: Lua finished modifying a layer, trigger sync
    /// **We do**: Commit LoroDoc changes, broadcast to peers
    CommitLayer {
        layer_name: String,
    },

    /// Get layer data as JSON value
    ///
    /// **Context**: Lua bindings need to read layer content
    /// **We do**: Export layer as serde_json::Value
    GetLayerData {
        layer_name: String,
        reply: tokio::sync::oneshot::Sender<Result<serde_json::Value>>,
    },

    /// Get current snapshot for a layer (for testing/debugging)
    ///
    /// **Context**: Test wants to verify Scribe state without waiting for persistence
    /// **We do**: Export current snapshot and return via reply channel
    GetSnapshot {
        layer_name: String,
        reply: tokio::sync::oneshot::Sender<Option<Vec<u8>>>,
    },

    /// Get layer data as JSON (for egui rendering)
    ///
    /// **Context**: egui window wants to read Loro data directly
    /// **We do**: Export layer content as JSON value
    GetLayerJson {
        layer_name: String,
        reply: tokio::sync::oneshot::Sender<Option<serde_json::Value>>,
    },

    /// Get all layers as JSON context (for egui rendering)
    ///
    /// **Context**: egui window wants full context for CEL evaluation
    /// **We do**: Export all layer contents as a single JSON object
    GetContext {
        reply: tokio::sync::oneshot::Sender<serde_json::Value>,
    },

    /// Execute a query and return results
    ///
    /// **Context**: HUML renderer needs filtered/sorted/paginated data
    /// **We do**: Execute query against layer, apply filter/sort/limit, return results
    Query {
        spec: QuerySpec,
        reply: tokio::sync::oneshot::Sender<Result<crate::models::QueryResult>>,
    },

    /// Subscribe to query updates
    ///
    /// **Context**: HUML renderer wants incremental updates when query results change
    /// **We do**: Store subscription, send QueryDelta when underlying data changes
    SubscribeQuery {
        spec: QuerySpec,
        /// Channel to send deltas when results change
        delta_tx: mpsc::Sender<QueryDelta>,
    },

    /// Unsubscribe from query updates
    UnsubscribeQuery {
        query_id: String,
    },

    /// Update layer from JSON (for UI commits via CEL commit())
    ///
    /// **Context**: HUML renderer's CEL `commit()` returns JSON values to persist.
    /// **We do**: Update the layer at the given path, notify query subscribers.
    UpdateFromJson {
        layer_name: String,
        path: String,
        value: serde_json::Value,
        /// Optional reply for error handling
        reply: Option<tokio::sync::oneshot::Sender<Result<()>>>,
    },

    // =========================================================================
    // Typed CRDT Operations (from template actions)
    // =========================================================================

    /// Push item to a list (append)
    ///
    /// **Context**: Template `action: { messages: append({...}) }`
    /// **We do**: Layer.list_push, broadcast, notify subscribers
    ListPush {
        layer_name: String,
        path: String,
        item: serde_json::Value,
    },

    /// Insert item at index in a list
    ListInsert {
        layer_name: String,
        path: String,
        index: usize,
        item: serde_json::Value,
    },

    /// Delete item at index from a list
    ListDelete {
        layer_name: String,
        path: String,
        index: usize,
    },

    /// Insert/update key in a map
    ///
    /// **Context**: Template `action: { users: set("id", {...}) }`
    MapInsert {
        layer_name: String,
        path: String,
        key: String,
        value: serde_json::Value,
    },

    /// Delete key from a map
    MapDelete {
        layer_name: String,
        path: String,
        key: String,
    },

    /// Increment/decrement a counter
    ///
    /// **Context**: Template `action: { likes: increment(1) }`
    CounterInc {
        layer_name: String,
        path: String,
        amount: i64,
    },

    /// Shutdown the actor
    Shutdown,

    // =========================================================================
    // Ephemeral Events (from datagrams via PeerActor)
    // =========================================================================

    /// Remote ephemeral data (from datagram via PeerActor)
    ///
    /// **Context**: Peer sent ephemeral data (cursor, typing, etc.) for this page
    /// **We do**: Forward to subscribed apps AND relay to other peers via ephemeral channels
    /// **Note**: Not persisted in CRDT - purely ephemeral
    /// **Design**: Protocol layer routes opaque bytes; app layer defines meaning
    RemoteEphemeral {
        /// Who sent this (from peer state, may be None if not authenticated)
        user_did: Option<String>,
        /// Device/connection ID of sender (for relay exclusion)
        device_id: Option<String>,
        /// Opaque payload - app defines format (JSON with type/x/y, etc.)
        payload: Vec<u8>,
    },

    /// Send ephemeral data to peers (from Lua)
    ///
    /// **Context**: Local user wants to broadcast ephemeral data (cursor, typing, etc.)
    /// **We do**: Forward via ephemeral broadcast channel to Courier
    /// **Design**: App layer provides opaque payload; protocol layer routes by page_id
    SendEphemeral {
        /// Opaque payload - app defines format (JSON with type/x/y, etc.)
        payload: Vec<u8>,
    },

    /// Subscribe to ephemeral events (cursor, typing, presence)
    ///
    /// **Context**: App runtime subscribes to receive ephemeral events
    /// **We do**: Store sender, forward events when they arrive
    SubscribeEphemeral {
        tx: mpsc::Sender<EphemeralEvent>,
    },

    // Note: SetEphemeralBroadcast removed - ephemeral now goes directly via
    // SubscriberInfo.ephemeral_tx (Scribe → PeerActor channels)

    // =========================================================================
    // Derivation Operations (via ScribeLuaRuntime)
    // =========================================================================

    /// Rebuild a derived layer from all source layers
    ///
    /// **Context**: Called on startup or manual rebuild request
    /// **We do**: Clear derived layer, transform ALL entries from ALL matching sources
    RebuildDerived {
        target: String,
        reply: tokio::sync::oneshot::Sender<std::result::Result<usize, String>>,
    },

    /// Rebuild all derived layers (on startup)
    RebuildAllDerived {
        reply: tokio::sync::oneshot::Sender<std::result::Result<(), String>>,
    },

    /// Check if derivation is enabled on this Scribe
    ///
    /// **Context**: Lua bindings check before registering rules
    IsDerivationEnabled {
        reply: tokio::sync::oneshot::Sender<bool>,
    },

    /// Create an empty derived layer
    ///
    /// **Context**: When registering a derivation rule, create target layer
    CreateDerivedLayer {
        target_layer: String,
    },

    // =========================================================================
    // App Refresh Operations
    // =========================================================================

    /// Refresh app from filesystem (owner only)
    ///
    /// **Context**: Owner wants to reload app code from disk (development workflow)
    /// **We do**: Read files from app_dir, update app layer, commit (triggers broadcast)
    /// **Consumers**: UI Reload button, Debug socket command
    RefreshApp {
        app_name: String,
        app_dir: PathBuf,
        reply: tokio::sync::oneshot::Sender<std::result::Result<Vec<String>, String>>,
    },

    /// Get app files from in-memory layer
    ///
    /// **Context**: App restart needs current files from Scribe's in-memory layer
    /// **We do**: Extract files from app layer and return
    /// **Why**: Storage may be stale; Scribe has the latest synced content
    GetAppFiles {
        app_name: String,
        reply: tokio::sync::oneshot::Sender<std::result::Result<std::collections::HashMap<String, String>, String>>,
    },

    /// Get subscriber count (for Lua peers:count() binding)
    ///
    /// **Context**: Node script wants to check if anyone is listening before broadcasting
    /// **We do**: Return count of current subscribers
    GetSubscriberCount {
        reply: tokio::sync::oneshot::Sender<usize>,
    },
}

/// Payload sent to PeerActor for broadcast (3-step sync protocol)
///
/// **Context**: Scribe sends this to PeerActor, who then sends SyncOffer to peer
/// **state_vector**: Our state vector after this update (for 3-step protocol)
#[derive(Debug, Clone)]
pub struct BroadcastPayload {
    pub page_id: String,
    pub layer_name: String,
    pub update: Vec<u8>,
    /// Our state vector for this layer (for SyncOffer message)
    pub state_vector: Vec<u8>,
}

/// Ephemeral broadcast payload (generic)
///
/// **Context**: Scribe sends this to Butler/Courier for datagram broadcast
/// **Transport**: Unreliable QUIC datagram (fire-and-forget, lowest latency)
/// **Design**: Generic payload - app defines format; protocol routes by page_id
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EphemeralBroadcast {
    /// Page ID for routing
    pub page_id: String,
    /// Opaque payload - app defines format (JSON with type/x/y, etc.)
    pub payload: Vec<u8>,
}

/// Outbound ephemeral data (Scribe → PeerActor via channel)
///
/// **Context**: Same pattern as BroadcastPayload for CRDT data
/// **Flow**: Scribe broadcasts ephemeral to all subscriber channels,
///           PeerActor listener receives and sends datagram to peer
#[derive(Debug, Clone)]
pub struct EphemeralOutbound {
    /// Page ID for routing (used in datagram)
    pub page_id: String,
    /// Opaque payload - app defines format (JSON with type/x/y, etc.)
    pub payload: Vec<u8>,
}

/// Delta operations for different Loro container types
///
/// **Context**: Represents incremental changes to CRDT containers
/// **Usage**: Lua apps can apply deltas for efficient UI updates (preserve scroll, animations)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum LoroDelta {
    /// List delta: sequence of Retain/Insert/Delete operations
    List { ops: Vec<ListOp> },

    /// Map delta: updated keys
    Map { updated: HashMap<String, Option<serde_json::Value>> },

    /// Text delta (for future rich text support)
    Text { ops: Vec<TextOp> },
}

/// List delta operations (applied sequentially)
///
/// **Context**: Represents changes to a Loro list container
/// **Example**: [Retain(3), Insert([msg4]), Delete(1)] means "skip 3 items, insert msg4, delete 1 item"
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op")]
pub enum ListOp {
    /// Keep N items unchanged (advance cursor)
    Retain { count: usize },

    /// Insert values at current position
    Insert { values: Vec<serde_json::Value> },

    /// Delete N items at current position
    Delete { count: usize },
}

/// Text delta operations (for rich text editing)
///
/// **Context**: Represents changes to a Loro text container
/// **Note**: Deferred for now - will implement when we need rich text editing
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op")]
pub enum TextOp {
    /// Keep N characters unchanged
    Retain { count: usize },

    /// Insert text at current position
    Insert { text: String },

    /// Delete N characters at current position
    Delete { count: usize },
}

// =============================================================================
// Unified Page Event Channel
// =============================================================================

/// Unified event for all layer changes in a page
///
/// **Context**: Single event type for all layer changes (local and remote)
/// **Consumers**: UI/Lua callbacks, Broadcast to peers, Derivation engine
/// **Design**: One channel per page, all layer observers feed into it
#[derive(Debug, Clone)]
pub struct PageEvent {
    /// Which layer changed
    pub layer_name: String,

    /// Who caused this change (None = local, Some = from peer)
    pub from_peer: Option<(String, String)>,  // (user_did, device_id)

    /// Type of change
    pub event_type: PageEventType,

    /// Incremental change (for efficient sync)
    pub delta: Option<LoroDelta>,

    /// Full layer state as JSON (for UI rendering)
    pub full_data: serde_json::Value,
}

/// Type of page event
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PageEventType {
    /// New layer created
    Created,
    /// Existing layer updated
    Updated,
}

/// Events emitted for sync coordination (consumed by Coordinator)
///
/// **Context**: Scribe emits these when it needs to sync with a user.
/// NOTE: Scribe only knows user_did. Coordinator handles everything else:
/// - Device resolution
/// - Connection establishment
/// - Subscription (PeerActor discovers active Scribes on connect)
#[derive(Debug, Clone)]
pub enum SyncEvent {
    /// Ensure this user is synced (connected + subscribed to active Scribes)
    EnsureSync { user_did: String },
}

// =============================================================================
// Ephemeral Events (Live Data Streaming)
// =============================================================================

/// Ephemeral events for UI (not persisted in CRDT)
///
/// **Context**: Real-time events sent via datagrams (unreliable, low-latency)
/// **Consumers**: App runtime (Lua) for rendering remote cursors, typing indicators
/// **Note**: These are fire-and-forget, missing one is OK
/// **Design**: Generic payload - app layer defines meaning (cursor, typing, presence, etc.)
#[derive(Debug, Clone)]
pub enum EphemeralEvent {
    /// Ephemeral data from remote peer
    ///
    /// **payload**: Opaque bytes - app interprets format (JSON with type/x/y, etc.)
    Data {
        user_did: String,
        payload: Vec<u8>,
    },

    /// Peer joined this page (subscribed)
    ///
    /// **Context**: Remote peer subscribed to this page's Scribe
    /// **Consumers**: Lua apps for online counters, presence indicators
    PeerJoined {
        user_did: String,
    },

    /// Peer left this page (unsubscribed)
    ///
    /// **Context**: Remote peer unsubscribed from this page's Scribe
    /// **Consumers**: Lua apps for online counters, presence indicators
    PeerLeft {
        user_did: String,
    },
}
