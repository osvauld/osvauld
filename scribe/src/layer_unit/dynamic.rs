//! Dynamic Layer Management
//!
//! Schema validation, path generation, and permit preparation for dynamic layers.
//! Uses PermitIssuer trait — never touches signing keys directly.

use std::collections::HashMap;
use tracing::{info, warn};

use crate::loro_observer;
use crate::policy_compat;
use crate::state::ScribeState;

use super::LayerUnit;

/// Handle CreateDynamicLayer message
///
/// **Context**: Lua app calls scribe:create_layer("channels/{id}/messages", "general")
/// **We do**: Validate schema, generate path, issue self-permit, create layer, write __sync_meta
/// **Returns**: Full layer name (bare, without page_id prefix)
pub fn handle_create_dynamic_layer(
    state: &mut ScribeState,
    schema_key: &str,
    placeholders: &HashMap<String, String>,
    authorized_peers: Option<&[String]>,
) -> Result<String, String> {
    let permit = state
        .our_permit
        .as_ref()
        .ok_or_else(|| "No permit available".to_string())?;

    // Validate schema exists in our permit's dynamic_layer_schemas
    let schemas = policy_compat::dynamic_layer_schemas(permit);
    let schema = schemas
        .get(schema_key)
        .cloned()
        .ok_or_else(|| format!("Schema '{}' not found in permit", schema_key))?;

    // Generate full layer path with our DID
    // Schema: "channels/{id}/messages" → Path: "channels/{our_did}/{layer_id}/messages"
    let bare_path =
        generate_dynamic_path(schema_key, &schema.namespace, &state.our_did, placeholders)?;
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
        authorized_peers = ?authorized_peers,
        "Creating dynamic layer"
    );
    let mut unit = LayerUnit::new_empty();
    unit.set_local_only(!state.should_sync_layer(&bare_path));
    unit.is_dynamic = true;
    state.units.insert(bare_path.clone(), unit);

    // Add all connected subscribers to the new layer
    if let Ok(subs) = state.subscribers.read() {
        for ((did, device_id), info) in subs.iter() {
            if let Some(unit) = state.units.get(&bare_path) {
                unit.add_subscriber(
                    did.clone(),
                    device_id.clone(),
                    true,
                    info.broadcast_tx.clone(),
                );
                state.emit_layer_auth_capture(&bare_path, did, "subscriber_added_on_create");
            }
        }
    }

    // Set up observer for the new layer
    loro_observer::setup_layer_observer(state, &bare_path);

    // Issue self-signed layer_authority permit (using schema config)
    // Self-permit must be persisted BEFORE __sync_meta entry (which triggers discovery)
    if let Some(ref issuer) = state.permit_issuer {
        let config = schema.permissions.clone();
        let auth_peers = authorized_peers.map(|p| p.to_vec());
        match issuer.issue_layer_authority_permit(&state.our_did, &full_name, config, auth_peers, 1)
        {
            Ok((_token, _cid)) => {
                info!(layer = %full_name, "Issued self-permit for dynamic layer");
            }
            Err(e) => {
                warn!(layer = %full_name, error = %e, "Failed to issue self-permit for dynamic layer");
            }
        }
    }

    // Write entry to creator's __sync_meta for node discovery
    {
        use crate::sync::sync_meta;
        let our_did = state.our_did.clone();
        let meta_layer = sync_meta::sync_meta_layer_name(&our_did);
        if state.units.contains_key(&meta_layer) {
            sync_meta::write_sync_meta_entry(state, &our_did, &bare_path, false);
            info!(layer = %bare_path, "Wrote dynamic layer entry to creator's __sync_meta");
        }
    }

    Ok(full_name)
}

/// Generate a dynamic layer path from schema pattern, creator DID, and layer ID
///
/// **Pattern**: `channels/{id}/messages` → `channels/{did}/{layer_id}/messages`
/// **Pattern**: `orders/{id}` → `orders/{did}/{layer_id}`
///
/// The DID is inserted after the first literal segment, and `{id}` is replaced with layer_id.
fn generate_dynamic_path(
    schema_key: &str,
    namespace: &gurkha::LayerNamespace,
    our_did: &str,
    placeholders: &HashMap<String, String>,
) -> Result<String, String> {
    let parts: Vec<&str> = schema_key.split('/').collect();
    let mut result = Vec::new();

    if parts.is_empty() {
        return Err("Schema key cannot be empty".to_string());
    }

    for (idx, part) in parts.iter().enumerate() {
        let seg = if part.starts_with('{') && part.ends_with('}') {
            let key = &part[1..part.len() - 1];
            placeholders.get(key).cloned().ok_or_else(|| {
                format!("Missing placeholder '{}' for schema '{}'", key, schema_key)
            })?
        } else {
            part.to_string()
        };
        result.push(seg);

        if idx == 0 && matches!(namespace, gurkha::LayerNamespace::Creator) {
            result.push(our_did.to_string());
        }
    }

    Ok(result.join("/"))
}

/// Find matching dynamic schema for a layer received from a peer
///
/// **Context**: Node receives a new layer from peer. Check if it matches a dynamic schema.
/// **Returns**: (schema_key, schema, creator_did?) if matched, None otherwise
pub fn find_matching_dynamic_schema<'a>(
    permit: &gurkha::PolicyPermit,
    layer_name: &str,
    page_id: &str,
) -> Option<(String, gurkha::DynamicLayerSchema, Option<String>)> {
    let schemas = policy_compat::dynamic_layer_schemas(permit);
    find_matching_dynamic_schema_in_schemas(&schemas, layer_name, page_id)
}

fn find_matching_dynamic_schema_in_schemas(
    schemas: &HashMap<String, gurkha::DynamicLayerSchema>,
    layer_name: &str,
    page_id: &str,
) -> Option<(String, gurkha::DynamicLayerSchema, Option<String>)> {
    if schemas.is_empty() {
        return None;
    }

    // Strip page_id prefix to get bare layer path
    let page_prefix = format!("{}/", page_id);
    let bare_path = layer_name.strip_prefix(&page_prefix).unwrap_or(layer_name);

    if let Some(dynamic_ref) = gurkha::parse_dynamic_layer(schemas, bare_path) {
        for (schema_key, schema) in schemas {
            if *schema_key == dynamic_ref.schema_key {
                return Some((schema_key.clone(), schema.clone(), dynamic_ref.creator_did));
            }
        }
    }

    None
}

/// Handle AddLayerAccess message
///
/// **Context**: Lua app calls scribe:add_layer_access(layer_name, dids)
/// **We do**: Validate capability, merge new DIDs into self-permit's authorized_peers,
///   re-write __sync_meta entry to trigger node re-subscribe and get updated authority.
pub fn handle_add_layer_access(
    state: &mut ScribeState,
    layer_name: &str,
    dids: &[String],
) -> Result<(), String> {
    // 1. Check our permit has manage_layer_access capability
    let permit = state.our_permit.as_ref().ok_or_else(|| {
        warn!(page_id = %state.page_id, layer = %layer_name, dids = ?dids, "AddLayerAccess rejected: missing our_permit");
        "No permit".to_string()
    })?;
    if !policy_compat::can_manage_layer_access(permit) {
        warn!(
            page_id = %state.page_id,
            layer = %layer_name,
            dids = ?dids,
            "AddLayerAccess rejected: missing manage_layer_access capability"
        );
        return Err("No manage_layer_access capability".into());
    }

    // 2. Validate schema match + explicit grant
    let (_schema_key, schema, _creator) =
        find_matching_dynamic_schema(permit, layer_name, &state.page_id).ok_or_else(|| {
            warn!(
                page_id = %state.page_id,
                layer = %layer_name,
                dids = ?dids,
                "AddLayerAccess rejected: layer does not match dynamic schema"
            );
            "Layer does not match any dynamic schema".to_string()
        })?;
    if schema.grant != gurkha::GrantType::Explicit {
        return Err("AddLayerAccess only applies to explicit dynamic schemas".into());
    }

    // 3. Get existing self-permit and merge new DIDs
    let issuer = state
        .permit_issuer
        .as_ref()
        .ok_or_else(|| "No permit issuer".to_string())?;

    let full_name = if layer_name.starts_with(&state.page_id) {
        layer_name.to_string()
    } else {
        format!("{}/{}", state.page_id, layer_name)
    };

    // Read existing authorized_peers from self-permit
    let (mut existing_peers, existing_version) =
        match issuer.get_layer_authority_permit(&state.our_did, &full_name) {
            Ok(Some((version, token))) => {
                if let Ok(p) = gurkha::PolicyPermit::from_token(&token) {
                    let peers = policy_compat::authorized_peers(&p).unwrap_or_default();
                    (peers, version)
                } else {
                    (Vec::new(), version)
                }
            }
            _ => (Vec::new(), 0),
        };

    // Merge new DIDs
    for did in dids {
        if !existing_peers.contains(did) {
            existing_peers.push(did.clone());
        }
    }

    // 4. Re-issue self-permit with updated authorized_peers (incremented version)
    let config = schema.permissions.clone();
    issuer
        .issue_layer_authority_permit(
            &state.our_did,
            &full_name,
            config,
            Some(existing_peers),
            existing_version + 1,
        )
        .map_err(|e| format!("Failed to re-issue self-permit: {}", e))?;

    info!(
        page_id = %state.page_id,
        layer = %layer_name,
        dids = ?dids,
        "Updated self-permit with new authorized_peers"
    );

    // 5. Re-write __sync_meta entry as false to trigger node re-subscribe
    let bare_name = crate::state::normalize_layer_name(layer_name, &state.page_id);
    {
        use crate::sync::sync_meta;
        let our_did = state.our_did.clone();
        let meta_layer = sync_meta::sync_meta_layer_name(&our_did);
        if state.units.contains_key(&meta_layer) {
            sync_meta::write_sync_meta_entry(state, &our_did, &bare_name, false);
            info!(layer = %bare_name, "Re-wrote __sync_meta entry to trigger re-subscribe");
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_dynamic_path() {
        let mut placeholders = HashMap::new();
        placeholders.insert("id".to_string(), "general".to_string());
        assert_eq!(
            generate_dynamic_path(
                "channels/{id}/messages",
                &gurkha::LayerNamespace::Creator,
                "did:key:alice",
                &placeholders
            )
            .unwrap(),
            "channels/did:key:alice/general/messages",
        );
        placeholders.insert("id".to_string(), "uuid-123".to_string());
        assert_eq!(
            generate_dynamic_path(
                "orders/{id}",
                &gurkha::LayerNamespace::Creator,
                "did:key:bob",
                &placeholders
            )
            .unwrap(),
            "orders/did:key:bob/uuid-123",
        );
        placeholders.insert("id".to_string(), "room1".to_string());
        assert_eq!(
            generate_dynamic_path(
                "dms/{id}/messages",
                &gurkha::LayerNamespace::Shared,
                "did:key:alice",
                &placeholders
            )
            .unwrap(),
            "dms/room1/messages",
        );

        let mut named = HashMap::new();
        named.insert("channel".to_string(), "general".to_string());
        named.insert("period".to_string(), "2025-02".to_string());
        assert_eq!(
            generate_dynamic_path(
                "channels/{channel}/messages/{period}",
                &gurkha::LayerNamespace::Shared,
                "did:key:alice",
                &named
            )
            .unwrap(),
            "channels/general/messages/2025-02"
        );

        assert!(generate_dynamic_path(
            "channels/{channel}/messages/{period}",
            &gurkha::LayerNamespace::Shared,
            "did:key:alice",
            &placeholders
        )
        .is_err());
    }

    #[test]
    fn test_find_matching_dynamic_schema() {
        let mut schemas = HashMap::new();
        schemas.insert(
            "orders/{id}".to_string(),
            gurkha::DynamicLayerSchema {
                layer_type: "map".to_string(),
                grant: gurkha::GrantType::Open,
                permissions: gurkha::LayerConfig {
                    sync: true,
                    write: true,
                    layer_type: None,
                },
                namespace: gurkha::LayerNamespace::Creator,
                storage_strategy: gurkha::StorageStrategy::SingleDoc,
                resolution: None,
            },
        );

        // Match: orders/did:key:alice/uuid-123 — matches "orders/{id}" with DID inserted
        let result = find_matching_dynamic_schema_in_schemas(
            &schemas,
            "orders/did:key:alice/uuid-123",
            "page1",
        );
        assert!(result.is_some(), "Should match orders/{{id}} schema");
        let (schema_key, _schema, creator_did) = result.unwrap();
        assert_eq!(schema_key, "orders/{id}");
        assert_eq!(creator_did.as_deref(), Some("did:key:alice"));

        // Match with page_id prefix
        let result = find_matching_dynamic_schema_in_schemas(
            &schemas,
            "page1/orders/did:key:bob/order1",
            "page1",
        );
        assert!(result.is_some());
        let (_, _, creator_did) = result.unwrap();
        assert_eq!(creator_did.as_deref(), Some("did:key:bob"));

        // No match: wrong prefix
        let result = find_matching_dynamic_schema_in_schemas(
            &schemas,
            "channels/did:key:alice/general/messages",
            "page1",
        );
        assert!(result.is_none(), "Should not match — no channels schema");

        // No match: no DID in path
        let result = find_matching_dynamic_schema_in_schemas(&schemas, "orders/general", "page1");
        assert!(result.is_none(), "Should not match — no DID segment");

        // No match: too short
        let result = find_matching_dynamic_schema_in_schemas(&schemas, "orders", "page1");
        assert!(result.is_none(), "Should not match — too short");
    }
}
