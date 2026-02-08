//! Runtime helpers for app.lua
//!
//! Provides declarative helpers to reduce app boilerplate:
//! - `use_layers(config)` - Layer initialization
//! - `on_change(pattern, callback)` - Change subscriptions
//! - `require_role(role)` / `has_role(role)` - Permission guards
//!
//! These helpers are registered as global functions in the Lua environment.

use mlua::{Lua, Table, Value, Function, Error as LuaError, Result as LuaResult};
use ractor::ActorRef;
use std::sync::{Arc, Mutex};
use tracing::debug;

use butler::ScribeMessage;
use super::scribe::ScribeBindings;
use super::convert::matches_layer_pattern;

// Change Subscriptions

/// Subscription entry for on_change callbacks
pub struct ChangeSubscription {
    /// Pattern to match (e.g., "products", "orders/*")
    pub pattern: String,
    /// Lua function registry key
    pub callback_key: mlua::RegistryKey,
}

/// Container for change subscriptions
pub struct SubscriptionManager {
    subscriptions: Vec<ChangeSubscription>,
}

impl SubscriptionManager {
    pub fn new() -> Self {
        Self {
            subscriptions: Vec::new(),
        }
    }

    pub fn add(&mut self, pattern: String, callback_key: mlua::RegistryKey) {
        self.subscriptions.push(ChangeSubscription { pattern, callback_key });
    }

    /// Get all subscriptions matching a layer name
    pub fn matching(&self, layer_name: &str) -> Vec<&ChangeSubscription> {
        self.subscriptions
            .iter()
            .filter(|s| matches_layer_pattern(&s.pattern, layer_name))
            .collect()
    }
}

impl Default for SubscriptionManager {
    fn default() -> Self {
        Self::new()
    }
}

// Helper Registration

/// Context for helper functions
pub struct HelpersContext {
    pub page_id: String,
    pub our_did: String,
    pub our_role: String,
    pub subscription_manager: Arc<Mutex<SubscriptionManager>>,
}

impl HelpersContext {
    pub fn new(page_id: String, our_did: String, our_role: String) -> Self {
        Self {
            page_id,
            our_did,
            our_role,
            subscription_manager: Arc::new(Mutex::new(SubscriptionManager::new())),
        }
    }
}

/// Register helper functions in Lua globals
///
/// This registers:
/// - `use_layers(config)` - Initialize layers
/// - `on_change(pattern, callback)` - Subscribe to changes
/// - `require_role(role)` - Permission guard (throws on mismatch)
/// - `has_role(role)` - Permission check (returns bool)
pub fn register_helpers(
    lua: &Lua,
    ctx: Arc<HelpersContext>,
    scribe_ref: ActorRef<ScribeMessage>,
) -> LuaResult<()> {
    let globals = lua.globals();

    // Register use_layers
    register_use_layers(lua, &globals, ctx.clone(), scribe_ref.clone())?;

    // Register on_change
    register_on_change(lua, &globals, ctx.clone())?;

    // Register require_role
    register_require_role(lua, &globals, ctx.clone())?;

    // Register has_role
    register_has_role(lua, &globals, ctx.clone())?;

    // Set global context variables
    globals.set("page_id", ctx.page_id.clone())?;
    globals.set("my_did", ctx.our_did.clone())?;
    globals.set("my_role", ctx.our_role.clone())?;

    Ok(())
}

/// Register `use_layers(config)` function
///
/// Example:
/// ```lua
/// use_layers {
///     products = "list",
///     orders = "list",
///     drafts = { type = "map", sync = false }
/// }
/// ```
fn register_use_layers(
    lua: &Lua,
    globals: &Table,
    ctx: Arc<HelpersContext>,
    scribe_ref: ActorRef<ScribeMessage>,
) -> LuaResult<()> {
    let page_id = ctx.page_id.clone();

    let use_layers_fn = lua.create_function(move |lua, config: Table| {
        debug!("use_layers called");

        // Create layers table to store layer references
        let layers_table = lua.create_table()?;

        // Get scribe bindings for creating layers
        let scribe = ScribeBindings::new(scribe_ref.clone(), page_id.clone(), String::new(), None);

        // Iterate config and create layers
        for pair in config.pairs::<String, Value>() {
            let (name, type_or_config) = pair?;

            // Determine layer type
            let layer_type = match type_or_config {
                Value::String(s) => s.to_str()?.to_string(),
                Value::Table(t) => {
                    t.get::<String>("type").unwrap_or_else(|_| "list".to_string())
                }
                _ => "list".to_string(),
            };

            // Build full layer name
            let full_name = format!("{}/{}", page_id, name);
            debug!("Creating layer: {} (type: {})", full_name, layer_type);

            // Register layer name in layers table
            match layer_type.as_str() {
                "list" | "map" => {
                    layers_table.set(name.clone(), full_name.clone())?;
                }
                _ => {
                    return Err(LuaError::RuntimeError(
                        format!("Invalid layer type: {}", layer_type)
                    ));
                }
            }
        }

        // Set global layers table
        lua.globals().set("layers", layers_table)?;

        // Set scribe bindings as global (unified API)
        lua.globals().set("scribe", scribe)?;

        Ok(())
    })?;

    globals.set("use_layers", use_layers_fn)?;
    Ok(())
}

/// Register `on_change(pattern, callback)` function
///
/// Example:
/// ```lua
/// on_change("products", function(layer_name, delta)
///     refresh_products_ui()
/// end)
/// ```
fn register_on_change(
    lua: &Lua,
    globals: &Table,
    ctx: Arc<HelpersContext>,
) -> LuaResult<()> {
    let subscription_manager = ctx.subscription_manager.clone();

    let on_change_fn = lua.create_function(move |lua, (pattern, callback): (String, Function)| {
        debug!("on_change registered for pattern: {}", pattern);

        // Store callback in registry
        let callback_key = lua.create_registry_value(callback)?;

        // Add to subscription manager
        subscription_manager
            .lock()
            .map_err(|_| LuaError::RuntimeError("Failed to lock subscription manager".to_string()))?
            .add(pattern.clone(), callback_key);

        Ok(())
    })?;

    globals.set("on_change", on_change_fn)?;
    Ok(())
}

/// Register `require_role(role)` function
///
/// Throws an error if current role doesn't match.
///
/// Example:
/// ```lua
/// function on_delete_product()
///     require_role("owner")  -- Throws error if not owner
///     -- ... delete logic
/// end
/// ```
fn register_require_role(
    lua: &Lua,
    globals: &Table,
    ctx: Arc<HelpersContext>,
) -> LuaResult<()> {
    let our_role = ctx.our_role.clone();

    let require_role_fn = lua.create_function(move |_, required: String| {
        if our_role == required {
            Ok(())
        } else {
            Err(LuaError::RuntimeError(format!(
                "Permission denied: required role '{}', current role '{}'",
                required, our_role
            )))
        }
    })?;

    globals.set("require_role", require_role_fn)?;
    Ok(())
}

/// Register `has_role(role)` function
///
/// Returns true if current role matches.
///
/// Example:
/// ```lua
/// if has_role("owner") then
///     -- Show admin controls
/// end
/// ```
fn register_has_role(
    lua: &Lua,
    globals: &Table,
    ctx: Arc<HelpersContext>,
) -> LuaResult<()> {
    let our_role = ctx.our_role.clone();

    let has_role_fn = lua.create_function(move |_, required: String| {
        Ok(our_role == required)
    })?;

    globals.set("has_role", has_role_fn)?;
    Ok(())
}

// Change Notification

/// Notify subscribed callbacks about a layer change
///
/// Called by the runtime when a layer changes.
pub fn notify_change(
    lua: &Lua,
    subscription_manager: &SubscriptionManager,
    layer_name: &str,
    delta: Value,
) -> LuaResult<()> {
    let matching = subscription_manager.matching(layer_name);

    for sub in matching {
        // Get callback from registry
        let callback: Function = lua.registry_value(&sub.callback_key)?;

        // Call with (layer_name, delta)
        if let Err(e) = callback.call::<()>((layer_name.to_string(), delta.clone())) {
            tracing::warn!(
                pattern = %sub.pattern,
                layer = %layer_name,
                error = %e,
                "on_change callback failed"
            );
        }
    }

    Ok(())
}

// Drafts Helper

/// Drafts helper for managing local drafts
///
/// Example:
/// ```lua
/// drafts:init(layers.drafts)
/// drafts:create("order", { id = "123", items = {...} })
/// local order = drafts:get("order", "123")
/// drafts:submit("order", "123", layers.orders)
/// ```
pub struct DraftsHelper {
    drafts_layer_name: Option<String>,
    #[allow(dead_code)]
    scribe_ref: ActorRef<ScribeMessage>,
}

impl DraftsHelper {
    pub fn new(scribe_ref: ActorRef<ScribeMessage>) -> Self {
        Self {
            drafts_layer_name: None,
            scribe_ref,
        }
    }
}

impl mlua::UserData for DraftsHelper {
    fn add_methods<M: mlua::UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method_mut("init", |_, this, layer_name: String| {
            this.drafts_layer_name = Some(layer_name);
            Ok(())
        });

        methods.add_method("create", |_, this, (draft_type, data): (String, Table)| {
            let _layer_name = this.drafts_layer_name.as_ref()
                .ok_or_else(|| LuaError::RuntimeError("Drafts not initialized. Call drafts:init() first.".to_string()))?;

            // Get id from data
            let id: String = data.get("id")?;
            let draft_key = format!("{}:{}", draft_type, id);

            debug!("Creating draft: {}", draft_key);

            // TODO: Store in drafts layer through scribe

            Ok(())
        });

        methods.add_method("get", |_, this, (draft_type, id): (String, String)| {
            let _layer_name = this.drafts_layer_name.as_ref()
                .ok_or_else(|| LuaError::RuntimeError("Drafts not initialized. Call drafts:init() first.".to_string()))?;

            let draft_key = format!("{}:{}", draft_type, id);
            debug!("Getting draft: {}", draft_key);

            // TODO: Get from drafts layer
            Ok(Value::Nil)
        });

        methods.add_method("submit", |_, this, (draft_type, id, _target_layer): (String, String, String)| {
            let _layer_name = this.drafts_layer_name.as_ref()
                .ok_or_else(|| LuaError::RuntimeError("Drafts not initialized. Call drafts:init() first.".to_string()))?;

            let draft_key = format!("{}:{}", draft_type, id);
            debug!("Submitting draft: {} to target layer", draft_key);

            // TODO: Move from drafts to target layer
            Ok(())
        });
    }
}

/// Register drafts helper in Lua globals
pub fn register_drafts(lua: &Lua, scribe_ref: ActorRef<ScribeMessage>) -> LuaResult<()> {
    let drafts = DraftsHelper::new(scribe_ref);
    lua.globals().set("drafts", drafts)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_matching() {
        // Test pattern matching logic
        // Note: matches_layer_pattern uses * as wildcard, not {aud}
        assert!(matches_layer_pattern("products", "products"));
        assert!(matches_layer_pattern("orders/*", "orders/user123"));
        assert!(matches_layer_pattern("orders/*", "orders/did:key:xyz"));
        assert!(!matches_layer_pattern("products", "orders"));
    }

    #[test]
    fn test_subscription_manager_creation() {
        let manager = SubscriptionManager::new();
        // Empty manager should have no matching subscriptions
        assert!(manager.matching("products").is_empty());
    }
}
