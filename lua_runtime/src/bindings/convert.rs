//! Value conversion between Loro, Lua, and JSON types
//!
//! Shared conversion functions used by:
//! - LuaRuntime (lua_runtime)
//! - LuaWorker (lua_runtime)
//! - ScribeLuaRuntime (lua_runtime)

use mlua::{Error as LuaError, Value as LuaValue};
use serde_json::Value as JsonValue;

// JSON <-> Lua conversions

/// Convert JSON value to Lua value
pub fn json_to_lua(lua: &mlua::Lua, json: &JsonValue) -> Result<LuaValue, LuaError> {
    match json {
        JsonValue::Null => Ok(LuaValue::Nil),
        JsonValue::Bool(b) => Ok(LuaValue::Boolean(*b)),
        JsonValue::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(LuaValue::Integer(i))
            } else if let Some(f) = n.as_f64() {
                Ok(LuaValue::Number(f))
            } else {
                Ok(LuaValue::Nil)
            }
        }
        JsonValue::String(s) => {
            let lua_str = lua.create_string(s)?;
            Ok(LuaValue::String(lua_str))
        }
        JsonValue::Array(arr) => {
            let table = lua.create_table()?;
            for (i, v) in arr.iter().enumerate() {
                let lua_v = json_to_lua(lua, v)?;
                table.set(i + 1, lua_v)?;
            }
            Ok(LuaValue::Table(table))
        }
        JsonValue::Object(obj) => {
            let table = lua.create_table()?;
            for (k, v) in obj {
                let lua_v = json_to_lua(lua, v)?;
                table.set(k.clone(), lua_v)?;
            }
            Ok(LuaValue::Table(table))
        }
    }
}

/// Convert Lua value to JSON value
pub fn lua_to_json(value: &LuaValue) -> Result<JsonValue, LuaError> {
    match value {
        LuaValue::Nil => Ok(JsonValue::Null),
        LuaValue::Boolean(b) => Ok(JsonValue::Bool(*b)),
        LuaValue::Integer(i) => Ok(JsonValue::Number((*i).into())),
        LuaValue::Number(f) => serde_json::Number::from_f64(*f)
            .map(JsonValue::Number)
            .ok_or_else(|| LuaError::RuntimeError("Invalid float value".to_string())),
        LuaValue::String(s) => {
            let str_val = s.to_str()?;
            Ok(JsonValue::String(str_val.to_string()))
        }
        LuaValue::Table(table) => {
            // Check if it's an array (sequential integer keys starting at 1)
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
                // Array
                let mut arr = Vec::with_capacity(max_index);
                for i in 1..=max_index {
                    let val: LuaValue = table.get(i)?;
                    arr.push(lua_to_json(&val)?);
                }
                Ok(JsonValue::Array(arr))
            } else if is_array && max_index == 0 {
                // Empty table - treat as empty array
                Ok(JsonValue::Array(vec![]))
            } else {
                // Object (has non-integer keys)
                let mut obj = serde_json::Map::new();
                for pair in table.clone().pairs::<LuaValue, LuaValue>() {
                    let (key, val) = pair?;
                    let key_str = match key {
                        LuaValue::String(s) => s.to_str()?.to_string(),
                        LuaValue::Integer(i) => i.to_string(),
                        _ => continue,
                    };
                    obj.insert(key_str, lua_to_json(&val)?);
                }
                Ok(JsonValue::Object(obj))
            }
        }
        _ => Err(LuaError::RuntimeError(format!(
            "Unsupported Lua type for JSON conversion: {:?}",
            value
        ))),
    }
}

/// Convert Lua value to JSON and normalize all failures as RuntimeError.
pub fn lua_to_json_err(value: &LuaValue) -> Result<JsonValue, LuaError> {
    lua_to_json(value).map_err(|e| LuaError::RuntimeError(e.to_string()))
}

// Pattern matching for layer names

/// Check if layer name matches a glob pattern
///
/// Supports `*` as wildcard for a single path segment.
///
/// **Examples**:
/// - `"orders/*"` matches `"orders/did:key:abc"`
/// - `"shop/orders/*"` matches `"shop/orders/customer_123"`
/// - `"*"` matches any single-segment name
/// - `"*/*"` matches any two-segment name
pub fn matches_layer_pattern(pattern: &str, layer_name: &str) -> bool {
    if pattern == "*" && !layer_name.contains('/') {
        return true;
    }

    let pattern_parts: Vec<&str> = pattern.split('/').collect();
    let layer_parts: Vec<&str> = layer_name.split('/').collect();

    if pattern_parts.len() != layer_parts.len() {
        return false;
    }

    pattern_parts
        .iter()
        .zip(layer_parts.iter())
        .all(|(p, l)| *p == "*" || p == l)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_matches_layer_pattern() {
        // Wildcard matches
        assert!(matches_layer_pattern("orders/*", "orders/did:key:abc"));
        assert!(matches_layer_pattern("orders/*", "orders/did:key:xyz"));
        assert!(matches_layer_pattern(
            "shop/orders/*",
            "shop/orders/customer"
        ));
        assert!(matches_layer_pattern("*/*", "orders/customer"));

        // Exact matches
        assert!(matches_layer_pattern("products", "products"));
        assert!(matches_layer_pattern("shop/products", "shop/products"));

        // Non-matches
        assert!(!matches_layer_pattern("orders/*", "bookings/did:key:abc"));
        assert!(!matches_layer_pattern("orders/*", "orders/sub/did:key:abc"));
        assert!(!matches_layer_pattern("orders", "orders/extra"));
    }
}
