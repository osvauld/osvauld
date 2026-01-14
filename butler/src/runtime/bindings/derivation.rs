//! Derivation bindings for Lua
//!
//! Provides derivation:register(), derivation:rebuild(), derivation:rebuild_all() methods.
//! For node-only operations that transform source layers into derived layers.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use mlua::{UserData, UserDataMethods, Value as LuaValue, Error as LuaError, Function as LuaFunction};
use ractor::ActorRef;
use tokio::sync::oneshot;
use tracing::info;

use crate::scribe::ScribeMessage;
use super::{block_on_async, lua_to_loro_value};

// =============================================================================
// Local Derivation Rule
// =============================================================================

/// A locally-stored derivation rule (functions bound to this Lua instance)
struct LocalDerivationRule {
    /// Source pattern to watch (e.g., "orders/*")
    source_pattern: String,
    /// Target derived layer name (e.g., "orders_summary")
    target_layer: String,
    /// Lua function to extract key from entry: fn(entry) -> key
    key_fn: LuaFunction,
    /// Lua function to transform entry: fn(source_layer_name, entry) -> derived_entry
    transform_fn: LuaFunction,
    /// Optional filter function: fn(entry) -> bool
    filter_fn: Option<LuaFunction>,
}

// =============================================================================
// Derivation Bindings
// =============================================================================

/// Derivation bindings for Lua (node-only operations)
///
/// **Methods**:
/// - `derivation:register({source, target, key_fn, transform, filter})` - Register derivation rule
/// - `derivation:rebuild(target)` - Rebuild derived layer from sources
/// - `derivation:rebuild_all()` - Rebuild all derived layers
///
/// **Note**: Rules are stored locally (same Lua instance) to avoid cross-instance issues.
pub struct DerivationBindings {
    scribe_ref: ActorRef<ScribeMessage>,
    /// Locally stored rules (bound to this Lua instance)
    rules: Arc<RwLock<Vec<LocalDerivationRule>>>,
    /// Whether this is enabled (only on node)
    #[allow(dead_code)]
    enabled: Arc<RwLock<bool>>,
}

impl DerivationBindings {
    pub fn new(scribe_ref: ActorRef<ScribeMessage>) -> Self {
        Self {
            scribe_ref,
            rules: Arc::new(RwLock::new(Vec::new())),
            enabled: Arc::new(RwLock::new(false)),
        }
    }
}

impl UserData for DerivationBindings {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        // derivation:register({source = "orders/*", target = "orders_summary", ...})
        // NOTE: Always allow registration. Rules are stored locally in this DerivationBindings
        // instance and executed via rebuild_all() in the same Lua context.
        // The Scribe check was removed because HeadlessRuntime manages derivation locally.
        methods.add_method("register", |_lua, this, config: mlua::Table| {
            // Store enabled state
            if let Ok(mut e) = this.enabled.write() {
                *e = true;
            }

            let source_pattern: String = config.get("source")
                .map_err(|e| LuaError::RuntimeError(format!("Missing 'source' field: {}", e)))?;
            let target_layer: String = config.get("target")
                .map_err(|e| LuaError::RuntimeError(format!("Missing 'target' field: {}", e)))?;
            let key_fn: mlua::Function = config.get("key_fn")
                .map_err(|e| LuaError::RuntimeError(format!("Missing 'key_fn' field: {}", e)))?;
            let transform_fn: mlua::Function = config.get("transform")
                .map_err(|e| LuaError::RuntimeError(format!("Missing 'transform' field: {}", e)))?;
            let filter_fn: Option<mlua::Function> = config.get("filter").ok();

            // Store rule locally
            let rule = LocalDerivationRule {
                source_pattern: source_pattern.clone(),
                target_layer: target_layer.clone(),
                key_fn,
                transform_fn,
                filter_fn,
            };

            if let Ok(mut rules) = this.rules.write() {
                rules.push(rule);
                info!(source = %source_pattern, target = %target_layer, "Registered local derivation rule");
            }

            // Notify Scribe to create the derived layer (empty)
            this.scribe_ref.cast(ScribeMessage::CreateDerivedLayer {
                target_layer,
            }).map_err(|e| LuaError::RuntimeError(format!("Failed to create derived layer: {}", e)))?;

            Ok(())
        });

        // derivation:rebuild(target) -> count
        methods.add_method("rebuild", |lua, this, target: String| {
            this.rebuild_target(lua, &target)
        });

        // derivation:rebuild_all()
        methods.add_method("rebuild_all", |lua, this, ()| {
            let targets: Vec<String> = {
                let rules = this.rules.read().map_err(|e| LuaError::RuntimeError(format!("Lock error: {}", e)))?;
                rules.iter().map(|r| r.target_layer.clone()).collect()
            };

            for target in targets {
                this.rebuild_target(lua, &target)?;
            }

            Ok(())
        });
    }
}

impl DerivationBindings {
    /// Rebuild a specific derived layer
    fn rebuild_target(&self, lua: &mlua::Lua, target: &str) -> Result<i64, LuaError> {
        let rules = self.rules.read()
            .map_err(|e| LuaError::RuntimeError(format!("Lock error: {}", e)))?;

        let rule = rules.iter()
            .find(|r| r.target_layer == target)
            .ok_or_else(|| LuaError::RuntimeError(format!("No rule for target: {}", target)))?;

        info!(target = %target, source = %rule.source_pattern, "Rebuilding derived layer locally");

        // Get source layers matching pattern
        let (tx, rx) = oneshot::channel();
        self.scribe_ref.cast(ScribeMessage::ListLayers {
            pattern: rule.source_pattern.clone(),
            reply: tx,
        }).map_err(|e| LuaError::RuntimeError(format!("Failed to list layers: {}", e)))?;

        let source_layers = block_on_async(async {
            rx.await.map_err(|e| LuaError::RuntimeError(format!("Channel error: {}", e)))
        })??;

        // Collect all derived entries
        let mut derived_entries: HashMap<String, LuaValue> = HashMap::new();

        for source_layer in source_layers {
            // Get source layer data
            let (tx, rx) = oneshot::channel();
            self.scribe_ref.cast(ScribeMessage::GetLayerData {
                layer_name: source_layer.clone(),
                reply: tx,
            }).map_err(|e| LuaError::RuntimeError(format!("Failed to get layer data: {}", e)))?;

            let layer_data = block_on_async(async {
                rx.await
                    .map_err(|e| LuaError::RuntimeError(format!("Channel error: {}", e)))?
                    .map_err(|e| LuaError::RuntimeError(format!("Get data error: {}", e)))
            })??;

            // Convert to Lua table
            // Layer data structure: { "container_name": [entries...] } for lists
            //                    or { "container_name": { key: entry, ... } } for maps
            let data_lua = json_to_lua(lua, &layer_data)?;

            // Extract entries from the container structure
            let entries = extract_layer_entries(&data_lua)?;

            for entry in entries {
                // Apply filter if present
                if let Some(ref filter_fn) = rule.filter_fn {
                    let passes: bool = filter_fn.call(entry.clone())
                        .map_err(|e| LuaError::RuntimeError(format!("Filter error: {}", e)))?;
                    if !passes {
                        continue;
                    }
                }

                // Get key for derived map
                let key: String = rule.key_fn.call(entry.clone())
                    .map_err(|e| LuaError::RuntimeError(format!("Key function error: {}", e)))?;

                // Transform entry
                let transformed: LuaValue = rule.transform_fn.call((source_layer.clone(), entry))
                    .map_err(|e| LuaError::RuntimeError(format!("Transform error: {}", e)))?;

                derived_entries.insert(key, transformed);
            }
        }

        let count = derived_entries.len() as i64;

        // Write to derived layer via Scribe
        let derived_table = lua.create_table()?;
        for (key, value) in derived_entries {
            derived_table.set(key, value)?;
        }

        // Get or create the derived layer and set all values
        let (tx, rx) = oneshot::channel();
        self.scribe_ref.cast(ScribeMessage::GetOrCreateLoroMap {
            layer_name: target.to_string(),
            reply: tx,
        }).map_err(|e| LuaError::RuntimeError(format!("Failed to get derived layer: {}", e)))?;

        let derived_map = block_on_async(async {
            rx.await
                .map_err(|e| LuaError::RuntimeError(format!("Channel error: {}", e)))?
                .map_err(|e| LuaError::RuntimeError(format!("Get map error: {}", e)))
        })??;

        // Clear and repopulate
        for key in derived_map.keys() {
            let _ = derived_map.delete(&key);
        }

        for pair in derived_table.pairs::<String, LuaValue>() {
            let (key, value) = pair?;
            let loro_value = lua_to_loro_value(&value)?;
            derived_map.insert(&key, loro_value)
                .map_err(|e| LuaError::RuntimeError(format!("Failed to insert: {}", e)))?;
        }

        // Commit the changes
        self.scribe_ref.cast(ScribeMessage::CommitLayer {
            layer_name: target.to_string(),
        }).map_err(|e| LuaError::RuntimeError(format!("Failed to commit: {}", e)))?;

        info!(target = %target, count = %count, "Derived layer rebuilt");

        Ok(count)
    }
}

/// Extract entries from layer data structure
///
/// Layer data comes in one of these formats:
/// - List layer: `{ "container_name": [entry1, entry2, ...] }`
/// - Map layer: `{ "container_name": { "key1": entry1, "key2": entry2, ... } }`
///
/// This function extracts the individual entries regardless of format.
fn extract_layer_entries(data: &LuaValue) -> Result<Vec<LuaValue>, LuaError> {
    let mut entries = Vec::new();

    // data is a table with container names as keys
    if let LuaValue::Table(outer_table) = data {
        // Iterate over containers (usually just one per layer)
        for pair in outer_table.pairs::<LuaValue, LuaValue>() {
            let (_container_key, container_value) = pair?;

            // container_value is either an array (list) or a table (map)
            if let LuaValue::Table(inner_table) = container_value {
                // Check if it's array-like (has sequential numeric keys starting at 1)
                // or map-like (has string keys)
                let first_key: Result<i64, _> = inner_table.get(1i64);
                if first_key.is_ok() {
                    // Array-like: iterate numeric keys
                    for pair in inner_table.pairs::<i64, LuaValue>() {
                        let (_idx, entry) = pair?;
                        entries.push(entry);
                    }
                } else {
                    // Map-like: iterate string keys
                    for pair in inner_table.pairs::<String, LuaValue>() {
                        let (_key, entry) = pair?;
                        entries.push(entry);
                    }
                }
            }
        }
    }

    Ok(entries)
}

/// Convert serde_json::Value to Lua value
fn json_to_lua(lua: &mlua::Lua, json: &serde_json::Value) -> Result<LuaValue, LuaError> {
    match json {
        serde_json::Value::Null => Ok(LuaValue::Nil),
        serde_json::Value::Bool(b) => Ok(LuaValue::Boolean(*b)),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(LuaValue::Integer(i))
            } else if let Some(f) = n.as_f64() {
                Ok(LuaValue::Number(f))
            } else {
                Ok(LuaValue::Nil)
            }
        }
        serde_json::Value::String(s) => {
            let lua_str = lua.create_string(s)?;
            Ok(LuaValue::String(lua_str))
        }
        serde_json::Value::Array(arr) => {
            let table = lua.create_table()?;
            for (i, v) in arr.iter().enumerate() {
                let lua_v = json_to_lua(lua, v)?;
                table.set(i + 1, lua_v)?;
            }
            Ok(LuaValue::Table(table))
        }
        serde_json::Value::Object(obj) => {
            let table = lua.create_table()?;
            for (k, v) in obj {
                let lua_v = json_to_lua(lua, v)?;
                table.set(k.clone(), lua_v)?;
            }
            Ok(LuaValue::Table(table))
        }
    }
}
