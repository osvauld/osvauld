//! Sync Metadata Protocol Layer
//!
//! Each peer has a `__sync_meta:{did}` layer that serves as:
//! - **Discovery catalog**: what layers exist and should be synced
//! - **Access intent**: who should get access to explicit-grant layers
//! - **Sync status tracker**: what has been synced
//!
//! Contents are a Loro Map: `layer_name → synced (bool)`.
//! - `false` = layer discovered but not yet synced
//! - `true` = layer data has been synced
//!
//! Both the peer and the node write to the same CRDT doc.
//! Concurrent writes merge naturally via Loro.

use tracing::{debug, info, instrument, warn};

use crate::layer_unit::LayerUnit;
use crate::loro_observer;
use crate::state::ScribeState;

/// Prefix for sync metadata protocol layers
pub const SYNC_META_PREFIX: &str = "__sync_meta:";

/// Check if a layer name is a sync metadata protocol layer
pub fn is_sync_meta_layer(name: &str) -> bool {
    name.starts_with(SYNC_META_PREFIX)
}

/// Check if a layer name is any protocol-internal layer (filtered from Lua callbacks)
pub fn is_protocol_layer(name: &str) -> bool {
    is_sync_meta_layer(name)
}

/// Extract the peer DID from a sync metadata layer name
///
/// `__sync_meta:did:key:z6Mk...` → `Some("did:key:z6Mk...")`
pub fn extract_peer_did(name: &str) -> Option<&str> {
    name.strip_prefix(SYNC_META_PREFIX)
}

/// Build the sync metadata layer name for a peer
pub fn sync_meta_layer_name(peer_did: &str) -> String {
    format!("{}{}", SYNC_META_PREFIX, peer_did)
}

/// A single entry in a __sync_meta layer
#[derive(Debug, Clone)]
pub struct SyncMetaEntry {
    /// Bare layer name (no page_id prefix)
    pub layer_name: String,
    /// Whether the layer data has been synced
    pub synced: bool,
}

/// Create or get the __sync_meta layer for a peer
///
/// **Context**: Called on node when a peer subscribes, or on peer for their own layer.
/// Creates the LayerUnit if it doesn't exist, sets up observer.
#[instrument(skip(state), fields(page_id = %state.page_id))]
pub fn ensure_sync_meta_layer(state: &mut ScribeState, peer_did: &str) -> String {
    let layer_name = sync_meta_layer_name(peer_did);

    if !state.units.contains_key(&layer_name) {
        let mut unit = LayerUnit::new_empty();
        // sync_meta layers DO sync (they're not local-only)
        unit.set_local_only(false);
        state.units.insert(layer_name.clone(), unit);

        // Set up observer so changes propagate
        loro_observer::setup_layer_observer(state, &layer_name);

        info!(layer = %layer_name, peer_did = %peer_did, "Created __sync_meta layer");
    }

    layer_name
}

/// Write an entry to a peer's __sync_meta layer
///
/// **Context**: Node writes entries for layers the peer should discover.
/// Peer writes entries for layers they've created locally.
#[instrument(skip(state), fields(page_id = %state.page_id, peer_did = %peer_did))]
pub fn write_sync_meta_entry(
    state: &mut ScribeState,
    peer_did: &str,
    layer_name: &str,
    synced: bool,
) {
    let meta_layer = sync_meta_layer_name(peer_did);

    // Auto-create the layer if it doesn't exist
    if !state.units.contains_key(&meta_layer) {
        ensure_sync_meta_layer(state, peer_did);
    }

    let Some(unit) = state.units.get(&meta_layer) else {
        warn!(
            meta_layer = %meta_layer,
            layer = %layer_name,
            "Cannot write sync_meta entry: layer creation failed"
        );
        return;
    };

    let map = unit.layer().loro().get_map(meta_layer.as_str());
    let value = if synced {
        loro::LoroValue::Bool(true)
    } else {
        loro::LoroValue::Bool(false)
    };

    if let Err(e) = map.insert(layer_name, value) {
        warn!(
            meta_layer = %meta_layer,
            entry = %layer_name,
            error = %e,
            "Failed to write sync_meta entry"
        );
        return;
    }

    unit.layer().commit();

    if let Some(unit) = state.units.get_mut(&meta_layer) {
        unit.mark_dirty();
    }

    debug!(
        meta_layer = %meta_layer,
        entry = %layer_name,
        synced = synced,
        "Wrote sync_meta entry"
    );
}

/// Read all entries from a peer's __sync_meta layer
///
/// **Returns**: Vec of (layer_name, synced) entries
pub fn read_sync_meta_entries(state: &ScribeState, peer_did: &str) -> Vec<SyncMetaEntry> {
    let meta_layer = sync_meta_layer_name(peer_did);
    let Some(unit) = state.units.get(&meta_layer) else {
        return Vec::new();
    };

    let map = unit.layer().loro().get_map(meta_layer.as_str());
    let mut entries = Vec::new();

    for key in map.keys() {
        let synced = map
            .get(&key)
            .and_then(|v| {
                let deep = v.get_deep_value();
                if let loro::LoroValue::Bool(b) = deep {
                    Some(b)
                } else {
                    None
                }
            })
            .unwrap_or(false);

        entries.push(SyncMetaEntry {
            layer_name: key.to_string(),
            synced,
        });
    }

    entries
}

/// Read unsynced entries (synced == false) from a peer's __sync_meta layer
pub fn read_unsynced_entries(state: &ScribeState, peer_did: &str) -> Vec<String> {
    read_sync_meta_entries(state, peer_did)
        .into_iter()
        .filter(|e| !e.synced)
        .map(|e| e.layer_name)
        .collect()
}

/// Mark a sync_meta entry as synced (true)
#[instrument(skip(state), fields(page_id = %state.page_id))]
pub fn mark_entry_synced(state: &mut ScribeState, peer_did: &str, layer_name: &str) {
    write_sync_meta_entry(state, peer_did, layer_name, true);
}

/// Populate a peer's __sync_meta with static layer entries
///
/// **Context**: Node calls this after creating the peer's __sync_meta layer.
/// Writes entries for all static layers the peer has access to.
/// Owner gets entries marked `true` (already has data), viewers get `false`.
#[instrument(skip(state), fields(page_id = %state.page_id, peer_did = %peer_did))]
pub fn populate_static_layers(
    state: &mut ScribeState,
    peer_did: &str,
    peer_permit: &gurkha::Permit,
    is_owner: bool,
) {
    let static_layers = peer_permit.static_layers(&state.page_id, peer_did);

    for layer_name in &static_layers {
        if is_protocol_layer(layer_name) {
            continue;
        }
        let synced = is_owner;
        write_sync_meta_entry(state, peer_did, layer_name, synced);
    }

    info!(
        peer_did = %peer_did,
        entry_count = static_layers.len(),
        is_owner = is_owner,
        "Populated static layers in __sync_meta"
    );
}

/// Detect new dynamic layer entries in a creator's __sync_meta
///
/// **Context**: Node observes changes in a creator's __sync_meta. Returns layer names
/// that need a LayerSubscribe (unsynced entries matching dynamic schemas where
/// path_creator == creator_did).
///
/// Detection-only: no fanout, no GrantType branching. The node sends LayerSubscribe
/// for each detected layer, gets authority from creator, then fans out.
#[instrument(skip(state), fields(page_id = %state.page_id, creator_did = %creator_did))]
pub fn detect_new_dynamic_layers(state: &ScribeState, creator_did: &str) -> Vec<String> {
    use crate::layer_unit::find_matching_dynamic_schema;

    let our_permit = match state.our_permit.as_ref() {
        Some(p) => p,
        None => return Vec::new(),
    };
    if our_permit.dynamic_layer_schemas().is_empty() {
        return Vec::new();
    }

    let entries = read_sync_meta_entries(state, creator_did);
    let page_id = &state.page_id;

    let mut new_layers = Vec::new();

    for entry in &entries {
        if entry.synced {
            continue;
        }
        if is_protocol_layer(&entry.layer_name) {
            continue;
        }

        let Some((_schema_key, _schema, path_creator_did)) =
            find_matching_dynamic_schema(our_permit, &entry.layer_name, page_id)
        else {
            continue;
        };

        if path_creator_did != creator_did {
            debug!(
                layer = %entry.layer_name,
                creator = %creator_did,
                path_creator = %path_creator_did,
                "Skipping non-owned __sync_meta entry"
            );
            continue;
        }

        new_layers.push(entry.layer_name.clone());
    }

    if !new_layers.is_empty() {
        info!(
            creator_did = %creator_did,
            count = new_layers.len(),
            layers = ?new_layers,
            "Detected new dynamic layers in creator's __sync_meta"
        );
    }

    new_layers
}

/// Populate a late joiner's __sync_meta with existing dynamic layer entries
///
/// **Context**: New peer connects after dynamic layers already exist.
/// Node checks stored authorities to determine which layers the peer should discover.
/// Open-grant (no authorized_peers) → write entry. Explicit → check if peer is in list.
#[instrument(skip(state), fields(page_id = %state.page_id, peer_did = %peer_did))]
pub fn populate_dynamic_layers_for_late_joiner(state: &mut ScribeState, peer_did: &str) {
    use crate::layer_unit::find_matching_dynamic_schema;

    let our_permit = match state.our_permit.as_ref() {
        Some(p) => p,
        None => return,
    };
    if our_permit.dynamic_layer_schemas().is_empty() {
        return;
    }

    let page_id = state.page_id.clone();

    // Collect all sync_meta layers to scan
    let sync_meta_layers: Vec<(String, String)> = state
        .units
        .keys()
        .filter_map(|name| {
            if is_sync_meta_layer(name) {
                extract_peer_did(name).map(|did| (name.clone(), did.to_string()))
            } else {
                None
            }
        })
        .filter(|(_, did)| did != peer_did)
        .collect();

    let peer_entries = read_sync_meta_entries(state, peer_did);

    struct PendingWrite {
        layer_name: String,
    }
    let mut pending_writes: Vec<PendingWrite> = Vec::new();

    for (_meta_layer, creator_did) in &sync_meta_layers {
        let creator_entries = read_sync_meta_entries(state, creator_did);

        for entry in creator_entries {
            if is_protocol_layer(&entry.layer_name) {
                continue;
            }

            if find_matching_dynamic_schema(our_permit, &entry.layer_name, &page_id).is_none() {
                continue;
            }

            let already_has = peer_entries
                .iter()
                .any(|e| e.layer_name == entry.layer_name);
            if already_has {
                continue;
            }
            let already_pending = pending_writes
                .iter()
                .any(|w| w.layer_name == entry.layer_name);
            if already_pending {
                continue;
            }

            // Check stored authority to determine if this peer is authorized
            let full_name = format!("{}/{}", page_id, entry.layer_name);
            let is_authorized = if let Some(ref issuer) = state.permit_issuer {
                match issuer.get_authority_for_layer(&full_name) {
                    Ok(Some((_creator, _ver, token))) => {
                        if let Ok(auth) = gurkha::Permit::from_token(&token) {
                            auth.is_peer_authorized(peer_did)
                        } else {
                            false
                        }
                    }
                    _ => {
                        // No authority stored yet — could be open-grant where
                        // the layer hasn't been subscribed via LayerSubscribe yet.
                        // Allow for now; the actual permit issuance will check.
                        true
                    }
                }
            } else {
                true
            };

            if is_authorized {
                pending_writes.push(PendingWrite {
                    layer_name: entry.layer_name.clone(),
                });
            }
        }
    }

    // Apply all pending writes
    for write in pending_writes {
        write_sync_meta_entry(state, peer_did, &write.layer_name, false);
        info!(
            layer = %write.layer_name,
            peer_did = %peer_did,
            "Late joiner: wrote dynamic entry to __sync_meta"
        );
    }
}

/// Handle StoreLayerAuthority message (node-side)
///
/// **Context**: Node received authority from creator via LayerSubscribeAck.
/// **We do**: Store authority, apply layer data, fan out to authorized peers.
pub fn handle_store_layer_authority(
    state: &mut ScribeState,
    layer_name: &str,
    creator_did: &str,
    authority_token: &str,
    layer_data: &[u8],
) {
    let bare = crate::state::normalize_layer_name(layer_name, &state.page_id);

    // 1. Store authority permit via PermitIssuer
    if let Some(ref issuer) = state.permit_issuer {
        if let Err(e) = issuer.store_authority_permit(creator_did, layer_name, authority_token, 1) {
            tracing::error!(layer = %layer_name, error = %e, "Failed to store authority permit");
            return;
        }
        info!(layer = %layer_name, creator = %creator_did, "Stored authority permit from creator");
    }

    // 2. Create LayerUnit if needed and apply layer data
    if !state.units.contains_key(&bare) {
        let mut unit = LayerUnit::new_empty();
        unit.set_local_only(false);
        unit.is_dynamic = true;
        state.units.insert(bare.clone(), unit);
        loro_observer::setup_layer_observer(state, &bare);
        info!(layer = %bare, "Created dynamic layer from authority");
    }

    if !layer_data.is_empty() {
        if let Some(unit) = state.units.get(&bare) {
            if let Err(e) = unit.layer().apply(layer_data) {
                warn!(layer = %bare, error = %e, "Failed to apply layer data from authority");
            } else {
                unit.layer().commit();
                if let Some(unit) = state.units.get_mut(&bare) {
                    unit.mark_dirty();
                }
            }
        }
    }

    // 3. Parse authorized_peers from authority and fan out
    if let Ok(authority_permit) = gurkha::Permit::from_token(authority_token) {
        let authorized_peers = authority_permit.authorized_peers();

        handle_fan_out_layer_to_users(state, &bare, authorized_peers.as_deref());
    }

    // 4. Mark creator's __sync_meta entry as synced
    mark_entry_synced(state, creator_did, &bare);
}

/// Fan out a layer entry to authorized users' __sync_meta
///
/// **Context**: Node needs to notify authorized peers about a new dynamic layer.
/// **authorized_peers**: None = all subscribers, Some(list) = specific DIDs only.
pub fn handle_fan_out_layer_to_users(
    state: &mut ScribeState,
    layer_name: &str,
    authorized_peers: Option<&[String]>,
) {
    // Collect all subscriber DIDs
    let all_peer_dids: Vec<String> = {
        let mut dids = Vec::new();
        if let Ok(subs) = state.subscribers.read() {
            for ((did, _), _) in subs.iter() {
                if !dids.contains(did) {
                    dids.push(did.clone());
                }
            }
        }
        dids
    };

    let target_dids: Vec<&String> = match authorized_peers {
        None => all_peer_dids.iter().collect(),
        Some(list) => all_peer_dids
            .iter()
            .filter(|did| list.iter().any(|d| d == *did))
            .collect(),
    };

    for peer_did in target_dids {
        let peer_entries = read_sync_meta_entries(state, peer_did);
        let already_has = peer_entries.iter().any(|e| e.layer_name == layer_name);
        if !already_has {
            write_sync_meta_entry(state, peer_did, layer_name, false);
            info!(
                layer = %layer_name,
                peer_did = %peer_did,
                "Fan out: wrote dynamic entry to peer's __sync_meta"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::atomic::AtomicU64;
    use std::sync::{Arc, Mutex};

    use crate::storage::{
        LayerStorageRef, NullLayerStorage, NullPeerVectorStorage, NullPermitIssuer,
        PeerVectorStorageRef, PermitIssuerRef,
    };

    fn make_test_state() -> ScribeState {
        ScribeState {
            page_id: "page1".to_string(),
            units: HashMap::new(),
            subscribers: Arc::new(std::sync::RwLock::new(HashMap::new())),
            query_subscribers: HashMap::new(),
            layer_storage: Arc::new(NullLayerStorage) as LayerStorageRef,
            vector_storage: Arc::new(NullPeerVectorStorage) as PeerVectorStorageRef,
            peer_resolver: None,
            permit_issuer: Some(Arc::new(NullPermitIssuer) as PermitIssuerRef),
            sync_config: None,
            sync_event_tx: None,
            capture_tx: None,
            capture_seq: AtomicU64::new(0),
            page_update_subscribers: Arc::new(std::sync::RwLock::new(Vec::new())),
            pending_update_source: Arc::new(Mutex::new(None)),
            validation_handle: None,
            our_permit: None,
            our_did: "did:key:node".to_string(),
            node_script_shutdown: None,
            peer_role_cache: HashMap::new(),
        }
    }

    #[test]
    fn test_is_sync_meta_layer() {
        assert!(is_sync_meta_layer("__sync_meta:did:key:z6MkhaX"));
        assert!(!is_sync_meta_layer("messages"));
        assert!(!is_sync_meta_layer("__sync_other:foo"));
        assert!(!is_sync_meta_layer(
            "channels/did:key:alice/general/messages"
        ));
    }

    #[test]
    fn test_extract_peer_did() {
        assert_eq!(
            extract_peer_did("__sync_meta:did:key:z6MkhaX"),
            Some("did:key:z6MkhaX")
        );
        assert_eq!(extract_peer_did("messages"), None);
    }

    #[test]
    fn test_sync_meta_layer_name() {
        assert_eq!(
            sync_meta_layer_name("did:key:z6MkhaX"),
            "__sync_meta:did:key:z6MkhaX"
        );
    }

    #[tokio::test]
    async fn test_create_and_read_sync_meta() {
        let mut state = make_test_state();

        let layer = ensure_sync_meta_layer(&mut state, "did:key:alice");
        assert_eq!(layer, "__sync_meta:did:key:alice");
        assert!(state.units.contains_key(&layer));

        write_sync_meta_entry(&mut state, "did:key:alice", "messages", true);
        write_sync_meta_entry(&mut state, "did:key:alice", "reactions", false);

        let entries = read_sync_meta_entries(&state, "did:key:alice");
        assert_eq!(entries.len(), 2);

        let messages = entries.iter().find(|e| e.layer_name == "messages").unwrap();
        assert!(messages.synced);

        let reactions = entries
            .iter()
            .find(|e| e.layer_name == "reactions")
            .unwrap();
        assert!(!reactions.synced);
    }

    #[tokio::test]
    async fn test_read_unsynced_entries() {
        let mut state = make_test_state();
        ensure_sync_meta_layer(&mut state, "did:key:bob");

        write_sync_meta_entry(&mut state, "did:key:bob", "messages", true);
        write_sync_meta_entry(&mut state, "did:key:bob", "reactions", false);
        write_sync_meta_entry(&mut state, "did:key:bob", "presence", false);

        let unsynced = read_unsynced_entries(&state, "did:key:bob");
        assert_eq!(unsynced.len(), 2);
        assert!(unsynced.contains(&"reactions".to_string()));
        assert!(unsynced.contains(&"presence".to_string()));
    }

    #[tokio::test]
    async fn test_mark_entry_synced() {
        let mut state = make_test_state();
        ensure_sync_meta_layer(&mut state, "did:key:carol");

        write_sync_meta_entry(&mut state, "did:key:carol", "messages", false);
        assert!(!read_sync_meta_entries(&state, "did:key:carol")[0].synced);

        mark_entry_synced(&mut state, "did:key:carol", "messages");
        assert!(read_sync_meta_entries(&state, "did:key:carol")[0].synced);
    }

    #[test]
    fn test_is_protocol_layer() {
        assert!(is_protocol_layer("__sync_meta:did:key:alice"));
        assert!(!is_protocol_layer("messages"));
        assert!(!is_protocol_layer(
            "channels/did:key:alice/general/messages"
        ));
    }
}
