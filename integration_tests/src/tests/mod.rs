//! Integration tests
//!
//! Protocol and app tests using the Peer/Scenario infrastructure.
//!
//! ## Test Categories
//!
//! ### Real Network Tests (IrohConnection)
//!
//! - `handshake` - Protocol handshake flows (owner, viewer, reconnection)
//!
//! ### Protocol Tests (MockConnection) - Fast, Deterministic
//!
//! - `protocol_handshake` - Handshake protocol flows
//! - `protocol_sync` - Sync protocol flows (SyncOffer/Accept/Ack)
//! - `protocol_permission` - Permission enforcement
//! - `protocol_subscription` - Subscription behavior
//!
//! ### DST Tests (Deterministic Simulation Testing) - CRDT Level
//!
//! - `dst_sync` - Strong eventual consistency, no lost writes
//! - `dst_permission` - Unauthorized write rejection
//! - `dst_partition` - Convergence after network partition
//! - `dst_reconnection` - Clean state management on reconnect
//! - `dst_bounded_retry` - Infinite loop prevention
//! - `dst_sync_reset` - Permit-aware snapshot recovery

// Real Network Tests (IrohConnection)

mod handshake;

// Protocol Tests (MockConnection) - Fast, Deterministic

mod protocol_handshake;
mod protocol_sync;
mod protocol_permission;
mod protocol_subscription;
mod protocol_validation;

// Presence Tests (Lua + Scribe integration)

mod presence_heartbeat;

// DST Tests (CRDT Level)
// Note: DST tests are temporarily disabled due to incomplete Layer API
// They test CRDT-level invariants directly using domains::Layer

// mod dst_sync;
// mod dst_permission;
// mod dst_partition;
// mod dst_reconnection;
// mod dst_bounded_retry;
// mod dst_sync_reset;
