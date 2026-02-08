//! Loro bindings for Lua
//!
//! Provides loro:list, loro:map, and related methods.
//!
//! **Design**: Lua doesn't hold direct LoroList/LoroMap handles because they can
//! become stale after ReplaceLayer (SyncReset recovery). All operations go through
//! Scribe messages which always operate on the current LoroDoc.

use mlua::{UserData, UserDataMethods, Value as LuaValue, Error as LuaError};
use ractor::ActorRef;
use tokio::sync::oneshot;
use tracing::trace;

use butler::ScribeMessage;
use super::block_on_async;
use super::convert::lua_to_json;

// Layer Wrapper (for get_or_create_layer return type)

/// Wrapper for List or Map layer types
pub enum LayerWrapper {
    List(LuaLoroList),
    Map(LuaLoroMap),
}

impl UserData for LayerWrapper {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        // Forward list methods
        methods.add_method("push", |_, this, value: LuaValue| {
            match this {
                LayerWrapper::List(list) => list.push_value(value),
                LayerWrapper::Map(_) => Err(LuaError::RuntimeError("push not supported on map".to_string())),
            }
        });

        methods.add_method("get", |lua, this, key: LuaValue| {
            match this {
                LayerWrapper::List(list) => {
                    let index = match key {
                        LuaValue::Integer(i) => i as usize,
                        _ => return Err(LuaError::RuntimeError("List index must be integer".to_string())),
                    };
                    list.get_at(lua, index)
                }
                LayerWrapper::Map(map) => {
                    let key_str = match key {
                        LuaValue::String(s) => s.to_str()?.to_string(),
                        _ => return Err(LuaError::RuntimeError("Map key must be string".to_string())),
                    };
                    map.get_value(lua, &key_str)
                }
            }
        });

        methods.add_method("set", |_, this, (key, value): (LuaValue, LuaValue)| {
            match this {
                LayerWrapper::List(list) => {
                    let index = match key {
                        LuaValue::Integer(i) => i as usize,
                        _ => return Err(LuaError::RuntimeError("List index must be integer".to_string())),
                    };
                    list.set_at(index, value)
                }
                LayerWrapper::Map(map) => {
                    let key_str = match key {
                        LuaValue::String(s) => s.to_str()?.to_string(),
                        _ => return Err(LuaError::RuntimeError("Map key must be string".to_string())),
                    };
                    map.set_value(&key_str, value)
                }
            }
        });

        methods.add_method("length", |_, this, ()| {
            match this {
                LayerWrapper::List(list) => list.length(),
                LayerWrapper::Map(map) => map.length(),
            }
        });

        methods.add_method("keys", |lua, this, ()| {
            match this {
                LayerWrapper::List(_) => Err(LuaError::RuntimeError("keys not supported on list".to_string())),
                LayerWrapper::Map(map) => map.keys(lua),
            }
        });

        methods.add_method("delete", |_, this, key: LuaValue| {
            match this {
                LayerWrapper::List(list) => {
                    let index = match key {
                        LuaValue::Integer(i) => i as usize,
                        _ => return Err(LuaError::RuntimeError("List index must be integer".to_string())),
                    };
                    list.delete_at(index)
                }
                LayerWrapper::Map(map) => {
                    let key_str = match key {
                        LuaValue::String(s) => s.to_str()?.to_string(),
                        _ => return Err(LuaError::RuntimeError("Map key must be string".to_string())),
                    };
                    map.delete_key(&key_str)
                }
            }
        });
    }
}

// Lua Loro List

/// Lua wrapper for LoroList
///
/// **Design**: Doesn't hold direct LoroList handle - uses Scribe messages for all operations.
/// This prevents stale handle issues after ReplaceLayer (SyncReset recovery).
pub struct LuaLoroList {
    scribe_ref: ActorRef<ScribeMessage>,
    layer_name: String,
}

impl LuaLoroList {
    pub fn new(scribe_ref: ActorRef<ScribeMessage>, layer_name: String) -> Self {
        Self { scribe_ref, layer_name }
    }

    /// Push value to the list (via Scribe message)
    pub(crate) fn push_value(&self, value: LuaValue) -> Result<(), LuaError> {
        let json_value = lua_to_json(&value)?;
        trace!(layer = %self.layer_name, "LuaLoroList::push via Scribe message");

        // Send ListPush with empty path (Scribe will use layer_name as container)
        self.scribe_ref.cast(ScribeMessage::ListPush {
            layer_name: self.layer_name.clone(),
            path: String::new(), // Empty path = use layer_name as container
            item: json_value,
        }).map_err(|e| LuaError::RuntimeError(format!("Failed to push: {}", e)))
    }

    /// Get item at index (via Scribe message)
    pub(crate) fn get_at(&self, lua: &mlua::Lua, index: usize) -> Result<LuaValue, LuaError> {
        let (tx, rx) = oneshot::channel();
        self.scribe_ref.cast(ScribeMessage::ListGet {
            layer_name: self.layer_name.clone(),
            index,
            reply: tx,
        }).map_err(|e| LuaError::RuntimeError(format!("Failed to get: {}", e)))?;

        let result = block_on_async(async {
            rx.await
                .map_err(|e| LuaError::RuntimeError(format!("Channel error: {}", e)))?
                .map_err(|e| LuaError::RuntimeError(format!("Scribe error: {}", e)))
        })??;

        match result {
            Some(json_value) => super::convert::json_to_lua(lua, &json_value),
            None => Ok(LuaValue::Nil),
        }
    }

    /// Set item at index (delete + insert via Scribe messages)
    pub(crate) fn set_at(&self, index: usize, value: LuaValue) -> Result<(), LuaError> {
        let json_value = lua_to_json(&value)?;
        trace!(layer = %self.layer_name, index, "LuaLoroList::set_at via Scribe messages");

        // Delete at index first
        self.scribe_ref.cast(ScribeMessage::ListDelete {
            layer_name: self.layer_name.clone(),
            path: String::new(),
            index,
        }).map_err(|e| LuaError::RuntimeError(format!("Failed to delete for set: {}", e)))?;

        // Insert at index
        self.scribe_ref.cast(ScribeMessage::ListInsert {
            layer_name: self.layer_name.clone(),
            path: String::new(),
            index,
            item: json_value,
        }).map_err(|e| LuaError::RuntimeError(format!("Failed to insert for set: {}", e)))
    }

    /// Delete item at index (via Scribe message)
    pub(crate) fn delete_at(&self, index: usize) -> Result<(), LuaError> {
        trace!(layer = %self.layer_name, index, "LuaLoroList::delete_at via Scribe message");

        self.scribe_ref.cast(ScribeMessage::ListDelete {
            layer_name: self.layer_name.clone(),
            path: String::new(),
            index,
        }).map_err(|e| LuaError::RuntimeError(format!("Failed to delete: {}", e)))
    }

    /// Get list length (via Scribe message)
    pub(crate) fn length(&self) -> Result<usize, LuaError> {
        let (tx, rx) = oneshot::channel();
        self.scribe_ref.cast(ScribeMessage::ListLength {
            layer_name: self.layer_name.clone(),
            reply: tx,
        }).map_err(|e| LuaError::RuntimeError(format!("Failed to get length: {}", e)))?;

        block_on_async(async {
            rx.await
                .map_err(|e| LuaError::RuntimeError(format!("Channel error: {}", e)))?
                .map_err(|e| LuaError::RuntimeError(format!("Scribe error: {}", e)))
        })?
    }
}

impl UserData for LuaLoroList {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("push", |_, this, value: LuaValue| {
            this.push_value(value)
        });

        methods.add_method("get", |lua, this, index: usize| {
            this.get_at(lua, index)
        });

        methods.add_method("set", |_, this, (index, value): (usize, LuaValue)| {
            this.set_at(index, value)
        });

        methods.add_method("delete", |_, this, index: usize| {
            this.delete_at(index)
        });

        methods.add_method("length", |_, this, ()| {
            this.length()
        });
    }
}

// Lua Loro Map

/// Lua wrapper for LoroMap
///
/// **Design**: Doesn't hold direct LoroMap handle - uses Scribe messages for all operations.
/// This prevents stale handle issues after ReplaceLayer (SyncReset recovery).
pub struct LuaLoroMap {
    scribe_ref: ActorRef<ScribeMessage>,
    layer_name: String,
}

impl LuaLoroMap {
    pub fn new(scribe_ref: ActorRef<ScribeMessage>, layer_name: String) -> Self {
        Self { scribe_ref, layer_name }
    }

    /// Set value for key (via Scribe message)
    pub(crate) fn set_value(&self, key: &str, value: LuaValue) -> Result<(), LuaError> {
        let json_value = lua_to_json(&value)?;
        trace!(layer = %self.layer_name, key, "LuaLoroMap::set via Scribe message");

        self.scribe_ref.cast(ScribeMessage::MapInsert {
            layer_name: self.layer_name.clone(),
            path: String::new(), // Empty path = use layer_name as container
            key: key.to_string(),
            value: json_value,
        }).map_err(|e| LuaError::RuntimeError(format!("Failed to set: {}", e)))
    }

    /// Get value for key (via Scribe message)
    pub(crate) fn get_value(&self, lua: &mlua::Lua, key: &str) -> Result<LuaValue, LuaError> {
        let (tx, rx) = oneshot::channel();
        self.scribe_ref.cast(ScribeMessage::MapGet {
            layer_name: self.layer_name.clone(),
            key: key.to_string(),
            reply: tx,
        }).map_err(|e| LuaError::RuntimeError(format!("Failed to get: {}", e)))?;

        let result = block_on_async(async {
            rx.await
                .map_err(|e| LuaError::RuntimeError(format!("Channel error: {}", e)))?
                .map_err(|e| LuaError::RuntimeError(format!("Scribe error: {}", e)))
        })??;

        match result {
            Some(json_value) => super::convert::json_to_lua(lua, &json_value),
            None => Ok(LuaValue::Nil),
        }
    }

    /// Delete key (via Scribe message)
    pub(crate) fn delete_key(&self, key: &str) -> Result<(), LuaError> {
        trace!(layer = %self.layer_name, key, "LuaLoroMap::delete via Scribe message");

        self.scribe_ref.cast(ScribeMessage::MapDelete {
            layer_name: self.layer_name.clone(),
            path: String::new(),
            key: key.to_string(),
        }).map_err(|e| LuaError::RuntimeError(format!("Failed to delete: {}", e)))
    }

    /// Get map length (via Scribe message)
    pub(crate) fn length(&self) -> Result<usize, LuaError> {
        let (tx, rx) = oneshot::channel();
        self.scribe_ref.cast(ScribeMessage::MapLength {
            layer_name: self.layer_name.clone(),
            reply: tx,
        }).map_err(|e| LuaError::RuntimeError(format!("Failed to get length: {}", e)))?;

        block_on_async(async {
            rx.await
                .map_err(|e| LuaError::RuntimeError(format!("Channel error: {}", e)))?
                .map_err(|e| LuaError::RuntimeError(format!("Scribe error: {}", e)))
        })?
    }

    /// Get all keys (via Scribe message)
    pub(crate) fn keys(&self, lua: &mlua::Lua) -> Result<mlua::Table, LuaError> {
        let (tx, rx) = oneshot::channel();
        self.scribe_ref.cast(ScribeMessage::MapKeys {
            layer_name: self.layer_name.clone(),
            reply: tx,
        }).map_err(|e| LuaError::RuntimeError(format!("Failed to get keys: {}", e)))?;

        let keys = block_on_async(async {
            rx.await
                .map_err(|e| LuaError::RuntimeError(format!("Channel error: {}", e)))?
                .map_err(|e| LuaError::RuntimeError(format!("Scribe error: {}", e)))
        })??;

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

        methods.add_method("get", |lua, this, key: String| {
            this.get_value(lua, &key)
        });

        methods.add_method("delete", |_, this, key: String| {
            this.delete_key(&key)
        });

        methods.add_method("length", |_, this, ()| {
            this.length()
        });

        methods.add_method("keys", |lua, this, ()| {
            this.keys(lua)
        });
    }
}
