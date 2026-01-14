//! Loro bindings for Lua
//!
//! Provides loro:list, loro:map, and related methods.

use loro::{LoroList, LoroMap, LoroValue};
use mlua::{UserData, UserDataMethods, Value as LuaValue, Error as LuaError};
use ractor::ActorRef;
use tokio::sync::oneshot;
use tracing::info;

use crate::scribe::ScribeMessage;
use super::block_on_async;
use super::convert::{loro_value_to_lua, lua_to_loro_value};

// =============================================================================
// Loro Bindings
// =============================================================================

/// Loro bindings for Lua
///
/// **Methods**:
/// - `loro:get_list(layer_name)` - Get a LoroList container
/// - `loro:get_map(layer_name)` - Get a LoroMap container
/// - `loro:get_or_create_layer(name, type)` - Get or create a layer
/// - `loro:list_layers(pattern)` - List layer names matching pattern
/// - `loro:get_layer(name, type)` - Get a layer with optional type
pub struct LoroBindings {
    scribe_ref: ActorRef<ScribeMessage>,
    #[allow(dead_code)]
    page_id: String,
}

impl LoroBindings {
    pub fn new(scribe_ref: ActorRef<ScribeMessage>, page_id: String) -> Self {
        Self { scribe_ref, page_id }
    }
}

impl UserData for LoroBindings {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        // loro:list(layer_name) -> LuaLoroList (alias for get_list)
        methods.add_method("list", |_, this, layer_name: String| {
            let (tx, rx) = oneshot::channel();
            this.scribe_ref.cast(ScribeMessage::GetOrCreateLoroList {
                layer_name: layer_name.clone(),
                reply: tx,
            }).map_err(|e| LuaError::RuntimeError(format!("Failed to get list: {}", e)))?;

            let result = block_on_async(async {
                rx.await
                    .map_err(|e| LuaError::RuntimeError(format!("Channel error: {}", e)))?
                    .map_err(|e| LuaError::RuntimeError(format!("Scribe error: {}", e)))
            })??;

            Ok(LuaLoroList::new(result, this.scribe_ref.clone(), layer_name))
        });

        // loro:map(layer_name) -> LuaLoroMap (alias for get_map)
        methods.add_method("map", |_, this, layer_name: String| {
            let (tx, rx) = oneshot::channel();
            this.scribe_ref.cast(ScribeMessage::GetOrCreateLoroMap {
                layer_name: layer_name.clone(),
                reply: tx,
            }).map_err(|e| LuaError::RuntimeError(format!("Failed to get map: {}", e)))?;

            let result = block_on_async(async {
                rx.await
                    .map_err(|e| LuaError::RuntimeError(format!("Channel error: {}", e)))?
                    .map_err(|e| LuaError::RuntimeError(format!("Scribe error: {}", e)))
            })??;

            Ok(LuaLoroMap::new(result, this.scribe_ref.clone(), layer_name))
        });

        // loro:get_list(layer_name) -> LuaLoroList | nil
        // Returns nil if layer doesn't exist (unlike list() which creates it)
        methods.add_method("get_list", |_, this, layer_name: String| {
            let (tx, rx) = oneshot::channel();
            this.scribe_ref.cast(ScribeMessage::GetLoroList {
                layer_name: layer_name.clone(),
                reply: tx,
            }).map_err(|e| LuaError::RuntimeError(format!("Failed to send GetLoroList message: {}", e)))?;

            let channel_result = block_on_async(async {
                rx.await.map_err(|e| LuaError::RuntimeError(format!("Channel error: {}", e)))
            })?;

            match channel_result {
                Ok(Ok(list)) => Ok(Some(LuaLoroList::new(list, this.scribe_ref.clone(), layer_name))),
                Ok(Err(_)) => Ok(None),
                Err(_) => Ok(None),
            }
        });

        // loro:get_map(layer_name) -> LuaLoroMap | nil
        // Returns nil if layer doesn't exist (unlike map() which creates it)
        methods.add_method("get_map", |_, this, layer_name: String| {
            let (tx, rx) = oneshot::channel();
            this.scribe_ref.cast(ScribeMessage::GetLoroMap {
                layer_name: layer_name.clone(),
                reply: tx,
            }).map_err(|e| LuaError::RuntimeError(format!("Failed to send GetLoroMap message: {}", e)))?;

            let channel_result = block_on_async(async {
                rx.await.map_err(|e| LuaError::RuntimeError(format!("Channel error: {}", e)))
            })?;

            match channel_result {
                Ok(Ok(map)) => Ok(Some(LuaLoroMap::new(map, this.scribe_ref.clone(), layer_name))),
                Ok(Err(_)) => Ok(None),
                Err(_) => Ok(None),
            }
        });

        // loro:get_or_create_layer(name, type) -> LuaLoroList | LuaLoroMap
        methods.add_method("get_or_create_layer", |_, this, (name, layer_type): (String, String)| {
            match layer_type.as_str() {
                "list" => {
                    let (tx, rx) = oneshot::channel();
                    this.scribe_ref.cast(ScribeMessage::GetOrCreateLoroList {
                        layer_name: name.clone(),
                        reply: tx,
                    }).map_err(|e| LuaError::RuntimeError(format!("Failed to get/create list: {}", e)))?;

                    let result = block_on_async(async {
                        rx.await
                            .map_err(|e| LuaError::RuntimeError(format!("Channel error: {}", e)))?
                            .map_err(|e| LuaError::RuntimeError(format!("Scribe error: {}", e)))
                    })??;

                    Ok(LayerWrapper::List(LuaLoroList::new(result, this.scribe_ref.clone(), name)))
                }
                "map" => {
                    let (tx, rx) = oneshot::channel();
                    this.scribe_ref.cast(ScribeMessage::GetOrCreateLoroMap {
                        layer_name: name.clone(),
                        reply: tx,
                    }).map_err(|e| LuaError::RuntimeError(format!("Failed to get/create map: {}", e)))?;

                    let result = block_on_async(async {
                        rx.await
                            .map_err(|e| LuaError::RuntimeError(format!("Channel error: {}", e)))?
                            .map_err(|e| LuaError::RuntimeError(format!("Scribe error: {}", e)))
                    })??;

                    Ok(LayerWrapper::Map(LuaLoroMap::new(result, this.scribe_ref.clone(), name)))
                }
                _ => Err(LuaError::RuntimeError(format!("Invalid layer type: {}", layer_type))),
            }
        });

        // loro:get_layer(name, type) -> LuaLoroList | LuaLoroMap | nil
        methods.add_method("get_layer", |_, this, (name, layer_type): (String, String)| {
            match layer_type.as_str() {
                "list" => {
                    let (tx, rx) = oneshot::channel();
                    this.scribe_ref.cast(ScribeMessage::GetLoroList {
                        layer_name: name.clone(),
                        reply: tx,
                    }).map_err(|e| LuaError::RuntimeError(format!("Failed to get list: {}", e)))?;

                    let channel_result = block_on_async(async {
                        rx.await.map_err(|e| LuaError::RuntimeError(format!("Channel error: {}", e)))
                    })?;

                    match channel_result {
                        Ok(Ok(list)) => Ok(Some(LayerWrapper::List(LuaLoroList::new(list, this.scribe_ref.clone(), name)))),
                        Ok(Err(_)) => Ok(None),
                        Err(e) => Err(e),
                    }
                }
                "map" => {
                    let (tx, rx) = oneshot::channel();
                    this.scribe_ref.cast(ScribeMessage::GetLoroMap {
                        layer_name: name.clone(),
                        reply: tx,
                    }).map_err(|e| LuaError::RuntimeError(format!("Failed to get map: {}", e)))?;

                    let channel_result = block_on_async(async {
                        rx.await.map_err(|e| LuaError::RuntimeError(format!("Channel error: {}", e)))
                    })?;

                    match channel_result {
                        Ok(Ok(map)) => Ok(Some(LayerWrapper::Map(LuaLoroMap::new(map, this.scribe_ref.clone(), name)))),
                        Ok(Err(_)) => Ok(None),
                        Err(e) => Err(e),
                    }
                }
                _ => Err(LuaError::RuntimeError(format!("Invalid layer type: {}", layer_type))),
            }
        });

        // loro:list_layers(pattern) -> table of layer names
        methods.add_method("list_layers", |lua, this, pattern: String| {
            let (tx, rx) = oneshot::channel();
            this.scribe_ref.cast(ScribeMessage::ListLayers {
                pattern,
                reply: tx,
            }).map_err(|e| LuaError::RuntimeError(format!("Failed to list layers: {}", e)))?;

            let channel_result = block_on_async(async {
                rx.await
                    .map_err(|e| LuaError::RuntimeError(format!("Channel error: {}", e)))
            })?;

            let names = channel_result
                .map_err(|e| LuaError::RuntimeError(format!("List layers error: {:?}", e)))?;

            let table = lua.create_table()?;
            for (i, name) in names.iter().enumerate() {
                table.set(i + 1, name.clone())?;
            }
            Ok(table)
        });
    }
}

// =============================================================================
// Layer Wrapper (for get_or_create_layer return type)
// =============================================================================

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
                LayerWrapper::List(list) => {
                    let loro_value = lua_to_loro_value(&value)?;
                    list.push_value(loro_value)?;
                    Ok(())
                }
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
                    let loro_value = lua_to_loro_value(&value)?;
                    list.set_at(index, loro_value)
                }
                LayerWrapper::Map(map) => {
                    let key_str = match key {
                        LuaValue::String(s) => s.to_str()?.to_string(),
                        _ => return Err(LuaError::RuntimeError("Map key must be string".to_string())),
                    };
                    let loro_value = lua_to_loro_value(&value)?;
                    map.set_value(&key_str, loro_value)
                }
            }
        });

        methods.add_method("length", |_, this, ()| {
            match this {
                LayerWrapper::List(list) => Ok(list.length()),
                LayerWrapper::Map(map) => Ok(map.length()),
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

// =============================================================================
// Lua Loro List
// =============================================================================

/// Lua wrapper for LoroList
pub struct LuaLoroList {
    list: LoroList,
    scribe_ref: ActorRef<ScribeMessage>,
    layer_name: String,
}

impl LuaLoroList {
    pub fn new(list: LoroList, scribe_ref: ActorRef<ScribeMessage>, layer_name: String) -> Self {
        Self { list, scribe_ref, layer_name }
    }

    pub(crate) fn push_value(&self, value: LoroValue) -> Result<(), LuaError> {
        self.list.push(value)
            .map_err(|e| LuaError::RuntimeError(format!("Failed to push: {}", e)))?;

        self.commit_layer()
    }

    pub(crate) fn get_at(&self, lua: &mlua::Lua, index: usize) -> Result<LuaValue, LuaError> {
        match self.list.get(index) {
            Some(value_or_container) => {
                let value = value_or_container.get_deep_value();
                loro_value_to_lua(lua, &value)
            }
            None => Ok(LuaValue::Nil),
        }
    }

    pub(crate) fn set_at(&self, index: usize, value: LoroValue) -> Result<(), LuaError> {
        info!(layer = %self.layer_name, index, "LuaLoroList::set_at called");

        self.list.delete(index, 1)
            .map_err(|e| LuaError::RuntimeError(format!("Failed to delete for set: {}", e)))?;
        self.list.insert(index, value)
            .map_err(|e| LuaError::RuntimeError(format!("Failed to insert for set: {}", e)))?;

        info!(layer = %self.layer_name, "LuaLoroList::set_at calling commit_layer");
        self.commit_layer()
    }

    pub(crate) fn delete_at(&self, index: usize) -> Result<(), LuaError> {
        self.list.delete(index, 1)
            .map_err(|e| LuaError::RuntimeError(format!("Failed to delete: {}", e)))?;

        self.commit_layer()
    }

    pub(crate) fn length(&self) -> usize {
        self.list.len()
    }

    fn commit_layer(&self) -> Result<(), LuaError> {
        info!(layer = %self.layer_name, "LuaLoroList::commit_layer sending CommitLayer to Scribe");
        self.scribe_ref.cast(ScribeMessage::CommitLayer {
            layer_name: self.layer_name.clone(),
        }).map_err(|e| LuaError::RuntimeError(format!("Failed to commit: {}", e)))
    }
}

impl UserData for LuaLoroList {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("push", |_, this, value: LuaValue| {
            let loro_value = lua_to_loro_value(&value)?;
            this.push_value(loro_value)
        });

        methods.add_method("get", |lua, this, index: usize| {
            this.get_at(lua, index)
        });

        methods.add_method("set", |_, this, (index, value): (usize, LuaValue)| {
            let loro_value = lua_to_loro_value(&value)?;
            this.set_at(index, loro_value)
        });

        methods.add_method("delete", |_, this, index: usize| {
            this.delete_at(index)
        });

        methods.add_method("length", |_, this, ()| {
            Ok(this.length())
        });
    }
}

// =============================================================================
// Lua Loro Map
// =============================================================================

/// Lua wrapper for LoroMap
pub struct LuaLoroMap {
    map: LoroMap,
    scribe_ref: ActorRef<ScribeMessage>,
    layer_name: String,
}

impl LuaLoroMap {
    pub fn new(map: LoroMap, scribe_ref: ActorRef<ScribeMessage>, layer_name: String) -> Self {
        Self { map, scribe_ref, layer_name }
    }

    pub(crate) fn set_value(&self, key: &str, value: LoroValue) -> Result<(), LuaError> {
        self.map.insert(key, value)
            .map_err(|e| LuaError::RuntimeError(format!("Failed to set: {}", e)))?;

        self.commit_layer()
    }

    pub(crate) fn get_value(&self, lua: &mlua::Lua, key: &str) -> Result<LuaValue, LuaError> {
        match self.map.get(key) {
            Some(value_or_container) => {
                let value = value_or_container.get_deep_value();
                loro_value_to_lua(lua, &value)
            }
            None => Ok(LuaValue::Nil),
        }
    }

    pub(crate) fn delete_key(&self, key: &str) -> Result<(), LuaError> {
        self.map.delete(key)
            .map_err(|e| LuaError::RuntimeError(format!("Failed to delete: {}", e)))?;

        self.commit_layer()
    }

    pub(crate) fn length(&self) -> usize {
        self.map.len()
    }

    pub(crate) fn keys(&self, lua: &mlua::Lua) -> Result<mlua::Table, LuaError> {
        let table = lua.create_table()?;
        for (i, key) in self.map.keys().enumerate() {
            table.set(i + 1, key.to_string())?;
        }
        Ok(table)
    }

    fn commit_layer(&self) -> Result<(), LuaError> {
        self.scribe_ref.cast(ScribeMessage::CommitLayer {
            layer_name: self.layer_name.clone(),
        }).map_err(|e| LuaError::RuntimeError(format!("Failed to commit: {}", e)))
    }
}

impl UserData for LuaLoroMap {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("set", |_, this, (key, value): (String, LuaValue)| {
            let loro_value = lua_to_loro_value(&value)?;
            this.set_value(&key, loro_value)
        });

        methods.add_method("get", |lua, this, key: String| {
            this.get_value(lua, &key)
        });

        methods.add_method("delete", |_, this, key: String| {
            this.delete_key(&key)
        });

        methods.add_method("length", |_, this, ()| {
            Ok(this.length())
        });

        methods.add_method("keys", |lua, this, ()| {
            this.keys(lua)
        });
    }
}
