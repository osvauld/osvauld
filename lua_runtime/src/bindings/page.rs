//! Page Bindings for Lua
//!
//! Provides `page:on_change()` API for registering layer event handlers.

use mlua::{Error as LuaError, RegistryKey, UserData, UserDataMethods};
use std::sync::Arc;

/// Registered page event handler
pub struct PageHandler {
    /// Pattern to match (e.g., "{page_id}/orders/*")
    pub pattern: String,
    /// Lua function reference (stored in registry)
    pub callback: RegistryKey,
}

/// Page bindings providing `page:on_change()` API
///
/// **Usage in Lua**:
/// ```lua
/// page:on_change("{page_id}/orders/*", function(event)
///     -- event.layer_name = "shop123/orders/did:key:customer"
///     -- event.event_type = "created" | "updated"
///     -- event.full_data = { ... }
///     local summary_layer = loro:get_layer("{page_id}/derived/orders_summary", "map")
///     summary_layer:set(event.full_data.id, transform(event.full_data))
/// end)
/// ```
pub struct PageBindings {
    /// Registered handlers (pattern -> callback)
    pub handlers: Arc<std::sync::RwLock<Vec<PageHandler>>>,
    /// Page ID for pattern expansion
    pub page_id: String,
}

impl UserData for PageBindings {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        // page:on_change(pattern, callback)
        methods.add_method(
            "on_change",
            |lua, this, (pattern, callback): (String, mlua::Function)| {
                // Expand {page_id} in pattern
                let expanded_pattern = pattern.replace("{page_id}", &this.page_id);

                // Store callback in Lua registry (keeps it alive)
                let registry_key = lua.create_registry_value(callback).map_err(|e| {
                    LuaError::RuntimeError(format!("Failed to store callback: {}", e))
                })?;

                // Add handler
                let handler = PageHandler {
                    pattern: expanded_pattern.clone(),
                    callback: registry_key,
                };

                if let Ok(mut handlers) = this.handlers.write() {
                    handlers.push(handler);
                    tracing::info!(
                        pattern = %expanded_pattern,
                        handler_count = handlers.len(),
                        "Registered page:on_change handler"
                    );
                }

                Ok(())
            },
        );
    }
}

