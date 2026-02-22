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

use domains::{Layer, OpKind, Sthithi};

use super::broadcast::{broadcast_update, notify_layer_discovered};
use super::sync_meta;
use crate::layer_unit::LayerUnit;
use crate::loro_observer::{
    clear_pending_update_source, set_pending_update_source, setup_layer_observer,
};
use crate::message::SyncEvent;
use crate::permit::Permissions;
use crate::policy_compat;
use crate::state::normalize_layer_name;
use crate::state::ScribeState;
use crate::Parivarta;

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
    /// Does this update need compiled schema enforcement?
    pub should_enforce_schema: bool,
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
    pub fn new(
        state: &mut ScribeState,
        layer_name: &str,
        from_peer: Option<(String, String)>,
    ) -> Self {
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
        // Schema enforcement runs whenever schema artifact exists.
        let should_enforce_schema = state.schema_artifact.is_some();

        // Should broadcast: remote update (local updates use observer)
        let should_broadcast = is_remote;

        Self {
            layer_name: layer_name.to_string(),
            from_peer,
            peer_role,
            is_remote,
            is_local_only,
            should_validate,
            should_enforce_schema,
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
        ops: Vec<Parivarta>,
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
    let extracted_ops = if ctx.should_validate || ctx.should_enforce_schema {
        // Use canonical Parivarta extraction path
        match Layer::extract_ops(update) {
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

    // Step 4: Enforce compiled schema (required/immutable) before apply
    if ctx.should_enforce_schema {
        if let Err(e) = validate_with_compiled_schema(state, ctx, &extracted_ops) {
            return ApplyOutcome::Rejected { reason: e };
        }
    }

    // Step 5: Validate with Lua (if needed)
    if ctx.should_validate {
        if let Err(e) = validate_update_with_context(state, ctx, &extracted_ops).await {
            return ApplyOutcome::Rejected { reason: e };
        }
    }

    // Step 6: Create layer if needed (remote only)
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

    // Step 7: Apply CRDT update
    if let Err(e) = apply_crdt_update(state, &ctx.layer_name, update, ctx.from_peer.clone()) {
        return ApplyOutcome::Rejected { reason: e };
    }

    // Step 8: Apply entity on-update rules from compiled schema
    if let Err(e) = apply_entity_on_update_rules(state, ctx, &extracted_ops) {
        warn!(layer = %ctx.layer_name, error = %e, "Failed to apply entity on-update rules");
    }

    ApplyOutcome::Applied {
        layer_name: ctx.layer_name.clone(),
        ops: extracted_ops.unwrap_or_default(),
        created_layer,
    }
}

fn validate_with_compiled_schema(
    state: &ScribeState,
    ctx: &UpdateContext,
    extracted_ops: &Option<Vec<Parivarta>>,
) -> Result<(), String> {
    let Some(schema) = state.schema_artifact.as_ref() else {
        return Ok(());
    };
    let Some(ops) = extracted_ops.as_ref() else {
        return Ok(());
    };
    if ops.is_empty() {
        return Ok(());
    }

    let Some(entity_name) = find_bound_entity_name(schema, &ctx.layer_name) else {
        return Ok(());
    };

    let Some(entity_schema) = schema.entities.get(entity_name) else {
        return Ok(());
    };

    for op in ops {
        validate_entity_delete_rules(entity_schema, op)?;
        validate_required_fields(entity_schema, op)?;
        validate_immutable_fields(state, &ctx.layer_name, entity_schema, op)?;
        validate_transition_rules(state, ctx, &ctx.layer_name, entity_schema, op)?;
    }

    Ok(())
}

fn validate_entity_delete_rules(
    entity_schema: &gurkha::EntitySchema,
    op: &Parivarta,
) -> Result<(), String> {
    if !matches!(op.op, OpKind::Delete) {
        return Ok(());
    }

    let reject_delete = entity_schema
        .entity_rules
        .iter()
        .any(|rule| matches!(rule, gurkha::EntityRuleSchema::OnDeleteReject));
    if reject_delete {
        return Err("delete rejected by entity rule".to_string());
    }

    Ok(())
}

fn apply_entity_on_update_rules(
    state: &mut ScribeState,
    ctx: &UpdateContext,
    extracted_ops: &Option<Vec<Parivarta>>,
) -> Result<(), String> {
    let Some(schema) = state.schema_artifact.as_ref() else {
        return Ok(());
    };
    let Some(ops) = extracted_ops.as_ref() else {
        return Ok(());
    };

    let has_update = ops
        .iter()
        .any(|op| matches!(op.op, OpKind::Insert | OpKind::Update | OpKind::Set));
    if !has_update {
        return Ok(());
    }

    let Some(entity_name) = find_bound_entity_name(schema, &ctx.layer_name) else {
        return Ok(());
    };
    let Some(entity_schema) = schema.entities.get(entity_name) else {
        return Ok(());
    };

    let mut updates = Vec::new();
    for rule in &entity_schema.entity_rules {
        if let gurkha::EntityRuleSchema::OnUpdateSet {
            field,
            source,
            literal,
        } = rule
        {
            if let Some(value) = resolve_entity_rule_value(source, literal.as_ref(), state, ctx) {
                updates.push((field.clone(), value));
            }
        }
    }

    if updates.is_empty() {
        return Ok(());
    }

    let Some(layer_unit) = state.units.get(&ctx.layer_name) else {
        return Ok(());
    };
    for (field, value) in updates {
        layer_unit
            .layer()
            .set_from_sthithi(&field, &value)
            .map_err(|e| format!("failed to set entity rule field '{}': {}", field, e))?;
    }

    Ok(())
}

fn resolve_entity_rule_value(
    source: &str,
    literal: Option<&gurkha::LiteralValue>,
    state: &ScribeState,
    ctx: &UpdateContext,
) -> Option<Sthithi> {
    match source {
        "from_clock" | "fromclock" => {
            let millis = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .ok()?
                .as_millis() as i64;
            Some(Sthithi::Int(millis))
        }
        "from_peer" | "frompeer" => {
            let did = if let Some((did, _)) = &ctx.from_peer {
                did.clone()
            } else {
                state.our_did.clone()
            };
            Some(Sthithi::Str(did))
        }
        "from_mode" | "frommode" => {
            let mode = if let Some(sync_config) = &state.sync_config {
                match sync_config.mode {
                    crate::state::SyncMode::ToSource => "to_source",
                    crate::state::SyncMode::Broadcast => "broadcast",
                }
            } else {
                "local"
            };
            Some(Sthithi::Str(mode.to_string()))
        }
        "to_literal" => match literal? {
            value => Some(literal_value_to_sthithi(value)),
        },
        _ => None,
    }
}

fn literal_value_to_sthithi(literal: &gurkha::LiteralValue) -> Sthithi {
    match literal {
        gurkha::LiteralValue::String(v) => Sthithi::Str(v.clone()),
        gurkha::LiteralValue::Int(v) => Sthithi::Int(*v),
        gurkha::LiteralValue::Bool(v) => Sthithi::Bool(*v),
        gurkha::LiteralValue::Null => Sthithi::Null,
    }
}

fn validate_transition_rules(
    state: &ScribeState,
    ctx: &UpdateContext,
    layer_name: &str,
    entity_schema: &gurkha::EntitySchema,
    op: &Parivarta,
) -> Result<(), String> {
    let Some(field_name) = &op.key else {
        return Ok(());
    };

    let has_transition_for_field = entity_schema
        .transitions
        .iter()
        .any(|t| t.field == *field_name);
    if !has_transition_for_field {
        return Ok(());
    }

    let Some(new_state) = op.value.as_ref().and_then(sthithi_as_str) else {
        return Ok(());
    };

    let Some(old_state) = current_field_value(state.units.get(layer_name).map(|u| u.layer()), field_name)
        .as_ref()
        .and_then(sthithi_as_str)
        .map(|s| s.to_string())
    else {
        return Ok(());
    };

    if old_state == new_state {
        return Ok(());
    }

    let actor_role = actor_role_for_update(state, ctx);
    let actor_did = actor_did_for_update(state, ctx);
    let layer = state.units.get(layer_name).map(|u| u.layer());
    if is_transition_allowed(
        entity_schema,
        layer,
        op,
        field_name,
        &old_state,
        new_state,
        &actor_role,
        &actor_did,
    ) {
        return Ok(());
    }

    Err(format!(
        "transition denied for field '{}': '{}' -> '{}' by role '{}'",
        field_name, old_state, new_state, actor_role
    ))
}

fn actor_role_for_update(state: &ScribeState, ctx: &UpdateContext) -> String {
    if ctx.is_remote {
        return ctx.peer_role.clone();
    }

    state
        .our_permit
        .as_ref()
        .and_then(policy_compat::role)
        .unwrap_or_else(|| "local".to_string())
}

fn actor_did_for_update(state: &ScribeState, ctx: &UpdateContext) -> String {
    if let Some((did, _)) = &ctx.from_peer {
        return did.clone();
    }
    state.our_did.clone()
}

fn is_transition_allowed(
    entity_schema: &gurkha::EntitySchema,
    layer: Option<&Layer>,
    op: &Parivarta,
    field_name: &str,
    from_state: &str,
    to_state: &str,
    actor_role: &str,
    actor_did: &str,
) -> bool {
    entity_schema.transitions.iter().any(|t| {
        t.field == field_name
            && t.from == from_state
            && t.to == to_state
            && t.allowed_roles.iter().any(|r| r == actor_role)
            && transition_predicates_match(layer, op, &t.predicates, actor_did)
    })
}

fn transition_predicates_match(
    layer: Option<&Layer>,
    op: &Parivarta,
    predicates: &[gurkha::Predicate],
    actor_did: &str,
) -> bool {
    predicates
        .iter()
        .all(|predicate| evaluate_transition_predicate(layer, op, predicate, actor_did))
}

fn evaluate_transition_predicate(
    layer: Option<&Layer>,
    op: &Parivarta,
    predicate: &gurkha::Predicate,
    actor_did: &str,
) -> bool {
    use gurkha::{Predicate, Ref, Value};

    let resolve_value = |value: &Value| -> Option<Sthithi> {
        match value {
            Value::Str(v) => Some(Sthithi::Str(v.clone())),
            Value::Num(v) => Some(Sthithi::Int(*v)),
            Value::Bool(v) => Some(Sthithi::Bool(*v)),
            Value::Ref(reference) => match reference {
                Ref::ActorDid => Some(Sthithi::Str(actor_did.to_string())),
                Ref::TargetField { field } => {
                    if op.key.as_deref() == Some(field.as_str()) {
                        op.value.clone()
                    } else {
                        current_field_value(layer, field)
                    }
                }
                _ => None,
            },
        }
    };

    let eq = |l: &Sthithi, r: &Sthithi| l == r;
    let gt = |l: &Sthithi, r: &Sthithi| match (l, r) {
        (Sthithi::Int(a), Sthithi::Int(b)) => a > b,
        (Sthithi::Float(a), Sthithi::Float(b)) => a > b,
        (Sthithi::Int(a), Sthithi::Float(b)) => (*a as f64) > *b,
        (Sthithi::Float(a), Sthithi::Int(b)) => *a > (*b as f64),
        _ => false,
    };
    let lt = |l: &Sthithi, r: &Sthithi| match (l, r) {
        (Sthithi::Int(a), Sthithi::Int(b)) => a < b,
        (Sthithi::Float(a), Sthithi::Float(b)) => a < b,
        (Sthithi::Int(a), Sthithi::Float(b)) => (*a as f64) < *b,
        (Sthithi::Float(a), Sthithi::Int(b)) => *a < (*b as f64),
        _ => false,
    };

    match predicate {
        Predicate::Eq { left, right } => match (resolve_value(left), resolve_value(right)) {
            (Some(l), Some(r)) => eq(&l, &r),
            _ => false,
        },
        Predicate::Ne { left, right } => match (resolve_value(left), resolve_value(right)) {
            (Some(l), Some(r)) => !eq(&l, &r),
            _ => false,
        },
        Predicate::Le { left, right } => match (resolve_value(left), resolve_value(right)) {
            (Some(l), Some(r)) => lt(&l, &r) || eq(&l, &r),
            _ => false,
        },
        Predicate::Gt { left, right } => match (resolve_value(left), resolve_value(right)) {
            (Some(l), Some(r)) => gt(&l, &r),
            _ => false,
        },
        Predicate::Ge { left, right } => match (resolve_value(left), resolve_value(right)) {
            (Some(l), Some(r)) => gt(&l, &r) || eq(&l, &r),
            _ => false,
        },
        Predicate::Lt { left, right } => match (resolve_value(left), resolve_value(right)) {
            (Some(l), Some(r)) => lt(&l, &r),
            _ => false,
        },
        Predicate::In { item, set } => {
            let Some(item_val) = resolve_value(item) else {
                return false;
            };
            set.iter()
                .filter_map(resolve_value)
                .any(|candidate| candidate == item_val)
        }
        Predicate::Matches { .. }
        | Predicate::Exists { .. }
        | Predicate::RoleIs { .. } => false,
    }
}

fn sthithi_as_str(value: &Sthithi) -> Option<&str> {
    match value {
        Sthithi::Str(s) => Some(s.as_str()),
        _ => None,
    }
}

fn find_bound_entity_name<'a>(
    schema: &'a gurkha::SchemaArtifact,
    layer_name: &str,
) -> Option<&'a String> {
    schema
        .layer_bindings
        .iter()
        .find_map(|(template, entity)| path_matches_template(template, layer_name).then_some(entity))
}

fn path_matches_template(template: &str, layer_name: &str) -> bool {
    let t_parts: Vec<&str> = template.split('/').collect();
    let l_parts: Vec<&str> = layer_name.split('/').collect();
    if t_parts.len() != l_parts.len() {
        return false;
    }
    t_parts.iter().zip(l_parts.iter()).all(|(t, l)| {
        (t.starts_with('{') && t.ends_with('}')) || *t == *l
    })
}

fn validate_required_fields(entity_schema: &gurkha::EntitySchema, op: &Parivarta) -> Result<(), String> {
    if let Some(key) = &op.key {
        if let Some(field) = entity_schema.fields.iter().find(|f| f.name == *key && f.required) {
            if matches!(op.op, OpKind::Delete) || matches!(op.value, Some(Sthithi::Null) | None) {
                return Err(format!("required field '{}' cannot be unset", field.name));
            }
        }
    }

    if matches!(op.op, OpKind::Insert | OpKind::Set | OpKind::Update) {
        if let Some(Sthithi::Map(entries)) = &op.value {
            for field in entity_schema.fields.iter().filter(|f| f.required) {
                let present = entries
                    .iter()
                    .find(|(k, _)| k == &field.name)
                    .map(|(_, v)| !matches!(v, Sthithi::Null))
                    .unwrap_or(false);
                if !present {
                    return Err(format!("required field '{}' missing in entity payload", field.name));
                }
            }
        }
    }

    Ok(())
}

fn validate_immutable_fields(
    state: &ScribeState,
    layer_name: &str,
    entity_schema: &gurkha::EntitySchema,
    op: &Parivarta,
) -> Result<(), String> {
    let Some(key) = &op.key else {
        return Ok(());
    };
    let Some(field) = entity_schema
        .fields
        .iter()
        .find(|f| f.name == *key && f.immutable)
    else {
        return Ok(());
    };

    let Some(layer_unit) = state.units.get(layer_name) else {
        return Ok(());
    };

    let existing = current_field_value(Some(layer_unit.layer()), key);
    let new_value = op.value.as_ref();

    if let Some(existing_value) = existing {
        if new_value != Some(&existing_value) {
            return Err(format!("immutable field '{}' cannot be modified", field.name));
        }
    }

    Ok(())
}

fn current_field_value(layer: Option<&Layer>, key: &str) -> Option<Sthithi> {
    let layer = layer?;
    let root = layer.get_content_sthithi("root");
    let Sthithi::Map(entries) = root else {
        return None;
    };
    entries
        .into_iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_path_matches_dynamic_layer_name() {
        assert!(path_matches_template(
            "channels/{id}/messages",
            "channels/general/messages"
        ));
        assert!(!path_matches_template(
            "channels/{id}/messages",
            "channels/general/threads"
        ));
    }

    #[test]
    fn required_field_delete_is_rejected() {
        let schema = gurkha::EntitySchema {
            fields: vec![gurkha::FieldSchema {
                name: "text".to_string(),
                required: true,
                immutable: false,
            }],
            transitions: vec![],
            entity_rules: vec![],
        };

        let op = Parivarta {
            layer: "messages".to_string(),
            op: OpKind::Delete,
            path: "root".to_string(),
            key: Some("text".to_string()),
            index: None,
            value: None,
            old_value: None,
            intent: None,
            from_peer: None,
        };

        let result = validate_required_fields(&schema, &op);
        assert!(result.is_err());
    }

    #[test]
    fn transition_is_allowed_for_matching_role_and_path() {
        let schema = gurkha::EntitySchema {
            fields: vec![],
            transitions: vec![gurkha::TransitionSchema {
                field: "status".to_string(),
                from: "pending".to_string(),
                to: "confirmed".to_string(),
                allowed_roles: vec!["owner".to_string()],
                predicates: vec![],
            }],
            entity_rules: vec![],
        };

        let op = Parivarta {
            layer: "messages".to_string(),
            op: OpKind::Update,
            path: "root".to_string(),
            key: Some("status".to_string()),
            index: None,
            value: Some(Sthithi::Str("confirmed".to_string())),
            old_value: Some(Sthithi::Str("pending".to_string())),
            intent: None,
            from_peer: None,
        };

        assert!(is_transition_allowed(
            &schema,
            None,
            &op,
            "status",
            "pending",
            "confirmed",
            "owner",
            "did:key:owner"
        ));
    }

    #[test]
    fn transition_is_rejected_for_non_matching_role() {
        let schema = gurkha::EntitySchema {
            fields: vec![],
            transitions: vec![gurkha::TransitionSchema {
                field: "status".to_string(),
                from: "pending".to_string(),
                to: "confirmed".to_string(),
                allowed_roles: vec!["owner".to_string()],
                predicates: vec![],
            }],
            entity_rules: vec![],
        };

        let op = Parivarta {
            layer: "messages".to_string(),
            op: OpKind::Update,
            path: "root".to_string(),
            key: Some("status".to_string()),
            index: None,
            value: Some(Sthithi::Str("confirmed".to_string())),
            old_value: Some(Sthithi::Str("pending".to_string())),
            intent: None,
            from_peer: None,
        };

        assert!(!is_transition_allowed(
            &schema,
            None,
            &op,
            "status",
            "pending",
            "confirmed",
            "customer",
            "did:key:customer"
        ));
    }

    #[test]
    fn entity_on_delete_reject_blocks_delete_ops() {
        let schema = gurkha::EntitySchema {
            fields: vec![],
            transitions: vec![],
            entity_rules: vec![gurkha::EntityRuleSchema::OnDeleteReject],
        };

        let op = Parivarta {
            layer: "messages".to_string(),
            op: OpKind::Delete,
            path: "root".to_string(),
            key: Some("text".to_string()),
            index: None,
            value: None,
            old_value: None,
            intent: None,
            from_peer: None,
        };

        assert!(validate_entity_delete_rules(&schema, &op).is_err());
    }

    #[test]
    fn transition_predicate_eq_with_actor_did_matches() {
        let op = Parivarta {
            layer: "messages".to_string(),
            op: OpKind::Update,
            path: "root".to_string(),
            key: Some("owner".to_string()),
            index: None,
            value: Some(Sthithi::Str("did:key:alice".to_string())),
            old_value: Some(Sthithi::Str("did:key:bob".to_string())),
            intent: None,
            from_peer: None,
        };

        let predicate = gurkha::Predicate::Eq {
            left: gurkha::Value::Ref(gurkha::Ref::TargetField {
                field: "owner".to_string(),
            }),
            right: gurkha::Value::Ref(gurkha::Ref::ActorDid),
        };

        assert!(evaluate_transition_predicate(
            None,
            &op,
            &predicate,
            "did:key:alice"
        ));
    }

    #[test]
    fn transition_predicate_gt_reads_other_field_from_layer() {
        let layer = Layer::new();
        layer
            .set_from_sthithi("priority", &Sthithi::Int(9))
            .expect("set priority");

        let op = Parivarta {
            layer: "messages".to_string(),
            op: OpKind::Update,
            path: "root".to_string(),
            key: Some("status".to_string()),
            index: None,
            value: Some(Sthithi::Str("confirmed".to_string())),
            old_value: Some(Sthithi::Str("pending".to_string())),
            intent: None,
            from_peer: None,
        };

        let predicate = gurkha::Predicate::Gt {
            left: gurkha::Value::Ref(gurkha::Ref::TargetField {
                field: "priority".to_string(),
            }),
            right: gurkha::Value::Num(5),
        };

        assert!(evaluate_transition_predicate(
            Some(&layer),
            &op,
            &predicate,
            "did:key:alice"
        ));
    }

    #[test]
    fn literal_value_to_sthithi_converts_to_runtime_value() {
        let value = literal_value_to_sthithi(&gurkha::LiteralValue::String("confirmed".to_string()));
        assert_eq!(value, Sthithi::Str("confirmed".to_string()));
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
    extracted_ops: &Option<Vec<Parivarta>>,
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
    let is_dynamic = state
        .our_permit
        .as_ref()
        .and_then(|permit| {
            crate::layer_unit::find_matching_dynamic_schema(permit, layer_name, &state.page_id)
        })
        .is_some();
    unit.is_dynamic = is_dynamic;
    unit.mark_dirty();
    state.units.insert(layer_name.to_string(), unit);
    info!(layer = %layer_name, "Created new layer from peer sync");

    // Add existing subscribers only for static layers.
    // Dynamic layers are access-controlled via __sync_meta + LayerSubscribe permit issuance.
    if !is_dynamic {
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
                        state.emit_layer_auth_capture(
                            layer_name,
                            did,
                            "subscriber_added_from_peer",
                        );
                    }
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
    state
        .peer_role_cache
        .insert(peer_did.to_string(), role.clone());
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

    // Node mode: look up peer's stored permit and extract role via gurkha::PolicyPermit
    if let Some(ref resolver) = state.peer_resolver {
        if let Some(permit_token) = resolver.load_user_permit(peer_did) {
            let role = gurkha::PolicyPermit::from_token(&permit_token)
                .ok()
                .and_then(|permit| policy_compat::relationship(&permit))
                .unwrap_or_else(|| "peer".to_string());
            debug!(peer_did = %peer_did, role = %role, "Got peer role from stored permit via gurkha::PolicyPermit");
            return role;
        }
    }

    // Default role
    "peer".to_string()
}
