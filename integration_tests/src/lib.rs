//! Integration tests for Osvauld P2P protocol
//!
//! This crate provides test infrastructure for multi-peer protocol testing
//! without actual network infrastructure.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                      TestHarness                             │
//! │                                                              │
//! │  ┌─────────────┐      ┌─────────────┐      ┌─────────────┐  │
//! │  │  TestPeer   │      │  TestPeer   │      │  TestPeer   │  │
//! │  │  (Owner)    │      │  (Node)     │      │  (Viewer)   │  │
//! │  │             │      │             │      │             │  │
//! │  │ Coordinator │      │ Coordinator │      │ Coordinator │  │
//! │  │ PeerActors  │      │ PeerActors  │      │ PeerActors  │  │
//! │  │ Butler      │      │ Butler      │      │ Butler      │  │
//! │  └──────┬──────┘      └──────┬──────┘      └──────┬──────┘  │
//! │         │                    │                    │         │
//! │         └────────────────────┼────────────────────┘         │
//! │                              │                              │
//! │                    ┌─────────┴─────────┐                    │
//! │                    │   MockTransport   │                    │
//! │                    │   (routes bytes)  │                    │
//! │                    └───────────────────┘                    │
//! └─────────────────────────────────────────────────────────────┘
//! ```

mod mock_transport;
mod test_harness;
mod test_peer;

pub mod assertions;
pub mod fixtures;
pub mod helpers;
pub mod scenarios;

pub use mock_transport::MockTransport;
pub use test_harness::TestHarness;
pub use test_peer::TestPeer;

// Re-export commonly used scenario types
pub use scenarios::{OwnerNodeScenario, PublishedPageScenario, MultiPartyScenario};

#[cfg(test)]
mod tests;
