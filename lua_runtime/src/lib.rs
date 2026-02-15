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
//!     scribe,
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
mod bindings;

// Timer and coroutine scheduler
mod scheduler;

// UI types (VecModelOp, UiMutation, PropertyUpdate, UiQuery)
mod ui_types;

// ScribeHandle trait + ActorScribeHandle (production impl)
mod scribe_handle;

// MockScribeHandle (in-memory impl for testing)
mod mock_scribe;

mod commands;
mod runtime;

pub use commands::{LuaCommand, UiEventType, ValidationContext, ValidationResult};
pub use mock_scribe::{MockScribeHandle, MockScribeState};
pub use runtime::{BufferedUiState, LuaRuntime, LuaRuntimeConfig, StepResult};
pub use scribe_handle::{ActorScribeHandle, ScribeHandle};

// Re-export binding types (public API only)
pub use bindings::{json_to_lua, ScribeBindings};

// Re-export UI types
pub use ui_types::{PropertyUpdate, UiMutation, UiQuery, VecModelOp};

// Constants

/// API Lua module (exported function registry)
pub(crate) const LUA_API_MODULE: &str = include_str!("lua_libs/api.lua");

/// Date Lua module
pub(crate) const LUA_DATE_MODULE: &str = include_str!("lua_libs/date.lua");

/// Presence Lua module
pub(crate) const LUA_PRESENCE_MODULE: &str = include_str!("lua_libs/presence.lua");

/// Reactive Binding module for surgical UI updates
pub(crate) const LUA_BINDING_MODULE: &str = include_str!("lua_libs/binding.lua");
