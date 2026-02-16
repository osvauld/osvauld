//! Loro bindings for Lua
//!
//! Provides loro:list, loro:map, and related methods.
//!
//! **Design**: Lua doesn't hold direct LoroList/LoroMap handles because they can
//! become stale after ReplaceLayer (SyncReset recovery). All operations go through
//! Scribe messages which always operate on the current LoroDoc.

use mlua::{Error as LuaError, UserData, UserDataMethods, Value as LuaValue};
use std::sync::Arc;
use tracing::trace;

use super::convert::lua_to_json;
use crate::scribe_handle::ScribeHandle;

// Lua Loro List

/// Lua wrapper for LoroList
///
/// **Design**: Doesn't hold direct LoroList handle - uses Scribe messages for all operations.
/// This prevents stale handle issues after ReplaceLayer (SyncReset recovery).
pub struct LuaLoroList {
    scribe: Arc<dyn ScribeHandle>,
    layer_name: String,
}

impl LuaLoroList {
    pub fn new(scribe: Arc<dyn ScribeHandle>, layer_name: String) -> Self {
        Self { scribe, layer_name }
    }

    /// Push value to the list (via ScribeHandle)
    pub(crate) fn push_value(&self, value: LuaValue) -> Result<(), LuaError> {
        let json_value = lua_to_json(&value)?;
        trace!(layer = %self.layer_name, "LuaLoroList::push via ScribeHandle");

        self.scribe
            .list_push(&self.layer_name, "", json_value)
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
            Some(json_value) => super::convert::json_to_lua(lua, &json_value),
            None => Ok(LuaValue::Nil),
        }
    }

    /// Set item at index (delete + insert via ScribeHandle)
    pub(crate) fn set_at(&self, index: usize, value: LuaValue) -> Result<(), LuaError> {
        let json_value = lua_to_json(&value)?;
        trace!(layer = %self.layer_name, index, "LuaLoroList::set_at via ScribeHandle");

        // Delete at index first
        self.scribe
            .list_delete(&self.layer_name, "", index)
            .map_err(|e| LuaError::RuntimeError(e))?;

        // Insert at index
        self.scribe
            .list_insert(&self.layer_name, "", index, json_value)
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
    scribe: Arc<dyn ScribeHandle>,
    layer_name: String,
}

impl LuaLoroMap {
    pub fn new(scribe: Arc<dyn ScribeHandle>, layer_name: String) -> Self {
        Self { scribe, layer_name }
    }

    /// Set value for key (via ScribeHandle)
    pub(crate) fn set_value(&self, key: &str, value: LuaValue) -> Result<(), LuaError> {
        let json_value = lua_to_json(&value)?;
        trace!(layer = %self.layer_name, key, "LuaLoroMap::set via ScribeHandle");

        self.scribe
            .map_insert(&self.layer_name, "", key, json_value)
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
            Some(json_value) => super::convert::json_to_lua(lua, &json_value),
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
