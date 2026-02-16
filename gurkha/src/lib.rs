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
//! - `crypto` - UCAN token signing (Ed25519 + CID)
//! - `service` - Stateless permit functions (takes key bytes, returns permits)
//! - `errors` - GurkhaError types

pub mod builder;
pub mod crypto;
pub mod decision;
pub mod errors;
pub mod parser;
pub mod service;
pub mod types;

#[cfg(any(test, feature = "test-support"))]
pub mod test_strategies;

// Test fixtures are available for tests and when test-support feature is enabled
#[cfg(any(test, feature = "test-support"))]
pub mod test_fixtures;

// Re-export commonly used items
pub use decision::{
    can_access_layer, can_access_with_layer_permits, can_receive_updates, extract_issue_template,
    matches_dynamic_schema, should_send_updates, DelegationDecision, SyncContext, TokenDecision,
};
pub use parser::{
    expand_pattern, matches_schema_pattern, matches_wildcard, resolve_page_id_in_facts,
}; // Pattern/resolution utilities
pub use parser::{
    DelegationTemplate, LayerConfig, LayerPatternConfig, PeerCapabilities, Permit, PermitError,
    PermitResult,
};
pub use parser::{DynamicLayerSchema, GrantType}; // Dynamic layer types
pub use types::*;

// Re-export stateless permit functions
pub use service::{
    // Page tokens
    delegate_page,
    delegate_space,
    extract_space_id,
    // Utilities
    get_public_key,
    issue_layer_authority_permit,
    // Dynamic layer permits (node-issued)
    issue_layer_permit,
    // Connection tokens
    issue_one_time,
    issue_page_owner_token,
    issue_page_viewer_auth,
    issue_peer_connection,
    issue_space_node_to_owner,
    // Space tokens
    issue_space_owner_token,
    issue_space_viewer_auth,
    issue_sync_layer_consent,
    issue_sync_page_consent,
    // Sync consent tokens (viewer-issued)
    issue_sync_space_consent,
    // Dynamic layer re-issuance
    reissue_permit_with_layers,
};
