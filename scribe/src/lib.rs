//! Scribe - CRDT document sync actor with Loro
//!
//! This crate provides the complete Scribe actor for CRDT document synchronization:
//!
//! ## Core Components
//!
//! - **Scribe actor**: Message-driven actor (via ractor) that owns LoroDoc layers
//! - **ScribeMessage**: All messages the actor can handle
//! - **ScribeState**: Actor state including layers, subscribers, storage
//! - **Storage traits**: LayerStorage, PeerVectorStorage, PeerResolver
//!
//! ## Shared Types
//!
//! - Message types (BroadcastPayload, PageUpdate, LoroDelta, etc.)
//! - ValidationHandle for delegating validation to kunki
//! - JsonOp (re-exported from domains for ops extraction)
//!
//! ## Design
//!
//! Scribe is storage-agnostic via trait injection:
//! - Butler implements LayerStorage, PeerVectorStorage, PeerResolver
//! - Scribe only knows about the abstract interface
//! - This allows testing with mock implementations

// Core modules
pub mod error;
pub mod message;
pub mod validation_handle;
pub mod storage;

// Actor modules
pub mod actor;
pub mod state;
pub mod permit;
pub mod query;
pub mod operations;
pub mod loro_observer;
pub mod sync;
pub mod ephemeral;
pub mod layer_unit;

// Test infrastructure (only compiled in test mode)
#[cfg(test)]
pub mod test_fixtures;
#[cfg(test)]
pub mod test_strategies;

// Re-export common types
pub use message::{
    BroadcastPayload, EphemeralBroadcast, EphemeralOutbound,
    LoroDelta, ListOp, TextOp,
    PageUpdate, PageUpdateTx, SyncEvent,
    ScribeMessage,
};

// JsonOp comes from domains (Layer::extract_ops_from_bytes)
pub use domains::JsonOp;
pub use validation_handle::{ValidationHandle, ValidationRequest};


pub use error::{ScribeError, Result};

// Storage traits
pub use storage::{
    LayerStorage, PeerVectorStorage, PeerResolver, PermitIssuer,
    LayerStorageRef, PeerVectorStorageRef, PeerResolverRef, PermitIssuerRef,
    NullLayerStorage, NullPeerVectorStorage, NullPeerResolver, NullPermitIssuer,
};

// Actor and state types
pub use actor::Scribe;
pub use state::{
    ScribeState, ScribeArgs, SyncConfig, SyncMode,
    SubscriberInfo, QuerySubscriberInfo,
};
pub use layer_unit::{LayerUnit, Capabilities, LayerConfig, LayerSubscriber};
pub use permit::{PermitContext, Permissions, glob_match};
