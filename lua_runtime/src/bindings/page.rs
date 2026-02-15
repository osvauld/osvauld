//! Page Bindings for Lua
//!
//! Provides `page:open_app()` API for in-page navigation.

use mlua::{Error as LuaError, UserData, UserDataMethods};

/// Page bindings providing `page:open_app()` API.
pub struct PageBindings {
    /// Channel for in-page navigation (sends app name to tab switcher)
    pub navigate_tx: Option<std::sync::mpsc::Sender<String>>,
}

impl UserData for PageBindings {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        // page:open_app(app_name) — navigate to a sibling app in the same page
        methods.add_method("open_app", |_lua, this, app_name: String| {
            if let Some(ref tx) = this.navigate_tx {
                tx.send(app_name.clone()).map_err(|e| {
                    LuaError::RuntimeError(format!(
                        "Failed to navigate to app '{}': {}",
                        app_name, e
                    ))
                })?;
            }
            Ok(())
        });
    }
}
