//! Lua bindings for headless runtime
//!
//! Provides loro and permit access to Lua without UI dependencies.
//! These are the canonical implementations - app_runtime re-exports these.

mod loro;
mod convert;
mod permit;
mod derivation;
mod peers;

pub use loro::{LoroBindings, LayerWrapper, LuaLoroList, LuaLoroMap};
pub use convert::{
    // Loro <-> Lua
    loro_value_to_lua, lua_to_loro_value,
    // JSON <-> Lua
    json_to_lua, lua_to_json,
    // JSON <-> Loro
    json_to_loro_value, loro_value_to_json,
    // Pattern matching
    matches_layer_pattern,
};
pub use permit::PermitBindings;
pub use derivation::DerivationBindings;
pub use peers::PeersBindings;

use mlua::Error as LuaError;

// =============================================================================
// Helper: Block on async operation
// =============================================================================

/// Block on async operation, handling both tokio runtime and non-runtime contexts
///
/// **Context**: When inside tokio runtime, uses block_in_place for efficiency.
/// When outside (e.g., Qt/Slint event loop), creates temporary runtime.
pub(crate) fn block_on_async<T, F: std::future::Future<Output = T>>(future: F) -> Result<T, LuaError> {
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => {
            // We're in a Tokio runtime, use block_in_place for efficiency
            Ok(tokio::task::block_in_place(|| handle.block_on(future)))
        }
        Err(_) => {
            // Not in a runtime (e.g., headless context), use futures executor
            Ok(futures::executor::block_on(future))
        }
    }
}
