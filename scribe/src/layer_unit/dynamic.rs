//! Dynamic Layer Management
//!
//! Schema validation, path generation, and permit preparation for dynamic layers.
//! Uses PermitIssuer trait — never touches signing keys directly.

use tracing::{info, warn};

use crate::state::ScribeState;
use crate::loro_observer;

use super::LayerUnit;

/// Handle CreateDynamicLayer message
///
/// **Context**: Lua app calls scribe:create_layer("channels/{id}/messages", "general")
/// **We do**: Validate schema, generate path, issue self-permit, create layer, write __sync_meta
/// **Returns**: Full layer name (bare, without page_id prefix)
pub fn handle_create_dynamic_layer(
    state: &mut ScribeState,
    schema_key: &str,
    layer_id: &str,
    authorized_peers: Option<&[String]>,
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
        authorized_peers = ?authorized_peers,
        "Creating dynamic layer"
    );
    let mut unit = LayerUnit::new_empty();
    unit.set_local_only(!state.should_sync_layer(&bare_path));
    unit.is_dynamic = true;
    state.units.insert(bare_path.clone(), unit);

    // Add all connected subscribers to the new layer
    if let Ok(subs) = state.subscribers.read() {
        for ((did, _), info) in subs.iter() {
            if let Some(unit) = state.units.get(&bare_path) {
                let caps = super::Capabilities { read: true, write: true, sync: true };
                unit.add_subscriber(did.clone(), caps, info.broadcast_tx.clone());
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
        match issuer.issue_layer_authority_permit(
            &state.our_did, &full_name, config, auth_peers, 1,
        ) {
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
    if !permit.can_manage_layer_access() {
        warn!(
            page_id = %state.page_id,
            layer = %layer_name,
            dids = ?dids,
            "AddLayerAccess rejected: missing manage_layer_access capability"
        );
        return Err("No manage_layer_access capability".into());
    }

    // 2. Validate schema match + explicit grant
    let (_schema_key, schema, _creator) = find_matching_dynamic_schema(permit, layer_name, &state.page_id)
        .ok_or_else(|| {
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
    let issuer = state.permit_issuer.as_ref().ok_or_else(|| {
        "No permit issuer".to_string()
    })?;

    let full_name = if layer_name.starts_with(&state.page_id) {
        layer_name.to_string()
    } else {
        format!("{}/{}", state.page_id, layer_name)
    };

    // Read existing authorized_peers from self-permit
    let (mut existing_peers, existing_version) = match issuer.get_layer_authority_permit(&state.our_did, &full_name) {
        Ok(Some((version, token))) => {
            if let Ok(p) = gurkha::Permit::from_token(&token) {
                let peers = p.get_fact("authorized_peers")
                    .and_then(|v| v.as_array().map(|arr| {
                        arr.iter().filter_map(|v| v.as_str().map(String::from)).collect::<Vec<_>>()
                    }))
                    .unwrap_or_default();
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
    issuer.issue_layer_authority_permit(
        &state.our_did, &full_name, config, Some(existing_peers), existing_version + 1,
    ).map_err(|e| format!("Failed to re-issue self-permit: {}", e))?;

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

#[cfg(test)]
mod tests {
    use super::*;

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

}
