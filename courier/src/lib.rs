#![recursion_limit = "256"]
//! Courier - P2P Protocol Layer
//!
//! This crate owns all protocol message types and handles peer connections.
//!
//! # Architecture (v3 - Simplified Ephemeral)
//!
//! ```text
//! Transport (dumb primitives)
//!     │
//!     │ TransportEvent::Connected { node_id, conn }
//!     ▼
//! ┌─────────────────────────────────────┐
//! │         Coordinator                  │
//! │   - Receives lifecycle events       │
//! │   - Spawns PeerActor per peer       │
//! │   - Provides broadcast APIs         │
//! └─────────────────────────────────────┘
//!     │
//!     │ Spawns with ConnectionHandle
//!     ▼
//! ┌──────────┐    ┌──────────┐    ┌──────────┐
//! │ PeerActor│   │ PeerActor│   │ PeerActor│  (1 per peer)
//! │           │   │           │   │           │
//! │ Owns conn │   │ Owns conn │   │ Owns conn │
//! │ Stream RX │   │ Stream RX │   │ Stream RX │
//! │ Datagram  │   │ Datagram  │   │ Datagram  │
//! │   RX loop │   │   RX loop │   │   RX loop │
//! └──────────┘    └──────────┘    └──────────┘
//!     │                │                │
//!     └────────────────┴────────────────┘
//!                      │
//!                      ▼ Route by page_id via page_subscriptions
//!                   Scribe
//! ```
//!
//! **Benefits:**
//! - PeerActor owns everything (connection, read loops, Scribe connections)
//! - Protocol layer is dumb - just routes opaque bytes by page_id
//! - Ephemeral data goes directly to Scribe via page_subscriptions

mod handle;
mod message;
mod state;

pub mod coordinator;
pub mod handshake;
pub mod peer_actor;
pub mod trace;

pub use coordinator::{ConnectRequest, Coordinator, CoordinatorMessage, CourierMode};
pub use handle::{
    Courier, CourierEvent, CourierHandle, HandshakeServices, IrohCoordinator,
    IrohCoordinatorMessage,
};
pub use message::{
    AssetAckMsg, AssetPrepareMsg, AssetReadyMsg, ConnectionString, EphemeralDatagram, ErrorCode,
    ErrorMsg, GetShareableLinkRequestMsg, GetShareableLinkResponseMsg, HelloMsg, LayerType,
    Message, PageAnnounceAckMsg, PageAnnounceMsg, PermitGrantMsg, PermitScope, PermitUpdateMsg,
    PublishErrorMsg, PublishSpaceAckMsg, PublishSpaceMsg, PublishedPageMeta, PublishedSpace,
    RejectedMsg, SpaceDataAckMsg, SpaceDataMsg, SpaceRequestErrorMsg, SpaceRequestMsg,
    SyncAcceptMsg, SyncAckMsg, SyncConsentAckMsg, SyncConsentGrantMsg, SyncOfferMsg, SyncResetMsg,
    SyncSnapshotMsg, WelcomeMsg, PROTOCOL_VERSION,
};
pub use state::PeerState;
pub use trace::{MessageTrace, TraceDirection};

// Re-export transport types for P2P initialization
pub use transport::{Transport, TransportConfig};
