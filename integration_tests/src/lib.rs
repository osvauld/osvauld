//! Integration tests for Osvauld P2P protocol
//!
//! Event-driven test infrastructure supporting both real network (Iroh) and
//! mock transport (MockConnection) for fast protocol testing.
//!
//! ## Core Types
//!
//! - [`Peer<C>`] - Single participant in P2P network, generic over Connection
//! - [`Scenario<C>`] - Multi-peer orchestration, generic over Connection
//! - [`MessageTracer`] - Protocol message recording for test assertions
//!
//! ## Type Aliases
//!
//! - [`IrohPeer`] / [`IrohScenario`] - Real network (slower, requires relay)
//! - [`MockPeer`] / [`MockScenario`] - In-memory (fast, deterministic)
//!
//! ## Example
//!
//! ```ignore
//! // Real network test
//! let mut s = IrohScenario::new(true, true, 0).await?;
//! s.wait_ready().await;
//! s.connect_owner_to_node().await?;
//!
//! // Mock protocol test (fast)
//! let mut s = MockScenario::new_mock(true, true, 0).await?;
//! s.connect_owner_to_node_mock().await?;
//!
//! // With message tracing
//! let tracer = MessageTracer::new();
//! let mut s = MockScenario::with_tracer_mock(true, true, 0, tracer.clone()).await?;
//! // ... run test ...
//! tracer.assert_sequence(&["Hello", "Welcome", "PermitGrant", "Ack"]);
//! ```

pub mod peer;
pub mod scenario;
pub mod tracing;

#[cfg(test)]
mod tests;

// Re-exports from peer module

pub use peer::{
    Peer,
    IrohPeer,
    MockPeer,
    IrohCoordinatorMessage,
    MockCoordinatorMessage,
    PeerFixture,
};

// Re-exports from scenario module

pub use scenario::{
    Scenario,
    IrohScenario,
    MockScenario,
    SpaceInfo,
    init_tracing,
};

// Templates and constants
pub use scenario::{
    SPACE_TEMPLATE,
    PAGE_TEMPLATE,
    PAGE_LAYERS,
    CHAT_PAGE_TEMPLATE,
    CHAT_PAGE_LAYERS,
    RELAY_READY_DELAY,
    HANDSHAKE_TIMEOUT,
    MOCK_TIMEOUT,
    PAGE_SYNC_TIMEOUT,
};

// Re-exports from tracing module

pub use tracing::{
    MessageTracer,
    TracedMessage,
    TracedEphemeral,
    Direction,
};
