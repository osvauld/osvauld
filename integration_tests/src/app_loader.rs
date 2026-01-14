//! App code loader for integration tests
//!
//! Loads Lua code from sample_apps for use in HeadlessRuntime tests.

use std::path::Path;

/// Load my-shop owner app Lua code
pub fn load_myshop_owner_app() -> String {
    load_lua_file("sample_apps/my-shop/shop-owner/app.lua")
}

/// Load my-shop customer app Lua code
pub fn load_myshop_customer_app() -> String {
    load_lua_file("sample_apps/my-shop/shop-customer/app.lua")
}

/// Load my-shop shared validation Lua code
pub fn load_myshop_validation() -> String {
    load_lua_file("sample_apps/my-shop/shared/validation.lua")
}

/// Load my-shop shared init Lua code (derivation rules)
pub fn load_myshop_init() -> String {
    load_lua_file("sample_apps/my-shop/shared/init.lua")
}

/// Load my-shop owner app with init code prepended
/// This combines init.lua (derivation rules) with the owner app.
pub fn load_myshop_owner_with_init() -> String {
    let init = load_myshop_init();
    let owner = load_myshop_owner_app();
    format!("{}\n\n{}", init, owner)
}

/// Load a Lua file from the project root
fn load_lua_file(relative_path: &str) -> String {
    // Try from integration_tests directory first (when running `cargo test -p integration_tests`)
    let from_integration_tests = Path::new("..").join(relative_path);
    if from_integration_tests.exists() {
        return std::fs::read_to_string(&from_integration_tests)
            .unwrap_or_else(|e| panic!("Failed to load {}: {}", relative_path, e));
    }

    // Try from project root (when running from workspace root)
    let from_root = Path::new(relative_path);
    if from_root.exists() {
        return std::fs::read_to_string(from_root)
            .unwrap_or_else(|e| panic!("Failed to load {}: {}", relative_path, e));
    }

    panic!(
        "Could not find Lua file: {}. Tried:\n  - {}\n  - {}",
        relative_path,
        from_integration_tests.display(),
        from_root.display()
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_owner_app() {
        let code = load_myshop_owner_app();
        assert!(code.contains("function add_product"));
        assert!(code.contains("function on_init"));
    }

    #[test]
    fn test_load_customer_app() {
        let code = load_myshop_customer_app();
        assert!(code.contains("function create_order"));
        assert!(code.contains("function submit_order"));
    }
}
