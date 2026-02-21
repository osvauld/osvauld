//! Protocol integration tests for Osvauld P2P
//!
//! MockConnection-only test infrastructure with builder pattern.
//!
//! ## Quick Start
//!
//! ```ignore
//! // Handshake test with tracer
//! let mut s = Scenario::builder()
//!     .owner().node().connected().with_tracer()
//!     .build().await?;
//! s.tracer().assert_contains_sequence(&["Hello", "Welcome"]);
//!
//! // Full publish flow
//! let mut s = Scenario::builder()
//!     .app("osvauld-demos").published().with_tracer()
//!     .build().await?;
//! assert!(s.node().butler.pages().get(&s.space().page_id)?.is_some());
//! ```

pub mod fixtures;
pub mod peer;
pub mod scenario;
pub mod tracer;

#[cfg(test)]
mod tests;

pub use fixtures::{
    app_dir, init_tracing, workspace_root,
    SpaceInfo, MOCK_TIMEOUT, PAGE_SYNC_TIMEOUT,
};
pub use peer::Peer;
pub use scenario::{Scenario, ScenarioBuilder};
pub use tracer::Tracer;
