//! Slint VecModel FFI bindings for Lua.
//!
//! Lua gets an Rc handle (shared with Slint, no copy) and mutations trigger ModelNotify.

use lua_runtime::{json_to_lua, lua_to_json_err};
use mlua::{Error as LuaError, UserData, UserDataMethods, Value as LuaValue};
use slint::{Model, VecModel};
use slint_interpreter::Value as SlintValue;
use std::rc::Rc;

use crate::value_convert::{json_to_slint_value, slint_value_to_json};

/// Lua wrapper for Slint VecModel (shared Rc; mutations trigger ModelNotify).
pub struct LuaSlintModel {
    model: Rc<VecModel<SlintValue>>,
    property_name: String,
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
        methods.add_method("len", |_, this, ()| Ok(this.model.row_count()));

        methods.add_method("push", |_lua, this, item: LuaValue| {
            let slint_val = lua_to_slint_value_via_json(&item)?;
            this.model.push(slint_val);
            tracing::debug!(
                property = %this.property_name,
                "VecModel::push triggered (UI will update row)"
            );
            Ok(())
        });

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

        methods.add_method("get", |lua, this, index: usize| {
            match this.model.row_data(index) {
                Some(val) => slint_to_lua_value(lua, &val),
                None => Ok(LuaValue::Nil),
            }
        });

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
