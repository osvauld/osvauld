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

mod message;
mod state;
mod handle;

pub mod coordinator;
pub mod peer_actor;
pub mod handshake;
pub mod trace;

pub use message::{
    Message, ErrorCode, PublishedSpace, PublishedPageMeta, ConnectionString, EphemeralDatagram,
    LayerType, PermitScope, PROTOCOL_VERSION,
    HelloMsg, WelcomeMsg, PermitGrantMsg, RejectedMsg,
    SyncOfferMsg, SyncAcceptMsg, SyncAckMsg, SyncResetMsg, SyncSnapshotMsg,
    PublishSpaceMsg, PublishSpaceAckMsg, PageAnnounceMsg, PageAnnounceAckMsg,
    PermitUpdateMsg, PublishErrorMsg,
    SpaceRequestMsg, SpaceDataMsg, SpaceDataAckMsg, SpaceRequestErrorMsg,
    GetShareableLinkRequestMsg, GetShareableLinkResponseMsg,
    AssetPrepareMsg, AssetReadyMsg, AssetAckMsg,
    SyncConsentGrantMsg, SyncConsentAckMsg,
    LayerPermitMsg, LayerConsentGrantMsg, LayerConsentAckMsg,
    ErrorMsg,
};
pub use state::PeerState;
pub use coordinator::{Coordinator, CoordinatorMessage, CourierMode, ConnectRequest};
pub use handle::{CourierHandle, CourierEvent, Courier, HandshakeServices, IrohCoordinator, IrohCoordinatorMessage};
pub use peer_actor::{OutboundUpdate, OutboundUpdateTx};
pub use trace::{MessageTrace, TraceDirection};

// Re-export transport types for P2P initialization
pub use transport::{Transport, TransportConfig};
