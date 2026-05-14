//! ActorScribeHandle — concrete Scribe actor access for Lua bindings
//!
//! **Pattern**: Wraps `ActorRef<ScribeMessage>` with a synchronous blocking bridge.
//! All methods are synchronous because the Lua runtime runs on an OS thread
//! without a tokio runtime. The async-to-sync bridge is handled internally.
//!
//! **Production**: `ActorScribeHandle` wraps `ActorRef<ScribeMessage>` with `block_in_place()`

use std::collections::HashMap;
use std::sync::Arc;

use domains::Sthithi;
use ractor::ActorRef;
use scribe::{FlatTreeNode, ScribeMessage, TreeNodeView};
use tokio::sync::oneshot;

/// Helper: block_on an async oneshot that returns `Result<T, ScribeError>`,
/// flattening both channel errors and scribe errors to `String`.
fn rpc<T, E: std::fmt::Display>(rx: oneshot::Receiver<Result<T, E>>) -> Result<T, String> {
    let inner = match tokio::runtime::Handle::try_current() {
        Ok(handle) => tokio::task::block_in_place(|| handle.block_on(rx)),
        Err(_) => futures::executor::block_on(rx),
    };
    inner
        .map_err(|e| format!("Channel error: {}", e))?
        .map_err(|e| format!("{}", e))
}

/// Helper: block_on an async oneshot that returns `T` directly (no inner Result)
fn rpc_direct<T>(rx: oneshot::Receiver<T>) -> Result<T, String> {
    let result = match tokio::runtime::Handle::try_current() {
        Ok(handle) => tokio::task::block_in_place(|| handle.block_on(rx)),
        Err(_) => futures::executor::block_on(rx),
    };
    result.map_err(|e| format!("Channel error: {}", e))
}

/// Concrete Scribe handle wrapping an `ActorRef<ScribeMessage>`
///
/// All operations delegate to the Scribe actor via cast/call with `block_in_place`.
/// Identical behavior to the previous direct `scribe_ref` usage in bindings.
pub struct ActorScribeHandle {
    scribe_ref: ActorRef<ScribeMessage>,
}

impl ActorScribeHandle {
    pub fn new(scribe_ref: ActorRef<ScribeMessage>) -> Arc<Self> {
        Arc::new(Self { scribe_ref })
    }

    /// Get the underlying ActorRef (for operations not covered by the methods,
    /// like SubscribeToPageUpdates)
    pub fn actor_ref(&self) -> &ActorRef<ScribeMessage> {
        &self.scribe_ref
    }

    // -- Layer lifecycle --

    /// Ensure a list layer exists (creates if needed)
    pub fn ensure_list(&self, layer_name: &str) -> Result<(), String> {
        let (tx, rx) = oneshot::channel();
        self.scribe_ref
            .cast(ScribeMessage::EnsureLoroList {
                layer_name: layer_name.to_string(),
                reply: tx,
            })
            .map_err(|e| format!("Failed to ensure list: {}", e))?;
        rpc(rx)
    }

    /// Ensure a map layer exists (creates if needed)
    pub fn ensure_map(&self, layer_name: &str) -> Result<(), String> {
        let (tx, rx) = oneshot::channel();
        self.scribe_ref
            .cast(ScribeMessage::EnsureLoroMap {
                layer_name: layer_name.to_string(),
                reply: tx,
            })
            .map_err(|e| format!("Failed to ensure map: {}", e))?;
        rpc(rx)
    }

    /// Create an empty derived layer (for derivation engine)
    pub fn create_derived_layer(&self, target_layer: &str) -> Result<(), String> {
        self.scribe_ref
            .cast(ScribeMessage::CreateDerivedLayer {
                target_layer: target_layer.to_string(),
            })
            .map_err(|e| format!("Failed to create derived layer: {}", e))
    }

    // -- List operations --

    /// Push item to end of list
    pub fn list_push(&self, layer_name: &str, path: &str, item: Sthithi) -> Result<(), String> {
        self.scribe_ref
            .cast(ScribeMessage::ListPush {
                layer_name: layer_name.to_string(),
                path: path.to_string(),
                item,
            })
            .map_err(|e| format!("Failed to push: {}", e))
    }

    /// Insert item at index
    pub fn list_insert(
        &self,
        layer_name: &str,
        path: &str,
        index: usize,
        item: Sthithi,
    ) -> Result<(), String> {
        self.scribe_ref
            .cast(ScribeMessage::ListInsert {
                layer_name: layer_name.to_string(),
                path: path.to_string(),
                index,
                item,
            })
            .map_err(|e| format!("Failed to insert: {}", e))
    }

    /// Delete item at index
    pub fn list_delete(&self, layer_name: &str, path: &str, index: usize) -> Result<(), String> {
        self.scribe_ref
            .cast(ScribeMessage::ListDelete {
                layer_name: layer_name.to_string(),
                path: path.to_string(),
                index,
            })
            .map_err(|e| format!("Failed to delete: {}", e))
    }

    /// Get item at index
    pub fn list_get(&self, layer_name: &str, index: usize) -> Result<Option<Sthithi>, String> {
        let (tx, rx) = oneshot::channel();
        self.scribe_ref
            .cast(ScribeMessage::ListGet {
                layer_name: layer_name.to_string(),
                index,
                reply: tx,
            })
            .map_err(|e| format!("Failed to get: {}", e))?;
        rpc(rx)
    }

    /// Get list length
    pub fn list_length(&self, layer_name: &str) -> Result<usize, String> {
        let (tx, rx) = oneshot::channel();
        self.scribe_ref
            .cast(ScribeMessage::ListLength {
                layer_name: layer_name.to_string(),
                reply: tx,
            })
            .map_err(|e| format!("Failed to get length: {}", e))?;
        rpc(rx)
    }

    // -- Map operations --

    /// Insert or update a key-value pair
    pub fn map_insert(
        &self,
        layer_name: &str,
        path: &str,
        key: &str,
        value: Sthithi,
    ) -> Result<(), String> {
        self.scribe_ref
            .cast(ScribeMessage::MapInsert {
                layer_name: layer_name.to_string(),
                path: path.to_string(),
                key: key.to_string(),
                value,
            })
            .map_err(|e| format!("Failed to set: {}", e))
    }

    /// Get value by key
    pub fn map_get(&self, layer_name: &str, key: &str) -> Result<Option<Sthithi>, String> {
        let (tx, rx) = oneshot::channel();
        self.scribe_ref
            .cast(ScribeMessage::MapGet {
                layer_name: layer_name.to_string(),
                key: key.to_string(),
                reply: tx,
            })
            .map_err(|e| format!("Failed to get: {}", e))?;
        rpc(rx)
    }

    /// Delete key from map
    pub fn map_delete(&self, layer_name: &str, path: &str, key: &str) -> Result<(), String> {
        self.scribe_ref
            .cast(ScribeMessage::MapDelete {
                layer_name: layer_name.to_string(),
                path: path.to_string(),
                key: key.to_string(),
            })
            .map_err(|e| format!("Failed to delete: {}", e))
    }

    /// Get map length
    pub fn map_length(&self, layer_name: &str) -> Result<usize, String> {
        let (tx, rx) = oneshot::channel();
        self.scribe_ref
            .cast(ScribeMessage::MapLength {
                layer_name: layer_name.to_string(),
                reply: tx,
            })
            .map_err(|e| format!("Failed to get length: {}", e))?;
        rpc(rx)
    }

    /// Get all keys from map
    pub fn map_keys(&self, layer_name: &str) -> Result<Vec<String>, String> {
        let (tx, rx) = oneshot::channel();
        self.scribe_ref
            .cast(ScribeMessage::MapKeys {
                layer_name: layer_name.to_string(),
                reply: tx,
            })
            .map_err(|e| format!("Failed to get keys: {}", e))?;
        rpc(rx)
    }

    // -- Tree operations --

    /// Ensure a tree layer exists (creates if needed)
    pub fn ensure_tree(&self, layer_name: &str) -> Result<(), String> {
        let (tx, rx) = oneshot::channel();
        self.scribe_ref
            .cast(ScribeMessage::EnsureLoroTree {
                layer_name: layer_name.to_string(),
                reply: tx,
            })
            .map_err(|e| format!("Failed to ensure tree: {}", e))?;
        rpc(rx)
    }

    /// Create a new tree node. `parent` = None for root.
    ///
    /// `text_keys` lists meta keys that should be materialised as nested
    /// LoroText containers in the same commit as the node creation. This
    /// makes the creator solely responsible for container instantiation, so
    /// peers who later edit the same field can't race on `meta.insert_container`.
    /// Pass an empty Vec for plain prop-only nodes.
    pub fn tree_create(
        &self,
        layer_name: &str,
        parent: Option<String>,
        index: Option<usize>,
        props: Sthithi,
        text_keys: Vec<String>,
    ) -> Result<String, String> {
        let (tx, rx) = oneshot::channel();
        self.scribe_ref
            .cast(ScribeMessage::TreeCreate {
                layer_name: layer_name.to_string(),
                parent,
                index,
                props,
                text_keys,
                reply: tx,
            })
            .map_err(|e| format!("Failed to create tree node: {}", e))?;
        rpc(rx)
    }

    /// Move a tree node to a new parent/index.
    pub fn tree_move(
        &self,
        layer_name: &str,
        node_id: &str,
        parent: Option<String>,
        index: Option<usize>,
    ) -> Result<(), String> {
        let (tx, rx) = oneshot::channel();
        self.scribe_ref
            .cast(ScribeMessage::TreeMove {
                layer_name: layer_name.to_string(),
                node_id: node_id.to_string(),
                parent,
                index,
                reply: tx,
            })
            .map_err(|e| format!("Failed to move tree node: {}", e))?;
        rpc(rx)
    }

    /// Delete a tree node.
    pub fn tree_delete(&self, layer_name: &str, node_id: &str) -> Result<(), String> {
        let (tx, rx) = oneshot::channel();
        self.scribe_ref
            .cast(ScribeMessage::TreeDelete {
                layer_name: layer_name.to_string(),
                node_id: node_id.to_string(),
                reply: tx,
            })
            .map_err(|e| format!("Failed to delete tree node: {}", e))?;
        rpc(rx)
    }

    /// Set a single property on a tree node's meta map.
    pub fn tree_set_prop(
        &self,
        layer_name: &str,
        node_id: &str,
        key: &str,
        value: Sthithi,
    ) -> Result<(), String> {
        let (tx, rx) = oneshot::channel();
        self.scribe_ref
            .cast(ScribeMessage::TreeSetProp {
                layer_name: layer_name.to_string(),
                node_id: node_id.to_string(),
                key: key.to_string(),
                value,
                reply: tx,
            })
            .map_err(|e| format!("Failed to set tree prop: {}", e))?;
        rpc(rx)
    }

    /// Read a tree node's view (props + immediate children).
    pub fn tree_get_node(
        &self,
        layer_name: &str,
        node_id: &str,
    ) -> Result<Option<TreeNodeView>, String> {
        let (tx, rx) = oneshot::channel();
        self.scribe_ref
            .cast(ScribeMessage::TreeGetNode {
                layer_name: layer_name.to_string(),
                node_id: node_id.to_string(),
                reply: tx,
            })
            .map_err(|e| format!("Failed to get tree node: {}", e))?;
        rpc(rx)
    }

    /// Depth-first flattened walk from root.
    pub fn tree_walk(&self, layer_name: &str) -> Result<Vec<FlatTreeNode>, String> {
        let (tx, rx) = oneshot::channel();
        self.scribe_ref
            .cast(ScribeMessage::TreeWalk {
                layer_name: layer_name.to_string(),
                reply: tx,
            })
            .map_err(|e| format!("Failed to walk tree: {}", e))?;
        rpc(rx)
    }

    // -- Text operations --

    /// Ensure a text layer exists (creates if needed).
    pub fn ensure_text(&self, layer_name: &str) -> Result<(), String> {
        let (tx, rx) = oneshot::channel();
        self.scribe_ref
            .cast(ScribeMessage::EnsureLoroText {
                layer_name: layer_name.to_string(),
                reply: tx,
            })
            .map_err(|e| format!("Failed to ensure text: {}", e))?;
        rpc(rx).map(|_| ()).map_err(|e: String| e)
    }

    /// Insert a string at the given codepoint position.
    pub fn text_insert(
        &self,
        layer_name: &str,
        pos: usize,
        content: &str,
    ) -> Result<(), String> {
        let (tx, rx) = oneshot::channel();
        self.scribe_ref
            .cast(ScribeMessage::TextInsert {
                layer_name: layer_name.to_string(),
                pos,
                content: content.to_string(),
                reply: tx,
            })
            .map_err(|e| format!("Failed to insert text: {}", e))?;
        rpc(rx)
    }

    /// Delete `len` codepoints starting at `pos`.
    pub fn text_delete(
        &self,
        layer_name: &str,
        pos: usize,
        len: usize,
    ) -> Result<(), String> {
        let (tx, rx) = oneshot::channel();
        self.scribe_ref
            .cast(ScribeMessage::TextDelete {
                layer_name: layer_name.to_string(),
                pos,
                len,
                reply: tx,
            })
            .map_err(|e| format!("Failed to delete text: {}", e))?;
        rpc(rx)
    }

    /// Snapshot the current full text content as a `String`.
    pub fn text_snapshot(&self, layer_name: &str) -> Result<String, String> {
        let (tx, rx) = oneshot::channel();
        self.scribe_ref
            .cast(ScribeMessage::TextSnapshot {
                layer_name: layer_name.to_string(),
                reply: tx,
            })
            .map_err(|e| format!("Failed to snapshot text: {}", e))?;
        rpc(rx)
    }

    /// Length of the text in unicode codepoints.
    pub fn text_length(&self, layer_name: &str) -> Result<usize, String> {
        let (tx, rx) = oneshot::channel();
        self.scribe_ref
            .cast(ScribeMessage::TextLength {
                layer_name: layer_name.to_string(),
                reply: tx,
            })
            .map_err(|e| format!("Failed to get text length: {}", e))?;
        rpc(rx)
    }

    // Per-tree-node nested-LoroText ops. Each tree node's meta map can carry
    // a nested LoroText container under a meta key (typically `"text"`),
    // giving char-level CRDT merge for that block's content. The `tree:text(
    // node_id)` Lua accessor returns a userdata that calls into these.

    /// Insert into the nested LoroText at `meta[key]` of a tree node.
    pub fn tree_text_insert(
        &self,
        layer_name: &str,
        node_id: &str,
        key: &str,
        pos: usize,
        content: &str,
    ) -> Result<(), String> {
        let (tx, rx) = oneshot::channel();
        self.scribe_ref
            .cast(ScribeMessage::TreeTextInsert {
                layer_name: layer_name.to_string(),
                node_id: node_id.to_string(),
                key: key.to_string(),
                pos,
                content: content.to_string(),
                reply: tx,
            })
            .map_err(|e| format!("Failed to insert tree text: {}", e))?;
        rpc(rx)
    }

    /// Delete from the nested LoroText at `meta[key]` of a tree node.
    pub fn tree_text_delete(
        &self,
        layer_name: &str,
        node_id: &str,
        key: &str,
        pos: usize,
        len: usize,
    ) -> Result<(), String> {
        let (tx, rx) = oneshot::channel();
        self.scribe_ref
            .cast(ScribeMessage::TreeTextDelete {
                layer_name: layer_name.to_string(),
                node_id: node_id.to_string(),
                key: key.to_string(),
                pos,
                len,
                reply: tx,
            })
            .map_err(|e| format!("Failed to delete tree text: {}", e))?;
        rpc(rx)
    }

    /// Snapshot the nested LoroText at `meta[key]` of a tree node.
    pub fn tree_text_snapshot(
        &self,
        layer_name: &str,
        node_id: &str,
        key: &str,
    ) -> Result<String, String> {
        let (tx, rx) = oneshot::channel();
        self.scribe_ref
            .cast(ScribeMessage::TreeTextSnapshot {
                layer_name: layer_name.to_string(),
                node_id: node_id.to_string(),
                key: key.to_string(),
                reply: tx,
            })
            .map_err(|e| format!("Failed to snapshot tree text: {}", e))?;
        rpc(rx)
    }

    /// Codepoint length of the nested LoroText at `meta[key]` of a tree node.
    pub fn tree_text_length(
        &self,
        layer_name: &str,
        node_id: &str,
        key: &str,
    ) -> Result<usize, String> {
        let (tx, rx) = oneshot::channel();
        self.scribe_ref
            .cast(ScribeMessage::TreeTextLength {
                layer_name: layer_name.to_string(),
                node_id: node_id.to_string(),
                key: key.to_string(),
                reply: tx,
            })
            .map_err(|e| format!("Failed to get tree text length: {}", e))?;
        rpc(rx)
    }

    // -- Query --

    /// List layer names matching a glob pattern
    pub fn list_layers(&self, pattern: &str) -> Result<Vec<String>, String> {
        let (tx, rx) = oneshot::channel();
        self.scribe_ref
            .cast(ScribeMessage::ListLayers {
                pattern: pattern.to_string(),
                reply: tx,
            })
            .map_err(|e| format!("Failed to list layers: {}", e))?;
        rpc_direct(rx)
    }

    /// Get layer content as Sthithi (returns None if layer doesn't exist)
    pub fn get_layer_sthithi(&self, layer_name: &str) -> Result<Option<Sthithi>, String> {
        let (tx, rx) = oneshot::channel();
        self.scribe_ref
            .cast(ScribeMessage::GetLayerJson {
                layer_name: layer_name.to_string(),
                reply: tx,
            })
            .map_err(|e| format!("Failed to request layer data: {}", e))?;
        rpc_direct(rx)
    }

    /// Get layer data as Sthithi (returns error if layer doesn't exist)
    pub fn get_layer_data(&self, layer_name: &str) -> Result<Sthithi, String> {
        let (tx, rx) = oneshot::channel();
        self.scribe_ref
            .cast(ScribeMessage::GetLayerData {
                layer_name: layer_name.to_string(),
                reply: tx,
            })
            .map_err(|e| format!("Failed to request layer data: {}", e))?;
        rpc(rx)
    }

    // -- Dynamic layers --

    /// Create a dynamic layer from a schema pattern
    ///
    /// **Context**: Lua app calls
    /// `scribe:create_layer("channels/{channel}/messages/{period}", { channel = "general", period = "2025-02" })`
    /// **Returns**: Full layer name (e.g., "{page_id}/channels/{our_did}/general/messages/2025-02")
    pub fn create_layer(
        &self,
        schema_key: &str,
        placeholders: HashMap<String, String>,
        authorized_peers: Option<Vec<String>>,
    ) -> Result<String, String> {
        let (tx, rx) = oneshot::channel();
        self.scribe_ref
            .cast(ScribeMessage::CreateDynamicLayer {
                schema_key: schema_key.to_string(),
                placeholders,
                authorized_peers,
                reply: tx,
            })
            .map_err(|e| format!("Failed to create dynamic layer: {}", e))?;
        rpc(rx)
    }

    /// Backward-compatible helper for `{id}`-only schemas.
    pub fn create_layer_with_id(
        &self,
        schema_key: &str,
        layer_id: &str,
        authorized_peers: Option<Vec<String>>,
    ) -> Result<String, String> {
        let mut placeholders = HashMap::new();
        placeholders.insert("id".to_string(), layer_id.to_string());
        self.create_layer(schema_key, placeholders, authorized_peers)
    }

    // -- Layer access --

    /// Add DIDs as participants to an explicit dynamic layer
    pub fn add_layer_access(&self, layer_name: &str, dids: &[String]) -> Result<(), String> {
        let (tx, rx) = oneshot::channel();
        self.scribe_ref
            .cast(ScribeMessage::AddLayerAccess {
                layer_name: layer_name.to_string(),
                dids: dids.to_vec(),
                reply: tx,
            })
            .map_err(|e| format!("Failed to add layer access: {}", e))?;
        rpc(rx)
    }

    // -- Peers & Ephemeral --

    /// Get count of subscribers to this page
    pub fn get_subscriber_count(&self) -> usize {
        let (tx, rx) = oneshot::channel();
        if self
            .scribe_ref
            .cast(ScribeMessage::GetSubscriberCount { reply: tx })
            .is_err()
        {
            return 0;
        }
        let result = match tokio::runtime::Handle::try_current() {
            Ok(handle) => tokio::task::block_in_place(|| handle.block_on(rx)),
            Err(_) => futures::executor::block_on(rx),
        };
        result.unwrap_or(0)
    }

    /// Send ephemeral data to all peers
    pub fn send_ephemeral(&self, payload: Vec<u8>) -> Result<(), String> {
        self.scribe_ref
            .cast(ScribeMessage::SendEphemeral { payload })
            .map_err(|e| format!("Failed to send ephemeral: {}", e))
    }
}
