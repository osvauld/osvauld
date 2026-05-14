//! Typed CRDT operation handlers
//!
//! Handles list, map, and counter operations for Scribe actor.
//! **Broadcast**: Handled automatically by Loro observer after commit()
//!
//! **Path convention**:
//! - Empty path ("") = use layer_name as container name directly (Lua bindings)
//! - Non-empty path = use root map pattern with nested containers (template actions)

use loro::{TreeID, TreeParentId};
use tracing::{debug, info, instrument};

use crate::message::{FlatTreeNode, TreeNodeView};
use crate::{Result, ScribeError};
use domains::Sthithi;

use crate::layer_unit::{find_matching_dynamic_schema, LayerUnit};
use crate::loro_observer;
use crate::state::ScribeState;

// Typed Read Operations (for Lua bindings - avoids stale handles)

/// Handle ListGet - read a single item from a list container
pub fn handle_list_get(
    state: &ScribeState,
    layer_name: &str,
    index: usize,
) -> Result<Option<Sthithi>> {
    let layer = state
        .units
        .get(layer_name)
        .map(|unit| unit.layer())
        .ok_or_else(|| ScribeError::LayerNotFound(layer_name.to_string()))?;
    let list = layer.loro().get_list(layer_name);
    Ok(list
        .get(index)
        .map(|v| Sthithi::from(v.get_deep_value())))
}

/// Handle ListLength - get the length of a list container
pub fn handle_list_length(state: &ScribeState, layer_name: &str) -> Result<usize> {
    let layer = state
        .units
        .get(layer_name)
        .map(|unit| unit.layer())
        .ok_or_else(|| ScribeError::LayerNotFound(layer_name.to_string()))?;
    let list = layer.loro().get_list(layer_name);
    Ok(list.len())
}

/// Handle MapGet - read a single value from a map container
pub fn handle_map_get(
    state: &ScribeState,
    layer_name: &str,
    key: &str,
) -> Result<Option<Sthithi>> {
    let layer = state
        .units
        .get(layer_name)
        .map(|unit| unit.layer())
        .ok_or_else(|| ScribeError::LayerNotFound(layer_name.to_string()))?;
    let map = layer.loro().get_map(layer_name);
    Ok(map.get(key).map(|v| Sthithi::from(v.get_deep_value())))
}

/// Handle MapLength - get the number of entries in a map container
pub fn handle_map_length(state: &ScribeState, layer_name: &str) -> Result<usize> {
    let layer = state
        .units
        .get(layer_name)
        .map(|unit| unit.layer())
        .ok_or_else(|| ScribeError::LayerNotFound(layer_name.to_string()))?;
    let map = layer.loro().get_map(layer_name);
    Ok(map.len())
}

/// Handle MapKeys - get all keys from a map container
pub fn handle_map_keys(state: &ScribeState, layer_name: &str) -> Result<Vec<String>> {
    let layer = state
        .units
        .get(layer_name)
        .map(|unit| unit.layer())
        .ok_or_else(|| ScribeError::LayerNotFound(layer_name.to_string()))?;
    let map = layer.loro().get_map(layer_name);
    Ok(map.keys().map(|k| k.to_string()).collect())
}

// Typed CRDT Operation Handlers

/// Helper to ensure a layer exists, returns true if layer was newly created.
///
/// **Sync-target auto-subscription**: When a new syncable layer is created and we have
/// a sync_target (owner/viewer mode), the sync_target is auto-added as subscriber.
/// This ensures new dynamic layers propagate to the node via the normal CRDT observer
/// broadcast path — no separate __sync_meta discovery needed.
fn ensure_layer_exists(state: &mut ScribeState, layer_name: &str) -> bool {
    let is_new = !state.units.contains_key(layer_name);
    if is_new {
        let mut unit = LayerUnit::new_empty();
        unit.set_local_only(!state.should_sync_layer(layer_name));

        // Mark as dynamic if it matches a dynamic schema (node-side detection)
        let is_dynamic = if let Some(ref permit) = state.our_permit {
            find_matching_dynamic_schema(permit, layer_name, &state.page_id).is_some()
        } else {
            false
        };
        if is_dynamic {
            unit.is_dynamic = true;
        }

        state.units.insert(layer_name.to_string(), unit);

        // Auto-subscribe sync_target to new syncable layers
        auto_subscribe_sync_target(state, layer_name);

        debug!(layer = %layer_name, "Created new layer");

        // If layer matches a dynamic schema, write to __sync_meta for discovery
        // This ensures the __sync_meta flow triggers LayerSubscribe on the node
        if is_dynamic {
            use crate::sync::sync_meta;
            let our_did = state.our_did.clone();
            let meta_layer = sync_meta::sync_meta_layer_name(&our_did);
            if state.units.contains_key(&meta_layer) {
                sync_meta::write_sync_meta_entry(state, &our_did, layer_name, false);
                info!(layer = %layer_name, "Wrote __sync_meta entry for ensure_layer_exists (matches dynamic schema)");
            }
        }
    }

    is_new
}

/// Auto-subscribe the sync_target to a newly created syncable layer.
///
/// **Context**: Owner/viewer creates a new layer. The sync_target (node) needs to
/// receive data for it. By adding them as subscriber, the Loro observer broadcasts
/// will include them, and the PeerActor sends SyncOffer to the node.
pub(crate) fn auto_subscribe_sync_target(state: &ScribeState, layer_name: &str) {
    if !state.should_sync_layer(layer_name) {
        info!(layer = %layer_name, "auto_subscribe_sync_target: layer not syncable, skipping");
        return;
    }

    let sync_target_did = match state.sync_config.as_ref() {
        Some(cfg) => match cfg.sync_target.as_deref() {
            Some(did) => did.to_string(),
            None => {
                info!(layer = %layer_name, "auto_subscribe_sync_target: no sync_target in config");
                return;
            }
        },
        None => {
            info!(layer = %layer_name, "auto_subscribe_sync_target: no sync_config");
            return;
        }
    };

    let sub_count = state.subscribers.read().map(|s| s.len()).unwrap_or(0);
    let broadcast_tx = state.subscribers.read().ok().and_then(|subs| {
        subs.iter()
            .find(|((d, _), _)| d == &sync_target_did)
            .map(|(_, info)| info.broadcast_tx.clone())
    });

    if broadcast_tx.is_none() {
        info!(
            layer = %layer_name,
            sync_target = %sync_target_did,
            subscriber_count = sub_count,
            "auto_subscribe_sync_target: sync_target not found in subscribers"
        );
        // Log all subscriber DIDs for debugging
        if let Ok(subs) = state.subscribers.read() {
            for ((did, _), _) in subs.iter() {
                info!(layer = %layer_name, subscriber_did = %did, "  existing subscriber");
            }
        }
    }

    if let (Some(unit), Some(tx)) = (state.units.get(layer_name), broadcast_tx) {
        // Use device_id from global subscribers if available, otherwise empty string
        // for logical sync targets
        let device_id = state
            .subscribers
            .read()
            .ok()
            .and_then(|subs| {
                subs.iter()
                    .find(|((d, _), _)| d == &sync_target_did)
                    .map(|((_, dev), _)| dev.clone())
            })
            .unwrap_or_default();
        unit.add_subscriber(sync_target_did.clone(), device_id, true, tx);
        info!(layer = %layer_name, target = %sync_target_did, "Auto-subscribed sync_target to new layer");
    }
}

/// Handle list push operation (append)
///
/// **Broadcast**: Loro observer triggers broadcast after commit()
/// **Path**: Empty = use layer_name as container; non-empty = root map pattern
#[instrument(skip(state, item), fields(page_id = %state.page_id, layer = %layer_name, path = %path))]
pub async fn handle_list_push(
    state: &mut ScribeState,
    layer_name: &str,
    path: &str,
    item: Sthithi,
) -> Result<()> {
    info!("ListPush operation");

    // Ensure layer exists
    let is_new_layer = ensure_layer_exists(state, layer_name);

    let unit = state
        .units
        .get_mut(layer_name)
        .ok_or_else(|| ScribeError::LayerNotFound(layer_name.to_string()))?;

    if path.is_empty() {
        // Lua pattern: use layer_name as container name directly
        let list = unit.layer().loro().get_list(layer_name);
        let loro_value = loro::LoroValue::from(&item);
        list.push(loro_value)
            .map_err(|e| ScribeError::CrdtError(format!("ListPush: {}", e)))?;
        unit.layer().commit();
    } else {
        // Template pattern: use root map with nested path
        unit.layer()
            .list_push_sthithi(path, &item)
            .map_err(|e| ScribeError::CrdtError(format!("Layer: {}", e)))?;
    }

    unit.mark_dirty();

    // Set up observer if new layer (after first write)
    if is_new_layer {
        loro_observer::setup_layer_observer(state, layer_name);
    }

    Ok(())
}

/// Handle list insert operation
///
/// **Broadcast**: Loro observer triggers broadcast after commit()
/// **Path**: Empty = use layer_name as container; non-empty = root map pattern
#[instrument(skip(state, item), fields(page_id = %state.page_id, layer = %layer_name, path = %path))]
pub async fn handle_list_insert(
    state: &mut ScribeState,
    layer_name: &str,
    path: &str,
    index: usize,
    item: Sthithi,
) -> Result<()> {
    info!("ListInsert operation");

    let is_new_layer = ensure_layer_exists(state, layer_name);

    let unit = state
        .units
        .get_mut(layer_name)
        .ok_or_else(|| ScribeError::LayerNotFound(layer_name.to_string()))?;

    if path.is_empty() {
        // Lua pattern: use layer_name as container name directly
        let list = unit.layer().loro().get_list(layer_name);
        let loro_value = loro::LoroValue::from(&item);
        list.insert(index, loro_value)
            .map_err(|e| ScribeError::CrdtError(format!("ListInsert: {}", e)))?;
        unit.layer().commit();
    } else {
        // Template pattern: use root map with nested path
        unit.layer()
            .list_insert_sthithi(path, index, &item)
            .map_err(|e| ScribeError::CrdtError(format!("Layer: {}", e)))?;
    }

    unit.mark_dirty();

    if is_new_layer {
        loro_observer::setup_layer_observer(state, layer_name);
    }

    Ok(())
}

/// Handle list delete operation
///
/// **Broadcast**: Loro observer triggers broadcast after commit()
/// **Path**: Empty = use layer_name as container; non-empty = root map pattern
#[instrument(skip(state), fields(page_id = %state.page_id, layer = %layer_name, path = %path))]
pub async fn handle_list_delete(
    state: &mut ScribeState,
    layer_name: &str,
    path: &str,
    index: usize,
) -> Result<()> {
    info!("ListDelete operation");

    let is_new_layer = ensure_layer_exists(state, layer_name);

    let unit = state
        .units
        .get_mut(layer_name)
        .ok_or_else(|| ScribeError::LayerNotFound(layer_name.to_string()))?;

    if path.is_empty() {
        // Lua pattern: use layer_name as container name directly
        let list = unit.layer().loro().get_list(layer_name);
        list.delete(index, 1)
            .map_err(|e| ScribeError::CrdtError(format!("ListDelete: {}", e)))?;
        unit.layer().commit();
    } else {
        // Template pattern: use root map with nested path
        unit.layer()
            .list_delete(path, index)
            .map_err(|e| ScribeError::CrdtError(format!("Layer: {}", e)))?;
    }

    unit.mark_dirty();

    if is_new_layer {
        loro_observer::setup_layer_observer(state, layer_name);
    }

    Ok(())
}

/// Handle map insert operation
///
/// **Broadcast**: Loro observer triggers broadcast after commit()
/// **Path**: Empty = use layer_name as container; non-empty = root map pattern
#[instrument(skip(state, value), fields(page_id = %state.page_id, layer = %layer_name, path = %path))]
pub async fn handle_map_insert(
    state: &mut ScribeState,
    layer_name: &str,
    path: &str,
    key: &str,
    value: Sthithi,
) -> Result<()> {
    info!("MapInsert operation");

    let is_new_layer = ensure_layer_exists(state, layer_name);

    let unit = state
        .units
        .get_mut(layer_name)
        .ok_or_else(|| ScribeError::LayerNotFound(layer_name.to_string()))?;

    if path.is_empty() {
        // Lua pattern: use layer_name as container name directly
        let map = unit.layer().loro().get_map(layer_name);
        let loro_value = loro::LoroValue::from(&value);
        map.insert(key, loro_value)
            .map_err(|e| ScribeError::CrdtError(format!("MapInsert: {}", e)))?;
        unit.layer().commit();
    } else {
        // Template pattern: use root map with nested path
        unit.layer()
            .map_insert_sthithi(path, key, &value)
            .map_err(|e| ScribeError::CrdtError(format!("Layer: {}", e)))?;
    }

    unit.mark_dirty();

    if is_new_layer {
        loro_observer::setup_layer_observer(state, layer_name);
    }

    Ok(())
}

/// Handle map delete operation
///
/// **Broadcast**: Loro observer triggers broadcast after commit()
/// **Path**: Empty = use layer_name as container; non-empty = root map pattern
#[instrument(skip(state), fields(page_id = %state.page_id, layer = %layer_name, path = %path))]
pub async fn handle_map_delete(
    state: &mut ScribeState,
    layer_name: &str,
    path: &str,
    key: &str,
) -> Result<()> {
    info!("MapDelete operation");

    let is_new_layer = ensure_layer_exists(state, layer_name);

    let unit = state
        .units
        .get_mut(layer_name)
        .ok_or_else(|| ScribeError::LayerNotFound(layer_name.to_string()))?;

    if path.is_empty() {
        // Lua pattern: use layer_name as container name directly
        let map = unit.layer().loro().get_map(layer_name);
        map.delete(key).ok(); // Ignore if key doesn't exist
        unit.layer().commit();
    } else {
        // Template pattern: use root map with nested path
        unit.layer()
            .map_delete(path, key)
            .map_err(|e| ScribeError::CrdtError(format!("Layer: {}", e)))?;
    }

    unit.mark_dirty();

    if is_new_layer {
        loro_observer::setup_layer_observer(state, layer_name);
    }

    Ok(())
}

// Tree Operation Handlers (LoroTree-backed layers)

/// Parse a stringified `TreeID` into a Loro `TreeParentId`.
///
/// **Context**: Lua passes parent ids as strings (or nil for root).
/// **None** ⇒ `Root`, **Some(s)** ⇒ `Node(parsed)`.
fn parse_tree_parent(parent: Option<&str>) -> std::result::Result<TreeParentId, String> {
    match parent {
        None => Ok(TreeParentId::Root),
        Some(s) => TreeID::try_from(s)
            .map(TreeParentId::Node)
            .map_err(|e| format!("Invalid TreeID '{}': {}", s, e)),
    }
}

/// Handle TreeCreate — create a new node under `parent` at optional `index`.
///
/// **Container materialisation**: for each name in `text_keys`, we insert a
/// fresh `LoroText` container at `meta[name]` *in the same commit* as the
/// node creation. This makes the creator solely responsible for container
/// instantiation; other peers receive the wired-up container via sync and
/// never call `insert_container` themselves, so they can't race on the
/// `LoroMap` LWW-per-key resolution. If a string also appears in `props`
/// under one of these keys, we seed the LoroText with it instead of writing
/// a plain string (avoids overwriting the container we just made).
#[instrument(skip(state, props, text_keys), fields(page_id = %state.page_id, layer = %layer_name))]
pub async fn handle_tree_create(
    state: &mut ScribeState,
    layer_name: &str,
    parent: Option<String>,
    index: Option<usize>,
    props: Sthithi,
    text_keys: Vec<String>,
) -> std::result::Result<String, String> {
    info!("TreeCreate operation");

    let is_new_layer = ensure_layer_exists(state, layer_name);

    let unit = state
        .units
        .get_mut(layer_name)
        .ok_or_else(|| format!("layer not found: {}", layer_name))?;

    let layer = unit.layer().clone();
    let tree = layer.loro().get_tree(layer_name);

    let parent_id = parse_tree_parent(parent.as_deref())?;
    let node_id = match index {
        Some(idx) => tree
            .create_at(parent_id, idx)
            .map_err(|e| format!("tree.create_at: {}", e))?,
        None => tree
            .create(parent_id)
            .map_err(|e| format!("tree.create: {}", e))?,
    };

    // Plain props first (skipping any keys that will become LoroText
    // containers — apply_node_props would otherwise insert them as plain
    // strings which the container creation below would overwrite).
    apply_node_props_filtered(&layer, &tree, node_id, &props, &text_keys)?;

    // Materialise nested LoroText containers, seeding each from a string in
    // props if present. After this commit broadcasts, every peer sees the
    // node + meta map + LoroText containers as one unit — no lazy creation,
    // no race window.
    if !text_keys.is_empty() {
        let meta = tree
            .get_meta(node_id)
            .map_err(|e| format!("get_meta: {}", e))?;
        for key in &text_keys {
            let seed = extract_text_seed(&props, key);
            let text = meta
                .insert_container(key.as_str(), loro::LoroText::new())
                .map_err(|e| format!("meta.insert_container({}): {}", key, e))?;
            if !seed.is_empty() {
                text.insert(0, &seed)
                    .map_err(|e| format!("text seed insert: {}", e))?;
            }
        }
    }

    layer.commit();
    unit.mark_dirty();

    if is_new_layer {
        loro_observer::setup_layer_observer(state, layer_name);
    }

    Ok(node_id.to_string())
}

/// Like `apply_node_props`, but skips any keys named in `skip_keys`. Used by
/// `handle_tree_create` so a `text=""` prop doesn't get written as a plain
/// string before we materialise the LoroText container at the same key.
fn apply_node_props_filtered(
    layer: &domains::Layer,
    tree: &loro::LoroTree,
    node_id: TreeID,
    props: &Sthithi,
    skip_keys: &[String],
) -> std::result::Result<(), String> {
    let entries = match props {
        Sthithi::Map(entries) => entries,
        Sthithi::Null => return Ok(()),
        other => return Err(format!("tree props must be a map, got {:?}", other)),
    };
    let meta = tree
        .get_meta(node_id)
        .map_err(|e| format!("get_meta: {}", e))?;
    for (key, value) in entries {
        if skip_keys.iter().any(|k| k == key) {
            continue;
        }
        meta.insert(key, loro::LoroValue::from(value))
            .map_err(|e| format!("meta.insert({}): {}", key, e))?;
    }
    layer.commit();
    Ok(())
}

/// If `props` carries a string at `key`, return it as the seed content for a
/// LoroText container; otherwise return an empty string.
fn extract_text_seed(props: &Sthithi, key: &str) -> String {
    match props {
        Sthithi::Map(entries) => entries
            .iter()
            .find(|(k, _)| k == key)
            .and_then(|(_, v)| match v {
                Sthithi::Str(s) => Some(s.clone()),
                _ => None,
            })
            .unwrap_or_default(),
        _ => String::new(),
    }
}

/// Handle TreeMove — relocate a node to a new parent/index.
#[instrument(skip(state), fields(page_id = %state.page_id, layer = %layer_name, node = %node_id))]
pub async fn handle_tree_move(
    state: &mut ScribeState,
    layer_name: &str,
    node_id: &str,
    parent: Option<String>,
    index: Option<usize>,
) -> std::result::Result<(), String> {
    info!("TreeMove operation");

    let unit = state
        .units
        .get_mut(layer_name)
        .ok_or_else(|| format!("layer not found: {}", layer_name))?;
    let layer = unit.layer().clone();
    let tree = layer.loro().get_tree(layer_name);

    let target = TreeID::try_from(node_id).map_err(|e| format!("invalid node_id: {}", e))?;
    let parent_id = parse_tree_parent(parent.as_deref())?;

    match index {
        Some(idx) => tree
            .mov_to(target, parent_id, idx)
            .map_err(|e| format!("tree.mov_to: {}", e))?,
        None => tree
            .mov(target, parent_id)
            .map_err(|e| format!("tree.mov: {}", e))?,
    }

    layer.commit();
    unit.mark_dirty();
    Ok(())
}

/// Handle TreeDelete — delete a node (and descendants).
#[instrument(skip(state), fields(page_id = %state.page_id, layer = %layer_name, node = %node_id))]
pub async fn handle_tree_delete(
    state: &mut ScribeState,
    layer_name: &str,
    node_id: &str,
) -> std::result::Result<(), String> {
    info!("TreeDelete operation");

    let unit = state
        .units
        .get_mut(layer_name)
        .ok_or_else(|| format!("layer not found: {}", layer_name))?;
    let layer = unit.layer().clone();
    let tree = layer.loro().get_tree(layer_name);

    let target = TreeID::try_from(node_id).map_err(|e| format!("invalid node_id: {}", e))?;
    tree.delete(target)
        .map_err(|e| format!("tree.delete: {}", e))?;

    layer.commit();
    unit.mark_dirty();
    Ok(())
}

/// Handle TreeSetProp — set a single key on a node's meta map.
#[instrument(skip(state, value), fields(page_id = %state.page_id, layer = %layer_name, node = %node_id, key = %key))]
pub async fn handle_tree_set_prop(
    state: &mut ScribeState,
    layer_name: &str,
    node_id: &str,
    key: &str,
    value: Sthithi,
) -> std::result::Result<(), String> {
    info!("TreeSetProp operation");

    let unit = state
        .units
        .get_mut(layer_name)
        .ok_or_else(|| format!("layer not found: {}", layer_name))?;
    let layer = unit.layer().clone();
    let tree = layer.loro().get_tree(layer_name);

    let target = TreeID::try_from(node_id).map_err(|e| format!("invalid node_id: {}", e))?;
    let meta = tree
        .get_meta(target)
        .map_err(|e| format!("get_meta: {}", e))?;
    meta.insert(key, loro::LoroValue::from(&value))
        .map_err(|e| format!("meta.insert: {}", e))?;

    layer.commit();
    unit.mark_dirty();
    Ok(())
}

/// Handle TreeGetNode — read props + immediate children for one node.
pub fn handle_tree_get_node(
    state: &ScribeState,
    layer_name: &str,
    node_id: &str,
) -> std::result::Result<Option<TreeNodeView>, String> {
    let layer = match state.units.get(layer_name).map(|u| u.layer()) {
        Some(l) => l,
        None => return Ok(None),
    };
    let tree = layer.loro().get_tree(layer_name);

    let target = TreeID::try_from(node_id).map_err(|e| format!("invalid node_id: {}", e))?;
    if !tree.contains(target) {
        return Ok(None);
    }

    let parent = tree.parent(target).and_then(|p| match p {
        TreeParentId::Node(id) => Some(id.to_string()),
        _ => None,
    });
    let children = tree
        .children(TreeParentId::Node(target))
        .unwrap_or_default()
        .into_iter()
        .map(|id| id.to_string())
        .collect();
    let meta = tree
        .get_meta(target)
        .map_err(|e| format!("get_meta: {}", e))?;
    let props = Sthithi::from(meta.get_deep_value());

    Ok(Some(TreeNodeView {
        id: node_id.to_string(),
        parent,
        children,
        props,
    }))
}

// LoroText handlers
//
// Positions are *unicode codepoint* indices (Loro's default for
// `LoroText::insert` / `delete`). UI layers that work in UTF-8 byte offsets
// (e.g. Slint's `cursor-position-byte-offset`) must convert before calling.

/// Handle TextInsert — insert a string at the given codepoint position.
#[instrument(skip(state, content), fields(page_id = %state.page_id, layer = %layer_name, pos, len = content.chars().count()))]
pub async fn handle_text_insert(
    state: &mut ScribeState,
    layer_name: &str,
    pos: usize,
    content: &str,
) -> std::result::Result<(), String> {
    info!("TextInsert operation");

    let is_new_layer = ensure_layer_exists(state, layer_name);

    let unit = state
        .units
        .get_mut(layer_name)
        .ok_or_else(|| format!("layer not found: {}", layer_name))?;
    let layer = unit.layer().clone();
    let text = layer.loro().get_text(layer_name);

    text.insert(pos, content)
        .map_err(|e| format!("text.insert: {}", e))?;

    layer.commit();
    unit.mark_dirty();

    if is_new_layer {
        loro_observer::setup_layer_observer(state, layer_name);
    }
    Ok(())
}

/// Handle TextDelete — delete `len` codepoints starting at `pos`.
#[instrument(skip(state), fields(page_id = %state.page_id, layer = %layer_name, pos, len))]
pub async fn handle_text_delete(
    state: &mut ScribeState,
    layer_name: &str,
    pos: usize,
    len: usize,
) -> std::result::Result<(), String> {
    info!("TextDelete operation");

    let unit = state
        .units
        .get_mut(layer_name)
        .ok_or_else(|| format!("layer not found: {}", layer_name))?;
    let layer = unit.layer().clone();
    let text = layer.loro().get_text(layer_name);

    text.delete(pos, len)
        .map_err(|e| format!("text.delete: {}", e))?;

    layer.commit();
    unit.mark_dirty();
    Ok(())
}

/// Handle TextSnapshot — read the current full text content.
pub fn handle_text_snapshot(
    state: &ScribeState,
    layer_name: &str,
) -> std::result::Result<String, String> {
    let layer = match state.units.get(layer_name).map(|u| u.layer()) {
        Some(l) => l,
        None => return Ok(String::new()),
    };
    Ok(layer.loro().get_text(layer_name).to_string())
}

/// Handle TextLength — return the text length in unicode codepoints.
pub fn handle_text_length(
    state: &ScribeState,
    layer_name: &str,
) -> std::result::Result<usize, String> {
    let layer = match state.units.get(layer_name).map(|u| u.layer()) {
        Some(l) => l,
        None => return Ok(0),
    };
    Ok(layer.loro().get_text(layer_name).len_unicode())
}

// Per-node nested-LoroText helpers.
//
// **Why this exists**: a block in the editor is a tree node. Its `text` was a
// plain string in the node's meta map — fine for single-author edits, but two
// peers typing in the same block clobber each other (LWW per `set_prop`).
// Stashing a nested `LoroText` container under the meta key gives us
// char-level CRDT merge per block, while keeping the rest of the meta map
// (kind, language, collapsed, …) as plain values.

/// Get the nested LoroText at `meta[key]` of a tree node, creating it on
/// first use. Returns the loro::LoroText handle; the caller commits.
fn ensure_node_text(
    tree: &loro::LoroTree,
    node_id: TreeID,
    key: &str,
) -> std::result::Result<loro::LoroText, String> {
    let meta = tree
        .get_meta(node_id)
        .map_err(|e| format!("get_meta: {}", e))?;
    match meta.get(key) {
        Some(loro::ValueOrContainer::Container(loro::Container::Text(t))) => Ok(t),
        Some(loro::ValueOrContainer::Container(other)) => Err(format!(
            "node meta key '{}' is a {:?} container, expected Text",
            key, other
        )),
        Some(loro::ValueOrContainer::Value(_)) => {
            // Plain string (or other LWW value) lived here previously. Replace
            // with a LoroText seeded from the existing string so we don't drop
            // user content on the floor when an app upgrades the schema.
            let prior = match meta.get(key) {
                Some(loro::ValueOrContainer::Value(loro::LoroValue::String(s))) => {
                    String::from(s.as_str())
                }
                _ => String::new(),
            };
            let text = meta
                .insert_container(key, loro::LoroText::new())
                .map_err(|e| format!("meta.insert_container({}): {}", key, e))?;
            if !prior.is_empty() {
                text.insert(0, &prior)
                    .map_err(|e| format!("text.insert (seed): {}", e))?;
            }
            Ok(text)
        }
        None => meta
            .insert_container(key, loro::LoroText::new())
            .map_err(|e| format!("meta.insert_container({}): {}", key, e)),
    }
}

/// Read-only access to the nested LoroText. Returns Ok(None) when the key
/// hasn't been created yet (so callers get "" / 0 rather than an error).
fn lookup_node_text(
    tree: &loro::LoroTree,
    node_id: TreeID,
    key: &str,
) -> std::result::Result<Option<loro::LoroText>, String> {
    let meta = tree
        .get_meta(node_id)
        .map_err(|e| format!("get_meta: {}", e))?;
    match meta.get(key) {
        Some(loro::ValueOrContainer::Container(loro::Container::Text(t))) => Ok(Some(t)),
        Some(loro::ValueOrContainer::Container(_)) => Err(format!(
            "node meta key '{}' is a non-text container",
            key
        )),
        // Plain string (legacy) — surface it as the snapshot too, so reads
        // before the first write don't pretend the block is empty.
        Some(loro::ValueOrContainer::Value(_)) | None => Ok(None),
    }
}

/// Read the legacy plain-string value at `meta[key]`, if any. Used as a
/// fallback for snapshot/length when the nested LoroText hasn't been created
/// yet but the node still carries an old-style string.
fn lookup_node_text_legacy_string(
    tree: &loro::LoroTree,
    node_id: TreeID,
    key: &str,
) -> std::result::Result<String, String> {
    let meta = tree
        .get_meta(node_id)
        .map_err(|e| format!("get_meta: {}", e))?;
    match meta.get(key) {
        Some(loro::ValueOrContainer::Value(loro::LoroValue::String(s))) => {
            Ok(String::from(s.as_str()))
        }
        _ => Ok(String::new()),
    }
}

/// Handle TreeTextInsert — insert into the nested LoroText at `meta[key]`.
#[instrument(skip(state, content), fields(page_id = %state.page_id, layer = %layer_name, node = %node_id, key = %key, pos))]
pub async fn handle_tree_text_insert(
    state: &mut ScribeState,
    layer_name: &str,
    node_id: &str,
    key: &str,
    pos: usize,
    content: &str,
) -> std::result::Result<(), String> {
    info!("TreeTextInsert operation");
    let unit = state
        .units
        .get_mut(layer_name)
        .ok_or_else(|| format!("layer not found: {}", layer_name))?;
    let layer = unit.layer().clone();
    let tree = layer.loro().get_tree(layer_name);
    let target = TreeID::try_from(node_id).map_err(|e| format!("invalid node_id: {}", e))?;

    let text = ensure_node_text(&tree, target, key)?;
    text.insert(pos, content)
        .map_err(|e| format!("text.insert: {}", e))?;

    layer.commit();
    unit.mark_dirty();
    Ok(())
}

/// Handle TreeTextDelete — delete from the nested LoroText at `meta[key]`.
#[instrument(skip(state), fields(page_id = %state.page_id, layer = %layer_name, node = %node_id, key = %key, pos, len))]
pub async fn handle_tree_text_delete(
    state: &mut ScribeState,
    layer_name: &str,
    node_id: &str,
    key: &str,
    pos: usize,
    len: usize,
) -> std::result::Result<(), String> {
    info!("TreeTextDelete operation");
    let unit = state
        .units
        .get_mut(layer_name)
        .ok_or_else(|| format!("layer not found: {}", layer_name))?;
    let layer = unit.layer().clone();
    let tree = layer.loro().get_tree(layer_name);
    let target = TreeID::try_from(node_id).map_err(|e| format!("invalid node_id: {}", e))?;

    // Delete-before-create is silently ignored — the user has nothing to
    // delete. Same shape as how the top-level TextDelete reacts to an empty
    // layer.
    let text = ensure_node_text(&tree, target, key)?;
    if len == 0 {
        return Ok(());
    }
    text.delete(pos, len)
        .map_err(|e| format!("text.delete: {}", e))?;

    layer.commit();
    unit.mark_dirty();
    Ok(())
}

/// Handle TreeTextSnapshot — read the current text at `meta[key]`.
pub fn handle_tree_text_snapshot(
    state: &ScribeState,
    layer_name: &str,
    node_id: &str,
    key: &str,
) -> std::result::Result<String, String> {
    let layer = match state.units.get(layer_name).map(|u| u.layer()) {
        Some(l) => l,
        None => return Ok(String::new()),
    };
    let tree = layer.loro().get_tree(layer_name);
    let target = TreeID::try_from(node_id).map_err(|e| format!("invalid node_id: {}", e))?;
    if !tree.contains(target) {
        return Ok(String::new());
    }
    if let Some(text) = lookup_node_text(&tree, target, key)? {
        return Ok(text.to_string());
    }
    lookup_node_text_legacy_string(&tree, target, key)
}

/// Handle TreeTextLength — codepoint length of the text at `meta[key]`.
pub fn handle_tree_text_length(
    state: &ScribeState,
    layer_name: &str,
    node_id: &str,
    key: &str,
) -> std::result::Result<usize, String> {
    let layer = match state.units.get(layer_name).map(|u| u.layer()) {
        Some(l) => l,
        None => return Ok(0),
    };
    let tree = layer.loro().get_tree(layer_name);
    let target = TreeID::try_from(node_id).map_err(|e| format!("invalid node_id: {}", e))?;
    if !tree.contains(target) {
        return Ok(0);
    }
    if let Some(text) = lookup_node_text(&tree, target, key)? {
        return Ok(text.len_unicode());
    }
    Ok(lookup_node_text_legacy_string(&tree, target, key)?
        .chars()
        .count())
}

/// Handle TreeWalk — depth-first flattened traversal from root.
pub fn handle_tree_walk(
    state: &ScribeState,
    layer_name: &str,
) -> std::result::Result<Vec<FlatTreeNode>, String> {
    let layer = match state.units.get(layer_name).map(|u| u.layer()) {
        Some(l) => l,
        None => return Ok(Vec::new()),
    };
    let tree = layer.loro().get_tree(layer_name);

    let mut out = Vec::new();
    walk_dfs(&tree, TreeParentId::Root, None, 0, &mut out);
    Ok(out)
}

fn walk_dfs(
    tree: &loro::LoroTree,
    parent: TreeParentId,
    parent_str: Option<String>,
    depth: usize,
    out: &mut Vec<FlatTreeNode>,
) {
    let children = match tree.children(parent) {
        Some(ids) => ids,
        None => return,
    };
    for (index, id) in children.into_iter().enumerate() {
        let props = tree
            .get_meta(id)
            .ok()
            .map(|m| Sthithi::from(m.get_deep_value()))
            .unwrap_or(Sthithi::Null);
        out.push(FlatTreeNode {
            id: id.to_string(),
            parent: parent_str.clone(),
            depth,
            index,
            props,
        });
        walk_dfs(
            tree,
            TreeParentId::Node(id),
            Some(id.to_string()),
            depth + 1,
            out,
        );
    }
}

/// Handle counter increment operation
///
/// **Broadcast**: Loro observer triggers broadcast after commit()
#[instrument(skip(state), fields(page_id = %state.page_id, layer = %layer_name, path = %path))]
pub async fn handle_counter_inc(
    state: &mut ScribeState,
    layer_name: &str,
    path: &str,
    amount: i64,
) -> Result<()> {
    info!("CounterInc operation");

    let is_new_layer = ensure_layer_exists(state, layer_name);

    let unit = state
        .units
        .get_mut(layer_name)
        .ok_or_else(|| ScribeError::LayerNotFound(layer_name.to_string()))?;

    unit.layer()
        .counter_inc(path, amount)
        .map_err(|e| ScribeError::CrdtError(format!("Layer: {}", e)))?;

    unit.mark_dirty();

    if is_new_layer {
        loro_observer::setup_layer_observer(state, layer_name);
    }

    Ok(())
}
