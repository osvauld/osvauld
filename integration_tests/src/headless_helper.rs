//! Headless runtime test helpers
//!
//! Provides utilities to create HeadlessRuntime instances for testing
//! with real Lua app code but mocked UI bindings.

use butler::runtime::HeadlessRuntime;
use butler::scribe::permit::extract_role_from_permit;
use butler::Butler;

/// Create a HeadlessRuntime for a test peer
///
/// **Context**: Sets up real Lua runtime with loro/permit bindings
/// **Mock**: Adds mock `ui` binding so app code doesn't fail on ui:set()
pub async fn create_headless_runtime(
    butler: &Butler,
    page_id: &str,
    app_lua_code: &str,
) -> Result<HeadlessRuntime, String> {
    // Get scribe for page (uses real open_page flow)
    let scribe_ref = butler
        .open_page(page_id)
        .await
        .map_err(|e| format!("Failed to open page: {}", e))?;

    // Get identity from butler
    let identity = butler
        .get_identity()
        .await
        .map_err(|e| format!("Failed to get identity: {}", e))?;

    // Get page data to extract role from permit
    let page = butler
        .get_page(page_id)
        .map_err(|e| format!("Failed to get page: {}", e))?
        .ok_or_else(|| format!("Page not found: {}", page_id))?;

    // Extract role from permit
    let role = page
        .get_permit()
        .map(|p| extract_role_from_permit(p))
        .unwrap_or_else(|| "viewer".to_string());

    // Create runtime with real bindings
    let runtime = HeadlessRuntime::new(page_id, scribe_ref, identity.did(), &role)
        .await
        .map_err(|e| format!("Failed to create HeadlessRuntime: {}", e))?;

    // Add mock UI binding so app code doesn't fail on ui:set()
    add_mock_ui_binding(&runtime)?;

    // Load app code
    runtime
        .load_code(app_lua_code)
        .map_err(|e| format!("Failed to load app code: {}", e))?;

    Ok(runtime)
}

/// Add mock UI binding to HeadlessRuntime
///
/// **Context**: App Lua code calls ui:set()/ui:get() for UI operations
/// **Mock**: Provides in-memory storage so apps work in headless mode
fn add_mock_ui_binding(runtime: &HeadlessRuntime) -> Result<(), String> {
    let lua = runtime.lua();

    // Create mock ui table with get/set methods using in-memory storage
    lua.load(
        r#"
        local ui_storage = {}
        ui = {
            set = function(self, key, value)
                ui_storage[key] = value
            end,
            get = function(self, key)
                return ui_storage[key]
            end,
            push = function(self, model, item) end,
            insert = function(self, model, index, item) end,
            remove = function(self, model, index) end,
            clear = function(self, model) end,
            update = function(self, model, index, item) end,
        }
        "#,
    )
    .exec()
    .map_err(|e| format!("Failed to setup mock ui: {}", e))?;

    Ok(())
}

/// Create a HeadlessRuntime with custom role override
///
/// **Context**: Useful when testing with specific roles that may not match permit
pub async fn create_headless_runtime_with_role(
    butler: &Butler,
    page_id: &str,
    app_lua_code: &str,
    role: &str,
) -> Result<HeadlessRuntime, String> {
    let scribe_ref = butler
        .open_page(page_id)
        .await
        .map_err(|e| format!("Failed to open page: {}", e))?;

    let identity = butler
        .get_identity()
        .await
        .map_err(|e| format!("Failed to get identity: {}", e))?;

    let runtime = HeadlessRuntime::new(page_id, scribe_ref, identity.did(), role)
        .await
        .map_err(|e| format!("Failed to create HeadlessRuntime: {}", e))?;

    add_mock_ui_binding(&runtime)?;

    runtime
        .load_code(app_lua_code)
        .map_err(|e| format!("Failed to load app code: {}", e))?;

    Ok(runtime)
}
