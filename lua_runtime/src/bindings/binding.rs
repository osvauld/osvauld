//! Declarative Binding System for Layer → UI Auto-Sync
//!
//! Provides `scribe:bind()` API that auto-syncs layer data to UI properties,
//! eliminating manual layer tracking and refresh_*_ui functions.
//!
//! ## Usage
//!
//! ```lua
//! -- Simple binding (auto-prepends page_id)
//! scribe:bind("products", "products")
//!
//! -- With transform
//! scribe:bind("products", "products", {
//!     transform = function(item) return {id=item.id, name=item.name} end
//! })
//!
//! -- With sort
//! scribe:bind("orders", "derived/orders_summary", {
//!     sort = function(a, b) return a.created_at > b.created_at end
//! })
//!
//! -- {me} placeholder expands to user's DID
//! scribe:bind("my_orders", "orders/{me}")
//!
//! -- Wildcard pattern (aggregates all matching layers)
//! scribe:bind("all_orders", "orders/*")
//! ```

use mlua::{Function, Lua, RegistryKey, Result as LuaResult, Value};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use tracing::debug;

use crate::ui_types::{PropertyUpdate, UiMutation, VecModelOp};
use butler::{ListOp, LoroDelta};

use super::convert::{json_to_lua, lua_to_json};

fn sthithi_to_json_value(value: &butler::Sthithi) -> serde_json::Value {
    serde_json::Value::from(value)
}

/// Options for a binding (transform, key, max_items)
///
/// **Note**: Sorting is NOT done in bindings - it's a Slint view concern.
/// This enables surgical updates (push/set/remove) with stable indices.
/// Sort your data in Slint using expressions or SortModel.
pub struct BindingOptions {
    /// Lua function to transform each item: fn(item) -> transformed_item
    /// For wildcards: fn(layer_name, item) -> transformed_item
    pub transform: Option<RegistryKey>,
    /// Field name to use as stable identity for surgical updates (e.g., "id", "did")
    pub key: Option<String>,
    /// Maximum number of items to keep in the UI model (keeps latest, drops oldest)
    pub max_items: Option<usize>,
}

impl Default for BindingOptions {
    fn default() -> Self {
        Self {
            transform: None,
            key: None,
            max_items: None,
        }
    }
}

/// A single layer → UI property binding
pub struct LayerBinding {
    /// UI property/model name to sync to
    pub ui_property: String,
    /// Expanded pattern with page_id prefix (e.g., "abc123/products")
    pub expanded_pattern: String,
    /// Original pattern as specified by user (e.g., "products")
    pub raw_pattern: String,
    /// Whether this is a wildcard pattern
    pub is_wildcard: bool,
    /// Transform/sort/filter options
    pub options: BindingOptions,
    /// Tracked model length for max_items enforcement on delta path
    pub model_len: Cell<usize>,
    /// Map key → array index cache for keyed Map bindings
    ///
    /// **Context**: Map layers are converted to arrays for UI. This cache tracks
    /// which map key is at which array index, enabling surgical Set/Insert/Remove
    /// ops from Map deltas without full Replace.
    pub key_index: RefCell<HashMap<String, usize>>,
}

/// Manages all bindings for a page/app
pub struct BindingManager {
    /// Map of ui_property → binding
    bindings: HashMap<String, LayerBinding>,
    /// Map of expanded layer name → list of ui_properties that need updating
    /// For exact matches (non-wildcards)
    layer_to_properties: HashMap<String, Vec<String>>,
    /// Wildcard patterns: (expanded_pattern_prefix, ui_property)
    /// e.g., ("abc123/orders/", "all_orders")
    wildcard_patterns: Vec<(String, String)>,
}

impl BindingManager {
    /// Create a new binding manager
    pub fn new() -> Self {
        Self {
            bindings: HashMap::new(),
            layer_to_properties: HashMap::new(),
            wildcard_patterns: Vec::new(),
        }
    }

    /// Register a new binding
    ///
    /// # Arguments
    /// * `ui_property` - The UI property/model name to sync to
    /// * `expanded_pattern` - Full layer pattern with page_id prefix
    /// * `raw_pattern` - Original pattern as specified
    /// * `is_wildcard` - Whether pattern contains wildcards
    /// * `options` - Transform/sort/filter options
    pub fn register(
        &mut self,
        ui_property: String,
        expanded_pattern: String,
        raw_pattern: String,
        is_wildcard: bool,
        options: BindingOptions,
    ) {
        debug!(
            ui_property = %ui_property,
            expanded_pattern = %expanded_pattern,
            is_wildcard = is_wildcard,
            "Registering binding"
        );

        let binding = LayerBinding {
            ui_property: ui_property.clone(),
            expanded_pattern: expanded_pattern.clone(),
            raw_pattern,
            is_wildcard,
            options,
            model_len: Cell::new(0),
            key_index: RefCell::new(HashMap::new()),
        };

        // Track layer → properties mapping
        if is_wildcard {
            // For wildcards like "abc123/orders/*", store prefix "abc123/orders/"
            let prefix = expanded_pattern.trim_end_matches('*').to_string();
            self.wildcard_patterns.push((prefix, ui_property.clone()));
        } else {
            self.layer_to_properties
                .entry(expanded_pattern)
                .or_default()
                .push(ui_property.clone());
        }

        self.bindings.insert(ui_property, binding);
    }

    /// Get bindings that should be updated when a layer changes
    ///
    /// Returns list of (ui_property, is_wildcard, binding reference)
    pub fn get_bindings_for_layer(&self, layer_name: &str) -> Vec<&LayerBinding> {
        let mut result = Vec::new();

        // Check exact matches
        if let Some(properties) = self.layer_to_properties.get(layer_name) {
            for prop in properties {
                if let Some(binding) = self.bindings.get(prop) {
                    result.push(binding);
                }
            }
        }

        // Check wildcard matches
        for (prefix, ui_property) in &self.wildcard_patterns {
            if layer_name.starts_with(prefix) {
                if let Some(binding) = self.bindings.get(ui_property) {
                    result.push(binding);
                }
            }
        }

        result
    }

    /// Get a binding by its UI property name
    pub fn get_binding(&self, ui_property: &str) -> Option<&LayerBinding> {
        self.bindings.get(ui_property)
    }

    /// Rebind an existing binding to a different layer
    ///
    /// **Context**: Used when an app dynamically switches which layer backs a UI property
    /// (e.g., channel switching in chat). The transform/key/max_items options are preserved;
    /// only the backing layer changes. Caches are cleared and the caller is responsible
    /// for fetching and syncing the new layer's data.
    ///
    /// Returns true if the binding was found and rebound, false if ui_property doesn't exist.
    pub fn rebind(
        &mut self,
        ui_property: &str,
        new_expanded_pattern: String,
        new_raw_pattern: String,
        new_is_wildcard: bool,
    ) -> bool {
        let binding = match self.bindings.get_mut(ui_property) {
            Some(b) => b,
            None => return false,
        };

        let old_expanded = binding.expanded_pattern.clone();
        let old_is_wildcard = binding.is_wildcard;

        // Remove old layer mapping
        if old_is_wildcard {
            let old_prefix = old_expanded.trim_end_matches('*').to_string();
            self.wildcard_patterns
                .retain(|(prefix, prop)| !(prefix == &old_prefix && prop == ui_property));
        } else {
            if let Some(props) = self.layer_to_properties.get_mut(&old_expanded) {
                props.retain(|p| p != ui_property);
                if props.is_empty() {
                    self.layer_to_properties.remove(&old_expanded);
                }
            }
        }

        // Update binding fields
        binding.expanded_pattern = new_expanded_pattern.clone();
        binding.raw_pattern = new_raw_pattern;
        binding.is_wildcard = new_is_wildcard;

        // Clear caches — new layer has different data
        binding.key_index.borrow_mut().clear();
        binding.model_len.set(0);

        // Add new layer mapping
        if new_is_wildcard {
            let prefix = new_expanded_pattern.trim_end_matches('*').to_string();
            self.wildcard_patterns
                .push((prefix, ui_property.to_string()));
        } else {
            self.layer_to_properties
                .entry(new_expanded_pattern)
                .or_default()
                .push(ui_property.to_string());
        }

        debug!(
            ui_property = %ui_property,
            old_layer = %old_expanded,
            new_layer = %binding.expanded_pattern,
            "Rebound binding to new layer"
        );

        true
    }

    /// Check if any bindings are registered
    pub fn has_bindings(&self) -> bool {
        !self.bindings.is_empty()
    }

    /// Get all bindings
    pub fn all_bindings(&self) -> impl Iterator<Item = &LayerBinding> {
        self.bindings.values()
    }
}

impl Default for BindingManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Expand a layer pattern by substituting placeholders
///
/// # Placeholders
/// - `{me}` - Expands to the user's DID
///
/// Scribe uses bare layer names (no page_id/ prefix).
///
/// # Examples
/// - `"products"` → `"products"`
/// - `"orders/{me}"` with my_did "did:key:xyz" → `"orders/did:key:xyz"`
pub fn expand_pattern(pattern: &str, page_id: &str, my_did: &str) -> String {
    // Substitute {me} placeholder and add page_id prefix
    let expanded = pattern.replace("{me}", my_did);
    format!("{}/{}", page_id, expanded)
}

/// Check if a pattern is a wildcard pattern
pub fn is_wildcard_pattern(pattern: &str) -> bool {
    pattern.contains('*')
}

/// Apply transform function to data
///
/// For regular bindings: transform(item) -> transformed (or nil to skip)
/// For wildcard bindings: transform(layer_name, item) -> transformed (or nil to skip)
///
/// If transform returns nil, the item is skipped (filtered out).
pub fn apply_transform(
    lua: &Lua,
    transform_key: &RegistryKey,
    data: &serde_json::Value,
    layer_name: Option<&str>,
) -> LuaResult<serde_json::Value> {
    let transform: Function = lua.registry_value(transform_key)?;

    match data {
        serde_json::Value::Array(items) => {
            let mut result = Vec::new();
            for item in items {
                let lua_item = json_to_lua(lua, item)?;

                let transformed: Value = if let Some(layer) = layer_name {
                    // Wildcard: pass layer_name as first arg
                    transform.call((layer.to_string(), lua_item))?
                } else {
                    // Regular: just pass item
                    transform.call(lua_item)?
                };

                // Skip nil returns (allows transform to filter items)
                if transformed == Value::Nil {
                    continue;
                }

                let json_item = lua_to_json(&transformed)?;
                result.push(json_item);
            }
            Ok(serde_json::Value::Array(result))
        }
        // For non-arrays, transform as single item
        _ => {
            let lua_item = json_to_lua(lua, data)?;
            let transformed: Value = if let Some(layer) = layer_name {
                transform.call((layer.to_string(), lua_item))?
            } else {
                transform.call(lua_item)?
            };

            // Return null for nil (caller can decide what to do)
            if transformed == Value::Nil {
                return Ok(serde_json::Value::Null);
            }

            lua_to_json(&transformed)
        }
    }
}

/// Process data through binding options (transform)
///
/// **Note**: Data now arrives unwrapped from Scribe (via Layer::get_content())
///
/// **Map handling**: LoroMap layers return objects like `{"key": value, ...}`.
/// These are converted to arrays of values for transform/sort/filter processing.
pub fn process_binding_data(
    lua: &Lua,
    binding: &LayerBinding,
    data: &serde_json::Value,
    layer_name: Option<&str>,
) -> LuaResult<serde_json::Value> {
    let data_type = match data {
        serde_json::Value::Array(a) => format!("array[{}]", a.len()),
        serde_json::Value::Object(o) => format!("object{{{}}}", o.len()),
        serde_json::Value::Null => "null".to_string(),
        _ => "other".to_string(),
    };
    debug!(
        ui_property = %binding.ui_property,
        data_type = %data_type,
        "Processing binding data (already unwrapped)"
    );

    // Convert map data to array of values for uniform processing
    // LoroMap layers return {"key": value, ...}, we need [value1, value2, ...] for UI
    // Empty objects {} are treated as empty arrays [] (no data yet)
    let mut result = match data {
        serde_json::Value::Object(map) => {
            let values: Vec<serde_json::Value> = map.values().cloned().collect();
            debug!(
                ui_property = %binding.ui_property,
                original_keys = map.len(),
                converted_items = values.len(),
                "Converted map to array for binding"
            );

            // Build key_index cache for keyed Map bindings (enables surgical delta updates)
            if binding.options.key.is_some() {
                let mut cache = binding.key_index.borrow_mut();
                cache.clear();
                for (idx, key) in map.keys().enumerate() {
                    cache.insert(key.clone(), idx);
                }
                debug!(
                    ui_property = %binding.ui_property,
                    cache_size = cache.len(),
                    "Built key_index cache for Map binding"
                );
            }

            serde_json::Value::Array(values)
        }
        _ => data.clone(),
    };

    // Apply transform
    if let Some(ref transform_key) = binding.options.transform {
        let layer_for_transform = if binding.is_wildcard {
            layer_name
        } else {
            None
        };
        result = apply_transform(lua, transform_key, &result, layer_for_transform)?;
    }

    // NOTE: Sorting is NOT done here - it's a Slint view concern
    // This keeps indices stable for surgical updates (push/set/remove)

    // Trim to max_items if specified (keep latest, drop oldest)
    if let Some(max) = binding.options.max_items {
        if let serde_json::Value::Array(ref mut items) = result {
            if items.len() > max {
                let drain_count = items.len() - max;
                items.drain(0..drain_count);
            }
        }
    }

    // Update tracked model length for delta path
    if let serde_json::Value::Array(ref items) = result {
        binding.model_len.set(items.len());
    }

    Ok(result)
}

/// Convert delta to VecModelOps for a specific binding
///
/// **Context**: Handles delta with optional transform for surgical updates
/// **Returns**: Vec of VecModelOps (Insert/Remove). Empty if delta can't be converted
///             (e.g., Map/Text delta, or transform returns different count).
///
/// For List deltas:
/// - `Retain { count }` → advance index
/// - `Insert { values }` → apply transform to NEW items only → VecModelOp::Insert
/// - `Delete { count }` → VecModelOp::Remove
///
/// For Map/Text deltas: returns empty (fall back to Replace)
pub fn convert_delta_for_binding(
    lua: &Lua,
    binding: &LayerBinding,
    delta: &LoroDelta,
    layer_name: Option<&str>,
) -> Vec<VecModelOp> {
    match delta {
        LoroDelta::List { ops } => {
            let mut result = Vec::new();
            let mut index = 0;
            let mut model_len = binding.model_len.get();

            for op in ops {
                match op {
                    ListOp::Retain { count } => {
                        index += count;
                    }
                    ListOp::Delete { count } => {
                        // Remove from highest index first to keep indices stable
                        for _ in 0..*count {
                            result.push(VecModelOp::Remove {
                                model_name: binding.ui_property.clone(),
                                index,
                            });
                        }
                        model_len = model_len.saturating_sub(*count);
                        // Don't advance index — items shift down after remove
                    }
                    ListOp::Insert { values } => {
                        // Apply transform to each inserted value if binding has one
                        for (i, value) in values.iter().enumerate() {
                            let value_json = sthithi_to_json_value(value);
                            let item = if let Some(ref transform_key) = binding.options.transform {
                                // Transform single item
                                match apply_transform_single(
                                    lua,
                                    transform_key,
                                    &value_json,
                                    layer_name,
                                ) {
                                    Ok(Some(transformed)) => transformed,
                                    Ok(None) => continue, // nil = skip (filtered out by transform)
                                    Err(e) => {
                                        debug!(error = %e, "Transform failed in delta, falling back");
                                        return Vec::new(); // Fall back to Replace
                                    }
                                }
                            } else {
                                value_json
                            };

                            // When max_items is active, the model is shorter than
                            // the Loro document. Clamp insert index to model length
                            // so appends work correctly after trimming.
                            let insert_index = if binding.options.max_items.is_some() {
                                model_len.min(index + i)
                            } else {
                                index + i
                            };
                            result.push(VecModelOp::Insert {
                                model_name: binding.ui_property.clone(),
                                index: insert_index,
                                item,
                            });
                        }
                        model_len += values.len();
                        index += values.len();
                    }
                }
            }

            // Enforce max_items: remove oldest items from front
            if let Some(max) = binding.options.max_items {
                while model_len > max {
                    result.push(VecModelOp::Remove {
                        model_name: binding.ui_property.clone(),
                        index: 0,
                    });
                    model_len -= 1;
                }
            }

            binding.model_len.set(model_len);
            result
        }
        LoroDelta::Map { updated } => {
            // Keyed Map bindings get surgical updates via key_index cache
            if binding.options.key.is_none() {
                return Vec::new(); // No key option — fall back to Replace
            }

            let mut result = Vec::new();
            let mut cache = binding.key_index.borrow_mut();
            let mut model_len = binding.model_len.get();

            for (map_key, value) in updated {
                match value {
                    Some(val) => {
                        let val_json = sthithi_to_json_value(val);
                        // Apply transform if binding has one
                        let item = if let Some(ref transform_key) = binding.options.transform {
                            match apply_transform_single(lua, transform_key, &val_json, layer_name)
                            {
                                Ok(Some(transformed)) => transformed,
                                Ok(None) => continue, // nil = filtered out
                                Err(e) => {
                                    debug!(error = %e, "Transform failed in Map delta, falling back");
                                    return Vec::new();
                                }
                            }
                        } else {
                            val_json
                        };

                        if let Some(&existing_idx) = cache.get(map_key) {
                            // Key exists — update in place
                            result.push(VecModelOp::Set {
                                model_name: binding.ui_property.clone(),
                                index: existing_idx,
                                item,
                            });
                        } else {
                            // New key — append
                            let insert_idx = model_len;
                            result.push(VecModelOp::Insert {
                                model_name: binding.ui_property.clone(),
                                index: insert_idx,
                                item,
                            });
                            cache.insert(map_key.clone(), insert_idx);
                            model_len += 1;
                        }
                    }
                    None => {
                        // Key deleted
                        if let Some(removed_idx) = cache.remove(map_key) {
                            result.push(VecModelOp::Remove {
                                model_name: binding.ui_property.clone(),
                                index: removed_idx,
                            });
                            // Shift down indices above the removed item
                            for idx in cache.values_mut() {
                                if *idx > removed_idx {
                                    *idx -= 1;
                                }
                            }
                            model_len = model_len.saturating_sub(1);
                        }
                    }
                }
            }

            // Enforce max_items: remove oldest items from front
            if let Some(max) = binding.options.max_items {
                while model_len > max {
                    // Remove from index 0 (oldest)
                    result.push(VecModelOp::Remove {
                        model_name: binding.ui_property.clone(),
                        index: 0,
                    });
                    // Shift all cached indices down by 1
                    let mut evicted_key = None;
                    for (k, idx) in cache.iter_mut() {
                        if *idx == 0 {
                            evicted_key = Some(k.clone());
                        } else {
                            *idx -= 1;
                        }
                    }
                    if let Some(k) = evicted_key {
                        cache.remove(&k);
                    }
                    model_len -= 1;
                }
            }

            binding.model_len.set(model_len);
            result
        }
    }
}

/// Apply transform to a single item
///
/// **Returns**: Some(transformed) or None if transform returns nil (skip item)
fn apply_transform_single(
    lua: &Lua,
    transform_key: &RegistryKey,
    value: &serde_json::Value,
    layer_name: Option<&str>,
) -> LuaResult<Option<serde_json::Value>> {
    let transform: Function = lua.registry_value(transform_key)?;
    let lua_item = json_to_lua(lua, value)?;

    let transformed: Value = if let Some(layer) = layer_name {
        transform.call((layer.to_string(), lua_item))?
    } else {
        transform.call(lua_item)?
    };

    if transformed == Value::Nil {
        return Ok(None);
    }

    let json_item = lua_to_json(&transformed)?;
    Ok(Some(json_item))
}

/// Convert processed data to UI mutation (takes ownership to avoid clones)
///
/// For array data, uses VecModelOp::Replace which:
/// - Uses the existing VecModel that's already connected to the Slint property
/// - Calls set_vec() to atomically replace all items
/// - Properly triggers Slint's model notifications
///
/// For non-array data, uses PropertyUpdate directly.
///
/// Special cases:
/// - Empty objects `{}` are skipped (no mutation) - typically means "no data yet"
/// - Null values are skipped (no mutation)
pub fn data_to_ui_mutation(
    app_id: &str,
    ui_property: &str,
    data: serde_json::Value,
) -> Option<UiMutation> {
    let data_type = match &data {
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::Bool(_) => "bool",
        serde_json::Value::Null => "null",
    };
    debug!(
        ui_property = %ui_property,
        data_type = %data_type,
        "data_to_ui_mutation called"
    );

    match data {
        serde_json::Value::Array(items) => Some(UiMutation {
            app_id: app_id.to_string(),
            properties: vec![],
            model_ops: vec![VecModelOp::Replace {
                model_name: ui_property.to_string(),
                items,
            }],
        }),
        serde_json::Value::Object(ref map) if map.is_empty() => {
            debug!(
                ui_property = %ui_property,
                "Skipping empty object mutation"
            );
            None
        }
        serde_json::Value::Null => {
            debug!(
                ui_property = %ui_property,
                "Skipping null mutation"
            );
            None
        }
        _ => Some(UiMutation {
            app_id: app_id.to_string(),
            properties: vec![PropertyUpdate {
                key: ui_property.to_string(),
                value: data,
            }],
            model_ops: vec![],
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_pattern() {
        assert_eq!(
            expand_pattern("products", "page123", "did:key:abc"),
            "page123/products"
        );

        assert_eq!(
            expand_pattern("orders/{me}", "page123", "did:key:abc"),
            "page123/orders/did:key:abc"
        );

        assert_eq!(
            expand_pattern("derived/orders_summary", "page123", "did:key:abc"),
            "page123/derived/orders_summary"
        );
    }

    #[test]
    fn test_is_wildcard_pattern() {
        assert!(!is_wildcard_pattern("products"));
        assert!(!is_wildcard_pattern("orders/{me}"));
        assert!(is_wildcard_pattern("orders/*"));
        assert!(is_wildcard_pattern("*/orders"));
    }

    #[test]
    fn test_binding_manager_exact_match() {
        let mut manager = BindingManager::new();
        manager.register(
            "products".to_string(),
            "page123/products".to_string(),
            "products".to_string(),
            false,
            BindingOptions::default(),
        );

        let bindings = manager.get_bindings_for_layer("page123/products");
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings[0].ui_property, "products");

        let bindings = manager.get_bindings_for_layer("page123/orders");
        assert_eq!(bindings.len(), 0);
    }

    #[test]
    fn test_binding_manager_wildcard() {
        let mut manager = BindingManager::new();
        manager.register(
            "all_orders".to_string(),
            "page123/orders/*".to_string(),
            "orders/*".to_string(),
            true,
            BindingOptions::default(),
        );

        let bindings = manager.get_bindings_for_layer("page123/orders/did:key:user1");
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings[0].ui_property, "all_orders");

        let bindings = manager.get_bindings_for_layer("page123/orders/did:key:user2");
        assert_eq!(bindings.len(), 1);

        let bindings = manager.get_bindings_for_layer("page123/products");
        assert_eq!(bindings.len(), 0);
    }

    #[test]
    fn test_rebind_exact_to_exact() {
        let mut manager = BindingManager::new();
        manager.register(
            "messages".to_string(),
            "channels/general/messages".to_string(),
            "channels/general/messages".to_string(),
            false,
            BindingOptions::default(),
        );

        // Old layer matches
        assert_eq!(
            manager
                .get_bindings_for_layer("channels/general/messages")
                .len(),
            1
        );
        // New layer doesn't match yet
        assert_eq!(
            manager
                .get_bindings_for_layer("channels/random/messages")
                .len(),
            0
        );

        let result = manager.rebind(
            "messages",
            "channels/random/messages".to_string(),
            "channels/random/messages".to_string(),
            false,
        );
        assert!(result);

        // Old layer no longer matches
        assert_eq!(
            manager
                .get_bindings_for_layer("channels/general/messages")
                .len(),
            0
        );
        // New layer matches
        assert_eq!(
            manager
                .get_bindings_for_layer("channels/random/messages")
                .len(),
            1
        );
        assert_eq!(
            manager.get_binding("messages").unwrap().expanded_pattern,
            "channels/random/messages"
        );
    }

    #[test]
    fn test_rebind_nonexistent() {
        let mut manager = BindingManager::new();
        let result = manager.rebind(
            "nonexistent",
            "some/layer".to_string(),
            "some/layer".to_string(),
            false,
        );
        assert!(!result);
    }

    #[test]
    fn test_rebind_same_layer() {
        let mut manager = BindingManager::new();
        manager.register(
            "messages".to_string(),
            "channels/general/messages".to_string(),
            "channels/general/messages".to_string(),
            false,
            BindingOptions::default(),
        );

        // Rebinding to the same layer should still work (clears caches)
        let result = manager.rebind(
            "messages",
            "channels/general/messages".to_string(),
            "channels/general/messages".to_string(),
            false,
        );
        assert!(result);
        assert_eq!(
            manager
                .get_bindings_for_layer("channels/general/messages")
                .len(),
            1
        );
    }

    #[test]
    fn test_rebind_clears_cache() {
        let mut manager = BindingManager::new();
        manager.register(
            "messages".to_string(),
            "channels/general/messages".to_string(),
            "channels/general/messages".to_string(),
            false,
            BindingOptions {
                key: Some("id".to_string()),
                ..Default::default()
            },
        );

        // Simulate populated cache
        let binding = manager.get_binding("messages").unwrap();
        binding.key_index.borrow_mut().insert("msg1".to_string(), 0);
        binding.key_index.borrow_mut().insert("msg2".to_string(), 1);
        binding.model_len.set(2);

        assert_eq!(binding.key_index.borrow().len(), 2);
        assert_eq!(binding.model_len.get(), 2);

        manager.rebind(
            "messages",
            "channels/random/messages".to_string(),
            "channels/random/messages".to_string(),
            false,
        );

        let binding = manager.get_binding("messages").unwrap();
        assert_eq!(binding.key_index.borrow().len(), 0);
        assert_eq!(binding.model_len.get(), 0);
    }
}
