//! Apply update handling for Scribe actor
//!
//! Handles incoming layer updates (local or remote), including permission checks,
//! validation, and CRDT merge.
//!
//! Note: Derivation is now handled externally by kunki/LuaRuntime which
//! subscribes to PageUpdate::LayerChanged events.
//!
//! ## Key Types
//!
//! - `UpdateContext`: Computed context for an incoming update - all decisions made upfront
//! - `ApplyOutcome`: Result of applying an update (Applied or Rejected)

use logging_utils::ShortLayer;
use tracing::{debug, error, info, instrument, warn};

use domains::Layer;

use super::broadcast::{broadcast_update, notify_layer_discovered};
use super::sync_meta;
use crate::layer_unit::LayerUnit;
use crate::loro_observer::{
    clear_pending_update_source, set_pending_update_source, setup_layer_observer,
};
use crate::message::SyncEvent;
use crate::permit::Permissions;
use crate::state::normalize_layer_name;
use crate::state::ScribeState;
use crate::JsonOp;

// Update Context - Centralized Decision Logic

/// Context for an incoming update - computed once, used throughout
///
/// **Design**: All decisions about how to handle an update are computed upfront.
/// This makes the apply flow clearer and easier to test.
#[derive(Debug)]
pub struct UpdateContext {
    /// Layer being updated
    pub layer_name: String,
    /// Source of update: None = local, Some = remote peer (user_did, device_id)
    pub from_peer: Option<(String, String)>,
    /// Role of the peer (for validation) - "node", "owner", "viewer", etc.
    pub peer_role: String,

    /// Is this a remote update (from a peer)?
    pub is_remote: bool,
    /// Is this layer local-only (never synced)?
    pub is_local_only: bool,
    /// Does this update need Lua validation?
    pub should_validate: bool,
    /// Should we broadcast after applying?
    pub should_broadcast: bool,
    /// Is this a new layer (needs creation)?
    pub is_new_layer: bool,
}

impl UpdateContext {
    /// Create context for an incoming update
    ///
    /// **Context**: Computes all decisions based on state and update metadata
    /// **Note**: Permission checks are done separately (may reject before context is useful)
    pub fn new(state: &mut ScribeState, layer_name: &str, from_peer: Option<(String, String)>) -> Self {
        let is_remote = from_peer.is_some();
        let is_local_only = Permissions::is_local_only(state, layer_name);
        let is_new_layer = !state.units.contains_key(layer_name);

        // Determine peer role for validation
        let peer_role = if let Some(ref peer) = from_peer {
            get_peer_role(state, &peer.0)
        } else {
            "local".to_string()
        };

        // Should validate: remote update AND ValidationHandle exists
        let should_validate = is_remote && state.validation_handle.is_some();

        // Should broadcast: remote update (local updates use observer)
        let should_broadcast = is_remote;

        Self {
            layer_name: layer_name.to_string(),
            from_peer,
            peer_role,
            is_remote,
            is_local_only,
            should_validate,
            should_broadcast,
            is_new_layer,
        }
    }
}

// Apply Outcome - Event-Driven Pattern

/// Outcome of applying an update
///
/// **Design**: Separates the "what happened" from "what to do next"
/// This enables cleaner outcome handling and better error messages
#[derive(Debug)]
pub enum ApplyOutcome {
    /// Update was successfully applied
    Applied {
        /// Layer that was updated
        layer_name: String,
        /// Ops extracted from update (for derivation)
        ops: Vec<JsonOp>,
        /// Was a new layer created?
        created_layer: bool,
    },
    /// Update was rejected
    Rejected {
        /// Reason for rejection
        reason: String,
    },
}

// Apply Update Handler

/// Handle layer update (local or remote)
///
/// **Context**: Edit from UI or sync push from peer
/// **We do**: Permission check, validation, CRDT merge
/// **Broadcast**: Handled automatically by Loro observer (set up at startup)
/// **Returns**: Ok(()) on success, Err(message) on failure
#[instrument(skip(state, update, permit), fields(page_id = %state.page_id, layer = %ShortLayer(layer_name)))]
pub async fn handle_apply_update(
    state: &mut ScribeState,
    layer_name: &str,
    update: &[u8],
    from_peer: Option<(String, String)>,
    permit: Option<&str>,
) -> std::result::Result<(), String> {
    let _ = permit; // Permit is used for subscription in Courier, not here

    // Defense-in-depth: normalize layer name (strip page_id/ prefix)
    let layer_name = normalize_layer_name(layer_name, &state.page_id);
    let layer_name = layer_name.as_str();

    // Build context - computes all decisions upfront
    let ctx = UpdateContext::new(state, layer_name, from_peer.clone());

    // Step 1: Apply the update (with permission checks, validation, CRDT merge)
    let outcome = apply_update_core(state, &ctx, update).await;

    // Step 2: Handle the outcome
    let from_peer_did = ctx.from_peer.as_ref().map(|(d, _)| d.as_str());
    match outcome {
        ApplyOutcome::Applied {
            layer_name,
            ops: _,
            created_layer,
        } => {
            // Mark layer as dirty for persistence
            if let Some(unit) = state.units.get_mut(&layer_name) {
                unit.mark_dirty();
            }
            debug!(layer = %layer_name, created = created_layer, "Update applied successfully");
            state.emit_apply_update_capture(&layer_name, from_peer_did, "applied", None);

            // Post-apply handling for remote updates
            if ctx.is_remote {
                handle_post_apply(state, &layer_name, ctx.from_peer.clone()).await;
            }

            // Ensure sender is a per-layer subscriber on this LayerUnit.
            // This handles the case where the LayerUnit was created (e.g. via
            // LayerSubscribeAck) before the sender's data arrived. Without this,
            // the sender would never receive broadcasts for this layer.
            ensure_sender_subscribed(state, &layer_name, ctx.from_peer.as_ref());

            // Node-side: detect new dynamic layers in creator's __sync_meta
            // Instead of doing fanout directly, emit SyncEvent::SubscribeLayers
            // so the node sends LayerSubscribe to the creator to get authority.
            if ctx.is_remote {
                if sync_meta::is_sync_meta_layer(&layer_name) {
                    if let Some(creator_did) = sync_meta::extract_peer_did(&layer_name) {
                        let creator_did = creator_did.to_string();
                        let new_layers = sync_meta::detect_new_dynamic_layers(state, &creator_did);
                        if !new_layers.is_empty() {
                            if let Some(ref tx) = state.sync_event_tx {
                                let _ = tx.try_send(SyncEvent::SubscribeLayers {
                                    page_id: state.page_id.clone(),
                                    creator_did: creator_did.to_string(),
                                    layers: new_layers,
                                });
                            }
                        }
                    }
                }
            }

            // Update sender's vector
            if let Some(ref peer) = ctx.from_peer {
                update_sender_vector(state, peer, &layer_name);
            }

            // Broadcast to eligible subscribers
            if ctx.should_broadcast {
                broadcast_update(state, &layer_name, ctx.from_peer.as_ref()).await;
            }

            // Note: Derivation is now handled externally by kunki/LuaRuntime
            // which subscribes to PageUpdate::LayerChanged events

            Ok(())
        }
        ApplyOutcome::Rejected { reason } => {
            warn!(layer = %layer_name, reason = %reason, "Update rejected");
            state.emit_apply_update_capture(layer_name, from_peer_did, "rejected", Some(&reason));
            Err(reason)
        }
    }
}

/// Core apply logic - returns ApplyOutcome
///
/// **Context**: Separated from handle_apply_update for cleaner testing
/// **Returns**: ApplyOutcome::Applied or ApplyOutcome::Rejected
#[instrument(skip_all, fields(page_id = %state.page_id, layer = %ctx.layer_name))]
async fn apply_update_core(
    state: &mut ScribeState,
    ctx: &UpdateContext,
    update: &[u8],
) -> ApplyOutcome {
    // Step 1: Permission check - reject local_only layers from remote
    if ctx.is_remote && ctx.is_local_only {
        return ApplyOutcome::Rejected {
            reason: "local_only layer cannot be updated by remote peer".to_string(),
        };
    }

    // Step 2: Permission check for remote updates
    if let Some(ref peer) = ctx.from_peer {
        if !Permissions::can_write(state, peer, &ctx.layer_name) {
            return ApplyOutcome::Rejected {
                reason: "Permission denied: cannot write to layer".to_string(),
            };
        }
    }

    // Step 3: Extract ops for validation (using optimized pre-commit method)
    let extracted_ops = if ctx.should_validate {
        // Use Layer::extract_ops_from_bytes (5-10x faster than old fork+diff)
        match Layer::extract_ops_from_bytes(update) {
            Ok(ops) => Some(ops),
            Err(e) => {
                // Log extraction failure - validation will proceed with empty ops
                // This could mask invalid updates, so log at warn level
                warn!(layer = %ctx.layer_name, error = %e, "Failed to extract ops from update, proceeding with empty ops");
                None
            }
        }
    } else {
        None
    };

    // Step 4: Validate with Lua (if needed)
    if ctx.should_validate {
        if let Err(e) = validate_update_with_context(state, ctx, &extracted_ops).await {
            return ApplyOutcome::Rejected { reason: e };
        }
    }

    // Step 5: Create layer if needed (remote only)
    let created_layer = if ctx.is_new_layer {
        if ctx.is_remote {
            create_layer_from_peer(state, &ctx.layer_name);
            true
        } else {
            return ApplyOutcome::Rejected {
                reason: "Layer not found".to_string(),
            };
        }
    } else {
        false
    };

    // Step 6: Apply CRDT update
    if let Err(e) = apply_crdt_update(state, &ctx.layer_name, update, ctx.from_peer.clone()) {
        return ApplyOutcome::Rejected { reason: e };
    }

    ApplyOutcome::Applied {
        layer_name: ctx.layer_name.clone(),
        ops: extracted_ops.unwrap_or_default(),
        created_layer,
    }
}

// Helper Functions

/// Validate update using ValidationHandle (kunki) or fallback to Lua runtime
///
/// **Context**: Business logic validation for remote updates
/// **Returns**: Ok(()) if validation passed, Err(message) if rejected
/// **Timeout**: 5 seconds for ValidationHandle (kunki), immediate for Lua runtime
#[instrument(skip_all, fields(page_id = %state.page_id, layer = %ctx.layer_name))]
async fn validate_update_with_context(
    state: &ScribeState,
    ctx: &UpdateContext,
    extracted_ops: &Option<Vec<JsonOp>>,
) -> Result<(), String> {
    let Some(ref ops) = extracted_ops else {
        return Ok(()); // No ops to validate
    };

    if ops.is_empty() {
        return Ok(()); // Empty ops = nothing to validate
    }

    // Extract actual DID from from_peer, not the role
    let from_did = ctx
        .from_peer
        .as_ref()
        .map(|(d, _)| d.as_str())
        .unwrap_or("local");

    // Use ValidationHandle (kunki mode with presence_lib)
    let Some(ref validation_handle) = state.validation_handle else {
        // No validation available - allow update (viewer/owner mode without kunki)
        // This is expected in viewer/owner mode but unexpected in node mode without kunki
        warn!(layer = %ctx.layer_name, from_did = %from_did, "No ValidationHandle available, allowing remote update without validation");
        return Ok(());
    };

    use tokio::time::{timeout, Duration};

    let validation_future = validation_handle.validate_ops(
        &state.page_id,
        &ctx.layer_name,
        ops,
        from_did,
        &ctx.peer_role,
    );

    match timeout(Duration::from_secs(5), validation_future).await {
        Ok(Ok((true, _))) => {
            debug!(layer = %ctx.layer_name, from_did = %from_did, role = %ctx.peer_role, "Validation passed");
            Ok(())
        }
        Ok(Ok((false, error_msg))) => {
            let msg = error_msg.unwrap_or_else(|| "Validation failed".to_string());
            warn!(layer = %ctx.layer_name, from_did = %from_did, role = %ctx.peer_role, error = %msg, "Validation rejected update");
            Err(format!("Validation failed: {}", msg))
        }
        Ok(Err(e)) => {
            warn!(layer = %ctx.layer_name, error = %e, "Validation error, rejecting update");
            Err(format!("Validation error: {}", e))
        }
        Err(_) => {
            warn!(layer = %ctx.layer_name, "Validation timeout (5s), rejecting update");
            Err("Validation timeout".to_string())
        }
    }
}

/// Create a new layer from peer sync
///
/// **Context**: Peer sent update for a layer we don't have yet
/// **We do**: Create layer, set up observer, notify UI
#[instrument(skip(state), fields(page_id = %state.page_id))]
fn create_layer_from_peer(state: &mut ScribeState, layer_name: &str) {
    // Defense-in-depth: normalize layer name (strip page_id/ prefix)
    let layer_name = normalize_layer_name(layer_name, &state.page_id);
    let layer_name = layer_name.as_str();
    let mut unit = LayerUnit::new_empty();
    unit.set_local_only(!state.should_sync_layer(layer_name));
    unit.mark_dirty();
    state.units.insert(layer_name.to_string(), unit);
    info!(layer = %layer_name, "Created new layer from peer sync");

    // Add existing subscribers to this new layer
    if let Ok(subs) = state.subscribers.read() {
        for ((did, device_id), info) in subs.iter() {
            if info.can_receive_layer(layer_name, &state.page_id) {
                if let Some(unit) = state.units.get(layer_name) {
                    let can_write = info.can_write_layer(layer_name, &state.page_id);
                    unit.add_subscriber(
                        did.clone(),
                        device_id.clone(),
                        can_write,
                        info.broadcast_tx.clone(),
                    );
                    state.emit_layer_auth_capture(layer_name, did, "subscriber_added_from_peer");
                }
            }
        }
    }

    // Set up Loro observer for new layer to enable sync broadcasts
    setup_layer_observer(state, layer_name);

    // Notify UI subscribers about the new layer
    notify_layer_discovered(state, layer_name);
}

/// Apply CRDT update to layer
///
/// **Context**: Actually merge the update into the layer
/// **We do**: Set pending source, apply, clear pending source
#[instrument(skip(state, update), fields(page_id = %state.page_id, layer = %layer_name))]
fn apply_crdt_update(
    state: &mut ScribeState,
    layer_name: &str,
    update: &[u8],
    from_peer: Option<(String, String)>,
) -> Result<(), String> {
    // Set pending update source BEFORE apply() so observer knows who caused this change
    set_pending_update_source(state, from_peer);

    // Get the layer and apply update
    let result = if let Some(unit) = state.units.get(layer_name) {
        unit.layer().apply(update)
    } else {
        clear_pending_update_source(state);
        return Err("Layer not found".to_string());
    };

    if let Err(e) = result {
        // Clear on error too
        clear_pending_update_source(state);
        error!(error = %e, "Failed to apply update to layer");
        return Err(format!("Failed to apply update: {}", e));
    }

    // Clear pending update source AFTER apply() (observer already captured it)
    clear_pending_update_source(state);

    Ok(())
}

/// Handle post-apply actions for remote updates
///
/// **Context**: Remote update was successfully applied
/// **We do**: Flush app layers immediately
/// **Note**: UI notification is handled by Loro observer (no duplicate emit here)
#[instrument(skip_all, fields(page_id = %state.page_id, layer = %layer_name))]
async fn handle_post_apply(
    state: &mut ScribeState,
    layer_name: &str,
    _from_peer: Option<(String, String)>,
) {
    // For app layers, flush immediately to storage so restart loads new code
    if layer_name.starts_with("app:") {
        if let Some(unit) = state.units.get_mut(layer_name) {
            let snapshot = unit.layer().export_snapshot();
            if let Err(e) = state.layer_storage.save_layer(layer_name, &snapshot) {
                error!(layer = %layer_name, error = %e, "Failed to save app layer immediately");
            } else {
                info!(layer = %layer_name, "Saved app layer immediately for restart");
                // Remove from dirty since we just saved it
                let _ = unit.take_dirty_snapshot();
            }
        }
    }

    // Note: UI notification via emit_layer_changed_after_apply was removed
    // The Loro observer handles PageUpdate::LayerChanged for all changes (local and remote)
    // This prevents duplicate notifications
}

/// Update sender's peer vector after receiving their update
#[instrument(skip_all, fields(page_id = %state.page_id, layer = %layer_name))]
fn update_sender_vector(state: &mut ScribeState, peer: &(String, String), layer_name: &str) {
    if let Some(unit) = state.units.get(layer_name) {
        let current_vector = unit.layer().version_vector();
        unit.update_subscriber_vector(&peer.0, &peer.1, current_vector);
        debug!(user_did = %peer.0, "Updated sender's peer vector");
    }
}

/// Ensure the update sender is a per-layer subscriber on the LayerUnit.
///
/// **Context**: When a LayerUnit is created via LayerSubscribeAck before the
/// creator's data arrives, the creator isn't in the per-layer subscriber list.
/// This adds them if they're a global subscriber with access to this layer.
fn ensure_sender_subscribed(
    state: &ScribeState,
    layer_name: &str,
    from_peer: Option<&(String, String)>,
) {
    let Some(peer) = from_peer else { return };
    let (peer_did, device_id) = peer;

    // Check if already subscribed at the layer level
    if let Some(unit) = state.units.get(layer_name) {
        if unit.can_push_to(peer_did, device_id) {
            return;
        }
    }

    // Look up in global subscribers
    if let Ok(subs) = state.subscribers.read() {
        if let Some(info) = subs.get(peer) {
            if info.can_receive_layer(layer_name, &state.page_id) {
                if let Some(unit) = state.units.get(layer_name) {
                    let can_write = info.can_write_layer(layer_name, &state.page_id);
                    unit.add_subscriber(
                        peer_did.clone(),
                        device_id.clone(),
                        can_write,
                        info.broadcast_tx.clone(),
                    );
                    state.emit_layer_auth_capture(layer_name, peer_did, "sender_added_post_apply");
                }
            }
        }
    }
}

/// Get role for a peer from their stored permit, with per-session caching
///
/// **Context**: Determining role for Lua validation
/// **Checks**:
/// 1. Cache hit → return cached role
/// 2. Sync target (owner/viewer mode) → our role's counterpart
/// 3. Stored permit (node mode) → extract role from permit
/// 4. Default → "peer"
/// **Performance**: Without cache, each call does redb read + UCAN parse (12% of node CPU at 5k msgs)
#[instrument(skip(state), fields(page_id = %state.page_id))]
fn get_peer_role(state: &mut ScribeState, peer_did: &str) -> String {
    // Check cache first
    if let Some(cached) = state.peer_role_cache.get(peer_did) {
        return cached.clone();
    }

    let role = get_peer_role_uncached(state, peer_did);
    state.peer_role_cache.insert(peer_did.to_string(), role.clone());
    role
}

fn get_peer_role_uncached(state: &ScribeState, peer_did: &str) -> String {
    // If peer is our sync target, they are the "node" from our perspective
    if let Some(ref config) = state.sync_config {
        if let Some(ref sync_target) = config.sync_target {
            if sync_target == peer_did {
                return "node".to_string();
            }
        }
    }

    // Node mode: look up peer's stored permit and extract role via gurkha::Permit
    if let Some(ref resolver) = state.peer_resolver {
        if let Some(permit_token) = resolver.load_user_permit(peer_did) {
            let role = gurkha::Permit::from_token(&permit_token)
                .map(|permit| permit.relationship().unwrap_or("peer").to_string())
                .unwrap_or_else(|_| "peer".to_string());
            debug!(peer_did = %peer_did, role = %role, "Got peer role from stored permit via gurkha::Permit");
            return role;
        }
    }

    // Default role
    "peer".to_string()
}
