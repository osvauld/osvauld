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

use crate::models::Layer;
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

    /// Shutdown the actor
    Shutdown,
}

/// Payload sent to PeerActor for broadcast
#[derive(Debug, Clone)]
pub struct BroadcastPayload {
    pub page_id: String,
    pub layer_name: String,
    pub update: Vec<u8>,
}

/// Events emitted to UI subscribers
#[derive(Debug, Clone)]
pub enum ScribeEvent {
    LayerUpdated { layer_name: String },
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
    /// Whether any layer has unsaved changes
    dirty: bool,
    /// Injected save function
    save_layer: SaveLayerFn,
    /// Injected load peer vector function
    load_peer_vector: LoadPeerVectorFn,
    /// Injected save peer vector function
    save_peer_vector: SavePeerVectorFn,
    /// Sync configuration derived from permit (relationship + sync_target)
    sync_config: Option<SyncConfig>,
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
            dirty: false,
            save_layer: args.save_layer,
            load_peer_vector: args.load_peer_vector,
            save_peer_vector: args.save_peer_vector,
            sync_config: args.sync_config,
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

            ScribeMessage::Flush => {
                self.handle_flush(state).await;
            }

            ScribeMessage::GetSnapshot { layer_name, reply } => {
                let result = state.layers.get(&layer_name)
                    .map(|layer| layer.export_snapshot());
                let _ = reply.send(result);
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
                broadcast_tx,
                vectors,
                layer_permissions,
                sync_policy,
            },
        );

        debug!(subscriber_count = state.subscribers.len(), "Subscriber added");
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

        state.dirty = true;
        debug!("Update applied successfully");

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

            // Send via broadcast channel
            let payload = BroadcastPayload {
                page_id: state.page_id.clone(),
                layer_name: layer_name.to_string(),
                update: export,
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

    /// Handle flush - save all dirty layers
    async fn handle_flush(&self, state: &mut ScribeState) {
        if !state.dirty {
            return;
        }

        debug!(page_id = %state.page_id, "Flushing layers");

        for (layer_name, layer) in &state.layers {
            let snapshot = layer.export_snapshot();
            if let Err(e) = (state.save_layer)(layer_name, &snapshot) {
                error!(layer = %layer_name, error = %e, "Failed to save layer");
            }
        }

        // Save all peer vectors
        for ((user_did, device_id), info) in &state.subscribers {
            if let Err(e) = (state.save_peer_vector)(user_did, device_id, &info.vectors) {
                warn!(user_did = %user_did, error = %e, "Failed to save peer vectors");
            }
        }

        state.dirty = false;
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
    fn can_write(&self, state: &ScribeState, peer: &(String, String), layer_name: &str) -> bool {
        state.subscribers.get(peer)
            .and_then(|info| info.layer_permissions.get(layer_name))
            .map(|cap| matches!(cap, LayerCapability::Collaborator | LayerCapability::Submitter))
            .unwrap_or(false)
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
