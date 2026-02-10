//! Slint VecModel FFI Bindings for Lua
//!
//! Provides direct access to Slint VecModel for fine-grained UI updates.
//! Lua receives model handles (Rc pointers), not data copies.
//!
//! **Pattern**: Mirrors loro_bindings.rs - Lua gets lightweight handles
//! **Memory**: Just an Rc pointer (shared with Slint, no copy)
//! **Operations**: Directly mutate the model, triggering ModelNotify

use slint::{VecModel, Model};
use slint_interpreter::Value as SlintValue;
use mlua::{UserData, UserDataMethods, Value as LuaValue, Error as LuaError};
use std::rc::Rc;

/// Lua wrapper for Slint VecModel
///
/// **Memory**: Just an Rc pointer (shared with Slint, no copy)
/// **Operations**: Directly mutate the model, triggering ModelNotify
pub struct LuaSlintModel {
    model: Rc<VecModel<SlintValue>>,
    property_name: String,  // For debugging/logging
}

impl LuaSlintModel {
    pub fn new(model: Rc<VecModel<SlintValue>>, property_name: String) -> Self {
        Self { model, property_name }
    }
}

impl UserData for LuaSlintModel {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        // model:len() - Get row count
        methods.add_method("len", |_, this, ()| {
            Ok(this.model.row_count())
        });

        // model:push(item) - Append item to end
        // Automatically triggers ModelNotify::row_added()
        methods.add_method("push", |lua, this, item: LuaValue| {
            let slint_val = lua_to_slint_value(lua, &item)?;
            this.model.push(slint_val);
            tracing::debug!(
                property = %this.property_name,
                "VecModel::push triggered (UI will update row)"
            );
            Ok(())
        });

        // model:insert(index, item) - Insert at position
        // Automatically triggers ModelNotify::row_added(index)
        methods.add_method("insert", |lua, this, (index, item): (usize, LuaValue)| {
            let slint_val = lua_to_slint_value(lua, &item)?;
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
        methods.add_method("set", |lua, this, (index, item): (usize, LuaValue)| {
            if index < this.model.row_count() {
                let slint_val = lua_to_slint_value(lua, &item)?;
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

/// Convert Lua value to Slint value
fn lua_to_slint_value(lua: &mlua::Lua, value: &LuaValue) -> Result<SlintValue, LuaError> {
    use slint_interpreter::Value;

    match value {
        LuaValue::Nil => Ok(Value::Void),
        LuaValue::Boolean(b) => Ok(Value::Bool(*b)),
        LuaValue::Integer(i) => Ok(Value::Number(*i as f64)),
        LuaValue::Number(f) => Ok(Value::Number(*f)),
        LuaValue::String(s) => {
            // BorrowedBytes implements AsRef<[u8]>
            let borrowed_bytes = s.as_bytes();
            let bytes: &[u8] = borrowed_bytes.as_ref();
            let str_val = std::str::from_utf8(bytes)
                .map_err(|e| LuaError::RuntimeError(format!("Invalid UTF-8: {}", e)))?;
            Ok(Value::String(slint::SharedString::from(str_val)))
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
                // Convert as array - create Slint Model
                let mut items = Vec::with_capacity(max_index);
                for i in 1..=max_index {
                    let val: LuaValue = table.get(i)?;
                    items.push(lua_to_slint_value(lua, &val)?);
                }
                Ok(Value::Model(std::rc::Rc::new(slint::VecModel::from(items)).into()))
            } else {
                // Convert as struct (Slint object)
                let mut fields = Vec::new();
                for pair in table.clone().pairs::<String, LuaValue>() {
                    let (key, val) = pair?;
                    let slint_val = lua_to_slint_value(lua, &val)?;
                    fields.push((key, slint_val));
                }
                Ok(Value::Struct(
                    slint_interpreter::Struct::from_iter(fields).into()
                ))
            }
        }
        _ => Err(LuaError::RuntimeError(format!(
            "Unsupported Lua type for Slint conversion: {:?}",
            value
        ))),
    }
}

/// Convert Slint value to Lua value
fn slint_to_lua_value(lua: &mlua::Lua, value: &SlintValue) -> Result<LuaValue, LuaError> {
    use slint_interpreter::Value;

    match value {
        Value::Void => Ok(LuaValue::Nil),
        Value::Bool(b) => Ok(LuaValue::Boolean(*b)),
        Value::Number(n) => Ok(LuaValue::Number(*n)),
        Value::String(s) => Ok(LuaValue::String(lua.create_string(s.as_str())?)),
        Value::Struct(s) => {
            // Convert Slint struct to Lua table
            let table = lua.create_table()?;
            // Iterate over the struct fields directly
            for field in s.iter() {
                let lua_val = slint_to_lua_value(lua, &field.1)?;
                // Use to_string() instead of unstable as_str()
                table.set(field.0.to_string(), lua_val)?;
            }
            Ok(LuaValue::Table(table))
        }
        Value::Model(_) => {
            // Model references - return nil for now
            // (Models should be accessed via ui.models, not queried)
            tracing::warn!("Attempted to convert Model to Lua (not supported)");
            Ok(LuaValue::Nil)
        }
        Value::Brush(_) | Value::Image(_) => {
            // UI-specific types - not used in app logic
            Ok(LuaValue::Nil)
        }
        _ => {
            // Catch-all for any new Slint value types
            tracing::warn!("Unknown Slint value type, returning nil");
            Ok(LuaValue::Nil)
        }
    }
}
