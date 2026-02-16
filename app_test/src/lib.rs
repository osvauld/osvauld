//! App Test — Lightweight test runner for Osvauld Lua apps
//!
//! Test app logic without the full stack (no Loro, no actors, no network).
//!
//! ## Quick Start
//!
//! ```ignore
//! use app_test::AppTestRunner;
//! use std::path::Path;
//!
//! let mut runner = AppTestRunner::load(
//!     Path::new("sample_apps/osvauld-demos/group-chat"),
//!     "viewer",
//!     "did:key:test_user",
//!     "Alice",
//! ).unwrap();
//!
//! runner.call_on_init();
//!
//! // Fire callback
//! runner.fire_callback("send_message", vec![json!("hello")]).unwrap();
//! runner.tick();
//!
//! // Check layer
//! let messages = runner.layer_data("test-page/messages").unwrap();
//! ```
//!
//! ## Multi-Peer Testing
//!
//! ```ignore
//! use app_test::{MultiPeerRunner, PeerRole};
//!
//! let mut multi = MultiPeerRunner::from_code(
//!     &lua_code,
//!     "chat",
//!     "test-page",
//!     &[
//!         PeerRole::new("did:key:alice", "Alice", "owner"),
//!         PeerRole::new("did:key:bob", "Bob", "viewer"),
//!     ],
//! ).unwrap();
//!
//! multi.init_all();
//! multi.peer(0).fire_callback("send_message", vec![json!("hello from alice")]).unwrap();
//! multi.tick_all();
//! ```

pub mod runner;
pub mod multi_peer;

pub use runner::AppTestRunner;
pub use multi_peer::{MultiPeerRunner, PeerRole};

// Re-export commonly used types from lua_runtime
pub use lua_runtime::{
    LuaCommand, StepResult, MockScribeHandle, MockScribeState,
};
