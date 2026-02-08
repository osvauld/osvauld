//! Value conversion between Loro, Lua, and JSON types
//!
//! Shared conversion functions used by:
//! - LuaRuntime (lua_runtime)
//! - LuaWorker (lua_runtime)
//! - ScribeLuaRuntime (lua_runtime)

use loro::LoroValue;
use mlua::{Value as LuaValue, Error as LuaError};
use serde_json::Value as JsonValue;

// Loro <-> Lua conversions

/// Convert LoroValue to Lua value
pub fn loro_value_to_lua(lua: &mlua::Lua, value: &LoroValue) -> Result<LuaValue, LuaError> {
    match value {
        LoroValue::Null => Ok(LuaValue::Nil),
        LoroValue::Bool(b) => Ok(LuaValue::Boolean(*b)),
        LoroValue::I64(i) => Ok(LuaValue::Integer(*i)),
        LoroValue::Double(d) => Ok(LuaValue::Number(*d)),
        LoroValue::String(s) => {
            let lua_str = lua.create_string(s.as_ref())?;
            Ok(LuaValue::String(lua_str))
        }
        LoroValue::Binary(b) => {
            let bytes: &[u8] = &**b;
            let lua_str = lua.create_string(bytes)?;
            Ok(LuaValue::String(lua_str))
        }
        LoroValue::List(list) => {
            let table = lua.create_table()?;
            for (i, v) in list.iter().enumerate() {
                let lua_v = loro_value_to_lua(lua, v)?;
                table.set(i + 1, lua_v)?;
            }
            Ok(LuaValue::Table(table))
        }
        LoroValue::Map(map) => {
            let table = lua.create_table()?;
            for (k, v) in map.iter() {
                let lua_v = loro_value_to_lua(lua, v)?;
                table.set(k.clone(), lua_v)?;
            }
            Ok(LuaValue::Table(table))
        }
        LoroValue::Container(_) => {
            // Container references - return nil for now
            Ok(LuaValue::Nil)
        }
    }
}

/// Convert Lua value to LoroValue
pub fn lua_to_loro_value(value: &LuaValue) -> Result<LoroValue, LuaError> {
    match value {
        LuaValue::Nil => Ok(LoroValue::Null),
        LuaValue::Boolean(b) => Ok(LoroValue::Bool(*b)),
        LuaValue::Integer(i) => Ok(LoroValue::I64(*i)),
        LuaValue::Number(n) => Ok(LoroValue::Double(*n)),
        LuaValue::String(s) => {
            let str_val = s.to_str()?.to_string();
            Ok(LoroValue::String(str_val.into()))
        }
        LuaValue::Table(table) => {
            // Check if it's an array (sequential integer keys starting at 1)
            let mut is_array = true;
            let mut max_index = 0;
            for pair in table.clone().pairs::<LuaValue, LuaValue>() {
                let (k, _) = pair?;
                match k {
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
                    let v: LuaValue = table.get(i)?;
                    arr.push(lua_to_loro_value(&v)?);
                }
                Ok(LoroValue::List(arr.into()))
            } else {
                // Map
                let mut map = std::collections::HashMap::new();
                for pair in table.clone().pairs::<LuaValue, LuaValue>() {
                    let (k, v) = pair?;
                    let key = match k {
                        LuaValue::String(s) => s.to_str()?.to_string(),
                        LuaValue::Integer(i) => i.to_string(),
                        _ => continue,
                    };
                    map.insert(key, lua_to_loro_value(&v)?);
                }
                Ok(LoroValue::Map(map.into()))
            }
        }
        _ => Err(LuaError::RuntimeError(format!("Unsupported Lua type: {:?}", value))),
    }
}

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
        LuaValue::Number(f) => {
            serde_json::Number::from_f64(*f)
                .map(JsonValue::Number)
                .ok_or_else(|| LuaError::RuntimeError("Invalid float value".to_string()))
        }
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

// JSON <-> Loro conversions

/// Convert JSON value to LoroValue
pub fn json_to_loro_value(json: &JsonValue) -> LoroValue {
    match json {
        JsonValue::Null => LoroValue::Null,
        JsonValue::Bool(b) => LoroValue::Bool(*b),
        JsonValue::Number(n) => {
            if let Some(i) = n.as_i64() {
                LoroValue::I64(i)
            } else if let Some(f) = n.as_f64() {
                LoroValue::Double(f)
            } else {
                LoroValue::Null
            }
        }
        JsonValue::String(s) => LoroValue::String(s.clone().into()),
        JsonValue::Array(arr) => {
            let list: Vec<LoroValue> = arr.iter().map(json_to_loro_value).collect();
            LoroValue::List(list.into())
        }
        JsonValue::Object(obj) => {
            let map: std::collections::HashMap<String, LoroValue> = obj
                .iter()
                .map(|(k, v)| (k.clone(), json_to_loro_value(v)))
                .collect();
            LoroValue::Map(map.into())
        }
    }
}

/// Convert LoroValue to JSON value
pub fn loro_value_to_json(value: &LoroValue) -> JsonValue {
    match value {
        LoroValue::Null => JsonValue::Null,
        LoroValue::Bool(b) => JsonValue::Bool(*b),
        LoroValue::I64(i) => JsonValue::Number((*i).into()),
        LoroValue::Double(d) => {
            serde_json::Number::from_f64(*d)
                .map(JsonValue::Number)
                .unwrap_or(JsonValue::Null)
        }
        LoroValue::String(s) => JsonValue::String(s.to_string()),
        LoroValue::Binary(b) => {
            // Encode binary as base64 string
            JsonValue::String(base64::Engine::encode(
                &base64::engine::general_purpose::STANDARD,
                &**b,
            ))
        }
        LoroValue::List(list) => {
            let arr: Vec<JsonValue> = list.iter().map(loro_value_to_json).collect();
            JsonValue::Array(arr)
        }
        LoroValue::Map(map) => {
            let obj: serde_json::Map<String, JsonValue> = map
                .iter()
                .map(|(k, v)| (k.clone(), loro_value_to_json(v)))
                .collect();
            JsonValue::Object(obj)
        }
        LoroValue::Container(_) => JsonValue::Null,
    }
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
        assert!(matches_layer_pattern("shop/orders/*", "shop/orders/customer"));
        assert!(matches_layer_pattern("*/*", "orders/customer"));

        // Exact matches
        assert!(matches_layer_pattern("products", "products"));
        assert!(matches_layer_pattern("shop/products", "shop/products"));

        // Non-matches
        assert!(!matches_layer_pattern("orders/*", "bookings/did:key:abc"));
        assert!(!matches_layer_pattern("orders/*", "orders/sub/did:key:abc"));
        assert!(!matches_layer_pattern("orders", "orders/extra"));
    }

    #[test]
    fn test_json_to_loro_roundtrip() {
        let json = serde_json::json!({
            "name": "Test",
            "count": 42,
            "active": true,
            "items": [1, 2, 3]
        });

        let loro = json_to_loro_value(&json);
        let back = loro_value_to_json(&loro);

        assert_eq!(json, back);
    }
}
