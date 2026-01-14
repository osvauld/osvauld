//! Scribe Lua Runtime - unified validation and derivation
//!
//! **Context**: Each Scribe owns one Lua instance for both validation and derivation.
//! This solves the Lua instance isolation issue where functions can't be called from
//! a different Lua instance than the one that created them.
//!
//! **Architecture**:
//! - All Scribes: Run validation.lua for incoming update validation
//! - Node Scribes: Also run init.lua for derivation rules
//!
//! **Key insight**: Derivation rules store LuaFunction objects. These MUST be called
//! from the same Lua instance that created them. By owning both validation and
//! derivation in one struct, we guarantee they use the same Lua instance.

use std::collections::HashMap;

use loro::LoroValue;
use mlua::{Function as LuaFunction, Lua, MultiValue, Value as LuaValue};
use serde_json::Value as JsonValue;
use tracing::{debug, info, warn};

use crate::models::Layer;
use crate::runtime::bindings::{
    json_to_lua as shared_json_to_lua,
    lua_to_json as shared_lua_to_json,
    json_to_loro_value as shared_json_to_loro_value,
    matches_layer_pattern,
};

// Re-export validation types
pub use super::validation::{JsonOp, extract_ops_from_update};
use super::validation::{is_nested_list_path, parse_list_index};

// =============================================================================
// Derivation Rule (local storage)
// =============================================================================

/// A derivation rule stored locally in the same Lua instance
///
/// **Why local storage?** Lua functions are bound to their creating instance.
/// Storing them here means on_layer_change() and rebuild() can call them directly.
struct LocalDerivationRule {
    /// Source pattern to watch (e.g., "orders/*")
    source_pattern: String,
    /// Target derived layer name (e.g., "orders_summary")
    target_layer: String,
    /// Lua function to extract key from entry: fn(entry) -> key
    key_fn: LuaFunction,
    /// Lua function to transform entry: fn(source_layer_name, entry) -> derived_entry
    transform_fn: LuaFunction,
    /// Optional update function: fn(source_layer_name, entry) -> derived_entry
    /// Called for updates to existing items. Falls back to transform_fn if not provided.
    update_fn: Option<LuaFunction>,
    /// Optional filter function: fn(entry) -> bool
    filter_fn: Option<LuaFunction>,
}

// =============================================================================
// Scribe Lua Runtime
// =============================================================================

/// Unified Lua runtime for validation and derivation
///
/// **One Lua instance** handles:
/// - Validation (validate_ops function from validation.lua)
/// - Derivation (register_derivation from init.lua, only on node)
///
/// **Solves**: "Lua instance passed Value created from a different main Lua state" error
pub struct ScribeLuaRuntime {
    lua: Lua,
    page_id: String,
    /// Whether derivation is active (only on node)
    is_node: bool,
    /// Locally stored derivation rules
    derivation_rules: Vec<LocalDerivationRule>,
}

impl ScribeLuaRuntime {
    /// Create runtime with validation only (for owner/customer)
    ///
    /// **Context**: Non-node peers only need validation
    /// **Code**: validation.lua content
    pub fn new_validation_only(page_id: &str, validation_code: &str) -> Result<Self, String> {
        let lua = Lua::new();

        // Load validation code
        lua.load(validation_code)
            .exec()
            .map_err(|e| format!("Failed to load validation code: {}", e))?;

        // Verify validate_ops function exists (optional, warn if missing)
        let globals = lua.globals();
        match globals.get::<LuaValue>("validate_ops") {
            Ok(LuaValue::Function(_)) => {
                info!(page_id = %page_id, "Loaded Lua validator with validate_ops function");
            }
            Ok(_) => {
                return Err("validate_ops is not a function".to_string());
            }
            Err(_) => {
                warn!(page_id = %page_id, "No validate_ops function found, validation will pass all ops");
            }
        }

        Ok(Self {
            lua,
            page_id: page_id.to_string(),
            is_node: false,
            derivation_rules: Vec::new(),
        })
    }

    /// Create runtime with validation + derivation (for node)
    ///
    /// **Context**: Node needs both validation and derivation rules
    /// **validation_code**: validation.lua content
    /// **init_code**: init.lua content with register_derivation() calls
    pub fn new_with_derivation(
        page_id: &str,
        validation_code: &str,
        init_code: &str,
    ) -> Result<Self, String> {
        let lua = Lua::new();

        // Load validation code first
        lua.load(validation_code)
            .exec()
            .map_err(|e| format!("Failed to load validation code: {}", e))?;

        // Setup permit bindings (for init.lua to access page_id)
        // Note: Lua code uses permit:page_id() (method call) which passes `permit` as first arg
        let page_id_for_lua = page_id.to_string();
        let permit_table = lua.create_table()
            .map_err(|e| format!("Failed to create permit table: {}", e))?;
        permit_table.set("page_id", lua.create_function(move |_, _args: mlua::MultiValue| {
            // Accept any args (method call passes self, function call doesn't)
            Ok(page_id_for_lua.clone())
        }).map_err(|e| format!("Failed to create page_id function: {}", e))?)
            .map_err(|e| format!("Failed to set page_id: {}", e))?;
        lua.globals().set("permit", permit_table)
            .map_err(|e| format!("Failed to set permit global: {}", e))?;

        // Create derivation table with register method
        // Rules are captured via callback and stored in Rust
        let derivation_table = lua.create_table()
            .map_err(|e| format!("Failed to create derivation table: {}", e))?;

        // We'll use a Lua registry to collect rules, then extract them
        let rules_table = lua.create_table()
            .map_err(|e| format!("Failed to create rules table: {}", e))?;
        lua.set_named_registry_value("derivation_rules", rules_table)
            .map_err(|e| format!("Failed to set registry: {}", e))?;

        // derivation:register(config) - stores config in registry
        // Note: Method call syntax passes `self` as first arg, config as second
        derivation_table.set("register", lua.create_function(|lua, (_self, config): (mlua::Value, mlua::Table)| {
            let rules: mlua::Table = lua.named_registry_value("derivation_rules")?;
            let next_idx = rules.len()? + 1;
            rules.set(next_idx, config)?;
            Ok(())
        }).map_err(|e| format!("Failed to create register function: {}", e))?)
            .map_err(|e| format!("Failed to set register: {}", e))?;

        lua.globals().set("derivation", derivation_table)
            .map_err(|e| format!("Failed to set derivation global: {}", e))?;

        // Load init code (calls register_derivation)
        lua.load(init_code)
            .exec()
            .map_err(|e| format!("Failed to load init code: {}", e))?;

        // Extract registered rules from registry
        let mut derivation_rules = Vec::new();
        let rules: mlua::Table = lua.named_registry_value("derivation_rules")
            .map_err(|e| format!("Failed to get rules from registry: {}", e))?;

        for pair in rules.pairs::<i64, mlua::Table>() {
            let (_, config) = pair.map_err(|e| format!("Failed to iterate rules: {}", e))?;

            let source_pattern: String = config.get("source")
                .map_err(|e| format!("Missing 'source' in derivation rule: {}", e))?;
            let target_layer: String = config.get("target")
                .map_err(|e| format!("Missing 'target' in derivation rule: {}", e))?;
            let key_fn: LuaFunction = config.get("key_fn")
                .map_err(|e| format!("Missing 'key_fn' in derivation rule: {}", e))?;
            let transform_fn: LuaFunction = config.get("transform")
                .map_err(|e| format!("Missing 'transform' in derivation rule: {}", e))?;
            let update_fn: Option<LuaFunction> = config.get("update_fn").ok();
            let filter_fn: Option<LuaFunction> = config.get("filter").ok();

            info!(source = %source_pattern, target = %target_layer, has_update_fn = update_fn.is_some(), "Registered derivation rule");

            derivation_rules.push(LocalDerivationRule {
                source_pattern,
                target_layer,
                key_fn,
                transform_fn,
                update_fn,
                filter_fn,
            });
        }

        info!(page_id = %page_id, rule_count = derivation_rules.len(), "ScribeLuaRuntime created with derivation");

        Ok(Self {
            lua,
            page_id: page_id.to_string(),
            is_node: true,
            derivation_rules,
        })
    }

    // =========================================================================
    // Validation
    // =========================================================================

    /// Validate operations before applying an update
    ///
    /// **Returns**: Ok(true) if valid, Ok(false) if invalid, Err on Lua error
    pub fn validate_ops(
        &self,
        layer_name: &str,
        ops: &[JsonOp],
        from_did: &str,
        role: &str,
    ) -> Result<(bool, Option<String>), String> {
        let globals = self.lua.globals();

        // Check if validate_ops function exists
        let validate_fn = match globals.get::<LuaValue>("validate_ops") {
            Ok(LuaValue::Function(f)) => f,
            _ => {
                debug!(layer = %layer_name, "No validate_ops function, allowing update");
                return Ok((true, None));
            }
        };

        // Convert ops to Lua table
        let ops_value = self.ops_to_lua(ops)?;

        // Call validate_ops(layer_name, ops, from_did, role, page_id)
        let result: Result<MultiValue, _> = validate_fn.call((
            layer_name.to_string(),
            ops_value,
            from_did.to_string(),
            role.to_string(),
            self.page_id.clone(),
        ));

        match result {
            Ok(values) => {
                let mut iter = values.into_iter();

                let valid = match iter.next() {
                    Some(LuaValue::Boolean(b)) => b,
                    Some(LuaValue::Nil) => true,
                    _ => {
                        warn!("validate_ops returned non-boolean first value");
                        false
                    }
                };

                let error_msg = match iter.next() {
                    Some(LuaValue::String(s)) => s.to_str().ok().map(|s| s.to_string()),
                    _ => None,
                };

                debug!(
                    layer = %layer_name,
                    from_did = %from_did,
                    role = %role,
                    valid = %valid,
                    error = ?error_msg,
                    "Validation result"
                );

                Ok((valid, error_msg))
            }
            Err(e) => {
                warn!(error = %e, "Lua validation error");
                Err(format!("Validation error: {}", e))
            }
        }
    }

    /// Convert JsonOp array to Lua table
    fn ops_to_lua(&self, ops: &[JsonOp]) -> Result<LuaValue, String> {
        let lua_ops = self.lua.create_table()
            .map_err(|e| format!("Failed to create Lua table: {}", e))?;

        for (i, op) in ops.iter().enumerate() {
            let op_table = self.lua.create_table()
                .map_err(|e| format!("Failed to create op table: {}", e))?;

            op_table.set("op", op.op.clone())
                .map_err(|e| format!("Failed to set op: {}", e))?;
            op_table.set("path", op.path.clone())
                .map_err(|e| format!("Failed to set path: {}", e))?;

            if let Some(ref key) = op.key {
                op_table.set("key", key.clone())
                    .map_err(|e| format!("Failed to set key: {}", e))?;
            }

            if let Some(index) = op.index {
                op_table.set("index", index as i64)
                    .map_err(|e| format!("Failed to set index: {}", e))?;
            }

            if let Some(ref value) = op.value {
                let lua_value = json_to_lua(&self.lua, value)?;
                op_table.set("value", lua_value)
                    .map_err(|e| format!("Failed to set value: {}", e))?;
            }

            if let Some(ref old_value) = op.old_value {
                let lua_value = json_to_lua(&self.lua, old_value)?;
                op_table.set("old_value", lua_value)
                    .map_err(|e| format!("Failed to set old_value: {}", e))?;
            }

            lua_ops.set(i + 1, op_table)
                .map_err(|e| format!("Failed to set op in array: {}", e))?;
        }

        Ok(LuaValue::Table(lua_ops))
    }

    // =========================================================================
    // Derivation
    // =========================================================================

    /// Check if derivation is enabled (only on node)
    pub fn is_derivation_enabled(&self) -> bool {
        self.is_node
    }

    /// Get list of derivation rules
    pub fn derivation_rules(&self) -> Vec<(String, String)> {
        self.derivation_rules
            .iter()
            .map(|r| (r.source_pattern.clone(), r.target_layer.clone()))
            .collect()
    }

    /// Process source layer ops for derivation
    ///
    /// **Context**: Called after validation passes with the extracted ops
    /// **We do**: Check if layer matches source patterns, transform only changed entries
    ///
    /// **Flow**: extract_ops_from_update → validate → apply → on_source_ops
    /// This is efficient because:
    /// 1. We only process entries that actually changed (from ops)
    /// 2. No need to scan all entries in the layer
    /// 3. Delete ops properly remove from derived layer
    pub fn on_source_ops(
        &self,
        layer_name: &str,
        ops: &[JsonOp],
        layers: &mut HashMap<String, Layer>,
    ) {
        if !self.is_node {
            return;
        }

        // Find rules matching this layer
        let matching_rules: Vec<&LocalDerivationRule> = self.derivation_rules
            .iter()
            .filter(|r| matches_layer_pattern(&r.source_pattern, layer_name))
            .collect();

        if matching_rules.is_empty() {
            return;
        }

        debug!(
            layer = %layer_name,
            op_count = ops.len(),
            matching_rules = matching_rules.len(),
            "Running ops-based derivation"
        );

        for rule in matching_rules {
            self.run_ops_derivation(rule, layer_name, ops, layers);
        }
    }

    /// Run derivation for a single rule using ops
    ///
    /// **Context**: Source layer updated, process only the changed entries from ops
    /// **Handles**:
    /// - insert/update ops: transform op.value and insert into derived layer
    /// - delete ops: compute key from op.old_value and delete from derived layer
    ///
    /// **Field-level updates**: When op.path indicates a field within a list item
    /// (e.g., "orders[0].status"), we fetch the full item from the source layer
    /// and use update_fn if available, falling back to transform_fn.
    fn run_ops_derivation(
        &self,
        rule: &LocalDerivationRule,
        source_layer: &str,
        ops: &[JsonOp],
        layers: &mut HashMap<String, Layer>,
    ) {
        // First, collect full items for any field-level updates from source layer
        // This avoids borrow conflicts when we later mutably borrow for target
        let mut field_update_items: HashMap<usize, JsonValue> = HashMap::new();
        if let Some(source) = layers.get(source_layer) {
            for op in ops {
                if matches!(op.op.as_str(), "insert" | "update" | "set") && is_nested_list_path(&op.path) {
                    if let Some(index) = parse_list_index(&op.path) {
                        if !field_update_items.contains_key(&index) {
                            if let Ok(item) = source.list_get(source_layer, index) {
                                field_update_items.insert(index, item);
                            }
                        }
                    }
                }
            }
        }

        // Get or create target layer
        let target = layers
            .entry(rule.target_layer.clone())
            .or_insert_with(Layer::new);

        let target_map = target.loro().get_map(rule.target_layer.clone());

        let mut insert_count = 0;
        let mut delete_count = 0;

        for op in ops {
            match op.op.as_str() {
                "insert" | "update" | "set" => {
                    // Check if this is a field-level update within a list item
                    let is_field_update = is_nested_list_path(&op.path);

                    if is_field_update {
                        // Field-level update: use pre-fetched full item
                        let index = match parse_list_index(&op.path) {
                            Some(i) => i,
                            None => {
                                warn!(path = %op.path, "Failed to parse list index from path");
                                continue;
                            }
                        };

                        let full_item = match field_update_items.get(&index) {
                            Some(item) => item,
                            None => {
                                warn!(index = index, "Full item not found in pre-fetched cache");
                                continue;
                            }
                        };

                        // Process the full item with update_fn (or transform_fn fallback)
                        match self.process_entry(rule, source_layer, full_item, true) {
                            Ok(Some((derived_key, derived_value))) => {
                                let loro_value = json_to_loro_value(&derived_value);
                                if let Err(e) = target_map.insert(&derived_key, loro_value) {
                                    warn!(key = %derived_key, error = %e, "Failed to update derived entry");
                                } else {
                                    insert_count += 1;
                                    debug!(key = %derived_key, op = %op.op, "Updated derived entry (field-level)");
                                }
                            }
                            Ok(None) => {
                                // Entry filtered out - might need to delete from derived if it was there before
                                debug!(op = %op.op, "Entry filtered out by derivation rule");
                            }
                            Err(e) => {
                                warn!(op = %op.op, path = %op.path, error = %e, "Derivation update failed");
                            }
                        }
                    } else {
                        // Full item insert/update: use op.value directly
                        if let Some(ref value) = op.value {
                            // If value is an array, iterate through each item
                            // Otherwise, process the single value
                            let items: Vec<&JsonValue> = if let Some(arr) = value.as_array() {
                                arr.iter().collect()
                            } else {
                                vec![value]
                            };

                            for item in items {
                                match self.process_entry(rule, source_layer, item, false) {
                                    Ok(Some((derived_key, derived_value))) => {
                                        let loro_value = json_to_loro_value(&derived_value);
                                        if let Err(e) = target_map.insert(&derived_key, loro_value) {
                                            warn!(key = %derived_key, error = %e, "Failed to insert derived entry");
                                        } else {
                                            insert_count += 1;
                                            debug!(key = %derived_key, op = %op.op, "Inserted derived entry");
                                        }
                                    }
                                    Ok(None) => {
                                        debug!(op = %op.op, "Entry filtered out by derivation rule");
                                    }
                                    Err(e) => {
                                        warn!(op = %op.op, error = %e, "Derivation transform failed");
                                    }
                                }
                            }
                        }
                    }
                }
                "delete" => {
                    // Compute key from old_value and delete from derived layer
                    if let Some(ref old_value) = op.old_value {
                        // Use key_fn to get the derived key from the old value
                        let entry_lua = match json_to_lua(&self.lua, old_value) {
                            Ok(v) => v,
                            Err(e) => {
                                warn!(error = %e, "Failed to convert old_value to Lua for delete");
                                continue;
                            }
                        };

                        match rule.key_fn.call::<String>(entry_lua) {
                            Ok(derived_key) => {
                                if let Err(e) = target_map.delete(&derived_key) {
                                    debug!(key = %derived_key, error = %e, "Failed to delete derived entry (may not exist)");
                                } else {
                                    delete_count += 1;
                                    debug!(key = %derived_key, "Deleted derived entry");
                                }
                            }
                            Err(e) => {
                                warn!(error = %e, "Failed to get key from old_value for delete");
                            }
                        }
                    }
                }
                _ => {
                    debug!(op = %op.op, "Unknown op type, skipping");
                }
            }
        }

        if insert_count > 0 || delete_count > 0 {
            target.commit();
            info!(
                source = %source_layer,
                target = %rule.target_layer,
                inserts = insert_count,
                deletes = delete_count,
                "Derivation complete"
            );
        }
    }

    /// Transform a single entry using the rule's Lua functions
    fn transform_entry(
        &self,
        rule: &LocalDerivationRule,
        source_layer: &str,
        entry: &JsonValue,
    ) -> Result<Option<(String, JsonValue)>, String> {
        debug!(
            source_layer = %source_layer,
            entry = %entry,
            "transform_entry called"
        );

        // Apply filter if present
        if let Some(ref filter_fn) = rule.filter_fn {
            let entry_lua = json_to_lua(&self.lua, entry)
                .map_err(|e| format!("Failed to convert entry to Lua: {}", e))?;

            let passes: bool = filter_fn.call(entry_lua.clone())
                .map_err(|e| format!("Filter function error: {}", e))?;

            if !passes {
                return Ok(None);
            }
        }

        // Get derived key using key_fn
        let entry_lua = json_to_lua(&self.lua, entry)
            .map_err(|e| format!("Failed to convert entry to Lua: {}", e))?;

        let derived_key: String = rule.key_fn.call(entry_lua.clone())
            .map_err(|e| format!("key_fn error: {}", e))?;

        // Transform entry using transform_fn
        let transformed: LuaValue = rule.transform_fn.call((source_layer.to_string(), entry_lua))
            .map_err(|e| format!("transform_fn error: {}", e))?;

        // Convert back to JSON
        let derived_value = lua_to_json(&transformed)
            .map_err(|e| format!("Failed to convert transform result to JSON: {}", e))?;

        Ok(Some((derived_key, derived_value)))
    }

    /// Process a single entry using the rule's Lua functions
    ///
    /// **Context**: Unified handler for both inserts and updates
    /// **is_update**: If true and update_fn exists, use update_fn; otherwise use transform_fn
    fn process_entry(
        &self,
        rule: &LocalDerivationRule,
        source_layer: &str,
        entry: &JsonValue,
        is_update: bool,
    ) -> Result<Option<(String, JsonValue)>, String> {
        debug!(
            source_layer = %source_layer,
            entry = %entry,
            is_update = is_update,
            "process_entry called"
        );

        // Apply filter if present
        if let Some(ref filter_fn) = rule.filter_fn {
            let entry_lua = json_to_lua(&self.lua, entry)
                .map_err(|e| format!("Failed to convert entry to Lua: {}", e))?;

            let passes: bool = filter_fn.call(entry_lua.clone())
                .map_err(|e| format!("Filter function error: {}", e))?;

            if !passes {
                return Ok(None);
            }
        }

        // Get derived key using key_fn
        let entry_lua = json_to_lua(&self.lua, entry)
            .map_err(|e| format!("Failed to convert entry to Lua: {}", e))?;

        let derived_key: String = rule.key_fn.call(entry_lua.clone())
            .map_err(|e| format!("key_fn error: {}", e))?;

        // Choose transform function: update_fn for updates if available, otherwise transform_fn
        let transformed: LuaValue = if is_update {
            if let Some(ref update_fn) = rule.update_fn {
                update_fn.call((source_layer.to_string(), entry_lua))
                    .map_err(|e| format!("update_fn error: {}", e))?
            } else {
                // Fallback to transform_fn
                rule.transform_fn.call((source_layer.to_string(), entry_lua))
                    .map_err(|e| format!("transform_fn error: {}", e))?
            }
        } else {
            rule.transform_fn.call((source_layer.to_string(), entry_lua))
                .map_err(|e| format!("transform_fn error: {}", e))?
        };

        // Convert back to JSON
        let derived_value = lua_to_json(&transformed)
            .map_err(|e| format!("Failed to convert transform result to JSON: {}", e))?;

        Ok(Some((derived_key, derived_value)))
    }

    /// Full rebuild of a derived layer from all source layers
    ///
    /// **Context**: Called on startup, manual trigger, or corruption recovery
    pub fn rebuild_derived(
        &self,
        target: &str,
        layers: &mut HashMap<String, Layer>,
    ) -> Result<usize, String> {
        if !self.is_node {
            return Err("Derivation not enabled (not on node)".to_string());
        }

        let rule = self.derivation_rules
            .iter()
            .find(|r| r.target_layer == target)
            .ok_or_else(|| format!("No derivation rule found for target: {}", target))?;

        info!(target = %target, source = %rule.source_pattern, "Rebuilding derived layer");

        // Find all source layers matching pattern
        let source_layers: Vec<String> = layers.keys()
            .filter(|name| matches_layer_pattern(&rule.source_pattern, name))
            .cloned()
            .collect();

        // Collect all derived entries
        let mut derived_entries: HashMap<String, JsonValue> = HashMap::new();

        for source_name in &source_layers {
            if let Some(source_layer) = layers.get(source_name) {
                let data = source_layer.to_json_value();
                if let Some(map) = data.as_object() {
                    for (_key, entry) in map {
                        match self.transform_entry(rule, source_name, entry) {
                            Ok(Some((derived_key, derived_value))) => {
                                derived_entries.insert(derived_key, derived_value);
                            }
                            Ok(None) => {}
                            Err(e) => {
                                warn!(error = %e, "Transform failed during rebuild");
                            }
                        }
                    }
                }
            }
        }

        // Get or create target layer and replace contents
        let target_layer = layers
            .entry(target.to_string())
            .or_insert_with(Layer::new);

        let target_map = target_layer.loro().get_map(target.to_string());

        // Clear existing entries
        for key in target_map.keys().collect::<Vec<_>>() {
            let _ = target_map.delete(&key);
        }

        // Insert all derived entries
        let count = derived_entries.len();
        for (key, value) in derived_entries {
            let loro_value = json_to_loro_value(&value);
            if let Err(e) = target_map.insert(&key, loro_value) {
                warn!(key = %key, error = %e, "Failed to insert during rebuild");
            }
        }

        target_layer.commit();

        info!(target = %target, entry_count = count, "Rebuild complete");
        Ok(count)
    }

    /// Rebuild all derived layers
    pub fn rebuild_all_derived(&self, layers: &mut HashMap<String, Layer>) -> Result<(), String> {
        if !self.is_node {
            return Err("Derivation not enabled (not on node)".to_string());
        }

        let targets: Vec<String> = self.derivation_rules
            .iter()
            .map(|r| r.target_layer.clone())
            .collect();

        for target in targets {
            self.rebuild_derived(&target, layers)?;
        }

        Ok(())
    }

    /// Get the Lua instance (for advanced usage)
    pub fn lua(&self) -> &Lua {
        &self.lua
    }
}

// =============================================================================
// Wrappers for shared conversion functions (with String error type)
// =============================================================================

/// Convert JSON to Lua value (wrapper with String error)
fn json_to_lua(lua: &Lua, json: &JsonValue) -> Result<LuaValue, String> {
    shared_json_to_lua(lua, json).map_err(|e| e.to_string())
}

/// Convert Lua value to JSON (wrapper with String error)
fn lua_to_json(value: &LuaValue) -> Result<JsonValue, String> {
    shared_lua_to_json(value).map_err(|e| e.to_string())
}

/// Convert JSON value to Loro value (direct alias)
fn json_to_loro_value(json: &JsonValue) -> LoroValue {
    shared_json_to_loro_value(json)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validation_only_runtime() {
        let code = r#"
            function validate_ops(layer_name, ops, from_did, role, page_id)
                return true, nil
            end
        "#;

        let runtime = ScribeLuaRuntime::new_validation_only("test_page", code).unwrap();
        assert!(!runtime.is_derivation_enabled());

        let ops = vec![JsonOp {
            op: "insert".to_string(),
            path: "orders".to_string(),
            key: Some("order_1".to_string()),
            index: None,
            value: Some(serde_json::json!({"status": "pending"})),
            old_value: None,
        }];

        let (valid, error) = runtime.validate_ops("orders", &ops, "did:key:customer", "customer").unwrap();
        assert!(valid);
        assert!(error.is_none());
    }

    #[test]
    fn test_derivation_runtime() {
        let validation_code = r#"
            function validate_ops(layer_name, ops, from_did, role, page_id)
                return true, nil
            end
        "#;

        let init_code = r#"
            local page_id = permit:page_id()

            derivation:register({
                source = page_id .. "/orders/*",
                target = page_id .. "/orders_summary",
                key_fn = function(order)
                    return order.id
                end,
                transform = function(source_layer, order)
                    return {
                        id = order.id,
                        status = order.status,
                    }
                end,
            })
        "#;

        let runtime = ScribeLuaRuntime::new_with_derivation("test_page", validation_code, init_code).unwrap();
        assert!(runtime.is_derivation_enabled());

        let rules = runtime.derivation_rules();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].0, "test_page/orders/*");
        assert_eq!(rules[0].1, "test_page/orders_summary");
    }

    #[test]
    fn test_pattern_matching() {
        assert!(matches_layer_pattern("orders/*", "orders/did:key:abc"));
        assert!(matches_layer_pattern("orders/*", "orders/did:key:xyz"));
        assert!(!matches_layer_pattern("orders/*", "bookings/did:key:abc"));
        assert!(!matches_layer_pattern("orders/*", "orders/sub/did:key:abc"));
        assert!(matches_layer_pattern("*/*", "orders/did:key:abc"));
        assert!(matches_layer_pattern("shop/orders/*", "shop/orders/did:key:abc"));
    }
}
