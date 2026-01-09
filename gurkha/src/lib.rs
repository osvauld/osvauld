//! Gurkha - Pure Permit Authorization Domain
//!
//! Standalone permit business logic with zero infrastructure dependencies.
//! Permits are extended UCAN tokens with facts-based authorization.
//!
//! ## Design Principles
//!
//! - **Pure domain logic** - No database, no crypto utilities, no services
//! - **Stateless functions** - Receives key bytes, returns results
//! - **OCaml-ready** - Can be replaced with OCaml implementation
//! - **Facts-driven** - All authorization in facts field, no typed wrappers
//!
//! ## Module Structure (V3: Facts-Only)
//!
//! - `types` - Domain types (Capability, DocType, SyncDecision, SyncFacts)
//! - `parser` - Permit, DelegationTemplate, PermitCore (facts-based parsing)
//! - `decision` - Decision/inference logic (pure facts-based logic)
//! - `crypto` - Crypto operations (signing, verification)
//! - `verification` - Proof chain validation (stateless, self-contained)
//! - `builder` - Execution layer (builds tokens from decisions)
//! - `service` - Stateless permit functions (takes key bytes, returns permits)
//! - `errors` - GurkhaError types
//!
//! Note: handshake module moved to courier
//! Note: merge and sync modules moved to butler

pub mod types;
pub mod parser;
pub mod decision;
pub mod crypto;
pub mod verification;
pub mod builder;
pub mod service;
pub mod errors;
// Re-export commonly used items
pub use types::*;
pub use parser::{Permit, DelegationTemplate, PermitCore, PermitError, PermitResult, LayerPatternConfig};
pub use decision::{TokenDecision, DelegationDecision, SyncContext, should_send_updates, can_receive_updates, can_access_layer};
pub use verification::{ProofCache, ProofChainTracer};
pub use builder::GurkhaPermitBuilder;

// Re-export stateless permit functions
pub use service::{
    // Connection tokens
    issue_one_time,
    issue_peer_connection,
    issue_page_viewer_auth,
    issue_space_viewer_auth,
    // Page tokens
    delegate_page,
    issue_page_owner_token,
    // Space tokens
    issue_space_owner_token,
    issue_space_node_to_owner,
    delegate_space,
    // Sync consent tokens (viewer-issued)
    issue_sync_space_consent,
    issue_sync_page_consent,
    // Utilities
    get_public_key,
    extract_space_id,
    extract_page_id,
    extract_capabilities,
    validate_permit_structure,
};

