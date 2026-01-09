//! Loro FFI Bindings for Lua
//!
//! Provides direct access to Loro CRDT containers from Lua scripts.
//! All operations work on references to Scribe's LoroDoc - no copying.

use loro::{LoroList, LoroMap, LoroValue};
use mlua::{UserData, UserDataMethods, Value as LuaValue, Error as LuaError};
use tokio::sync::oneshot;
use butler::ScribeMessage;
use ractor::ActorRef;

/// Lua wrapper for LoroList container
///
/// **Memory**: Just a handle (Arc pointer + container ID), not a copy of data
/// **Operations**: Directly mutate Scribe's LoroDoc
pub struct LuaLoroList {
    list: LoroList,
    scribe_ref: ActorRef<ScribeMessage>,
    layer_name: String,
}

impl LuaLoroList {
    pub fn new(list: LoroList, scribe_ref: ActorRef<ScribeMessage>, layer_name: String) -> Self {
        Self { list, scribe_ref, layer_name }
    }
}

impl UserData for LuaLoroList {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        // list:len() - Get length of list
        methods.add_method("len", |_, this, ()| {
            Ok(this.list.len())
        });

        // list:length() - Alias for len() (Lua convention)
        methods.add_method("length", |_, this, ()| {
            Ok(this.list.len())
        });

        // list:get(index) - Get item at index (0-based)
        methods.add_method("get", |lua, this, index: usize| {
            match this.list.get(index) {
                Some(value_or_container) => {
                    // Convert ValueOrContainer to LoroValue
                    let loro_val = value_or_container.as_value()
                        .ok_or_else(|| LuaError::RuntimeError("Item is a container, not a value".to_string()))?;
                    loro_value_to_lua(lua, &loro_val)
                }
                None => Ok(LuaValue::Nil),
            }
        });

        // list:push(item) - Append item to end
        methods.add_method("push", |_, this, item: LuaValue| {
            let loro_val = lua_to_loro_value(&item)?;
            this.list.push(loro_val)
                .map_err(|e| LuaError::RuntimeError(format!("LoroList push failed: {}", e)))?;

            // Notify Scribe that layer was modified
            let layer_name = this.layer_name.clone();
            this.scribe_ref.cast(ScribeMessage::LayerModifiedByLua { layer_name })
                .map_err(|e| LuaError::RuntimeError(format!("Failed to notify Scribe: {}", e)))?;

            Ok(())
        });

        // list:insert(index, item) - Insert item at index
        methods.add_method("insert", |_, this, (index, item): (usize, LuaValue)| {
            let loro_val = lua_to_loro_value(&item)?;
            this.list.insert(index, loro_val)
                .map_err(|e| LuaError::RuntimeError(format!("LoroList insert failed: {}", e)))?;
            Ok(())
        });

        // list:delete(index) - Delete item at index
        methods.add_method("delete", |_, this, index: usize| {
            this.list.delete(index, 1)
                .map_err(|e| LuaError::RuntimeError(format!("LoroList delete failed: {}", e)))?;
            Ok(())
        });

        // list:set(index, item) - Update item at index (delete + insert)
        methods.add_method("set", |_, this, (index, item): (usize, LuaValue)| {
            let loro_val = lua_to_loro_value(&item)?;
            // Delete then insert to update (LoroList doesn't have direct set)
            this.list.delete(index, 1)
                .map_err(|e| LuaError::RuntimeError(format!("LoroList set (delete) failed: {}", e)))?;
            this.list.insert(index, loro_val)
                .map_err(|e| LuaError::RuntimeError(format!("LoroList set (insert) failed: {}", e)))?;
            // Notify Scribe of the modification
            let _ = this.scribe_ref.cast(ScribeMessage::LayerModifiedByLua {
                layer_name: this.layer_name.clone(),
            });
            Ok(())
        });

        // TODO: Add iterator support via __pairs metamethod
    }
}

/// Lua wrapper for LoroMap container
///
/// **Memory**: Just a handle (Arc pointer + container ID), not a copy of data
/// **Operations**: Directly mutate Scribe's LoroDoc
pub struct LuaLoroMap {
    map: LoroMap,
    scribe_ref: ActorRef<ScribeMessage>,
    layer_name: String,
}

impl LuaLoroMap {
    pub fn new(map: LoroMap, scribe_ref: ActorRef<ScribeMessage>, layer_name: String) -> Self {
        Self { map, scribe_ref, layer_name }
    }
}

impl UserData for LuaLoroMap {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        // map:get(key) - Get value by key
        methods.add_method("get", |lua, this, key: String| {
            match this.map.get(&key) {
                Some(value_or_container) => {
                    // Convert ValueOrContainer to LoroValue
                    let loro_val = value_or_container.as_value()
                        .ok_or_else(|| LuaError::RuntimeError("Value is a container, not a value".to_string()))?;
                    loro_value_to_lua(lua, &loro_val)
                }
                None => Ok(LuaValue::Nil),
            }
        });

        // map:set(key, value) - Set value for key
        methods.add_method("set", |_, this, (key, value): (String, LuaValue)| {
            let loro_val = lua_to_loro_value(&value)?;
            this.map.insert(&key, loro_val)
                .map_err(|e| LuaError::RuntimeError(format!("LoroMap set failed: {}", e)))?;

            // Notify Scribe that layer was modified
            let layer_name = this.layer_name.clone();
            this.scribe_ref.cast(ScribeMessage::LayerModifiedByLua { layer_name })
                .map_err(|e| LuaError::RuntimeError(format!("Failed to notify Scribe: {}", e)))?;

            Ok(())
        });

        // map:delete(key) - Delete key
        methods.add_method("delete", |_, this, key: String| {
            this.map.delete(&key)
                .map_err(|e| LuaError::RuntimeError(format!("LoroMap delete failed: {}", e)))?;
            Ok(())
        });

        // map:has(key) - Check if key exists
        methods.add_method("has", |_, this, key: String| {
            Ok(this.map.get(&key).is_some())
        });

        // TODO: Add iterator support via __pairs metamethod
    }
}

/// Convert LoroValue to Lua value
fn loro_value_to_lua(lua: &mlua::Lua, value: &LoroValue) -> Result<LuaValue, LuaError> {
    match value {
        LoroValue::Null => Ok(LuaValue::Nil),
        LoroValue::Bool(b) => Ok(LuaValue::Boolean(*b)),
        LoroValue::Double(f) => Ok(LuaValue::Number(*f)),
        LoroValue::I64(i) => Ok(LuaValue::Number(*i as f64)),
        LoroValue::String(s) => Ok(LuaValue::String(lua.create_string(s.as_str())?)),
        LoroValue::Binary(bytes) => {
            // Convert binary to Lua string (Lua strings can contain arbitrary bytes)
            Ok(LuaValue::String(lua.create_string(bytes.as_slice())?))
        }
        LoroValue::List(arc_vec) => {
            // Convert to Lua table (array)
            let table = lua.create_table()?;
            for (i, item) in arc_vec.iter().enumerate() {
                table.set(i + 1, loro_value_to_lua(lua, item)?)?; // Lua arrays are 1-indexed
            }
            Ok(LuaValue::Table(table))
        }
        LoroValue::Map(arc_map) => {
            // Convert to Lua table (map)
            let table = lua.create_table()?;
            for (k, v) in arc_map.iter() {
                table.set(k.as_str(), loro_value_to_lua(lua, v)?)?;
            }
            Ok(LuaValue::Table(table))
        }
        LoroValue::Container(_container_id) => {
            // Container references - for now, return nil
            // TODO: Could return wrapped container (LuaLoroList/LuaLoroMap)
            tracing::warn!("Container value conversion not yet implemented");
            Ok(LuaValue::Nil)
        }
    }
}

/// Convert Lua value to LoroValue
fn lua_to_loro_value(value: &LuaValue) -> Result<LoroValue, LuaError> {
    match value {
        LuaValue::Nil => Ok(LoroValue::Null),
        LuaValue::Boolean(b) => Ok(LoroValue::Bool(*b)),
        LuaValue::Integer(i) => Ok(LoroValue::I64(*i)),
        LuaValue::Number(f) => Ok(LoroValue::Double(*f)),
        LuaValue::String(s) => {
            let bytes = s.as_bytes();
            // Try to interpret as UTF-8 string first
            match std::str::from_utf8(&bytes) {
                Ok(str_val) => Ok(LoroValue::String(str_val.into())),
                Err(_) => {
                    // If not valid UTF-8, store as binary
                    Ok(LoroValue::Binary(bytes.to_vec().into()))
                }
            }
        }
        LuaValue::Table(table) => {
            // Check if it's an array (sequential integer keys starting from 1)
            let mut is_array = true;
            let mut max_index = 0;

            for pair in table.clone().pairs::<LuaValue, LuaValue>() {
                let (key, _) = pair?;
                match key {
                    LuaValue::Integer(i) if i > 0 => {
                        max_index = max_index.max(i as usize);
                    }
                    _ => {
                        is_array = false;
                        break;
                    }
                }
            }

            if is_array && max_index > 0 {
                // Convert as array
                let mut arr = Vec::with_capacity(max_index);
                for i in 1..=max_index {
                    let val: LuaValue = table.get(i)?;
                    arr.push(lua_to_loro_value(&val)?);
                }
                Ok(LoroValue::List(arr.into()))
            } else {
                // Convert as map (use Vec of tuples for LoroMapValue)
                let mut map = Vec::new();
                for pair in table.clone().pairs::<String, LuaValue>() {
                    let (key, val) = pair?;
                    map.push((key, lua_to_loro_value(&val)?));
                }
                Ok(LoroValue::Map(map.into()))
            }
        }
        LuaValue::UserData(_) => {
            // If it's already a Loro container, we can't convert it directly
            // For now, return an error
            Err(LuaError::RuntimeError(
                "Cannot convert UserData to LoroValue".to_string()
            ))
        }
        _ => Err(LuaError::RuntimeError(format!(
            "Unsupported Lua type for LoroValue conversion: {:?}",
            value
        ))),
    }
}

/// Loro bindings - main entry point for Lua to access Loro containers
///
/// **Usage in Lua**:
/// ```lua
/// local messages = loro.get_list("messages")
/// local users = loro.get_map("users")
/// ```
pub struct LoroBindings {
    scribe_ref: ActorRef<ScribeMessage>,
    page_id: String,
}

impl LoroBindings {
    pub fn new(scribe_ref: ActorRef<ScribeMessage>, page_id: String) -> Self {
        Self { scribe_ref, page_id }
    }
}

impl UserData for LoroBindings {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        // loro.get_list(layer_name) - Get a LoroList container
        methods.add_method("get_list", |_, this, layer_name: String| {
            // Create oneshot channel for reply
            let (tx, rx) = oneshot::channel();

            // Send message to Scribe (cast = fire-and-forget style, but we use oneshot for reply)
            this.scribe_ref.cast(ScribeMessage::GetLoroList {
                layer_name: layer_name.clone(),
                reply: tx,
            }).map_err(|e| LuaError::RuntimeError(format!("Failed to send GetLoroList message: {}", e)))?;

            // Block waiting for response
            // Handle case where we might not be in a Tokio runtime context (e.g., Qt/Slint event loop)
            let loro_list = match tokio::runtime::Handle::try_current() {
                Ok(handle) => {
                    // We're in a Tokio runtime, use block_in_place for efficiency
                    tokio::task::block_in_place(|| {
                        handle.block_on(async {
                            rx.await
                                .map_err(|e| LuaError::RuntimeError(format!("Scribe dropped reply channel: {}", e)))?
                                .map_err(|e| LuaError::RuntimeError(format!("Scribe error: {}", e)))
                        })
                    })?
                }
                Err(_) => {
                    // Not in a runtime, create a temporary one
                    // This happens when called from Qt/Slint event loop
                    let rt = tokio::runtime::Runtime::new()
                        .map_err(|e| LuaError::RuntimeError(format!("Failed to create runtime: {}", e)))?;
                    rt.block_on(async {
                        rx.await
                            .map_err(|e| LuaError::RuntimeError(format!("Scribe dropped reply channel: {}", e)))?
                            .map_err(|e| LuaError::RuntimeError(format!("Scribe error: {}", e)))
                    })?
                }
            };

            Ok(LuaLoroList::new(loro_list, this.scribe_ref.clone(), layer_name))
        });

        // loro.get_map(layer_name) - Get a LoroMap container
        methods.add_method("get_map", |_, this, layer_name: String| {
            // Create oneshot channel for reply
            let (tx, rx) = oneshot::channel();

            // Send message to Scribe
            this.scribe_ref.cast(ScribeMessage::GetLoroMap {
                layer_name: layer_name.clone(),
                reply: tx,
            }).map_err(|e| LuaError::RuntimeError(format!("Failed to send GetLoroMap message: {}", e)))?;

            // Block waiting for response
            // Handle case where we might not be in a Tokio runtime context
            let loro_map = match tokio::runtime::Handle::try_current() {
                Ok(handle) => {
                    tokio::task::block_in_place(|| {
                        handle.block_on(async {
                            rx.await
                                .map_err(|e| LuaError::RuntimeError(format!("Scribe dropped reply channel: {}", e)))?
                                .map_err(|e| LuaError::RuntimeError(format!("Scribe error: {}", e)))
                        })
                    })?
                }
                Err(_) => {
                    let rt = tokio::runtime::Runtime::new()
                        .map_err(|e| LuaError::RuntimeError(format!("Failed to create runtime: {}", e)))?;
                    rt.block_on(async {
                        rx.await
                            .map_err(|e| LuaError::RuntimeError(format!("Scribe dropped reply channel: {}", e)))?
                            .map_err(|e| LuaError::RuntimeError(format!("Scribe error: {}", e)))
                    })?
                }
            };

            Ok(LuaLoroMap::new(loro_map, this.scribe_ref.clone(), layer_name))
        });

        // loro:get_or_create_layer(name, type) - Get or create a layer
        // type is "list" or "map"
        methods.add_method("get_or_create_layer", |lua, this, (layer_name, layer_type): (String, String)| {
            match layer_type.as_str() {
                "list" => {
                    let (tx, rx) = oneshot::channel();
                    this.scribe_ref.cast(ScribeMessage::GetOrCreateLoroList {
                        layer_name: layer_name.clone(),
                        reply: tx,
                    }).map_err(|e| LuaError::RuntimeError(format!("Failed to send message: {}", e)))?;

                    let loro_list = match tokio::runtime::Handle::try_current() {
                        Ok(handle) => {
                            tokio::task::block_in_place(|| {
                                handle.block_on(async {
                                    rx.await
                                        .map_err(|e| LuaError::RuntimeError(format!("Scribe dropped channel: {}", e)))?
                                        .map_err(|e| LuaError::RuntimeError(format!("Scribe error: {}", e)))
                                })
                            })?
                        }
                        Err(_) => {
                            let rt = tokio::runtime::Runtime::new()
                                .map_err(|e| LuaError::RuntimeError(format!("Failed to create runtime: {}", e)))?;
                            rt.block_on(async {
                                rx.await
                                    .map_err(|e| LuaError::RuntimeError(format!("Scribe dropped channel: {}", e)))?
                                    .map_err(|e| LuaError::RuntimeError(format!("Scribe error: {}", e)))
                            })?
                        }
                    };
                    let wrapper = LuaLoroList::new(loro_list, this.scribe_ref.clone(), layer_name);
                    lua.create_userdata(wrapper).map(LuaValue::UserData)
                }
                "map" => {
                    let (tx, rx) = oneshot::channel();
                    this.scribe_ref.cast(ScribeMessage::GetOrCreateLoroMap {
                        layer_name: layer_name.clone(),
                        reply: tx,
                    }).map_err(|e| LuaError::RuntimeError(format!("Failed to send message: {}", e)))?;

                    let loro_map = match tokio::runtime::Handle::try_current() {
                        Ok(handle) => {
                            tokio::task::block_in_place(|| {
                                handle.block_on(async {
                                    rx.await
                                        .map_err(|e| LuaError::RuntimeError(format!("Scribe dropped channel: {}", e)))?
                                        .map_err(|e| LuaError::RuntimeError(format!("Scribe error: {}", e)))
                                })
                            })?
                        }
                        Err(_) => {
                            let rt = tokio::runtime::Runtime::new()
                                .map_err(|e| LuaError::RuntimeError(format!("Failed to create runtime: {}", e)))?;
                            rt.block_on(async {
                                rx.await
                                    .map_err(|e| LuaError::RuntimeError(format!("Scribe dropped channel: {}", e)))?
                                    .map_err(|e| LuaError::RuntimeError(format!("Scribe error: {}", e)))
                            })?
                        }
                    };
                    let wrapper = LuaLoroMap::new(loro_map, this.scribe_ref.clone(), layer_name);
                    lua.create_userdata(wrapper).map(LuaValue::UserData)
                }
                _ => Err(LuaError::RuntimeError(
                    format!("Unknown layer type: {}. Use 'list' or 'map'", layer_type)
                ))
            }
        });

        // loro:list_layers(pattern) - List layers matching pattern
        methods.add_method("list_layers", |lua, this, pattern: String| {
            let (tx, rx) = oneshot::channel();

            this.scribe_ref.cast(ScribeMessage::ListLayers {
                pattern,
                reply: tx,
            }).map_err(|e| LuaError::RuntimeError(format!("Failed to send ListLayers message: {}", e)))?;

            let layer_names: Vec<String> = match tokio::runtime::Handle::try_current() {
                Ok(handle) => {
                    tokio::task::block_in_place(|| {
                        handle.block_on(async {
                            rx.await
                                .map_err(|e| LuaError::RuntimeError(format!("Scribe dropped channel: {}", e)))
                        })
                    })?
                }
                Err(_) => {
                    let rt = tokio::runtime::Runtime::new()
                        .map_err(|e| LuaError::RuntimeError(format!("Failed to create runtime: {}", e)))?;
                    rt.block_on(async {
                        rx.await
                            .map_err(|e| LuaError::RuntimeError(format!("Scribe dropped channel: {}", e)))
                    })?
                }
            };

            // Convert to Lua table
            let table = lua.create_table()?;
            for (i, name) in layer_names.iter().enumerate() {
                table.set(i + 1, name.as_str())?;
            }
            Ok(table)
        });

        // loro:get_layer(name, type) - Get a layer with optional type ("list" or "map")
        // If type is omitted, defaults to "map" for backward compatibility
        methods.add_method("get_layer", |lua, this, (layer_name, layer_type): (String, Option<String>)| {
            let layer_type = layer_type.unwrap_or_else(|| "map".to_string());

            match layer_type.as_str() {
                "list" => {
                    let (tx, rx) = oneshot::channel();
                    this.scribe_ref.cast(ScribeMessage::GetLoroList {
                        layer_name: layer_name.clone(),
                        reply: tx,
                    }).map_err(|e| LuaError::RuntimeError(format!("Failed to send GetLoroList message: {}", e)))?;

                    let loro_list = match tokio::runtime::Handle::try_current() {
                        Ok(handle) => {
                            tokio::task::block_in_place(|| {
                                handle.block_on(async {
                                    rx.await
                                        .map_err(|e| LuaError::RuntimeError(format!("Scribe dropped channel: {}", e)))?
                                        .map_err(|e| LuaError::RuntimeError(format!("Scribe error: {}", e)))
                                })
                            })
                        }
                        Err(_) => {
                            let rt = tokio::runtime::Runtime::new()
                                .map_err(|e| LuaError::RuntimeError(format!("Failed to create runtime: {}", e)))?;
                            rt.block_on(async {
                                rx.await
                                    .map_err(|e| LuaError::RuntimeError(format!("Scribe dropped channel: {}", e)))?
                                    .map_err(|e| LuaError::RuntimeError(format!("Scribe error: {}", e)))
                            })
                        }
                    };

                    match loro_list {
                        Ok(list) => {
                            let wrapper = LuaLoroList::new(list, this.scribe_ref.clone(), layer_name);
                            lua.create_userdata(wrapper).map(|ud| LuaValue::UserData(ud))
                        }
                        Err(_) => Ok(LuaValue::Nil), // Return nil if layer doesn't exist
                    }
                }
                "map" | _ => {
                    let (tx, rx) = oneshot::channel();
                    this.scribe_ref.cast(ScribeMessage::GetLoroMap {
                        layer_name: layer_name.clone(),
                        reply: tx,
                    }).map_err(|e| LuaError::RuntimeError(format!("Failed to send GetLoroMap message: {}", e)))?;

                    let loro_map = match tokio::runtime::Handle::try_current() {
                        Ok(handle) => {
                            tokio::task::block_in_place(|| {
                                handle.block_on(async {
                                    rx.await
                                        .map_err(|e| LuaError::RuntimeError(format!("Scribe dropped channel: {}", e)))?
                                        .map_err(|e| LuaError::RuntimeError(format!("Scribe error: {}", e)))
                                })
                            })
                        }
                        Err(_) => {
                            let rt = tokio::runtime::Runtime::new()
                                .map_err(|e| LuaError::RuntimeError(format!("Failed to create runtime: {}", e)))?;
                            rt.block_on(async {
                                rx.await
                                    .map_err(|e| LuaError::RuntimeError(format!("Scribe dropped channel: {}", e)))?
                                    .map_err(|e| LuaError::RuntimeError(format!("Scribe error: {}", e)))
                            })
                        }
                    };

                    match loro_map {
                        Ok(map) => {
                            let wrapper = LuaLoroMap::new(map, this.scribe_ref.clone(), layer_name);
                            lua.create_userdata(wrapper).map(|ud| LuaValue::UserData(ud))
                        }
                        Err(_) => Ok(LuaValue::Nil), // Return nil if layer doesn't exist
                    }
                }
            }
        });
    }
}
