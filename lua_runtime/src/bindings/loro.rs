//! Loro bindings for Lua
//!
//! Provides loro:list, loro:map, and related methods.
//!
//! **Design**: Lua doesn't hold direct LoroList/LoroMap handles because they can
//! become stale after ReplaceLayer (SyncReset recovery). All operations go through
//! Scribe messages which always operate on the current LoroDoc.

use mlua::{Error as LuaError, IntoLua, UserData, UserDataMethods, Value as LuaValue};
use std::sync::Arc;
use tracing::trace;

use super::convert::{lua_to_sthithi, sthithi_to_lua};
use crate::scribe_handle::ActorScribeHandle;

// Lua Loro List

/// Lua wrapper for LoroList
///
/// **Design**: Doesn't hold direct LoroList handle - uses Scribe messages for all operations.
/// This prevents stale handle issues after ReplaceLayer (SyncReset recovery).
pub struct LuaLoroList {
    scribe: Arc<ActorScribeHandle>,
    layer_name: String,
}

impl LuaLoroList {
    pub fn new(scribe: Arc<ActorScribeHandle>, layer_name: String) -> Self {
        Self { scribe, layer_name }
    }

    /// Push value to the list (via ScribeHandle)
    pub(crate) fn push_value(&self, value: LuaValue) -> Result<(), LuaError> {
        let sthithi_value = lua_to_sthithi(&value)?;
        trace!(layer = %self.layer_name, "LuaLoroList::push via ScribeHandle");

        self.scribe
            .list_push(&self.layer_name, "", sthithi_value)
            .map_err(|e| LuaError::RuntimeError(e))?;
        Ok(())
    }

    /// Get item at index (via ScribeHandle)
    pub(crate) fn get_at(&self, lua: &mlua::Lua, index: usize) -> Result<LuaValue, LuaError> {
        let result = self
            .scribe
            .list_get(&self.layer_name, index)
            .map_err(|e| LuaError::RuntimeError(e))?;

        match result {
            Some(sthithi_value) => sthithi_to_lua(lua, &sthithi_value),
            None => Ok(LuaValue::Nil),
        }
    }

    /// Set item at index (delete + insert via ScribeHandle)
    pub(crate) fn set_at(&self, index: usize, value: LuaValue) -> Result<(), LuaError> {
        let sthithi_value = lua_to_sthithi(&value)?;
        trace!(layer = %self.layer_name, index, "LuaLoroList::set_at via ScribeHandle");

        // Delete at index first
        self.scribe
            .list_delete(&self.layer_name, "", index)
            .map_err(|e| LuaError::RuntimeError(e))?;

        // Insert at index
        self.scribe
            .list_insert(&self.layer_name, "", index, sthithi_value)
            .map_err(|e| LuaError::RuntimeError(e))?;
        Ok(())
    }

    /// Delete item at index (via ScribeHandle)
    pub(crate) fn delete_at(&self, index: usize) -> Result<(), LuaError> {
        trace!(layer = %self.layer_name, index, "LuaLoroList::delete_at via ScribeHandle");

        self.scribe
            .list_delete(&self.layer_name, "", index)
            .map_err(|e| LuaError::RuntimeError(e))?;
        Ok(())
    }

    /// Get list length (via ScribeHandle)
    pub(crate) fn length(&self) -> Result<usize, LuaError> {
        self.scribe
            .list_length(&self.layer_name)
            .map_err(|e| LuaError::RuntimeError(e))
    }
}

impl UserData for LuaLoroList {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("push", |_, this, value: LuaValue| this.push_value(value));

        methods.add_method("get", |lua, this, index: usize| this.get_at(lua, index));

        methods.add_method("set", |_, this, (index, value): (usize, LuaValue)| {
            this.set_at(index, value)
        });

        methods.add_method("delete", |_, this, index: usize| this.delete_at(index));

        methods.add_method("length", |_, this, ()| this.length());
    }
}

// Lua Loro Map

/// Lua wrapper for LoroMap
///
/// **Design**: Doesn't hold direct LoroMap handle - uses Scribe messages for all operations.
/// This prevents stale handle issues after ReplaceLayer (SyncReset recovery).
pub struct LuaLoroMap {
    scribe: Arc<ActorScribeHandle>,
    layer_name: String,
}

impl LuaLoroMap {
    pub fn new(scribe: Arc<ActorScribeHandle>, layer_name: String) -> Self {
        Self { scribe, layer_name }
    }

    /// Set value for key (via ScribeHandle)
    pub(crate) fn set_value(&self, key: &str, value: LuaValue) -> Result<(), LuaError> {
        let sthithi_value = lua_to_sthithi(&value)?;
        trace!(layer = %self.layer_name, key, "LuaLoroMap::set via ScribeHandle");

        self.scribe
            .map_insert(&self.layer_name, "", key, sthithi_value)
            .map_err(|e| LuaError::RuntimeError(e))?;
        Ok(())
    }

    /// Get value for key (via ScribeHandle)
    pub(crate) fn get_value(&self, lua: &mlua::Lua, key: &str) -> Result<LuaValue, LuaError> {
        let result = self
            .scribe
            .map_get(&self.layer_name, key)
            .map_err(|e| LuaError::RuntimeError(e))?;

        match result {
            Some(sthithi_value) => sthithi_to_lua(lua, &sthithi_value),
            None => Ok(LuaValue::Nil),
        }
    }

    /// Delete key (via ScribeHandle)
    pub(crate) fn delete_key(&self, key: &str) -> Result<(), LuaError> {
        trace!(layer = %self.layer_name, key, "LuaLoroMap::delete via ScribeHandle");

        self.scribe
            .map_delete(&self.layer_name, "", key)
            .map_err(|e| LuaError::RuntimeError(e))?;
        Ok(())
    }

    /// Get map length (via ScribeHandle)
    pub(crate) fn length(&self) -> Result<usize, LuaError> {
        self.scribe
            .map_length(&self.layer_name)
            .map_err(|e| LuaError::RuntimeError(e))
    }

    /// Get all keys (via ScribeHandle)
    pub(crate) fn keys(&self, lua: &mlua::Lua) -> Result<mlua::Table, LuaError> {
        let keys = self
            .scribe
            .map_keys(&self.layer_name)
            .map_err(|e| LuaError::RuntimeError(e))?;

        let table = lua.create_table()?;
        for (i, key) in keys.iter().enumerate() {
            table.set(i + 1, key.clone())?;
        }
        Ok(table)
    }
}

impl UserData for LuaLoroMap {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("set", |_, this, (key, value): (String, LuaValue)| {
            this.set_value(&key, value)
        });

        methods.add_method("get", |lua, this, key: String| this.get_value(lua, &key));

        methods.add_method("delete", |_, this, key: String| this.delete_key(&key));

        methods.add_method("length", |_, this, ()| this.length());

        methods.add_method("keys", |lua, this, ()| this.keys(lua));
    }
}

// Lua Loro Text

/// Lua wrapper for a `LoroText`-backed layer.
///
/// **Design**: Same pattern as `LuaLoroList` / `LuaLoroMap` / `LuaLoroTree` —
/// no direct Loro handle; all ops route through `ScribeHandle`.
///
/// **Lua surface**:
/// - `text:insert(pos, content)`
/// - `text:delete(pos, len)`
/// - `text:to_string()` → current full text
/// - `text:length()` → codepoint count
///
/// `pos` and `len` are unicode-codepoint indices (Loro's default). Apps that
/// hold UTF-8 byte offsets (e.g. from Slint's `cursor-position-byte-offset`)
/// must convert before calling.
pub struct LuaLoroText {
    scribe: Arc<ActorScribeHandle>,
    layer_name: String,
}

impl LuaLoroText {
    pub fn new(scribe: Arc<ActorScribeHandle>, layer_name: String) -> Self {
        Self { scribe, layer_name }
    }
}

impl UserData for LuaLoroText {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("insert", |_, this, (pos, content): (usize, String)| {
            trace!(layer = %this.layer_name, pos, "LuaLoroText::insert");
            this.scribe
                .text_insert(&this.layer_name, pos, &content)
                .map_err(LuaError::RuntimeError)
        });

        methods.add_method("delete", |_, this, (pos, len): (usize, usize)| {
            trace!(layer = %this.layer_name, pos, len, "LuaLoroText::delete");
            this.scribe
                .text_delete(&this.layer_name, pos, len)
                .map_err(LuaError::RuntimeError)
        });

        methods.add_method("to_string", |lua, this, ()| {
            let s = this
                .scribe
                .text_snapshot(&this.layer_name)
                .map_err(LuaError::RuntimeError)?;
            s.into_lua(lua)
        });

        methods.add_method("length", |_, this, ()| {
            this.scribe
                .text_length(&this.layer_name)
                .map_err(LuaError::RuntimeError)
        });
    }
}

/// Lua wrapper for a *nested* `LoroText` living inside a tree node's meta map.
///
/// **Why**: each block in the editor is a tree node; its `text` benefits from
/// char-level CRDT merge instead of LWW per-keystroke `set_prop`. The node's
/// meta map carries a nested `LoroText` container under a key (typically
/// `"text"`); this userdata routes insert/delete/snapshot/length to that
/// container via the actor.
///
/// **Lua surface** (mirrors `LuaLoroText`):
/// - `node_text:insert(pos, content)`
/// - `node_text:delete(pos, len)`
/// - `node_text:to_string()` → string
/// - `node_text:length()` → codepoints
///
/// Created via `tree:text(node_id)` — the key defaults to `"text"`. Apps that
/// want multiple text fields per block (e.g. a heading with a separate caption)
/// can instead call `tree:text_at(node_id, "caption")`.
pub struct LuaNodeText {
    scribe: Arc<ActorScribeHandle>,
    layer_name: String,
    node_id: String,
    key: String,
}

impl LuaNodeText {
    pub fn new(
        scribe: Arc<ActorScribeHandle>,
        layer_name: String,
        node_id: String,
        key: String,
    ) -> Self {
        Self {
            scribe,
            layer_name,
            node_id,
            key,
        }
    }
}

impl UserData for LuaNodeText {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("insert", |_, this, (pos, content): (usize, String)| {
            trace!(layer = %this.layer_name, node = %this.node_id, key = %this.key, pos, "LuaNodeText::insert");
            this.scribe
                .tree_text_insert(&this.layer_name, &this.node_id, &this.key, pos, &content)
                .map_err(LuaError::RuntimeError)
        });

        methods.add_method("delete", |_, this, (pos, len): (usize, usize)| {
            trace!(layer = %this.layer_name, node = %this.node_id, key = %this.key, pos, len, "LuaNodeText::delete");
            this.scribe
                .tree_text_delete(&this.layer_name, &this.node_id, &this.key, pos, len)
                .map_err(LuaError::RuntimeError)
        });

        methods.add_method("to_string", |lua, this, ()| {
            let s = this
                .scribe
                .tree_text_snapshot(&this.layer_name, &this.node_id, &this.key)
                .map_err(LuaError::RuntimeError)?;
            s.into_lua(lua)
        });

        methods.add_method("length", |_, this, ()| {
            this.scribe
                .tree_text_length(&this.layer_name, &this.node_id, &this.key)
                .map_err(LuaError::RuntimeError)
        });
    }
}

// Lua Loro Tree

/// Lua wrapper for a `LoroTree`-backed layer.
///
/// **Design**: Same pattern as `LuaLoroList` / `LuaLoroMap` — no direct Loro
/// handle; all operations route through `ScribeHandle` so they survive
/// `ReplaceLayer` and stay observable by the broadcast pipeline.
///
/// **Lua surface**:
/// - `tree:create(parent, index, props)` → node_id (string)
/// - `tree:move(node_id, parent, index)`
/// - `tree:delete(node_id)`
/// - `tree:set_prop(node_id, key, value)`
/// - `tree:get(node_id)` → `{ id, parent, children, props }` or nil
/// - `tree:walk()` → array of `{ id, parent, depth, index, props }` (DFS)
///
/// `parent` may be `nil` to mean the root. `index` may be `nil` to append.
pub struct LuaLoroTree {
    scribe: Arc<ActorScribeHandle>,
    layer_name: String,
}

impl LuaLoroTree {
    pub fn new(scribe: Arc<ActorScribeHandle>, layer_name: String) -> Self {
        Self { scribe, layer_name }
    }
}

/// Convert an optional Lua string parent argument to `Option<String>`.
fn parent_arg(parent: Option<String>) -> Option<String> {
    parent.filter(|s| !s.is_empty())
}

impl UserData for LuaLoroTree {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        // tree:create(parent, index, props, text_keys)
        //
        // `text_keys` (optional) is an array of meta keys to materialise as
        // nested LoroText containers in the same commit as node creation.
        // Editors typically pass `{"text"}`. Pure-data trees pass nothing.
        // See `ScribeMessage::TreeCreate` for why this matters (single-
        // creator container materialisation eliminates a LoroMap LWW race).
        methods.add_method(
            "create",
            |_,
             this,
             (parent, index, props, text_keys): (
                Option<String>,
                Option<usize>,
                Option<LuaValue>,
                Option<mlua::Table>,
            )| {
                let props_sthithi = match props {
                    Some(LuaValue::Nil) | None => domains::Sthithi::Null,
                    Some(v) => lua_to_sthithi(&v)?,
                };
                let text_keys_vec = match text_keys {
                    None => Vec::new(),
                    Some(t) => {
                        let mut out = Vec::new();
                        for pair in t.pairs::<i64, String>() {
                            let (_, v) = pair?;
                            out.push(v);
                        }
                        out
                    }
                };
                trace!(layer = %this.layer_name, text_keys = ?text_keys_vec, "LuaLoroTree::create via ScribeHandle");
                this.scribe
                    .tree_create(
                        &this.layer_name,
                        parent_arg(parent),
                        index,
                        props_sthithi,
                        text_keys_vec,
                    )
                    .map_err(LuaError::RuntimeError)
            },
        );

        methods.add_method(
            "move",
            |_,
             this,
             (node_id, parent, index): (String, Option<String>, Option<usize>)| {
                trace!(layer = %this.layer_name, node = %node_id, "LuaLoroTree::move via ScribeHandle");
                this.scribe
                    .tree_move(&this.layer_name, &node_id, parent_arg(parent), index)
                    .map_err(LuaError::RuntimeError)
            },
        );

        methods.add_method("delete", |_, this, node_id: String| {
            trace!(layer = %this.layer_name, node = %node_id, "LuaLoroTree::delete via ScribeHandle");
            this.scribe
                .tree_delete(&this.layer_name, &node_id)
                .map_err(LuaError::RuntimeError)
        });

        methods.add_method(
            "set_prop",
            |_, this, (node_id, key, value): (String, String, LuaValue)| {
                let sthithi_value = lua_to_sthithi(&value)?;
                trace!(layer = %this.layer_name, node = %node_id, key = %key, "LuaLoroTree::set_prop");
                this.scribe
                    .tree_set_prop(&this.layer_name, &node_id, &key, sthithi_value)
                    .map_err(LuaError::RuntimeError)
            },
        );

        methods.add_method("get", |lua, this, node_id: String| {
            let view = this
                .scribe
                .tree_get_node(&this.layer_name, &node_id)
                .map_err(LuaError::RuntimeError)?;
            let view = match view {
                Some(v) => v,
                None => return Ok(LuaValue::Nil),
            };
            let table = lua.create_table()?;
            table.set("id", view.id)?;
            match view.parent {
                Some(p) => table.set("parent", p)?,
                None => table.set("parent", LuaValue::Nil)?,
            }
            let children_tbl = lua.create_table()?;
            for (i, child) in view.children.into_iter().enumerate() {
                children_tbl.set(i + 1, child)?;
            }
            table.set("children", children_tbl)?;
            table.set("props", sthithi_to_lua(lua, &view.props)?)?;
            Ok(LuaValue::Table(table))
        });

        // tree:text(node_id) — handle for the node's nested LoroText at the
        // conventional `"text"` meta key. tree:text_at(node_id, key) lets
        // apps with multiple text fields per node (e.g. caption, alt) target
        // a specific key.
        methods.add_method("text", |_, this, node_id: String| {
            Ok(LuaNodeText::new(
                this.scribe.clone(),
                this.layer_name.clone(),
                node_id,
                "text".to_string(),
            ))
        });

        methods.add_method(
            "text_at",
            |_, this, (node_id, key): (String, String)| {
                Ok(LuaNodeText::new(
                    this.scribe.clone(),
                    this.layer_name.clone(),
                    node_id,
                    key,
                ))
            },
        );

        methods.add_method("walk", |lua, this, ()| {
            let nodes = this
                .scribe
                .tree_walk(&this.layer_name)
                .map_err(LuaError::RuntimeError)?;
            let table = lua.create_table()?;
            for (i, node) in nodes.into_iter().enumerate() {
                let entry = lua.create_table()?;
                entry.set("id", node.id)?;
                match node.parent {
                    Some(p) => entry.set("parent", p)?,
                    None => entry.set("parent", LuaValue::Nil)?,
                }
                entry.set("depth", node.depth)?;
                entry.set("index", node.index)?;
                entry.set("props", sthithi_to_lua(lua, &node.props)?)?;
                table.set(i + 1, entry)?;
            }
            Ok(table)
        });
    }
}
