//! Slint VecModel FFI Bindings for Lua
//!
//! Provides direct access to Slint VecModel for fine-grained UI updates.
//! Lua receives model handles (Rc pointers), not data copies.
//!
//! **Pattern**: Mirrors loro_bindings.rs - Lua gets lightweight handles
//! **Memory**: Just an Rc pointer (shared with Slint, no copy)
//! **Operations**: Directly mutate the model, triggering ModelNotify

use lua_runtime::{json_to_lua, lua_to_json_err};
use mlua::{Error as LuaError, UserData, UserDataMethods, Value as LuaValue};
use slint::{Model, VecModel};
use slint_interpreter::Value as SlintValue;
use std::rc::Rc;

use crate::value_convert::{json_to_slint_value, slint_value_to_json};

/// Lua wrapper for Slint VecModel
///
/// **Memory**: Just an Rc pointer (shared with Slint, no copy)
/// **Operations**: Directly mutate the model, triggering ModelNotify
pub struct LuaSlintModel {
    model: Rc<VecModel<SlintValue>>,
    property_name: String, // For debugging/logging
}

impl LuaSlintModel {
    pub fn new(model: Rc<VecModel<SlintValue>>, property_name: String) -> Self {
        Self {
            model,
            property_name,
        }
    }
}

impl UserData for LuaSlintModel {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        // model:len() - Get row count
        methods.add_method("len", |_, this, ()| Ok(this.model.row_count()));

        // model:push(item) - Append item to end
        // Automatically triggers ModelNotify::row_added()
        methods.add_method("push", |_lua, this, item: LuaValue| {
            let slint_val = lua_to_slint_value_via_json(&item)?;
            this.model.push(slint_val);
            tracing::debug!(
                property = %this.property_name,
                "VecModel::push triggered (UI will update row)"
            );
            Ok(())
        });

        // model:insert(index, item) - Insert at position
        // Automatically triggers ModelNotify::row_added(index)
        methods.add_method("insert", |_lua, this, (index, item): (usize, LuaValue)| {
            let slint_val = lua_to_slint_value_via_json(&item)?;
            this.model.insert(index, slint_val);
            tracing::debug!(
                property = %this.property_name,
                index = index,
                "VecModel::insert triggered"
            );
            Ok(())
        });

        // model:remove(index) - Remove item at index
        // Automatically triggers ModelNotify::row_removed(index)
        methods.add_method("remove", |_, this, index: usize| {
            if index < this.model.row_count() {
                this.model.remove(index);
                tracing::debug!(
                    property = %this.property_name,
                    index = index,
                    "VecModel::remove triggered"
                );
                Ok(())
            } else {
                Err(LuaError::RuntimeError(format!(
                    "Index {} out of bounds (len={})",
                    index,
                    this.model.row_count()
                )))
            }
        });

        // model:get(index) - Get item at index (read-only access)
        methods.add_method("get", |lua, this, index: usize| {
            match this.model.row_data(index) {
                Some(val) => slint_to_lua_value(lua, &val),
                None => Ok(LuaValue::Nil),
            }
        });

        // model:set(index, item) - Update item at index
        // Triggers ModelNotify::row_changed(index)
        methods.add_method("set", |_lua, this, (index, item): (usize, LuaValue)| {
            if index < this.model.row_count() {
                let slint_val = lua_to_slint_value_via_json(&item)?;
                this.model.set_row_data(index, slint_val);
                tracing::debug!(
                    property = %this.property_name,
                    index = index,
                    "VecModel::set_row_data triggered"
                );
                Ok(())
            } else {
                Err(LuaError::RuntimeError(format!(
                    "Index {} out of bounds (len={})",
                    index,
                    this.model.row_count()
                )))
            }
        });

        // model:clear() - Remove all items
        methods.add_method("clear", |_, this, ()| {
            this.model.set_vec(vec![]);
            tracing::debug!(
                property = %this.property_name,
                "VecModel::clear triggered"
            );
            Ok(())
        });
    }
}

fn lua_to_slint_value_via_json(value: &LuaValue) -> Result<SlintValue, LuaError> {
    let json = lua_to_json_err(value)?;
    json_to_slint_value(&json).map_err(|e| LuaError::RuntimeError(e.to_string()))
}

/// Convert Slint value to Lua value
fn slint_to_lua_value(lua: &mlua::Lua, value: &SlintValue) -> Result<LuaValue, LuaError> {
    let json = slint_value_to_json(value);
    json_to_lua(lua, &json)
}
