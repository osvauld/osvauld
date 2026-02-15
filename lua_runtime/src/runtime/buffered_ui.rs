use std::collections::HashMap;
use std::sync::{Arc, Mutex as StdMutex};

use mlua::{UserData, UserDataMethods, Value};
use serde_json::Value as JsonValue;

use crate::bindings::convert::{json_to_lua, lua_to_json_err};
use crate::ui_types::{PropertyUpdate, UiMutation, VecModelOp};

/// Buffered UI bindings for headless runtime.
///
/// Stores properties in-memory and buffers all mutations for test inspection.
pub struct BufferedUiBindings {
    state: Arc<StdMutex<BufferedUiState>>,
}

/// Internal state for buffered UI bindings.
pub struct BufferedUiState {
    /// Property values (scalar values set via ui:set)
    pub properties: HashMap<String, JsonValue>,
    /// Accumulated mutations (for test inspection via drain_mutations)
    pub mutations: Vec<UiMutation>,
    /// Model data (arrays set via ui:set with array values)
    pub models: HashMap<String, Vec<JsonValue>>,
}

impl BufferedUiState {
    pub(crate) fn new() -> Self {
        Self {
            properties: HashMap::new(),
            mutations: Vec::new(),
            models: HashMap::new(),
        }
    }
}

impl BufferedUiBindings {
    pub(crate) fn new() -> Self {
        Self {
            state: Arc::new(StdMutex::new(BufferedUiState::new())),
        }
    }
}

impl UserData for BufferedUiBindings {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("get", |lua, this, key: String| {
            let state = this.state.lock().unwrap();
            match state.properties.get(&key) {
                Some(v) => json_to_lua(lua, v),
                None => Ok(Value::Nil),
            }
        });

        methods.add_method("set", |_lua, this, (key, value): (String, Value)| {
            let json_value = lua_to_json_err(&value)?;
            let mut state = this.state.lock().unwrap();

            if let JsonValue::Array(ref items) = json_value {
                state.models.insert(key.clone(), items.clone());
                state.mutations.push(UiMutation {
                    app_id: String::new(),
                    properties: vec![],
                    model_ops: vec![VecModelOp::Replace {
                        model_name: key,
                        items: items.clone(),
                    }],
                });
            } else {
                state.properties.insert(key.clone(), json_value.clone());
                state.mutations.push(UiMutation {
                    app_id: String::new(),
                    properties: vec![PropertyUpdate {
                        key,
                        value: json_value,
                    }],
                    model_ops: vec![],
                });
            }
            Ok(())
        });

        methods.add_method("push", |_lua, this, (model_name, item): (String, Value)| {
            let item_json = lua_to_json_err(&item)?;
            let mut state = this.state.lock().unwrap();
            state
                .models
                .entry(model_name.clone())
                .or_default()
                .push(item_json.clone());
            state.mutations.push(UiMutation {
                app_id: String::new(),
                properties: vec![],
                model_ops: vec![VecModelOp::Push {
                    model_name,
                    item: item_json,
                }],
            });
            Ok(())
        });

        methods.add_method(
            "insert",
            |_lua, this, (model_name, index, item): (String, usize, Value)| {
                let item_json = lua_to_json_err(&item)?;
                let mut state = this.state.lock().unwrap();
                let model = state.models.entry(model_name.clone()).or_default();
                if index <= model.len() {
                    model.insert(index, item_json.clone());
                }
                state.mutations.push(UiMutation {
                    app_id: String::new(),
                    properties: vec![],
                    model_ops: vec![VecModelOp::Insert {
                        model_name,
                        index,
                        item: item_json,
                    }],
                });
                Ok(())
            },
        );

        methods.add_method(
            "remove",
            |_lua, this, (model_name, index): (String, usize)| {
                let mut state = this.state.lock().unwrap();
                if let Some(model) = state.models.get_mut(&model_name) {
                    if index < model.len() {
                        model.remove(index);
                    }
                }
                state.mutations.push(UiMutation {
                    app_id: String::new(),
                    properties: vec![],
                    model_ops: vec![VecModelOp::Remove { model_name, index }],
                });
                Ok(())
            },
        );

        methods.add_method("clear", |_lua, this, model_name: String| {
            let mut state = this.state.lock().unwrap();
            state.models.insert(model_name.clone(), vec![]);
            state.mutations.push(UiMutation {
                app_id: String::new(),
                properties: vec![],
                model_ops: vec![VecModelOp::Clear { model_name }],
            });
            Ok(())
        });

        methods.add_method(
            "update",
            |_lua, this, (model_name, index, item): (String, usize, Value)| {
                let item_json = lua_to_json_err(&item)?;
                let mut state = this.state.lock().unwrap();
                if let Some(model) = state.models.get_mut(&model_name) {
                    if index < model.len() {
                        model[index] = item_json.clone();
                    }
                }
                state.mutations.push(UiMutation {
                    app_id: String::new(),
                    properties: vec![],
                    model_ops: vec![VecModelOp::Set {
                        model_name,
                        index,
                        item: item_json,
                    }],
                });
                Ok(())
            },
        );
    }
}
