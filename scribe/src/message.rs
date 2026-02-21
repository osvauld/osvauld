//! Scribe message types
//!
//! Contains both the actor messages (ScribeMessage) and shared types
//! (BroadcastPayload, PageUpdate, etc.)

use std::collections::HashMap;

use ractor::RpcReplyPort;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::Result;
use domains::{JsonOp, QueryDelta, QueryResult, QuerySpec};

/// Payload sent to PeerActor for broadcast (3-step sync protocol)
///
/// **Context**: Scribe sends this to PeerActor, who then sends SyncOffer to peer
/// **state_vector**: Our state vector after this update (for 3-step protocol)
/// **layer_type**: Protocol routing hint (App=snapshot, Data=incremental, Static=blob)
#[derive(Debug, Clone)]
pub struct BroadcastPayload {
    pub page_id: String,
    pub layer_name: String,
    pub update: Vec<u8>,
    /// Our state vector for this layer (for SyncOffer message)
    pub state_vector: Vec<u8>,
    /// Layer type for protocol routing
    pub layer_type: domains::LayerType,
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
    Map {
        updated: HashMap<String, Option<serde_json::Value>>,
    },
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

/// Parsed dynamic-layer metadata propagated to Lua/UI callbacks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamicLayerMeta {
    pub schema_key: String,
    pub creator_did: Option<String>,
    pub placeholders: HashMap<String, String>,
}

// Unified Page Update Channel (Replaces PageEvent + EphemeralEvent + QueryDelta)

/// Unified update enum for all page events
///
/// **Context**: Single channel replaces 3 separate broadcast types
/// **Benefits**: 3 listener tasks → 1, unified fan-out logic
/// **Consumers**: UI/Lua callbacks, Broadcast to peers, Derivation engine
#[derive(Debug, Clone, Serialize)]
pub enum PageUpdate {
    /// Layer data changed (local edit or remote sync)
    LayerChanged {
        layer: String,
        from_peer: Option<(String, String)>, // (did, device_id)
        /// Structured operations extracted from the update (same format as validation)
        /// Enables surgical UI updates - Lua can process individual ops
        ops: Option<Vec<JsonOp>>,
        delta: Option<LoroDelta>,
        state_vector: Vec<u8>,
        /// Full layer state as JSON — None when delta is available (avoids O(N) serialization)
        /// Some for initial load, wildcard bindings, or fallback scenarios
        full_data: Option<serde_json::Value>,
        /// True if this is a newly created layer (first time seeing it)
        created: bool,
        /// Parsed metadata when layer matches a dynamic schema.
        dynamic_ref: Option<DynamicLayerMeta>,
    },

    /// Ephemeral data from remote peer (cursor, typing, etc.)
    Ephemeral {
        user_did: String,
        device_id: String,
        payload: Vec<u8>,
    },

    /// Query result changed
    QueryUpdated { query_id: String, delta: QueryDelta },

    /// Peer subscribed to this page
    PeerSubscribed {
        did: String,
        /// Username if presence.share_username is true in their permit
        username: Option<String>,
    },

    /// Peer unsubscribed from this page
    PeerUnsubscribed { did: String },

    /// Structured ephemeral received (permission-validated)
    ///
    /// **Context**: Ephemeral with func name, validated against permit
    StructuredEphemeral {
        from_did: String,
        func: String,
        args: serde_json::Value,
    },
}

/// Type alias for unified page update channel
pub type PageUpdateTx = mpsc::Sender<PageUpdate>;

/// Events emitted for sync coordination (consumed by Coordinator)
///
/// **Context**: Scribe emits these when it needs to sync with a user.
/// NOTE: Scribe only knows user_did. Coordinator handles everything else:
/// - Device resolution
/// - Connection establishment
/// - Subscription (PeerActor discovers active Scribes on connect)
#[derive(Debug, Clone, Serialize)]
pub enum SyncEvent {
    /// Ensure this user is synced (connected + subscribed to active Scribes)
    EnsureSync { user_did: String },

    /// Subscribe to dynamic layers discovered in a creator's __sync_meta
    ///
    /// **Context**: Node detected new entries in creator's __sync_meta via apply.
    /// Coordinator should route to the PeerActor connected to creator_did
    /// and send LayerSubscribe for each layer.
    SubscribeLayers {
        page_id: String,
        creator_did: String,
        layers: Vec<String>,
    },
}

// ScribeMessage - Actor Messages for Scribe

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
    Unsubscribe { user_did: String, device_id: String },

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

    /// Replace layer entirely with snapshot (for SyncReset recovery)
    ///
    /// **Context**: User mode received SyncSnapshot after max resyncs exceeded
    /// **We do**: Replace local layer with authoritative snapshot from node
    /// **We also**: Set sender's peer vector atomically (if provided)
    /// **Invariant**: Node is source of truth for divergence resolution
    ReplaceLayer {
        layer_name: String,
        snapshot: Vec<u8>,
        /// Sender's peer info + state vector to set atomically after replacement.
        /// Without this, the replaced layer has empty subscriber vectors and
        /// the next broadcast would send a full snapshot unnecessarily.
        from_peer: Option<(String, String, Vec<u8>)>, // (user_did, device_id, state_vector)
        reply: tokio::sync::oneshot::Sender<std::result::Result<(), String>>,
    },

    /// Flush dirty layers to storage
    Flush,

    /// Periodic reconciliation - check all subscribers for divergence
    ///
    /// **Context**: Timer fires every 30 seconds
    /// **We do**: Compare vectors, re-broadcast if diverged
    ReconcileWithPeers,

    /// Ensure a list layer exists (creates if needed)
    ///
    /// **Context**: Lua calls loro:list("name") - ensures layer exists
    /// **We do**: Create layer if needed, reply with success
    /// **Why**: Lua doesn't hold direct LoroList handle (stale after ReplaceLayer)
    EnsureLoroList {
        layer_name: String,
        reply: tokio::sync::oneshot::Sender<Result<()>>,
    },

    /// Ensure a map layer exists (creates if needed)
    ///
    /// **Context**: Lua calls loro:map("name") - ensures layer exists
    /// **We do**: Create layer if needed, reply with success
    /// **Why**: Lua doesn't hold direct LoroMap handle (stale after ReplaceLayer)
    EnsureLoroMap {
        layer_name: String,
        reply: tokio::sync::oneshot::Sender<Result<()>>,
    },

    /// Check if a layer exists (for get_list/get_map that return nil if missing)
    ///
    /// **Context**: Lua calls loro:get_list("name") - returns nil if missing
    LayerExists {
        layer_name: String,
        reply: tokio::sync::oneshot::Sender<bool>,
    },

    // Typed read operations (Lua → Scribe, avoids stale handles)
    /// Get item at index from a list layer
    ///
    /// **Context**: Lua calls list:get(index)
    /// **We do**: Read from current LoroDoc (never stale)
    ListGet {
        layer_name: String,
        index: usize,
        reply: tokio::sync::oneshot::Sender<Result<Option<serde_json::Value>>>,
    },

    /// Get list length
    ///
    /// **Context**: Lua calls list:length()
    ListLength {
        layer_name: String,
        reply: tokio::sync::oneshot::Sender<Result<usize>>,
    },

    /// Get value by key from a map layer
    ///
    /// **Context**: Lua calls map:get(key)
    MapGet {
        layer_name: String,
        key: String,
        reply: tokio::sync::oneshot::Sender<Result<Option<serde_json::Value>>>,
    },

    /// Get map length
    ///
    /// **Context**: Lua calls map:length()
    MapLength {
        layer_name: String,
        reply: tokio::sync::oneshot::Sender<Result<usize>>,
    },

    /// Get all keys from a map
    ///
    /// **Context**: Lua calls map:keys()
    MapKeys {
        layer_name: String,
        reply: tokio::sync::oneshot::Sender<Result<Vec<String>>>,
    },

    /// List layers matching a pattern
    ///
    /// **Context**: Lua calls loro:list_layers("*:order")
    /// **We do**: Return layer names matching glob pattern
    ListLayers {
        pattern: String,
        reply: tokio::sync::oneshot::Sender<Vec<String>>,
    },

    /// Subscribe to all page updates (unified channel)
    ///
    /// **Context**: App runtime subscribes to receive ALL page updates
    /// **We do**: Store sender, forward PageUpdate events when they occur
    /// **Consumers**: UI/Lua callbacks, Broadcast to peers, Derivation engine - all filter as needed
    /// **Design**: Unified channel for layer changes, ephemeral data, and peer presence
    SubscribeToPageUpdates { tx: mpsc::Sender<PageUpdate> },

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
        reply: tokio::sync::oneshot::Sender<Result<QueryResult>>,
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
    UnsubscribeQuery { query_id: String },

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

    // Typed CRDT Operations (from template actions)
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

    /// Create a dynamic layer (from Lua app)
    ///
    /// **Context**: Lua calls scribe:create_layer("channels/general/messages")
    /// **We do**: Validate schema exists in permit, generate path with our DID, create layer
    /// **We reply**: Full layer name (with page_id and DID prefix)
    CreateDynamicLayer {
        /// Schema key from dynamic_layer_schemas, e.g. "channels/{id}/messages"
        schema_key: String,
        /// Placeholder values for schema variables.
        ///
        /// Example:
        /// - schema `channels/{id}/messages` -> `{ "id": "general" }`
        /// - schema `channels/{channel}/messages/{period}` ->
        ///   `{ "channel": "general", "period": "2025-02" }`
        placeholders: std::collections::HashMap<String, String>,
        /// Optional list of authorized peers (for explicit-grant layers like DMs)
        /// None = open/role-based, Some = only listed DIDs get access
        authorized_peers: Option<Vec<String>>,
        reply: tokio::sync::oneshot::Sender<std::result::Result<String, String>>,
    },

    /// Issue access for DIDs to an explicit dynamic layer
    ///
    /// **Context**: Lua calls scribe:add_layer_access(layer_name, dids)
    /// **We do**: Merge new DIDs into self-permit's authorized_peers, re-trigger sync
    AddLayerAccess {
        layer_name: String,
        dids: Vec<String>,
        reply: tokio::sync::oneshot::Sender<std::result::Result<(), String>>,
    },

    /// Export a layer's snapshot + state vector (for LayerSync bundling)
    ///
    /// **Context**: PeerActor needs snapshot data to bundle with layer permit
    /// **We do**: Export snapshot and version vector from LayerUnit, reply via oneshot
    ExportLayerSnapshot {
        layer_name: String,
        reply: tokio::sync::oneshot::Sender<std::result::Result<(Vec<u8>, Vec<u8>), String>>,
    },

    /// Authorize a subscriber DID for a specific layer
    ///
    /// **Context**: Viewer received LayerPermit from node. Authorize the node
    /// (subscriber) on the local Scribe's LayerUnit so the observer broadcasts to it.
    AuthorizeLayerSubscriber {
        layer_name: String,
        subscriber_did: String,
    },

    /// Get unsynced layer entries from our __sync_meta (peer-side)
    ///
    /// **Context**: PeerActor detects __sync_meta update, queries for unsynced entries
    /// **We do**: Read our __sync_meta layer, return entries with synced == false
    GetUnsyncedSyncMeta {
        reply: tokio::sync::oneshot::Sender<Vec<String>>,
    },

    /// Handle LayerSubscribe request (node-side)
    ///
    /// **Context**: Peer sends LayerSubscribe after discovering layer in __sync_meta
    /// **We do**: Validate, issue permit via PermitIssuer, export snapshot, add subscriber
    /// **Returns**: (snapshot_data, state_vector, layer_permit_token)
    HandleLayerSubscribe {
        layer_name: String,
        peer_did: String,
        reply:
            tokio::sync::oneshot::Sender<std::result::Result<(Vec<u8>, Vec<u8>, String), String>>,
    },

    /// Mark a __sync_meta entry as synced (peer-side)
    ///
    /// **Context**: Peer received LayerSubscribeAck, marks entry as synced
    MarkSyncMetaSynced { layer_name: String },

    /// Node received authority from creator. Store + fan out to authorized users.
    ///
    /// **Context**: Node's PeerActor received LayerSubscribeAck with layer_authority.
    /// **We do**: Store authority, apply layer data, fan out to authorized peers' __sync_meta.
    StoreLayerAuthority {
        layer_name: String,
        creator_did: String,
        authority_token: String,
        layer_data: Vec<u8>,
        state_vector: Vec<u8>,
    },

    /// Fan out a layer entry to authorized users' __sync_meta
    ///
    /// **Context**: Node stored authority, now needs to notify authorized peers.
    /// **authorized_peers**: None = all subscribers, Some = specific DIDs only.
    FanOutLayerToUsers {
        layer_name: String,
        authorized_peers: Option<Vec<String>>,
    },

    /// Shutdown the actor
    Shutdown,

    // Ephemeral Events (from datagrams via PeerActor)
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

    /// Send structured ephemeral with func name (permission-validated)
    ///
    /// **Context**: Local user wants to broadcast structured ephemeral (typing, cursor, etc.)
    /// **We do**: Validate func against our permit's ephemeral_funcs, then broadcast
    /// **Design**: Structured format allows permission control per-function
    SendStructuredEphemeral {
        /// Function name (validated against permit.ephemeral_funcs)
        func: String,
        /// Arguments (arbitrary JSON)
        args: serde_json::Value,
    },

    /// Remote structured ephemeral received (needs validation)
    ///
    /// **Context**: Peer sent structured ephemeral via datagram
    /// **We do**: Validate func against sender's permit, then emit to app subscribers
    RemoteStructuredEphemeral {
        /// Sender's DID
        from_did: String,
        /// Sender's device ID
        device_id: String,
        /// Function name (validated against sender's permit)
        func: String,
        /// Arguments
        args: serde_json::Value,
    },

    /// Create an empty derived layer
    ///
    /// **Context**: When registering a derivation rule, create target layer
    CreateDerivedLayer { target_layer: String },

    /// Get app files from in-memory layer
    ///
    /// **Context**: App restart needs current files from Scribe's in-memory layer
    /// **We do**: Extract files from app layer and return
    /// **Why**: Storage may be stale; Scribe has the latest synced content
    GetAppFiles {
        app_name: String,
        reply: tokio::sync::oneshot::Sender<std::result::Result<HashMap<String, String>, String>>,
    },

    /// Refresh app files in-memory (owner dev workflow)
    ///
    /// **Context**: Owner edited files on disk; Butler read them and sends here
    /// **We do**: Compare with current layer, update LoroMap, commit, mark dirty
    /// **Observer**: Loro observer broadcasts PageUpdate to subscribers
    /// **Why**: Goes through Scribe so CRDT state, observers, and sync all fire correctly
    RefreshAppFiles {
        app_name: String,
        files: HashMap<String, String>,
        reply: tokio::sync::oneshot::Sender<std::result::Result<Vec<String>, String>>,
    },

    /// Get subscriber count (for Lua peers:count() binding)
    ///
    /// **Context**: Node script wants to check if anyone is listening before broadcasting
    /// **We do**: Return count of current subscribers
    GetSubscriberCount {
        reply: tokio::sync::oneshot::Sender<usize>,
    },

    /// Update a peer's state vector on a specific layer (fire-and-forget)
    ///
    /// **Context**: PeerActor received SyncAccept with peer's current state vector
    /// **We do**: Update the subscriber's version vector on the target LayerUnit
    /// so subsequent broadcasts send only incremental diffs
    UpdatePeerVector {
        user_did: String,
        device_id: String,
        layer_name: String,
        state_vector: Vec<u8>,
    },
}
