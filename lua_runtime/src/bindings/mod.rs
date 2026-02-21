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
//! - `page` - In-page app navigation helpers

// Core bindings
pub(crate) mod binding;
pub(crate) mod clock;
pub(crate) mod convert;
pub(crate) mod derivation;
pub(crate) mod emoji;
pub(crate) mod layout;
pub(crate) mod loro;
pub(crate) mod page;
pub(crate) mod peers;
pub(crate) mod permit;
pub(crate) mod scribe;
pub(crate) mod ui;

// Unified Scribe binding (the single API for apps)
pub use scribe::ScribeBindings;

// Public exports
pub use convert::{json_to_lua, lua_to_json_err};
