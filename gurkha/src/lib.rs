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
//! - `types` - Domain types (Capability, DocType, SyncFacts)
//! - `token` - Token error types
//! - `parser` - Permit, DelegationTemplate, PermitCore (facts-based parsing)
//! - `decision` - Decision/inference logic (pure facts-based logic)
//! - `crypto` - Crypto operations (signing, verification)
//! - `verification` - Proof chain validation (stateless, self-contained)
//! - `builder` - Execution layer (builds tokens from decisions)
//! - `merge` - Pure CRDT merge logic (Loro document operations)
//! - `sync` - Pure sync logic (permit-driven document synchronization)
//! - `service` - Stateless permit functions (takes key bytes, returns permits)
//! - `errors` - GurkhaError types

pub mod types;
pub mod token;
pub mod parser;
pub mod decision;
pub mod handshake;
pub mod crypto;
pub mod verification;
pub mod builder;
pub mod merge;
pub mod sync;
pub mod service;
pub mod errors;

// Re-export commonly used items
pub use types::*;
pub use token::{PermitTokenError, PermitTokenResult};
pub use parser::{Permit, DelegationTemplate, PermitCore, PermitError, PermitResult};
pub use decision::{TokenDecision, DelegationDecision, SyncContext, should_send_updates, can_receive_updates};
pub use handshake::{
    HandshakeRole, HelloDecision, WelcomeDecision, PermitGrantDecision,
    HelloContext, WelcomeContext, PermitGrantContext,
    decide_hello_response, decide_welcome_response, decide_permit_grant_response,
    permit_type_for_role,
};
pub use verification::{ProofCache, ProofChainTracer};
pub use builder::GurkhaPermitBuilder;
pub use merge::MergeService;
pub use sync::{SyncRequestData, SyncResponseData, prepare_sync_request, generate_sync_response, apply_peer_docs, apply_peer_updates, generate_collaborative_updates};

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

