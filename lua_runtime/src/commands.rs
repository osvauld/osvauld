//! Commands for the Lua runtime event loop
//!
//! These commands are sent from external sources (UI, Scribe, peers)
//! to the runtime's event loop for processing.

use serde_json::Value as JsonValue;
use tokio::sync::oneshot;

use butler::{DynamicLayerMeta, LoroDelta};

// Debug State (for introspection)

/// Debug state snapshot
#[derive(Debug, Clone)]
pub struct DebugState {
    pub globals: Vec<String>,
    pub timers: usize,
    pub has_on_init: bool,
}

// Lua Commands

/// Commands sent to the Lua runtime event loop
///
/// **Sender**: Main thread (UI), Scribe observer, peers, timers
/// **Receiver**: LuaRuntime event loop
#[derive(Debug)]
pub enum LuaCommand {
    /// Layer changed or discovered
    ///
    /// **Context**: Scribe emitted a page layer update
    /// **created=true**: call `on_layer_discovered(layer_name)`
    /// **created=false**: process binding updates + derivation trigger
    LayerChanged {
        layer_name: String,
        created: bool,
        delta: Option<LoroDelta>,
        full_data: Option<JsonValue>,
        dynamic_ref: Option<DynamicLayerMeta>,
    },

    /// UI callback triggered (button click, etc.)
    ///
    /// **Context**: Slint callback fired
    /// **We do**: Call the registered Lua handler
    UiCallback {
        callback_name: String,
        args: Vec<JsonValue>,
    },

    /// Generic UI event
    ///
    /// **Context**: Slint event (key press, focus, etc.)
    /// **We do**: Call Lua's event handler if defined
    UiEvent { event: UiEventType },

    /// Shutdown the runtime
    ///
    /// **Context**: App closing, page unload
    /// **We do**: Call Lua's `on_shutdown()` if defined, then exit loop
    Shutdown,

    /// Timer fired - call the registered callback
    ///
    /// **Context**: Scheduler detected a timer is due
    /// **We do**: Look up callback in `_G._timers[id]` and call it
    TimerFired { timer_id: u64 },

    /// Raw ephemeral data from peer
    ///
    /// **Context**: Peer sent ephemeral data via datagram
    /// **We do**: Call Lua's `on_ephemeral(user_did, payload)` if defined
    Ephemeral { user_did: String, payload: Vec<u8> },

    /// Structured ephemeral message (typed RPC-style)
    ///
    /// **Context**: Peer sent structured ephemeral via scribe:send(func, args)
    /// **We do**: Call Lua's `on_ephemeral(from_did, func, args)` if defined
    StructuredEphemeral {
        from_did: String,
        func: String,
        args: JsonValue,
    },

    /// Peer joined (subscribed to page)
    ///
    /// **Context**: Remote peer subscribed to this page's Scribe
    /// **We do**: Call Lua's `on_peer_joined(user_did)` if defined
    PeerJoined { user_did: String },

    /// Peer left (unsubscribed from page)
    ///
    /// **Context**: Remote peer unsubscribed or disconnected
    /// **We do**: Call Lua's `on_peer_left(user_did)` if defined
    PeerLeft { user_did: String },

    /// Asset upload completed
    ///
    /// **Context**: User uploaded a file
    /// **We do**: Call Lua's `on_asset_uploaded(hash, filename, mime_type, size)`
    AssetUploaded {
        hash: String,
        filename: String,
        mime_type: String,
        size: u64,
    },

    /// Validate operations against app's validation logic
    ///
    /// **Context**: ValidationService sends ops for validation
    /// **We do**: Call Lua's `validate_ops(layer, ops, from_did, role, page_id)`
    /// **Returns**: ValidationResult via oneshot
    Validate {
        ctx: ValidationContext,
        response_tx: oneshot::Sender<Result<ValidationResult, String>>,
    },

    /// Trigger derivation rebuild
    ///
    /// **Context**: After loading init.lua with derivation rules
    /// **We do**: Call `derivation:rebuild_all()` on the derivation userdata
    RebuildDerivation,

    /// Debug: Execute Lua code and return result
    ///
    /// **Context**: Debug server introspection
    /// **Returns**: JSON-serialized result or error
    DebugEval {
        code: String,
        response_tx: oneshot::Sender<Result<JsonValue, String>>,
    },

    /// Debug: Get current state snapshot
    ///
    /// **Context**: Debug server state inspection
    /// **Returns**: DebugState with globals, timer count, etc.
    DebugGetState {
        response_tx: oneshot::Sender<DebugState>,
    },

    /// Test: Set runtime time (ManualClock only)
    ///
    /// **Context**: Test automation needs deterministic time control
    /// **Returns**: New unix timestamp or error if not in test mode
    SetTime {
        unix_seconds: i64,
        response_tx: oneshot::Sender<Result<i64, String>>,
    },

    /// Test: Advance runtime time (ManualClock only)
    ///
    /// **Context**: Test automation needs deterministic time control
    /// **Returns**: New unix timestamp or error if not in test mode
    AdvanceTime {
        seconds: u64,
        response_tx: oneshot::Sender<Result<i64, String>>,
    },
}

// UI Event Types

/// UI event types (for UiEvent command)
#[derive(Debug, Clone)]
pub enum UiEventType {
    /// Key pressed
    KeyPressed { key: String },

    /// Text input changed
    TextChanged { element: String, text: String },
}

// Validation Types

/// Context for validating operations
#[derive(Debug, Clone)]
pub struct ValidationContext {
    /// Layer being modified
    pub layer_name: String,

    /// Operations to validate (uses butler::JsonOp from domains for consistency)
    pub ops: Vec<butler::JsonOp>,

    /// Who's doing the operation
    pub from_did: String,

    /// Their role (owner, viewer, collaborator, etc.)
    pub role: String,

    /// Page context
    pub page_id: String,
}

/// Result of validation
#[derive(Debug, Clone)]
pub struct ValidationResult {
    /// Whether the operations are valid
    pub valid: bool,

    /// Error message if invalid
    pub error: Option<String>,

    /// Which specific op failed (for partial rejection)
    pub failed_op_index: Option<usize>,
}

impl Default for ValidationResult {
    fn default() -> Self {
        Self {
            valid: true,
            error: None,
            failed_op_index: None,
        }
    }
}
