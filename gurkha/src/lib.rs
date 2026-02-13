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

pub mod types;
pub mod parser;
pub mod decision;
pub mod crypto;
pub mod builder;
pub mod service;
pub mod errors;

#[cfg(any(test, feature = "test-support"))]
pub mod test_strategies;

// Test fixtures are available for tests and when test-support feature is enabled
#[cfg(any(test, feature = "test-support"))]
pub mod test_fixtures;

// Re-export commonly used items
pub use types::*;
pub use parser::{Permit, DelegationTemplate, PermitError, PermitResult, LayerPatternConfig, PeerCapabilities, LayerConfig};
pub use parser::{DynamicLayerSchema, GrantType};  // Dynamic layer types
pub use parser::{expand_pattern, matches_wildcard, matches_schema_pattern, resolve_page_id_in_facts};  // Pattern/resolution utilities
pub use decision::{TokenDecision, DelegationDecision, SyncContext, should_send_updates, can_receive_updates, can_access_layer, can_access_with_layer_permits, matches_dynamic_schema_for_role, extract_issue_template};

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
    issue_sync_layer_consent,
    // Dynamic layer permits (node-issued)
    issue_layer_permit,
    issue_layer_authority_permit,
    // Dynamic layer re-issuance
    reissue_permit_with_layers,
    // Utilities
    get_public_key,
    extract_space_id,
};
