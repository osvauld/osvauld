//! Integration tests for Osvauld P2P protocol
//!
//! Event-driven test infrastructure using real iroh transport.

pub mod event_tests;

// Re-export for external use
pub use event_tests::Peer;
