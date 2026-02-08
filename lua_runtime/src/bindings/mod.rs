//! Lua bindings for the runtime
//!
//! Provides unified bindings for all Lua runtime interactions.
//!
//! ## Core Bindings (always available)
//! - `scribe` - Unified CRDT API (map, list, get, set, bind, etc.)
//! - `permit` - User identity context (page_id, my_did, role)
//! - `peers` - Subscriber count for broadcast decisions
//! - `derivation` - Derived layer creation
//! - `layout` - Graph layout algorithms
//! - `emoji` - Emoji lookup by shortcode
//!
//! ## UI Bindings (when ui_enabled=true)
//! - `ui` - Property updates and VecModel operations
//! - `page` - Layer event handlers (on_change)

// Core bindings
pub mod binding;
mod convert;
mod derivation;
mod emoji;
mod helpers;
mod layout;
mod loro;
mod page;
mod peers;
mod permit;
mod scribe;
mod ui;

// Unified Scribe binding (the single API for apps)
pub use scribe::ScribeBindings;

// Binding system for declarative layer → UI sync
// NOTE: apply_sort removed - sorting is a Slint view concern for stable indices
pub use binding::{
    apply_transform, convert_delta_for_binding, data_to_ui_mutation, expand_pattern,
    is_wildcard_pattern, process_binding_data, BindingManager, BindingOptions, LayerBinding,
};

// Internal types used by ScribeBindings (not for direct app use)
pub use convert::{
    // JSON <-> Loro
    json_to_loro_value,
    // JSON <-> Lua
    json_to_lua,
    loro_value_to_json,
    // Loro <-> Lua
    loro_value_to_lua,
    lua_to_json,
    lua_to_loro_value,
    // Pattern matching
    matches_layer_pattern,
};
pub use derivation::DerivationBindings;
pub use helpers::{
    notify_change, register_drafts, register_helpers, ChangeSubscription, DraftsHelper,
    HelpersContext, SubscriptionManager,
};
pub use layout::LayoutBindings;
pub use loro::{LayerWrapper, LuaLoroList, LuaLoroMap};

// Additional bindings
pub use emoji::EmojiBindings;
pub use page::PageBindings;
pub use peers::PeersBindings;
pub use permit::PermitBindings;
pub use ui::{event_to_lua, parse_subscribe_options, UiBindings, UiSharedState};

use mlua::Error as LuaError;

// Helper: Block on async operation

/// Block on async operation, handling both tokio runtime and non-runtime contexts
///
/// **Context**: When inside tokio runtime, uses block_in_place for efficiency.
/// When outside (e.g., Qt/Slint event loop), creates temporary runtime.
pub(crate) fn block_on_async<T, F: std::future::Future<Output = T>>(
    future: F,
) -> Result<T, LuaError> {
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
