//! Lua Runtime - Unified Lua runtime for Osvauld
//!
//! Single runtime implementation used by both shell (viewer) and kunki (node).
//!
//! ## Features
//!
//! - **Unified Runtime**: One `LuaRuntime` for all contexts
//! - **Timer Support**: JS-style `setTimeout`/`setInterval` via Scheduler
//! - **Validation**: Built-in `validate_ops()` method for permission checking
//! - **Async Event Loop**: Uses `tokio::select!` for efficient multiplexing
//! - **Optional UI**: Enable/disable UI bindings based on configuration
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────┐     ┌─────────────────┐
//! │     sthalam     │     │     kunki       │
//! │  (ui_enabled)   │     │ (ui_disabled)   │
//! └────────┬────────┘     └────────┬────────┘
//!          │                       │
//!          │  LuaRuntimeConfig     │
//!          └───────────┬───────────┘
//!                      │
//!          ┌───────────▼───────────┐
//!          │      LuaRuntime       │
//!          │  - Scheduler (timers) │
//!          │  - ScribeBindings     │
//!          │  - validate_ops()     │
//!          │  - Event loop         │
//!          └───────────────────────┘
//! ```
//!
//! ## Usage
//!
//! ```ignore
//! let config = LuaRuntimeConfig {
//!     page_id: "page123".into(),
//!     scribe_ref,
//!     user_did: "did:key:...".into(),
//!     user_name: "Alice".into(),
//!     user_role: "viewer".into(),
//!     ui_enabled: false,  // true for shell
//!     ui_tx: None,
//!     cmd_rx,
//!     tick_enabled: false,
//! };
//!
//! let mut runtime = LuaRuntime::new(config).await?;
//! runtime.load_code(&app_lua)?;
//! runtime.run().await?;
//! ```

// Module declarations

// Core bindings (always available)
pub mod bindings;

// Event bus for UI interactions
pub mod event_bus;

// Timer and coroutine scheduler
pub mod scheduler;

// UI types (VecModelOp, UiMutation, PropertyUpdate, UiQuery)
pub mod ui_types;

mod runtime;
mod commands;

pub use runtime::{LuaRuntime, LuaRuntimeConfig};
pub use commands::{
    LuaCommand, UiEventType, DebugState,
    ValidationContext, ValidationResult,
    LoroDelta, ListOp,
};
// Re-export JsonOp from butler (domains) for consistency
pub use butler::JsonOp;
pub use scheduler::Scheduler;

// Re-export binding types
pub use bindings::{
    // Unified Scribe API
    ScribeBindings,
    // Internal types
    DerivationBindings, LayoutBindings,
    LayerWrapper, LuaLoroList, LuaLoroMap,
    // Loro <-> Lua
    loro_value_to_lua, lua_to_loro_value,
    // JSON <-> Lua
    json_to_lua, lua_to_json,
    // JSON <-> Loro
    json_to_loro_value, loro_value_to_json,
    // Pattern matching
    matches_layer_pattern,
    // Helpers
    HelpersContext, register_helpers, register_drafts, notify_change,
    SubscriptionManager, ChangeSubscription, DraftsHelper,
    // Additional bindings
    PermitBindings, PeersBindings, EmojiBindings,
    UiBindings, UiSharedState, event_to_lua, parse_subscribe_options,
    PageBindings,
};

// Re-export UI types
pub use ui_types::{VecModelOp, UiMutation, PropertyUpdate, UiQuery};

// Error types

use thiserror::Error;

#[derive(Debug, Error)]
pub enum LuaRuntimeError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Lua error: {0}")]
    Lua(#[from] mlua::Error),

    #[error("Slint error: {0}")]
    Slint(String),

    #[error("Missing function: {0}")]
    MissingFunction(String),

    #[error("Butler error: {0}")]
    Butler(String),

    #[error("Missing file: {0}")]
    MissingFile(String),

    #[error("Validation error: {0}")]
    Validation(String),
}

// Constants

/// API Lua module (exported function registry)
pub const LUA_API_MODULE: &str = include_str!("lua_libs/api.lua");

/// Date Lua module
pub const LUA_DATE_MODULE: &str = include_str!("lua_libs/date.lua");

/// Presence Lua module
pub const LUA_PRESENCE_MODULE: &str = include_str!("lua_libs/presence.lua");

/// Reactive Binding module for surgical UI updates
pub const LUA_BINDING_MODULE: &str = include_str!("lua_libs/binding.lua");
