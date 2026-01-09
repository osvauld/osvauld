//! Courier - Actor-based P2P Protocol Layer
//!
//! This crate owns all protocol message types and handles serialization.
//!
//! # Architecture
//!
//! ```text
//! Transport (dumb byte pipe)
//!     │
//!     ▼
//! ┌─────────────────────────────────────┐
//! │         Coordinator                  │
//! │   - Deserializes bytes → Message    │
//! │   - Routes to PeerActors            │
//! │   - Supervises child actors         │
//! └─────────────────────────────────────┘
//!     │
//!     ├──────────────────┬──────────────────┐
//!     ▼                  ▼                  ▼
//! ┌──────────┐    ┌──────────┐    ┌──────────┐
//! │PeerActor │    │PeerActor │    │PeerActor │  (1 per connection)
//! │          │    │          │    │          │
//! │ State:   │    │ State:   │    │ State:   │
//! │ Machine  │    │ Machine  │    │ Machine  │
//! └──────────┘    └──────────┘    └──────────┘
//! ```

mod message;
mod state;
mod handle;

pub mod coordinator;
pub mod peer_actor;
pub mod handshake;

pub use message::{Message, ErrorCode, PublishedSpace, PublishedPageMeta, ConnectionString};
pub use state::PeerState;
pub use coordinator::{Coordinator, CoordinatorMessage, CourierMode};
pub use handle::{CourierHandle, CourierEvent, Courier, HandshakeServices};

// Re-export transport types for P2P initialization
pub use transport::{Transport, TransportConfig};
