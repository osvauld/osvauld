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
pub mod storage;
pub mod validation_handle;

// Actor modules
pub mod actor;
pub mod ephemeral;
pub mod layer_unit;
pub mod loro_observer;
pub mod operations;
pub mod permit;
pub mod query;
pub mod state;
pub mod sync;

// Test infrastructure (only compiled in test mode)
#[cfg(test)]
pub mod test_fixtures;
#[cfg(test)]
pub mod test_strategies;

// Re-export common types
pub use message::{
    BroadcastPayload, EphemeralBroadcast, EphemeralOutbound, ListOp, LoroDelta, PageUpdate,
    PageUpdateTx, ScribeMessage, SyncEvent,
};

// JsonOp comes from domains (Layer::extract_ops_from_bytes)
pub use domains::JsonOp;
pub use validation_handle::{ValidationHandle, ValidationRequest};

pub use error::{Result, ScribeError};

// Storage traits
pub use storage::{
    LayerStorage, LayerStorageRef, NullLayerStorage, NullPeerResolver, NullPeerVectorStorage,
    NullPermitIssuer, PeerResolver, PeerResolverRef, PeerVectorStorage, PeerVectorStorageRef,
    PermitIssuer, PermitIssuerRef,
};

// Actor and state types
pub use actor::Scribe;
pub use layer_unit::{LayerConfig, LayerSubscriber, LayerUnit};
pub use permit::{glob_match, Permissions};
pub use state::{PeerConnection, ScribeArgs, ScribeState, SyncConfig, SyncMode};
