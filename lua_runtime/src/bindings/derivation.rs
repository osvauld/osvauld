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

use butler::ScribeMessage;
use super::{block_on_async, json_to_lua, lua_to_json};

// Local Derivation Rule

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

// Derivation Bindings

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
        // The Scribe check was removed because LuaRuntime manages derivation locally.
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

        // derivation:on_source_change(layer_name)
        // Called when a source layer changes - rebuilds affected derived layers
        methods.add_method("on_source_change", |lua, this, source_layer: String| {
            let rules = this.rules.read()
                .map_err(|e| LuaError::RuntimeError(format!("Lock error: {}", e)))?;

            // Find all rules that match this source layer pattern
            let matching_targets: Vec<String> = rules.iter()
                .filter(|r| super::matches_layer_pattern(&r.source_pattern, &source_layer))
                .map(|r| r.target_layer.clone())
                .collect();

            drop(rules);  // Release read lock before rebuilding

            // Rebuild each matching derived layer
            for target in matching_targets {
                info!(source = %source_layer, target = %target, "Rebuilding derived layer due to source change");
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

        // Ensure derived layer exists
        let (tx, rx) = oneshot::channel();
        self.scribe_ref.cast(ScribeMessage::EnsureLoroMap {
            layer_name: target.to_string(),
            reply: tx,
        }).map_err(|e| LuaError::RuntimeError(format!("Failed to ensure derived layer: {}", e)))?;

        block_on_async(async {
            rx.await
                .map_err(|e| LuaError::RuntimeError(format!("Channel error: {}", e)))?
                .map_err(|e| LuaError::RuntimeError(format!("Ensure map error: {}", e)))
        })??;

        // Get existing keys to clear (if any)
        let (tx, rx) = oneshot::channel();
        self.scribe_ref.cast(ScribeMessage::MapKeys {
            layer_name: target.to_string(),
            reply: tx,
        }).map_err(|e| LuaError::RuntimeError(format!("Failed to get keys: {}", e)))?;

        let existing_keys = block_on_async(async {
            rx.await
                .map_err(|e| LuaError::RuntimeError(format!("Channel error: {}", e)))?
                .map_err(|e| LuaError::RuntimeError(format!("Get keys error: {}", e)))
        })??;

        // Delete existing keys
        for key in existing_keys {
            self.scribe_ref.cast(ScribeMessage::MapDelete {
                layer_name: target.to_string(),
                path: String::new(),
                key,
            }).map_err(|e| LuaError::RuntimeError(format!("Failed to delete key: {}", e)))?;
        }

        // Insert new derived entries via MapInsert messages
        for (key, value) in derived_entries {
            let json_value = lua_to_json(&value)?;
            self.scribe_ref.cast(ScribeMessage::MapInsert {
                layer_name: target.to_string(),
                path: String::new(),
                key,
                value: json_value,
            }).map_err(|e| LuaError::RuntimeError(format!("Failed to insert: {}", e)))?;
        }

        info!(target = %target, count = %count, "Derived layer rebuilt");

        Ok(count)
    }
}

/// Extract entries from layer data structure
///
/// Layer data now arrives **unwrapped** via Layer::get_content():
/// - List layer: `[entry1, entry2, ...]` (array directly)
/// - Map layer: `{ "key1": entry1, "key2": entry2, ... }` (map directly)
///
/// This function extracts the individual entries regardless of format.
fn extract_layer_entries(data: &LuaValue) -> Result<Vec<LuaValue>, LuaError> {
    let mut entries = Vec::new();

    if let LuaValue::Table(table) = data {
        // Check if it's array-like (has sequential numeric keys starting at 1)
        // or map-like (has string keys)
        let first_key: Result<i64, _> = table.get(1i64);
        if first_key.is_ok() {
            // Array-like: iterate numeric keys (Lua arrays are 1-indexed)
            for pair in table.pairs::<i64, LuaValue>() {
                let (_idx, entry) = pair?;
                entries.push(entry);
            }
        } else {
            // Map-like: iterate string keys
            for pair in table.pairs::<String, LuaValue>() {
                let (_key, entry) = pair?;
                entries.push(entry);
            }
        }
    }

    Ok(entries)
}
