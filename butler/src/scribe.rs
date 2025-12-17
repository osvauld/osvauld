//! Scribe - Document actor for reactive CRDT synchronization
//!
//! **Context**: Each open page gets its own Scribe that owns LoroDoc layers
//! and handles bidirectional sync with permission-aware merge logic.
//!
//! **Key responsibilities:**
//! - Own LoroDoc layers for a page
//! - Handle permission checks (viewer/submitter/collaborator)
//! - Apply sync policies (local_only, no_incoming_updates, send_full_snapshot)
//! - Broadcast updates to subscribed PeerActors
//! - Notify UI subscribers of changes

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use ractor::{Actor, ActorProcessingErr, ActorRef, RpcReplyPort};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn, instrument};

use crate::models::{Layer, QuerySpec, QueryResult, QueryDelta};
use crate::error::{ButlerError, Result};

// =============================================================================
// Public Types
// =============================================================================

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
        /// Callback to send broadcasts to this peer
        broadcast_tx: mpsc::Sender<BroadcastPayload>,
        /// Raw permit string - we parse to extract permissions
        permit: String,
    },

    /// Peer unsubscription
    Unsubscribe {
        user_did: String,
        device_id: String,
    },

    /// UI subscription - Tauri event channel
    SubscribeUI {
        tx: mpsc::Sender<ScribeEvent>,
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

    /// Shutdown the actor
    Shutdown,
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

/// Events emitted to UI subscribers
#[derive(Debug, Clone)]
pub enum ScribeEvent {
    LayerUpdated { layer_name: String },
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

/// Layer capability - what operations a peer can perform
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayerCapability {
    /// Read only - can receive updates, cannot write
    Viewer,
    /// Write to isolated namespace only (scoped by user_did)
    Submitter,
    /// Full read/write
    Collaborator,
}

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

/// Sync configuration derived from permit
///
/// **Context**: Determines how this Scribe syncs based on role
/// - Owner: sync to sovereign node
/// - Node: broadcast only (no outbound sync)
/// - Viewer: sync to source node
#[derive(Debug, Clone)]
pub struct SyncConfig {
    /// Role from permit: "owner", "node", "viewer"
    pub relationship: String,
    /// For owner: sovereign node_id to sync to
    /// For viewer: source_node_id to sync to
    /// For node: None (broadcast only)
    pub sync_target: Option<String>,
}

// =============================================================================
// Injected Functions
// =============================================================================

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
// Actor State
// =============================================================================

/// Information about a subscribed peer
pub struct SubscriberInfo {
    /// Channel to send broadcasts
    broadcast_tx: mpsc::Sender<BroadcastPayload>,
    /// Their last known state vectors (layer_name → version_vector)
    vectors: HashMap<String, Vec<u8>>,
    /// Permissions for each layer
    layer_permissions: HashMap<String, LayerCapability>,
    /// Sync policies
    sync_policy: SyncPolicy,
}

/// Query subscription info
pub struct QuerySubscriberInfo {
    /// Query specification
    spec: QuerySpec,
    /// Channel to send deltas
    delta_tx: mpsc::Sender<QueryDelta>,
    /// Current version (monotonically increasing)
    version: u64,
    /// Last known result (for computing deltas)
    last_result: Option<QueryResult>,
}

/// Scribe actor state
pub struct ScribeState {
    /// Page identifier
    page_id: String,
    /// LoroDoc layers (layer_name → Layer)
    layers: HashMap<String, Layer>,
    /// Peer subscribers (user_did, device_id) → SubscriberInfo
    subscribers: HashMap<(String, String), SubscriberInfo>,
    /// UI subscribers
    ui_subscribers: Vec<mpsc::Sender<ScribeEvent>>,
    /// Query subscribers (query_id → QuerySubscriberInfo)
    query_subscribers: HashMap<String, QuerySubscriberInfo>,
    /// Layers with unsaved changes
    dirty_layers: HashSet<String>,
    /// Injected save function
    save_layer: SaveLayerFn,
    /// Injected load peer vector function
    load_peer_vector: LoadPeerVectorFn,
    /// Injected save peer vector function
    save_peer_vector: SavePeerVectorFn,
    /// Sync configuration derived from permit (relationship + sync_target)
    sync_config: Option<SyncConfig>,
    /// Channel to emit sync events (EnsureSync) - consumed by Coordinator
    sync_event_tx: Option<mpsc::Sender<SyncEvent>>,
    /// Callback to list authorized user_dids for this page (node mode)
    list_authorized_users: Option<ListAuthorizedUsersFn>,
    /// Callback to load a user's permit for this page (node mode - permit-based sync auth)
    load_user_permit: Option<LoadUserPermitFn>,
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
}

// =============================================================================
// Scribe Actor
// =============================================================================

/// Scribe - Document sync actor
///
/// One Scribe per open page. Owns layers, handles sync, broadcasts to peers.
pub struct Scribe;

impl Scribe {
    pub fn new() -> Self {
        Self
    }
}

impl Default for Scribe {
    fn default() -> Self {
        Self::new()
    }
}

impl Actor for Scribe {
    type Msg = ScribeMessage;
    type State = ScribeState;
    type Arguments = ScribeArgs;

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        args: Self::Arguments,
    ) -> std::result::Result<Self::State, ActorProcessingErr> {
        let role_info = args.sync_config.as_ref()
            .map(|c| format!("role={}, target={:?}", c.relationship, c.sync_target))
            .unwrap_or_else(|| "no sync config".to_string());
        info!(page_id = %args.page_id, %role_info, "Scribe started");

        Ok(ScribeState {
            page_id: args.page_id,
            layers: args.layers,
            subscribers: HashMap::new(),
            ui_subscribers: Vec::new(),
            query_subscribers: HashMap::new(),
            dirty_layers: HashSet::new(),
            save_layer: args.save_layer,
            load_peer_vector: args.load_peer_vector,
            save_peer_vector: args.save_peer_vector,
            sync_config: args.sync_config,
            sync_event_tx: args.sync_event_tx,
            list_authorized_users: args.list_authorized_users,
            load_user_permit: args.load_user_permit,
        })
    }

    async fn handle(
        &self,
        myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        state: &mut Self::State,
    ) -> std::result::Result<(), ActorProcessingErr> {
        match message {
            ScribeMessage::Subscribe { user_did, device_id, broadcast_tx, permit } => {
                self.handle_subscribe(state, user_did, device_id, broadcast_tx, permit).await;
            }

            ScribeMessage::Unsubscribe { user_did, device_id } => {
                self.handle_unsubscribe(state, &user_did, &device_id).await;
            }

            ScribeMessage::SubscribeUI { tx } => {
                state.ui_subscribers.push(tx);
                debug!(page_id = %state.page_id, "UI subscriber added");
            }

            ScribeMessage::ApplyUpdate { layer_name, update, from_peer } => {
                self.handle_apply_update(state, &layer_name, &update, from_peer).await;
            }

            ScribeMessage::SyncRequest { layer_name, their_vector, reply } => {
                let result = self.handle_sync_request(state, &layer_name, &their_vector);
                let _ = reply.send(result);
            }

            ScribeMessage::ExportSnapshot { layer_name, reply } => {
                let result = self.handle_export_snapshot(state, &layer_name);
                let _ = reply.send(result);
            }

            ScribeMessage::GetStateVector { layer_name, reply } => {
                let result = self.handle_get_state_vector(state, &layer_name);
                let _ = reply.send(result);
            }

            ScribeMessage::GetUpdatesSince { layer_name, state_vector, reply } => {
                let result = self.handle_get_updates_since(state, &layer_name, &state_vector);
                let _ = reply.send(result);
            }

            ScribeMessage::Flush => {
                self.handle_flush(state).await;
            }

            ScribeMessage::GetSnapshot { layer_name, reply } => {
                let result = state.layers.get(&layer_name)
                    .map(|layer| layer.export_snapshot());
                let _ = reply.send(result);
            }

            ScribeMessage::GetLayerJson { layer_name, reply } => {
                let result = state.layers.get(&layer_name)
                    .map(|layer| layer.to_json());
                let _ = reply.send(result);
            }

            ScribeMessage::GetContext { reply } => {
                let result = self.build_context_json(state);
                let _ = reply.send(result);
            }

            ScribeMessage::Query { spec, reply } => {
                let result = self.handle_query(state, &spec);
                let _ = reply.send(result);
            }

            ScribeMessage::SubscribeQuery { spec, delta_tx } => {
                self.handle_subscribe_query(state, spec, delta_tx).await;
            }

            ScribeMessage::UnsubscribeQuery { query_id } => {
                state.query_subscribers.remove(&query_id);
                debug!(page_id = %state.page_id, query_id = %query_id, "Query subscription removed");
            }

            ScribeMessage::UpdateFromJson { layer_name, path, value, reply } => {
                let result = self.handle_update_from_json(state, &layer_name, &path, value).await;
                if let Some(tx) = reply {
                    let _ = tx.send(result);
                }
            }

            ScribeMessage::Shutdown => {
                info!(page_id = %state.page_id, "Scribe shutting down");
                self.handle_flush(state).await;
                myself.stop(Some("shutdown".to_string()));
            }
        }

        Ok(())
    }

    async fn post_stop(
        &self,
        _myself: ActorRef<Self::Msg>,
        state: &mut Self::State,
    ) -> std::result::Result<(), ActorProcessingErr> {
        info!(page_id = %state.page_id, "Scribe stopped");
        Ok(())
    }
}

// =============================================================================
// Message Handlers
// =============================================================================

impl Scribe {
    /// Handle peer subscription
    ///
    /// **Context**: PeerActor subscribes to receive updates for this page
    /// **We do**: Parse permit, extract permissions, load stored vectors
    #[instrument(skip(self, state, broadcast_tx, permit), fields(page_id = %state.page_id))]
    async fn handle_subscribe(
        &self,
        state: &mut ScribeState,
        user_did: String,
        device_id: String,
        broadcast_tx: mpsc::Sender<BroadcastPayload>,
        permit: String,
    ) {
        info!(user_did = %user_did, device_id = %device_id, "Peer subscribing");

        // Parse permit to extract layer permissions and sync policies
        let (layer_permissions, sync_policy) = match parse_permit_for_layers(&permit) {
            Ok(result) => result,
            Err(e) => {
                warn!(error = %e, "Failed to parse permit, using empty permissions");
                (HashMap::new(), SyncPolicy::default())
            }
        };

        // Load stored state vectors for this peer
        let vectors = match (state.load_peer_vector)(&user_did, &device_id) {
            Ok(Some(v)) => v,
            Ok(None) => HashMap::new(),
            Err(e) => {
                warn!(error = %e, "Failed to load peer vectors");
                HashMap::new()
            }
        };

        state.subscribers.insert(
            (user_did.clone(), device_id.clone()),
            SubscriberInfo {
                broadcast_tx: broadcast_tx.clone(),
                vectors,
                layer_permissions: layer_permissions.clone(),
                sync_policy: sync_policy.clone(),
            },
        );

        debug!(subscriber_count = state.subscribers.len(), "Subscriber added");

        // Send initial state to new subscriber for layers they have access to
        self.send_initial_state_to_subscriber(state, &user_did, &device_id, &broadcast_tx, &layer_permissions, &sync_policy).await;
    }

    /// Send initial state to a newly subscribed peer
    ///
    /// **Context**: Peer just subscribed, may have missed previous updates
    /// **We do**: Send current snapshot for each layer they have permission for
    async fn send_initial_state_to_subscriber(
        &self,
        state: &ScribeState,
        user_did: &str,
        device_id: &str,
        broadcast_tx: &mpsc::Sender<BroadcastPayload>,
        layer_permissions: &HashMap<String, LayerCapability>,
        sync_policy: &SyncPolicy,
    ) {
        // Get peer's stored vectors (if any)
        let peer_vectors = match (state.load_peer_vector)(user_did, device_id) {
            Ok(Some(v)) => v,
            Ok(None) => HashMap::new(),
            Err(_) => HashMap::new(),
        };

        for (layer_name, layer) in &state.layers {
            // Skip layers they can't receive
            if !layer_permissions.contains_key(layer_name) {
                continue;
            }
            if sync_policy.no_incoming_updates.contains(layer_name) {
                continue;
            }

            // Determine what to send: incremental or full snapshot
            let data = if let Some(their_vector) = peer_vectors.get(layer_name) {
                if their_vector.is_empty() {
                    layer.export_snapshot()
                } else {
                    match layer.export_updates(their_vector) {
                        Ok(updates) if updates.is_empty() => {
                            // No new updates since their last sync
                            debug!(user_did = %user_did, layer = %layer_name, "No new updates for subscriber");
                            continue;
                        }
                        Ok(updates) => updates,
                        Err(e) => {
                            warn!(error = %e, layer = %layer_name, "Failed to export incremental, using snapshot");
                            layer.export_snapshot()
                        }
                    }
                }
            } else {
                // No stored vector - send full snapshot
                layer.export_snapshot()
            };

            // Skip empty data
            if data.is_empty() {
                continue;
            }

            // Get our current state vector for 3-step sync protocol
            let state_vector = layer.version_vector();

            let payload = BroadcastPayload {
                page_id: state.page_id.clone(),
                layer_name: layer_name.clone(),
                update: data,
                state_vector,
            };

            if let Err(e) = broadcast_tx.try_send(payload) {
                warn!(user_did = %user_did, layer = %layer_name, error = %e, "Failed to send initial state");
            } else {
                debug!(user_did = %user_did, layer = %layer_name, "Sent initial state to subscriber");
            }
        }
    }

    /// Handle peer unsubscription
    #[instrument(skip(self, state), fields(page_id = %state.page_id))]
    async fn handle_unsubscribe(
        &self,
        state: &mut ScribeState,
        user_did: &str,
        device_id: &str,
    ) {
        info!(user_did = %user_did, device_id = %device_id, "Peer unsubscribing");

        // Save their vectors before removing
        if let Some(info) = state.subscribers.get(&(user_did.to_string(), device_id.to_string())) {
            if let Err(e) = (state.save_peer_vector)(user_did, device_id, &info.vectors) {
                warn!(error = %e, "Failed to save peer vectors on unsubscribe");
            }
        }

        state.subscribers.remove(&(user_did.to_string(), device_id.to_string()));
        debug!(subscriber_count = state.subscribers.len(), "Subscriber removed");
    }

    /// Handle layer update (local or remote)
    ///
    /// **Context**: Edit from UI or sync push from peer
    /// **We do**: Permission check, CRDT merge, broadcast to subscribers
    #[instrument(skip(self, state, update), fields(page_id = %state.page_id, layer = %layer_name))]
    async fn handle_apply_update(
        &self,
        state: &mut ScribeState,
        layer_name: &str,
        update: &[u8],
        from_peer: Option<(String, String)>,
    ) {
        // Skip local_only layers for remote updates
        if from_peer.is_some() && self.is_local_only(state, layer_name) {
            warn!("Rejected update for local_only layer from remote peer");
            return;
        }

        // Permission check for remote updates
        if let Some(ref peer) = from_peer {
            if !self.can_write(state, peer, layer_name) {
                warn!(user_did = %peer.0, "Permission denied: cannot write to layer");
                return;
            }
            // Note: submitter namespace validation is client-side (trusted)
        }

        // Get layer and apply CRDT merge
        let Some(layer) = state.layers.get(layer_name) else {
            warn!("Layer not found");
            return;
        };

        if let Err(e) = layer.apply(update) {
            error!(error = %e, "Failed to apply update to layer");
            return;
        }

        state.dirty_layers.insert(layer_name.to_string());
        debug!("Update applied successfully to layer {}", layer_name);

        // Update sender's vector to reflect what they've sent us
        if let Some(ref peer) = from_peer {
            if let Some(info) = state.subscribers.get_mut(peer) {
                let current_vector = layer.version_vector();
                info.vectors.insert(layer_name.to_string(), current_vector);
                debug!(user_did = %peer.0, "Updated sender's peer vector");
            }
        }

        // Broadcast to subscribers
        self.broadcast_update(state, layer_name, from_peer.as_ref()).await;

        // Notify UI subscribers
        self.notify_ui(state, layer_name);

        // Notify query subscribers
        self.notify_query_subscribers(state, layer_name).await;

        // Flush to storage immediately
        self.handle_flush(state).await;
    }

    /// Broadcast layer update to all eligible subscribers
    ///
    /// **Context**: Layer has been updated, notify all subscribers
    /// **We do**: Send incremental or full snapshot to each subscriber
    /// **Cleanup**: Remove subscribers whose channels are closed (disconnected)
    async fn broadcast_update(
        &self,
        state: &mut ScribeState,
        layer_name: &str,
        from_peer: Option<&(String, String)>,
    ) {
        let Some(layer) = state.layers.get(layer_name) else {
            return;
        };

        // Collect version vector once
        let current_vector = layer.version_vector();

        // Track subscribers to remove (channel closed = peer disconnected)
        let mut to_remove: Vec<(String, String)> = Vec::new();

        for ((user_did, device_id), info) in &mut state.subscribers {
            // Skip sender
            if from_peer == Some(&(user_did.clone(), device_id.clone())) {
                continue;
            }

            // Check can receive (inline to avoid borrow issues)
            let can_receive = info.layer_permissions.contains_key(layer_name)
                && !info.sync_policy.no_incoming_updates.contains(layer_name);

            if !can_receive {
                continue;
            }

            // Determine what to send: full snapshot or incremental
            let export = if info.sync_policy.send_full_snapshot.contains(layer_name) {
                layer.export_snapshot()
            } else {
                // Incremental: export since their last known vector
                let their_vector = info.vectors.get(layer_name).cloned().unwrap_or_default();
                if their_vector.is_empty() {
                    layer.export_snapshot()
                } else {
                    match layer.export_updates(&their_vector) {
                        Ok(updates) => updates,
                        Err(e) => {
                            warn!(error = %e, "Failed to export incremental, falling back to snapshot");
                            layer.export_snapshot()
                        }
                    }
                }
            };

            // Send via broadcast channel with our state vector for 3-step sync
            let payload = BroadcastPayload {
                page_id: state.page_id.clone(),
                layer_name: layer_name.to_string(),
                update: export,
                state_vector: current_vector.clone(),
            };

            match info.broadcast_tx.try_send(payload) {
                Ok(()) => {
                    // Update their vector on successful send
                    info.vectors.insert(layer_name.to_string(), current_vector.clone());
                }
                Err(mpsc::error::TrySendError::Closed(_)) => {
                    // Channel closed = peer disconnected, mark for removal
                    info!(user_did = %user_did, device_id = %device_id, "Subscriber disconnected, removing");
                    to_remove.push((user_did.clone(), device_id.clone()));
                }
                Err(mpsc::error::TrySendError::Full(_)) => {
                    // Channel full - log warning but don't remove (temporary backpressure)
                    warn!(user_did = %user_did, "Broadcast channel full, update dropped");
                }
            }
        }

        // Remove disconnected subscribers
        for key in to_remove {
            if let Some(info) = state.subscribers.remove(&key) {
                // Save their final vectors before removal
                if let Err(e) = (state.save_peer_vector)(&key.0, &key.1, &info.vectors) {
                    warn!(error = %e, user_did = %key.0, device_id = %key.1, "Failed to save peer vector on disconnect");
                }
                info!(user_did = %key.0, device_id = %key.1, "Cleaned up disconnected subscriber");
            }
        }

        // Emit EnsureSync for any sync targets not currently subscribed
        self.emit_sync_events_for_missing_targets(state);
    }

    /// Notify UI subscribers of layer change
    fn notify_ui(&self, state: &ScribeState, layer_name: &str) {
        let event = ScribeEvent::LayerUpdated {
            layer_name: layer_name.to_string(),
        };

        for tx in &state.ui_subscribers {
            let _ = tx.try_send(event.clone());
        }
    }

    /// Handle sync request - export updates since their version
    fn handle_sync_request(
        &self,
        state: &ScribeState,
        layer_name: &str,
        their_vector: &[u8],
    ) -> Result<Vec<u8>> {
        let layer = state.layers.get(layer_name)
            .ok_or_else(|| ButlerError::NotFound(format!("Layer {} not found", layer_name)))?;

        layer.export_updates(their_vector)
            .map_err(|e| ButlerError::Layer(e.to_string()))
    }

    /// Handle export snapshot request
    fn handle_export_snapshot(
        &self,
        state: &ScribeState,
        layer_name: &str,
    ) -> Result<Vec<u8>> {
        let layer = state.layers.get(layer_name)
            .ok_or_else(|| ButlerError::NotFound(format!("Layer {} not found", layer_name)))?;

        Ok(layer.export_snapshot())
    }

    /// Handle get state vector request
    fn handle_get_state_vector(
        &self,
        state: &ScribeState,
        layer_name: &str,
    ) -> Result<Vec<u8>> {
        let layer = state.layers.get(layer_name)
            .ok_or_else(|| ButlerError::NotFound(format!("Layer {} not found", layer_name)))?;

        Ok(layer.version_vector())
    }

    /// Handle get updates since request (for resync after divergence)
    fn handle_get_updates_since(
        &self,
        state: &ScribeState,
        layer_name: &str,
        their_vector: &[u8],
    ) -> Result<Vec<u8>> {
        let layer = state.layers.get(layer_name)
            .ok_or_else(|| ButlerError::NotFound(format!("Layer {} not found", layer_name)))?;

        layer.export_updates(their_vector)
            .map_err(|e| ButlerError::Layer(e.to_string()))
    }

    /// Handle flush - save only dirty layers
    async fn handle_flush(&self, state: &mut ScribeState) {
        if state.dirty_layers.is_empty() {
            return;
        }

        debug!(page_id = %state.page_id, "Flushing layers: {:?}", state.dirty_layers);

        for layer_name in state.dirty_layers.drain() {
            if let Some(layer) = state.layers.get(&layer_name) {
                let snapshot = layer.export_snapshot();
                if let Err(e) = (state.save_layer)(&layer_name, &snapshot) {
                    error!(layer = %layer_name, error = %e, "Failed to save layer");
                }
            }
        }

        // Save all peer vectors
        for ((user_did, device_id), info) in &state.subscribers {
            if let Err(e) = (state.save_peer_vector)(user_did, device_id, &info.vectors) {
                warn!(user_did = %user_did, error = %e, "Failed to save peer vectors");
            }
        }
    }

    /// Build a JSON context from all layers for egui rendering
    ///
    /// **Context**: egui window needs all layer data for CEL evaluation
    /// **We do**: Export each layer's JSON and merge into a single object
    fn build_context_json(&self, state: &ScribeState) -> serde_json::Value {
        let mut context = serde_json::Map::new();

        for (layer_name, layer) in &state.layers {
            let layer_json = layer.to_json();

            // If layer JSON is an object, flatten its fields into context
            // Otherwise, store under layer name
            if let serde_json::Value::Object(map) = layer_json {
                for (key, value) in map {
                    context.insert(key, value);
                }
            } else {
                context.insert(layer_name.clone(), layer_json);
            }
        }

        serde_json::Value::Object(context)
    }
}

// =============================================================================
// Query Handlers
// =============================================================================

impl Scribe {
    /// Handle a query request
    ///
    /// **Context**: HUML renderer wants filtered/sorted/paginated data
    /// **We do**: Execute query against layer, apply filter/sort/limit
    fn handle_query(&self, state: &ScribeState, spec: &QuerySpec) -> Result<QueryResult> {
        // Layer may not exist yet for new pages - return empty result
        let Some(layer) = state.layers.get(&spec.layer_name) else {
            debug!(layer = %spec.layer_name, "Layer not found, returning empty result");
            return Ok(QueryResult {
                query_id: spec.query_id.clone(),
                items: Vec::new(),
                total_count: 0,
                has_more: false,
                version: 0,
            });
        };

        // Get the data at the specified path
        let layer_json = layer.to_json();
        let data = self.get_path_value(&layer_json, &spec.path);

        // Extract array items
        let items = match data {
            Some(serde_json::Value::Array(arr)) => arr,
            Some(other) => vec![other],
            None => Vec::new(),
        };

        // Apply filter, sort, and pagination
        // NOTE: For MVP, we don't have CEL runtime in Scribe yet
        // Full CEL filtering will be implemented in QueryBridge (Phase 3)
        let total_count = items.len();
        let filtered_items = items; // TODO: Apply CEL filter

        // Apply sort
        let sorted_items = self.apply_sort(&filtered_items, spec);

        // Apply pagination
        let offset = spec.offset;
        let limit = spec.limit.unwrap_or(usize::MAX);
        let paginated: Vec<serde_json::Value> = sorted_items
            .into_iter()
            .skip(offset)
            .take(limit)
            .collect();

        let has_more = offset + paginated.len() < total_count;

        Ok(QueryResult {
            query_id: spec.query_id.clone(),
            items: paginated,
            total_count,
            has_more,
            version: 0,
        })
    }

    /// Handle query subscription
    ///
    /// **Context**: HUML renderer wants incremental updates
    /// **We do**: Store subscription, send initial result as Reset delta
    async fn handle_subscribe_query(
        &self,
        state: &mut ScribeState,
        spec: QuerySpec,
        delta_tx: mpsc::Sender<QueryDelta>,
    ) {
        let query_id = spec.query_id.clone();
        info!(page_id = %state.page_id, query_id = %query_id, "Query subscription added");

        // Execute initial query
        let initial_result = match self.handle_query(state, &spec) {
            Ok(result) => result,
            Err(e) => {
                warn!(error = %e, query_id = %query_id, "Failed to execute initial query");
                QueryResult::empty(query_id.clone())
            }
        };

        // Send initial result as Reset delta
        let delta = QueryDelta::reset(query_id.clone(), initial_result.clone(), 1);
        if let Err(e) = delta_tx.try_send(delta) {
            warn!(error = %e, query_id = %query_id, "Failed to send initial query result");
        }

        // Store subscription
        state.query_subscribers.insert(
            query_id,
            QuerySubscriberInfo {
                spec,
                delta_tx,
                version: 1,
                last_result: Some(initial_result),
            },
        );
    }

    /// Notify query subscribers when a layer changes
    ///
    /// **Context**: Layer was updated, check if any query results changed
    /// **We do**: Re-execute affected queries, send deltas for changed results
    async fn notify_query_subscribers(&self, state: &mut ScribeState, layer_name: &str) {
        // Find queries that depend on this layer
        let affected_queries: Vec<String> = state.query_subscribers
            .iter()
            .filter(|(_, info)| info.spec.layer_name == layer_name)
            .map(|(id, _)| id.clone())
            .collect();

        for query_id in affected_queries {
            // Clone spec to avoid borrow issues
            let spec = match state.query_subscribers.get(&query_id) {
                Some(info) => info.spec.clone(),
                None => continue,
            };

            // Re-execute query with cloned spec
            let new_result = match self.handle_query(state, &spec) {
                Ok(result) => result,
                Err(e) => {
                    warn!(error = %e, query_id = %query_id, "Failed to re-execute query");
                    continue;
                }
            };

            // Now get mutable reference to update info
            if let Some(info) = state.query_subscribers.get_mut(&query_id) {
                // Increment version
                info.version += 1;

                // For MVP, send full Reset delta
                // TODO: Compute incremental diffs for Insert/Update/Remove
                let delta = QueryDelta::reset(query_id.clone(), new_result.clone(), info.version);

                match info.delta_tx.try_send(delta) {
                    Ok(()) => {
                        info.last_result = Some(new_result);
                    }
                    Err(mpsc::error::TrySendError::Closed(_)) => {
                        // Subscriber disconnected, will be cleaned up
                        debug!(query_id = %query_id, "Query subscriber disconnected");
                    }
                    Err(mpsc::error::TrySendError::Full(_)) => {
                        warn!(query_id = %query_id, "Query delta channel full");
                    }
                }
            }
        }

        // Clean up disconnected subscribers
        state.query_subscribers.retain(|id, info| {
            if info.delta_tx.is_closed() {
                debug!(query_id = %id, "Removing disconnected query subscriber");
                false
            } else {
                true
            }
        });
    }

    /// Get value at a JSON path (simple dot notation)
    ///
    /// **Example**: "messages" → data["root"]["messages"] (Loro stores under "root" container)
    /// **Example**: "users.active" → data["root"]["users"]["active"]
    fn get_path_value<'a>(&self, data: &'a serde_json::Value, path: &str) -> Option<serde_json::Value> {
        if path.is_empty() {
            return Some(data.clone());
        }

        // Loro layers store data under a "root" container, so prepend "root" to the path
        let full_path = format!("root.{}", path);
        let parts: Vec<&str> = full_path.split('.').collect();
        let mut current = data;

        for part in parts {
            match current {
                serde_json::Value::Object(map) => {
                    current = map.get(part)?;
                }
                serde_json::Value::Array(arr) => {
                    // If path is a number, index into array
                    if let Ok(idx) = part.parse::<usize>() {
                        current = arr.get(idx)?;
                    } else {
                        return None;
                    }
                }
                _ => return None,
            }
        }

        Some(current.clone())
    }

    /// Apply sorting to items
    ///
    /// **Note**: Basic implementation - sorts by string comparison
    /// Full sorting with type awareness will be in QueryBridge
    fn apply_sort(&self, items: &[serde_json::Value], spec: &QuerySpec) -> Vec<serde_json::Value> {
        let Some(sort_by) = &spec.sort_by else {
            return items.to_vec();
        };

        let mut sorted = items.to_vec();
        let desc = matches!(spec.sort_order, Some(crate::models::query::SortOrder::Desc));

        sorted.sort_by(|a, b| {
            let a_val = a.get(sort_by);
            let b_val = b.get(sort_by);

            let cmp = match (a_val, b_val) {
                (Some(serde_json::Value::Number(a)), Some(serde_json::Value::Number(b))) => {
                    a.as_f64().unwrap_or(0.0).partial_cmp(&b.as_f64().unwrap_or(0.0))
                        .unwrap_or(std::cmp::Ordering::Equal)
                }
                (Some(serde_json::Value::String(a)), Some(serde_json::Value::String(b))) => {
                    a.cmp(b)
                }
                (Some(a), Some(b)) => {
                    a.to_string().cmp(&b.to_string())
                }
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            };

            if desc { cmp.reverse() } else { cmp }
        });

        sorted
    }

    /// Handle JSON update from UI (CEL commit())
    ///
    /// **Context**: HUML renderer's CEL `commit()` returns JSON values to persist.
    /// **We do**: Convert JSON to Loro, update layer, notify subscribers, persist.
    #[instrument(skip(self, state, value), fields(page_id = %state.page_id, layer = %layer_name, path = %path))]
    async fn handle_update_from_json(
        &self,
        state: &mut ScribeState,
        layer_name: &str,
        path: &str,
        value: serde_json::Value,
    ) -> Result<()> {
        info!("Updating layer from JSON");

        // Get or create the layer
        let layer = state.layers.entry(layer_name.to_string())
            .or_insert_with(Layer::new);

        // Convert JSON to Loro and update
        layer.set_from_json(path, &value)
            .map_err(|e| ButlerError::Layer(e.to_string()))?;

        debug!("JSON value applied to layer");

        // Mark as dirty for persistence
        state.dirty_layers.insert(layer_name.to_string());

        // Broadcast to peer subscribers (as binary update)
        // The layer already has the update, so we export and broadcast
        self.broadcast_update(state, layer_name, None).await;

        // Notify UI subscribers
        self.notify_ui(state, layer_name);

        // Notify query subscribers (for reactive updates)
        self.notify_query_subscribers(state, layer_name).await;

        // Flush to storage immediately
        self.handle_flush(state).await;

        info!("Layer update from JSON complete");
        Ok(())
    }
}

// =============================================================================
// Permission Checking
// =============================================================================

impl Scribe {
    /// Check if layer is local_only (never synced)
    fn is_local_only(&self, state: &ScribeState, layer_name: &str) -> bool {
        // Check all subscribers' policies - if ANY marks it local_only, treat as such
        // Actually, local_only is typically a page-level setting, not per-peer
        // For simplicity, we'll check the layer name
        layer_name == "user_content_doc"
    }

    /// Check if peer can write to layer
    ///
    /// **Context**: Remote peer is trying to write to this layer
    /// **We check**:
    /// 1. Peer is a subscriber with collaborator/submitter permission
    /// 2. OR peer has a stored permit with write access (node mode - permit-based sync auth)
    fn can_write(&self, state: &ScribeState, peer: &(String, String), layer_name: &str) -> bool {
        // Check if peer is a subscriber with write permission
        let subscriber_can_write = state.subscribers.get(peer)
            .and_then(|info| info.layer_permissions.get(layer_name))
            .map(|cap| matches!(cap, LayerCapability::Collaborator | LayerCapability::Submitter))
            .unwrap_or(false);

        if subscriber_can_write {
            return true;
        }

        // Node mode: check stored permit for write access (permit-based auth)
        if let Some(ref load_fn) = state.load_user_permit {
            if let Some(permit) = load_fn(&state.page_id, &peer.0) {
                // Parse permit to extract layer permissions
                if let Ok((layer_permissions, _)) = parse_permit_for_layers(&permit) {
                    let has_write = layer_permissions.get(layer_name)
                        .map(|cap| matches!(cap, LayerCapability::Collaborator | LayerCapability::Submitter))
                        .unwrap_or(false);
                    if has_write {
                        debug!(user_did = %peer.0, layer = %layer_name, "Write allowed via stored permit");
                        return true;
                    }
                }
            }
        }

        false
    }

    /// Check if peer can receive updates for layer
    fn can_receive(&self, state: &ScribeState, peer: &(String, String), layer_name: &str) -> bool {
        state.subscribers.get(peer)
            .map(|info| {
                // Check capability exists AND not in no_incoming_updates
                info.layer_permissions.contains_key(layer_name)
                    && !info.sync_policy.no_incoming_updates.contains(layer_name)
            })
            .unwrap_or(false)
    }
}

// =============================================================================
// Sync Target Resolution
// =============================================================================

impl Scribe {
    /// Get user_dids that should receive updates for this page.
    ///
    /// **Context**: Called during broadcast to find unsubscribed targets.
    /// **Returns**: Vec<String> of user_dids - Coordinator handles device resolution.
    fn get_sync_targets(&self, state: &ScribeState) -> Vec<String> {
        match state.sync_config.as_ref().map(|c| c.relationship.as_str()) {
            Some("owner") | Some("viewer") => {
                // User mode: single target (the node's DID)
                state.sync_config.as_ref()
                    .and_then(|c| c.sync_target.as_ref())
                    .map(|node_did| vec![node_did.clone()])
                    .unwrap_or_default()
            }
            Some("node") | _ => {
                // Node mode: query storage for authorized user_dids
                state.list_authorized_users
                    .as_ref()
                    .map(|f| f(&state.page_id))
                    .unwrap_or_default()
            }
        }
    }

    /// Check if a user is currently subscribed (by any device).
    ///
    /// **Context**: Called during broadcast to filter already-subscribed users.
    fn is_user_subscribed(&self, state: &ScribeState, user_did: &str) -> bool {
        state.subscribers.keys().any(|(did, _device)| did == user_did)
    }

    /// Emit EnsureSync for users that should be synced but aren't subscribed.
    ///
    /// **Context**: Called after broadcasting to current subscribers.
    /// **We do**: Get sync targets, filter out already-subscribed, emit EnsureSync.
    fn emit_sync_events_for_missing_targets(&self, state: &ScribeState) {
        let sync_targets = self.get_sync_targets(state);

        for user_did in sync_targets {
            if self.is_user_subscribed(state, &user_did) {
                // Already subscribed, broadcast handled normally
                continue;
            }

            // User not subscribed - emit EnsureSync
            // Coordinator will handle: device resolution, connection, subscription
            if let Some(ref sync_event_tx) = state.sync_event_tx {
                let event = SyncEvent::EnsureSync { user_did: user_did.clone() };

                if let Err(e) = sync_event_tx.try_send(event) {
                    warn!(user_did = %user_did, error = %e, "Failed to emit EnsureSync");
                } else {
                    debug!(user_did = %user_did, "Emitted EnsureSync for unsubscribed target");
                }
            }
        }
    }
}

// =============================================================================
// Permit Parsing
// =============================================================================

/// Parse permit to extract layer permissions and sync policies
///
/// **Context**: PeerActor has already validated the permit. We just extract facts.
fn parse_permit_for_layers(permit: &str) -> Result<(HashMap<String, LayerCapability>, SyncPolicy)> {
    let parsed = gurkha::Permit::from_token(permit)
        .map_err(|e| ButlerError::PermitError(format!("Failed to parse permit: {:?}", e)))?;

    let mut layer_permissions = HashMap::new();
    let mut sync_policy = SyncPolicy::default();

    // Extract layers fact
    if let Some(layers_value) = parsed.get_fact("layers") {
        if let Some(layers_obj) = layers_value.as_object() {
            for (layer_name, config) in layers_obj {
                if let Some(cap_str) = config.get("capability").and_then(|v| v.as_str()) {
                    let capability = match cap_str {
                        "viewer" => LayerCapability::Viewer,
                        "submitter" => LayerCapability::Submitter,
                        "collaborator" => LayerCapability::Collaborator,
                        _ => LayerCapability::Viewer,
                    };
                    layer_permissions.insert(layer_name.clone(), capability);
                }
            }
        }
    }

    // Extract sync fact
    if let Some(sync_value) = parsed.get_fact("sync") {
        if let Some(sync_obj) = sync_value.as_object() {
            if let Some(local_only) = sync_obj.get("local_only").and_then(|v| v.as_array()) {
                for item in local_only {
                    if let Some(s) = item.as_str() {
                        sync_policy.local_only.insert(s.to_string());
                    }
                }
            }
            if let Some(no_incoming) = sync_obj.get("no_incoming_updates").and_then(|v| v.as_array()) {
                for item in no_incoming {
                    if let Some(s) = item.as_str() {
                        sync_policy.no_incoming_updates.insert(s.to_string());
                    }
                }
            }
            if let Some(full_snap) = sync_obj.get("send_full_snapshot").and_then(|v| v.as_array()) {
                for item in full_snap {
                    if let Some(s) = item.as_str() {
                        sync_policy.send_full_snapshot.insert(s.to_string());
                    }
                }
            }
        }
    }

    Ok((layer_permissions, sync_policy))
}
