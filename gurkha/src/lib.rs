//! Gurkha - Pure UCAN Authorization Domain
//!
//! Standalone UCAN business logic with zero infrastructure dependencies.
//!
//! ## Design Principles
//!
//! - **Pure domain logic** - No database, no crypto utilities, no services
//! - **Raw key input** - Receives Ed25519 SigningKey/VerifyingKey directly
//! - **OCaml-ready** - Can be replaced with OCaml implementation
//! - **Type-safe** - 11 typed token wrappers prevent misuse
//!
//! ## Module Structure (V3: Facts-Only)
//!
//! - `types` - Domain types (Capability, DocType, SyncFacts)
//! - `token` - Typed token wrappers
//! - `parser` - Permit, DelegationTemplate, UcanCore (facts-based parsing)
//! - `decision` - Decision/inference logic (pure facts-based logic)
//! - `crypto` - Crypto operations (signing, verification)
//! - `verification` - Proof chain validation (stateless, self-contained)
//! - `builder` - Execution layer (builds tokens from decisions)
//! - `merge` - Pure CRDT merge logic (Loro document operations)
//! - `sync` - Pure sync logic (permit-driven document synchronization)
//! - `service` - Public API (orchestrates decision + crypto)
//! - `cel` - CEL rule evaluation for authorization
//! - `errors` - GurkhaError types

pub mod types;
pub mod token;
pub mod parser;
pub mod decision;
pub mod crypto;
pub mod verification;
pub mod builder;
pub mod merge;
pub mod sync;
pub mod service;
pub mod errors;
pub mod cel;

// Re-export commonly used items
pub use types::*;
pub use token::{UcanTokenError, UcanTokenResult};
pub use parser::{Permit, DelegationTemplate, UcanCore};
pub use decision::{TokenDecision, DelegationDecision, SyncContext, should_send_updates, can_receive_updates};
pub use verification::{ProofCache, ProofChainTracer};
pub use builder::GurkhaUcanBuilder;
pub use merge::MergeService;
pub use sync::{SyncRequestData, SyncResponseData, prepare_sync_request, generate_sync_response, apply_peer_docs, apply_peer_updates, generate_collaborative_updates};
pub use service::UcanService;
pub use cel::{OperationValidator, CelError, CelResult};

// Factory function
pub fn create_ucan_service(signing_key: ed25519_dalek::SigningKey, verifying_key: ed25519_dalek::VerifyingKey) -> std::sync::Arc<std::sync::RwLock<UcanService>> {
    let mut service = UcanService::new();
    service.load_keys(signing_key, verifying_key);
    std::sync::Arc::new(std::sync::RwLock::new(service))
}
