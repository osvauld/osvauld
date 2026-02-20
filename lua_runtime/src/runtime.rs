//! LuaRuntime implementation.

use std::sync::Arc;

use mlua::{Function, Lua, Table, Value};
use parking_lot::Mutex;
use serde_json::Value as JsonValue;
use tokio::sync::mpsc;
use tracing::{debug, info, trace, warn};

use crate::scribe_handle::ActorScribeHandle;
use butler::LoroDelta;

use crate::bindings::binding::{data_to_ui_mutation, process_binding_data, BindingManager};
use crate::bindings::clock::ClockBindings;
use crate::bindings::convert::{json_to_lua, lua_to_json};
use crate::bindings::derivation::DerivationBindings;
use crate::bindings::emoji::EmojiBindings;
use crate::bindings::layout::LayoutBindings;
use crate::bindings::page::PageBindings;
use crate::bindings::peers::PeersBindings;
use crate::bindings::permit::PermitBindings;
use crate::bindings::scribe::ScribeBindings;
use crate::bindings::ui::{UiBindings, UiSharedState};
use crate::commands::{DebugState, LuaCommand, ValidationContext, ValidationResult};
use crate::scheduler::Scheduler;
use crate::ui_types::{UiMutation, UiQuery};
use crate::{LUA_API_MODULE, LUA_BINDING_MODULE, LUA_DATE_MODULE, LUA_PRESENCE_MODULE};

mod buffered_ui;
mod debug;
mod engine;
mod handlers;
mod init_impl;
mod timer_bindings;
#[cfg(test)]
mod validation_tests;

use buffered_ui::BufferedUiBindings;
pub use buffered_ui::BufferedUiState;
use debug::collect_lua_globals;
use timer_bindings::register_timer_functions;

/// Configuration for creating a LuaRuntime.
pub struct LuaRuntimeConfig {
    pub page_id: String,
    pub app_name: String,
    pub scribe: Arc<ActorScribeHandle>,
    pub user_did: String,
    pub user_name: String,
    pub user_role: String,
    pub lua_code: String,
    pub ui_enabled: bool,
    pub ui_tx: Option<mpsc::Sender<UiMutation>>,
    pub query_tx: Option<mpsc::Sender<UiQuery>>,
    pub navigate_tx: Option<std::sync::mpsc::Sender<String>>,
    pub clock: Arc<dyn domains::ClockSource>,
}

/// Result of a single step of the event loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepResult {
    Processed,
    Idle,
    Shutdown,
}

/// Unified Lua runtime for both shell and node.
pub struct LuaRuntime {
    lua: Lua,
    page_id: String,
    scheduler: Arc<Mutex<Scheduler>>,
    cmd_rx: mpsc::Receiver<LuaCommand>,
    ui_tx: Option<mpsc::Sender<UiMutation>>,
    ui_shared: Arc<Mutex<UiSharedState>>,
    binding_manager: Arc<Mutex<BindingManager>>,
    scribe: Arc<ActorScribeHandle>,
    ui_enabled: bool,
    handler_cache: HandlerCache,
    clock: Arc<dyn domains::ClockSource>,
}

struct HandlerCache {
    on_layer_discovered: bool,
    on_ephemeral: bool,
    on_peer_joined: bool,
    on_peer_left: bool,
    on_asset_uploaded: bool,
    on_key_pressed: bool,
    on_text_input: bool,
    on_shutdown: bool,
}

impl HandlerCache {
    fn populate(lua: &Lua) -> Self {
        let has = |name: &str| -> bool { lua.globals().get::<Function>(name).is_ok() };
        Self {
            on_layer_discovered: has("on_layer_discovered"),
            on_ephemeral: has("on_ephemeral"),
            on_peer_joined: has("on_peer_joined"),
            on_peer_left: has("on_peer_left"),
            on_asset_uploaded: has("on_asset_uploaded"),
            on_key_pressed: has("on_key_pressed"),
            on_text_input: has("on_text_input"),
            on_shutdown: has("on_shutdown"),
        }
    }
}

impl LuaRuntime {
    /// Whether a layer is internal protocol metadata and should be hidden from app discovery replay.
    fn is_protocol_layer(layer_name: &str) -> bool {
        layer_name.starts_with("__sync_meta:")
    }

    /// Replay all existing non-protocol layers into `on_layer_discovered` after runtime init.
    ///
    /// **Context**: App restarts (including refresh-triggered restart) create a fresh Lua VM.
    /// Dynamic-layer UI state is often rebuilt from discovery callbacks.
    /// **We do**: list all local layers from Scribe and synthesize discovery callbacks.
    fn replay_existing_layer_discovery(&self) {
        let layers = match self.scribe.list_layers("*") {
            Ok(layers) => layers,
            Err(error) => {
                warn!(page_id = %self.page_id, error = %error, "Failed to list layers for startup discovery replay");
                return;
            }
        };

        let mut replayed = 0usize;
        for layer_name in layers {
            if Self::is_protocol_layer(&layer_name) {
                continue;
            }

            if let Err(error) = self.handle_layer_discovered(&layer_name, None) {
                warn!(
                    page_id = %self.page_id,
                    layer = %layer_name,
                    error = %error,
                    "Startup discovery replay failed for layer"
                );
                continue;
            }

            replayed += 1;
        }

        debug!(page_id = %self.page_id, layer_count = replayed, "Startup discovery replay complete");
    }

    /// Spawn Lua runtime on an OS thread.
    pub fn spawn(
        config: LuaRuntimeConfig,
    ) -> Result<(std::thread::JoinHandle<()>, mpsc::Sender<LuaCommand>), String> {
        let (cmd_tx, cmd_rx) = mpsc::channel(4096);

        let handle = std::thread::spawn(move || {
            let mut runtime = match Self::new_internal(config, cmd_rx) {
                Ok(r) => r,
                Err(e) => {
                    tracing::error!(error = %e, "Failed to create LuaRuntime");
                    return;
                }
            };

            runtime.run();
        });

        Ok((handle, cmd_tx))
    }

    /// Create a headless runtime on the current thread (for testing).
    pub fn new_headless(
        config: LuaRuntimeConfig,
    ) -> Result<(Self, mpsc::Sender<LuaCommand>), String> {
        let (cmd_tx, cmd_rx) = mpsc::channel(4096);
        let runtime = Self::new_internal(config, cmd_rx)?;
        Ok((runtime, cmd_tx))
    }

    /// Call on_init handler (separated from run() for test harness).
    pub fn call_on_init(&mut self) {
        if self.has_function("on_init") {
            if let Err(e) = self.call_handler("on_init", ()) {
                warn!(page_id = %self.page_id, error = %e, "on_init failed");
            } else {
                debug!(page_id = %self.page_id, "on_init completed");
            }
        }

        self.flush_mutations();
        self.handler_cache = HandlerCache::populate(&self.lua);

        // Rebuild app-visible layer discovery state from local CRDT inventory.
        self.replay_existing_layer_discovery();
        self.flush_mutations();
    }

    /// Process a single step of the event loop.
    pub fn step(&mut self) -> StepResult {
        self.fire_due_timers();

        match self.cmd_rx.try_recv() {
            Ok(cmd) => {
                let should_exit = self.handle_command(cmd);
                self.flush_mutations();
                if should_exit {
                    StepResult::Shutdown
                } else {
                    StepResult::Processed
                }
            }
            Err(tokio::sync::mpsc::error::TryRecvError::Empty) => StepResult::Idle,
            Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => StepResult::Shutdown,
        }
    }

    /// Normalize a layer name for binding lookup.
    ///
    /// **Context**: Bindings are registered with expanded pattern including `page_id/` prefix.
    /// Observer/LuaCommand LayerChanged delivers bare layer names. This helper ensures
    /// consistent lookup by prepending the page_id prefix if not already present.
    ///
    /// # Arguments
    /// * `layer_name` - The layer name to normalize (may be bare or already prefixed)
    ///
    /// # Returns
    /// Layer name with `page_id/` prefix for binding lookup
    pub(crate) fn normalize_layer_name_for_lookup(&self, layer_name: &str) -> String {
        let prefix = format!("{}/", self.page_id);
        if layer_name.starts_with(&prefix) {
            layer_name.to_string()
        } else {
            format!("{}{}", prefix, layer_name)
        }
    }
}
