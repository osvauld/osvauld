//! Dynamic Layer Management
//!
//! Schema validation, path generation, and permit preparation for dynamic layers.
//! Uses PermitIssuer trait — never touches signing keys directly.

use tracing::{info, warn};

use crate::state::ScribeState;
use crate::storage::PermitIssuer;
use crate::loro_observer;

use super::LayerUnit;

/// Handle CreateDynamicLayer message
///
/// **Context**: Lua app calls scribe:create_layer("channels/{id}/messages", "general")
/// **We do**: Validate schema exists in permit, generate full path with our DID, create layer
/// **Returns**: Full layer name (bare, without page_id prefix)
pub fn handle_create_dynamic_layer(
    state: &mut ScribeState,
    schema_key: &str,
    layer_id: &str,
) -> Result<String, String> {
    let permit = state.our_permit.as_ref()
        .ok_or_else(|| "No permit available".to_string())?;

    // Validate schema exists in our permit's dynamic_layer_schemas
    let schemas = permit.dynamic_layer_schemas();
    let schema = schemas
        .get(schema_key)
        .cloned()
        .ok_or_else(|| format!("Schema '{}' not found in permit", schema_key))?;

    // Generate full layer path with our DID
    // Schema: "channels/{id}/messages" → Path: "channels/{our_did}/{layer_id}/messages"
    let bare_path = generate_dynamic_path(schema_key, &state.our_did, layer_id);
    let full_name = format!("{}/{}", state.page_id, bare_path);

    // Check if layer already exists
    if state.units.contains_key(&bare_path) {
        info!(layer = %bare_path, "Dynamic layer already exists");
        return Ok(full_name);
    }

    // Create the layer
    info!(
        page_id = %state.page_id,
        schema = %schema_key,
        layer = %bare_path,
        "Creating dynamic layer"
    );
    let mut unit = LayerUnit::new_empty();
    unit.set_local_only(!state.should_sync_layer(&bare_path));
    unit.is_dynamic = true;
    state.units.insert(bare_path.clone(), unit);
    state.track_new_sync_layer_in_meta(&bare_path);

    // Creator can sync — authorize all current subscribers (viewer mode: just the node)
    if let Ok(subs) = state.subscribers.read() {
        for ((did, _), _) in subs.iter() {
            if let Some(unit) = state.units.get(&bare_path) {
                unit.authorize_did(did);
                state.emit_layer_auth_capture(&bare_path, did, "authorized_on_create");
            }
        }
    }

    // Set up observer for the new layer
    loro_observer::setup_layer_observer(state, &bare_path);

    issue_creator_layer_authority_permits(state, &full_name, &schema);

    Ok(full_name)
}

fn issue_creator_layer_authority_permits(
    state: &ScribeState,
    full_layer_name: &str,
    schema: &gurkha::DynamicLayerSchema,
) {
    let Some(ref issuer) = state.permit_issuer else { return; };

    let mut targets: Vec<(String, Option<String>)> = Vec::new();
    if let Ok(subs) = state.subscribers.read() {
        for ((did, _device), info) in subs.iter() {
            let role = info
                .permit
                .get_fact("role")
                .and_then(|v| v.as_str())
                .map(|r| r.to_string())
                .or_else(|| {
                    info.permit
                        .get_fact("relationship")
                        .and_then(|v| v.as_str())
                        .map(|r| r.to_string())
                });
            targets.push((did.clone(), role));
        }
    }

    for (audience, role) in targets {
        let Some(config) = authority_config_from_template(schema, role.as_deref()) else {
            warn!(
                page_id = %state.page_id,
                layer = %full_layer_name,
                audience = %audience,
                role = role.as_deref().unwrap_or("unknown"),
                "Skipping layer authority issuance: no matching template config"
            );
            continue;
        };

        match issuer.issue_layer_authority_permit(
            &audience,
            full_layer_name,
            config,
            None,
            1,
        ) {
            Ok((_token, _cid)) => {
                info!(
                    page_id = %state.page_id,
                    layer = %full_layer_name,
                    audience = %audience,
                    "Issued creator layer authority permit"
                );
            }
            Err(e) => {
                warn!(
                    page_id = %state.page_id,
                    layer = %full_layer_name,
                    audience = %audience,
                    error = %e,
                    "Failed to issue creator layer authority permit"
                );
            }
        }
    }
}

fn authority_config_from_template(
    schema: &gurkha::DynamicLayerSchema,
    role: Option<&str>,
) -> Option<gurkha::LayerConfig> {
    let mut config = match schema.grant {
        gurkha::GrantType::Explicit => schema.permissions.clone(),
        gurkha::GrantType::Role => role.and_then(|r| schema.role_permissions.get(r).cloned()),
    }?;

    if config.layer_type.is_none() && !schema.layer_type.is_empty() {
        config.layer_type = Some(schema.layer_type.clone());
    }

    Some(config)
}

/// Generate a dynamic layer path from schema pattern, creator DID, and layer ID
///
/// **Pattern**: `channels/{id}/messages` → `channels/{did}/{layer_id}/messages`
/// **Pattern**: `orders/{id}` → `orders/{did}/{layer_id}`
///
/// The DID is inserted after the first literal segment, and `{id}` is replaced with layer_id.
fn generate_dynamic_path(schema_key: &str, our_did: &str, layer_id: &str) -> String {
    let parts: Vec<&str> = schema_key.split('/').collect();
    let mut result = Vec::new();

    if parts.is_empty() {
        return format!("{}/{}", our_did, layer_id);
    }

    // First segment: literal
    result.push(parts[0].to_string());

    // Insert our DID as the second segment (namespace)
    result.push(our_did.to_string());

    // Remaining segments: replace {id} with layer_id, keep literals
    for part in &parts[1..] {
        if *part == "{id}" {
            result.push(layer_id.to_string());
        } else {
            result.push(part.to_string());
        }
    }

    result.join("/")
}

/// Determine recipients and create layer permits for a new dynamic layer
///
/// **Context**: Called by Scribe when detecting a new dynamic layer from a peer.
/// **Returns**: Ready-to-send permits — courier just distributes them.
/// **Uses**: PermitIssuer trait (context-aware gurkha) — never touches signing keys.
pub fn prepare_layer_permits(
    permit_issuer: &dyn PermitIssuer,
    page_id: &str,
    layer_name: &str,
    schema: &gurkha::DynamicLayerSchema,
    creator_did: &str,
    connected_peers: &[(String, String)],  // (did, role)
) -> Result<Vec<(String, String)>, String> {
    let mut permits = Vec::new();

    match schema.grant {
        gurkha::GrantType::Role => {
            // Role-granted: all peers with matching roles get access
            for (did, role) in connected_peers {
                if let Some(perms) = schema.role_permissions.get(role) {
                    match permit_issuer.issue_layer_permit(did, layer_name, perms.clone(), None) {
                        Ok((token, _cid)) => {
                            info!(
                                page_id = %page_id,
                                layer = %layer_name,
                                recipient = %did,
                                role = %role,
                                "Issued role-based layer permit"
                            );
                            permits.push((did.clone(), token));
                        }
                        Err(e) => {
                            warn!(
                                page_id = %page_id,
                                layer = %layer_name,
                                recipient = %did,
                                error = %e,
                                "Failed to issue layer permit"
                            );
                        }
                    }
                }
            }
        }
        gurkha::GrantType::Explicit => {
            // Explicit-granted: authority-driven only (creator -> node permits).
            for (audience_did, _role) in connected_peers {
                let Some((config, intent_cid)) = authority_layer_config_and_intent(
                    permit_issuer,
                    audience_did,
                    layer_name,
                    page_id,
                ) else {
                    continue;
                };

                match permit_issuer.issue_layer_permit(
                    audience_did,
                    layer_name,
                    config,
                    Some(&intent_cid),
                ) {
                    Ok((token, _cid)) => {
                        info!(
                            page_id = %page_id,
                            layer = %layer_name,
                            audience = %audience_did,
                            creator = %creator_did,
                            "Issued explicit layer permit from authority"
                        );
                        permits.push((audience_did.clone(), token));
                    }
                    Err(e) => {
                        warn!(
                            page_id = %page_id,
                            layer = %layer_name,
                            audience = %audience_did,
                            error = %e,
                            "Failed to issue explicit layer permit from authority"
                        );
                    }
                }
            }
        }
    }

    Ok(permits)
}

/// Find matching dynamic schema for a layer received from a peer
///
/// **Context**: Node receives a new layer from peer. Check if it matches a dynamic schema.
/// **Returns**: (schema_key, schema, creator_did) if matched, None otherwise
pub fn find_matching_dynamic_schema<'a>(
    permit: &'a gurkha::Permit,
    layer_name: &str,
    page_id: &str,
) -> Option<(&'a str, &'a gurkha::DynamicLayerSchema, String)> {
    let schemas = permit.dynamic_layer_schemas();
    if schemas.is_empty() {
        return None;
    }

    // Strip page_id prefix to get bare layer path
    let page_prefix = format!("{}/", page_id);
    let bare_path = layer_name.strip_prefix(&page_prefix).unwrap_or(layer_name);

    let path_parts: Vec<&str> = bare_path.split('/').collect();

    // Path must have at least 3 segments: prefix/did/id[/suffix]
    if path_parts.len() < 3 {
        return None;
    }

    // Second segment should be a DID (the creator)
    let potential_creator = path_parts[1];
    if !potential_creator.starts_with("did:") {
        return None;
    }

    // Try each schema pattern
    for (schema_key, schema) in schemas {
        let pattern_parts: Vec<&str> = schema_key.split('/').collect();

        // Dynamic path has one extra segment (DID) compared to pattern
        if path_parts.len() != pattern_parts.len() + 1 {
            continue;
        }

        // First segment must match literally
        if path_parts[0] != pattern_parts[0] {
            continue;
        }

        // path[2..] must match pattern[1..] (with {id} as wildcard)
        let mut matched = true;
        for (path_seg, pat_seg) in path_parts[2..].iter().zip(pattern_parts[1..].iter()) {
            if *pat_seg != "{id}" && path_seg != pat_seg {
                matched = false;
                break;
            }
        }

        if matched {
            return Some((schema_key, schema, potential_creator.to_string()));
        }
    }

    None
}

/// Validate that a layer path created by a peer matches their DID (namespace security)
///
/// **Context**: Node receives a new layer from a peer. Before accepting, verify
/// that the creator's DID is in the layer path (prevents namespace forgery).
///
/// **Returns**: true if the layer path contains the sender's DID
pub fn validate_creator_namespace(layer_name: &str, page_id: &str, sender_did: &str) -> bool {
    let page_prefix = format!("{}/", page_id);
    let bare_path = layer_name.strip_prefix(&page_prefix).unwrap_or(layer_name);

    // The creator's DID must be the second segment in the path
    let parts: Vec<&str> = bare_path.split('/').collect();
    if parts.len() < 2 {
        return false;
    }

    parts[1] == sender_did
}

/// Handle AddLayerAccess message
///
/// **Context**: Lua app calls scribe:add_layer_access(layer_name, did)
/// **We do**: Validate capability, require authority permit, issue permit if peer is subscribed
pub async fn handle_add_layer_access(
    state: &ScribeState,
    layer_name: &str,
    did: &str,
) -> Result<(), String> {
    // 1. Check our permit has manage_layer_access capability
    let permit = state.our_permit.as_ref().ok_or_else(|| {
        warn!(page_id = %state.page_id, layer = %layer_name, audience = %did, "AddLayerAccess rejected: missing our_permit");
        "No permit".to_string()
    })?;
    if !permit.can_manage_layer_access() {
        warn!(
            page_id = %state.page_id,
            layer = %layer_name,
            audience = %did,
            "AddLayerAccess rejected: missing manage_layer_access capability"
        );
        return Err("No manage_layer_access capability".into());
    }

    // 2. Resolve schema/config and permit issuer
    let issuer = state.permit_issuer.as_ref().ok_or_else(|| {
        warn!(
            page_id = %state.page_id,
            layer = %layer_name,
            audience = %did,
            "AddLayerAccess rejected: no permit issuer (not node mode)"
        );
        "No permit issuer (not node mode)".to_string()
    })?;
    let (_schema_key, schema, _creator) = find_matching_dynamic_schema(permit, layer_name, &state.page_id)
        .ok_or_else(|| {
            warn!(
                page_id = %state.page_id,
                layer = %layer_name,
                audience = %did,
                "AddLayerAccess rejected: layer does not match dynamic schema"
            );
            "Layer does not match any dynamic schema".to_string()
        })?;
    if schema.grant != gurkha::GrantType::Explicit {
        warn!(
            page_id = %state.page_id,
            layer = %layer_name,
            audience = %did,
            "AddLayerAccess rejected: schema grant is not explicit"
        );
        return Err("AddLayerAccess only applies to explicit dynamic schemas".into());
    }

    let (config, intent_cid, source) = if let Some((cfg, cid)) = authority_layer_config_and_intent(
        issuer.as_ref(),
        did,
        layer_name,
        &state.page_id,
    ) {
        (cfg, Some(cid), "authority")
    } else {
        let cfg = schema
            .permissions
            .clone()
            .ok_or_else(|| "Explicit schema missing permissions and no authority permit present".to_string())?;
        warn!(
            page_id = %state.page_id,
            layer = %layer_name,
            audience = %did,
            "No layer authority permit found; issuing access from explicit schema permissions"
        );
        (cfg, None, "schema_permissions")
    };

    // 3. Issue permit (stored by coordinator even if peer is offline)
    let mut new_permits = Vec::new();
    match issuer.issue_layer_permit(did, layer_name, config, intent_cid.as_deref()) {
        Ok((token, _cid)) => {
            info!(
                page_id = %state.page_id,
                layer = %layer_name,
                audience = %did,
                source = %source,
                "Issued layer permit via AddLayerAccess"
            );
            new_permits.push((did.to_string(), token));
        }
        Err(e) => {
            warn!(error = %e, "Failed to issue layer permit for AddLayerAccess");
        }
    }

    // 4. Authorize DID on the LayerUnit (if permit was issued)
    if let Some((recipient_did, _token)) = new_permits.first() {
        let bare_name = crate::state::normalize_layer_name(layer_name, &state.page_id);
        if let Some(unit) = state.units.get(&bare_name) {
            unit.authorize_did(recipient_did);
            state.emit_layer_auth_capture(&bare_name, recipient_did, "authorized_add_access");
        }
    }

    // 5. Emit LayerAccessChanged for Coordinator to store/distribute permit
    if !new_permits.is_empty() {
        if let Some(ref sync_event_tx) = state.sync_event_tx {
            let event = crate::SyncEvent::LayerAccessChanged {
                page_id: state.page_id.clone(),
                layer_name: layer_name.to_string(),
                permits: new_permits,
            };
            state.emit_sync_event_capture(&event);
            if let Err(e) = sync_event_tx.send(event).await {
                warn!(error = %e, "Failed to emit LayerAccessChanged");
            }
        }
    }

    Ok(())
}

/// Handle RemoveLayerAccess message
///
/// **Context**: Lua app calls scribe:remove_layer_access(layer_name, did)
/// **We do**: Validate capability, reject metadata-era revoke path
pub fn handle_remove_layer_access(
    state: &ScribeState,
    layer_name: &str,
    did: &str,
) -> Result<(), String> {
    let permit = state.our_permit.as_ref().ok_or("No permit")?;
    if !permit.can_manage_layer_access() {
        return Err("No manage_layer_access capability".into());
    }

    let _ = layer_name;
    let _ = did;
    Err("RemoveLayerAccess is deprecated; rotate/revoke layer authority permits instead".to_string())
}

fn authority_layer_config_and_intent(
    issuer: &dyn PermitIssuer,
    audience_did: &str,
    layer_name: &str,
    page_id: &str,
) -> Option<(gurkha::LayerConfig, String)> {
    let (_version, authority_token) = issuer
        .get_layer_authority_permit(audience_did, layer_name)
        .ok()
        .flatten()?;

    let parsed = gurkha::Permit::from_token(&authority_token).ok()?;
    let config = if let Some(c) = parsed.layers().get(layer_name) {
        c.clone()
    } else {
        let bare = crate::state::normalize_layer_name(layer_name, page_id);
        parsed.layers().get(&bare)?.clone()
    };

    let intent_cid = gurkha::crypto::get_permit_cid(&authority_token).ok()?;
    Some((config, intent_cid))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};
    use std::sync::atomic::AtomicU64;

    use tokio::sync::{broadcast, mpsc};

    use crate::state::{ScribeState, SubscriberInfo};
    use crate::storage::{
        LayerStorageRef, NullLayerStorage, NullPeerVectorStorage, NullPermitIssuer,
        PeerVectorStorageRef, PermitIssuer, PermitIssuerRef,
    };

    #[derive(Default)]
    struct RecordingPermitIssuer {
        issued: Arc<Mutex<Vec<(String, String, gurkha::LayerConfig)>>>,
    }

    impl PermitIssuer for RecordingPermitIssuer {
        fn issue_layer_permit(
            &self,
            _audience: &str,
            _layer_name: &str,
            _config: gurkha::LayerConfig,
            _intent_cid: Option<&str>,
        ) -> crate::Result<(String, String)> {
            Ok(("unused-layer-token".to_string(), "unused-layer-cid".to_string()))
        }

        fn list_layer_authority_permits_for_audience(
            &self,
            _audience: &str,
        ) -> crate::Result<Vec<(String, u64, String)>> {
            Ok(Vec::new())
        }

        fn get_layer_authority_permit(
            &self,
            _audience: &str,
            _layer_name: &str,
        ) -> crate::Result<Option<(u64, String)>> {
            Ok(None)
        }

        fn has_layer_access_permit(&self, _audience: &str, _layer_name: &str) -> crate::Result<bool> {
            Ok(false)
        }

        fn issue_layer_authority_permit(
            &self,
            audience: &str,
            layer_name: &str,
            config: gurkha::LayerConfig,
            _authorized_peers: Option<Vec<String>>,
            _version: u64,
        ) -> crate::Result<(String, String)> {
            self.issued
                .lock()
                .expect("recording mutex poisoned")
                .push((audience.to_string(), layer_name.to_string(), config));
            Ok(("authority-token".to_string(), "authority-cid".to_string()))
        }
    }

    fn make_subscriber(permit: gurkha::Permit, did: &str) -> SubscriberInfo {
        let (tx, _rx) = mpsc::channel(4);
        SubscriberInfo {
            permit,
            subscriber_did: did.to_string(),
            is_visible: true,
            can_see_others: true,
            display_name: None,
            broadcast_tx: tx,
            ephemeral_tx: None,
            vectors: HashMap::new(),
        }
    }

    fn make_state(permit_issuer: PermitIssuerRef) -> ScribeState {
        ScribeState {
            page_id: "page1".to_string(),
            units: HashMap::new(),
            subscribers: Arc::new(std::sync::RwLock::new(HashMap::new())),
            query_subscribers: HashMap::new(),
            layer_storage: Arc::new(NullLayerStorage) as LayerStorageRef,
            vector_storage: Arc::new(NullPeerVectorStorage) as PeerVectorStorageRef,
            peer_resolver: None,
            permit_issuer: Some(permit_issuer),
            sync_config: None,
            sync_event_tx: None,
            capture_tx: None::<broadcast::Sender<String>>,
            capture_seq: AtomicU64::new(0),
            page_update_subscribers: Arc::new(std::sync::RwLock::new(Vec::new())),
            pending_update_source: Arc::new(Mutex::new(None)),
            validation_handle: None,
            our_permit: None,
            our_did: "did:key:owner".to_string(),
            node_script_shutdown: None,
            pending_layer_authorizations: HashMap::new(),
        }
    }

    #[test]
    fn test_generate_dynamic_path() {
        assert_eq!(
            generate_dynamic_path("channels/{id}/messages", "did:key:alice", "general"),
            "channels/did:key:alice/general/messages"
        );
        assert_eq!(
            generate_dynamic_path("orders/{id}", "did:key:bob", "uuid-123"),
            "orders/did:key:bob/uuid-123"
        );
        assert_eq!(
            generate_dynamic_path("dms/{id}/messages", "did:key:alice", "room1"),
            "dms/did:key:alice/room1/messages"
        );
    }

    #[test]
    fn test_validate_creator_namespace() {
        assert!(validate_creator_namespace(
            "page1/channels/did:key:alice/general/messages",
            "page1",
            "did:key:alice",
        ));
        // Wrong DID
        assert!(!validate_creator_namespace(
            "page1/channels/did:key:alice/general/messages",
            "page1",
            "did:key:bob",
        ));
        // Bare path (no page prefix)
        assert!(validate_creator_namespace(
            "channels/did:key:alice/general/messages",
            "other_page",
            "did:key:alice",
        ));
        // Too short
        assert!(!validate_creator_namespace(
            "page1/channels",
            "page1",
            "did:key:alice",
        ));
    }

    #[test]
    fn test_find_matching_dynamic_schema() {
        // Shop owner has dynamic_layer_schemas: { "orders/{id}": ... }
        let permit = gurkha::test_fixtures::shop_owner("page1", "did:key:owner");

        // Match: orders/did:key:alice/uuid-123 — matches "orders/{id}" with DID inserted
        let result = find_matching_dynamic_schema(&permit, "orders/did:key:alice/uuid-123", "page1");
        assert!(result.is_some(), "Should match orders/{{id}} schema");
        let (schema_key, _schema, creator_did) = result.unwrap();
        assert_eq!(schema_key, "orders/{id}");
        assert_eq!(creator_did, "did:key:alice");

        // Match with page_id prefix
        let result = find_matching_dynamic_schema(&permit, "page1/orders/did:key:bob/order1", "page1");
        assert!(result.is_some());
        let (_, _, creator_did) = result.unwrap();
        assert_eq!(creator_did, "did:key:bob");

        // No match: wrong prefix
        let result = find_matching_dynamic_schema(&permit, "channels/did:key:alice/general/messages", "page1");
        assert!(result.is_none(), "Should not match — no channels schema");

        // No match: no DID in path
        let result = find_matching_dynamic_schema(&permit, "orders/general", "page1");
        assert!(result.is_none(), "Should not match — no DID segment");

        // No match: too short
        let result = find_matching_dynamic_schema(&permit, "orders", "page1");
        assert!(result.is_none(), "Should not match — too short");
    }

    #[test]
    fn test_prepare_layer_permits_role_based() {
        let issuer = NullPermitIssuer;
        let mut role_permissions = std::collections::HashMap::new();
        role_permissions.insert("collaborator".to_string(), gurkha::LayerConfig {
            sync: true, write: true, layer_type: None,
        });

        let schema = gurkha::DynamicLayerSchema {
            layer_type: "list".to_string(),
            grant: gurkha::GrantType::Role,
            permissions: None,
            role_permissions,
        };

        let peers = vec![
            ("did:key:alice".to_string(), "collaborator".to_string()),
            ("did:key:bob".to_string(), "viewer".to_string()),
        ];

        let permits = prepare_layer_permits(
            &issuer, "page1", "page1/channels/did:key:creator/general/messages",
            &schema, "did:key:creator", &peers,
        ).unwrap();

        // Only alice (collaborator) should get a permit — bob (viewer) has no role match
        assert_eq!(permits.len(), 1);
        assert_eq!(permits[0].0, "did:key:alice");
    }

    #[test]
    fn test_prepare_layer_permits_explicit() {
        let issuer = NullPermitIssuer;
        let schema = gurkha::DynamicLayerSchema {
            layer_type: "map".to_string(),
            grant: gurkha::GrantType::Explicit,
            permissions: Some(gurkha::LayerConfig {
                sync: true, write: true, layer_type: None,
            }),
            role_permissions: std::collections::HashMap::new(),
        };

        let peers = vec![
            ("did:key:alice".to_string(), "collaborator".to_string()),
            ("did:key:bob".to_string(), "viewer".to_string()),
        ];

        let permits = prepare_layer_permits(
            &issuer, "page1", "page1/orders/did:key:creator/uuid1",
            &schema, "did:key:creator", &peers,
        ).unwrap();

        // No authority permits in NullPermitIssuer => no explicit permits issued
        assert_eq!(permits.len(), 0);
    }

    #[test]
    fn test_creator_authority_issuance_uses_schema_role_permissions() {
        let recorder = RecordingPermitIssuer::default();
        let issued = recorder.issued.clone();
        let state = make_state(Arc::new(recorder));

        // This fixture carries relationship=node_viewer in facts.
        let subscriber = make_subscriber(
            gurkha::test_fixtures::handshake_viewer("page1", "did:key:alice"),
            "did:key:alice",
        );
        state
            .subscribers
            .write()
            .expect("subscriber lock poisoned")
            .insert(("did:key:alice".to_string(), "device1".to_string()), subscriber);

        let mut role_permissions = HashMap::new();
        role_permissions.insert(
            "node_viewer".to_string(),
            gurkha::LayerConfig {
                sync: true,
                write: false,
                layer_type: None,
            },
        );

        let schema = gurkha::DynamicLayerSchema {
            layer_type: "map".to_string(),
            grant: gurkha::GrantType::Role,
            permissions: None,
            role_permissions,
        };

        issue_creator_layer_authority_permits(
            &state,
            "page1/channels/did:key:owner/general/messages",
            &schema,
        );

        let issued = issued.lock().expect("recording mutex poisoned");
        assert_eq!(issued.len(), 1, "exactly one authority permit should be issued");
        assert_eq!(issued[0].0, "did:key:alice");
        assert_eq!(issued[0].1, "page1/channels/did:key:owner/general/messages");
        assert_eq!(issued[0].2.sync, true);
        assert_eq!(issued[0].2.write, false, "must use template role_permissions, not hardcoded write=true");
        assert_eq!(issued[0].2.layer_type.as_deref(), Some("map"));
    }
}
