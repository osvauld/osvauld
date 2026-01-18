//! App Runtime - Loads and executes Slint + Lua app bundles
//!
//! This crate provides the runtime for user-defined apps in Sthalam.
//! Each app is a bundle containing:
//! - app.slint: UI definition (declarative, safe)
//! - app.lua: Logic (sandboxed via mlua)
//! - manifest.json: Metadata
//!
//! ## Architecture (Parallel Stateless Lua)
//!
//! - **Slint Thread (main)**: Owns ComponentInstance, VecModels (Rc-based)
//! - **Lua Worker Threads (OS threads)**: One per app, owns Lua VM, stateless computation
//! - **Communication**: mpsc channels with serializable VecModelOp messages
//! - **Lua Pattern**: Query Loro → Compute → Return operations (no permanent state)
//! - **UI State**: Persisted in Loro CRDT layers (survives restarts, syncs)

// New parallel architecture modules
mod vecmodel_ops;
mod lua_worker;
mod slint_runtime;
mod slint_model_bindings;
mod page_runtime;
mod event_bus;

// Existing modules
mod scribe_channel;
mod butler_bindings;

// New architecture exports
pub use vecmodel_ops::{VecModelOp, UiMutation, PropertyUpdate, UiQuery};
pub use lua_worker::{LuaWorker, LuaWorkerCommand};
pub use slint_runtime::SlintRuntime;
pub use slint_model_bindings::LuaSlintModel;
pub use page_runtime::{generate_page_shell, write_shell_slint, AppTab};
pub use event_bus::{EventBus, Event, EventSource, EventDelivery, SubscribeOptions, Recording};

// Re-export shared bindings from butler
pub use butler::{
    LoroBindings, PermitBindings, LuaLoroList, LuaLoroMap,
    // Loro <-> Lua
    loro_value_to_lua, lua_to_loro_value,
    // JSON <-> Lua
    json_to_lua, lua_to_json,
    // JSON <-> Loro
    json_to_loro_value, loro_value_to_json,
    // Pattern matching
    matches_layer_pattern,
};

// Existing exports
pub use scribe_channel::{ScribeChannel, ScribeCommand, ScribeEvent};
pub use butler_bindings::ButlerBindings;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
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
}

/// App manifest (manifest.json)
#[derive(Debug, Clone, serde::Deserialize)]
pub struct Manifest {
    pub name: String,
    pub version: String,
    pub entry_ui: String,
    pub entry_logic: String,
    /// Array properties that need VecModel tracking for incremental updates
    /// Example: ["products", "orders"]
    #[serde(default)]
    pub models: Vec<String>,
}
