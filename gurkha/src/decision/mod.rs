//! Decision Logic Module
//!
//! Pure decision/inference functions that determine WHAT should be in a token.
//! Returns data structures (TokenDecision, DelegationDecision) that describe the decision.
//! NO crypto calls - just logic for determining capabilities, facts, expiry, audience.
//!
//! These functions are replaceable - they could later be implemented in OCaml
//! and called via FFI, but would still return the same data structures.
//! The crypto layer (crypto.rs) then takes these decisions and signs them.

mod types;
mod connection;
mod delegation;
mod resource;
mod sync;
mod consent;
mod layer;

// Re-export types
pub use types::{DecisionResult, TokenDecision, DelegationDecision};

// Re-export connection token decisions
pub use connection::{
    decide_one_time_token,
    decide_peer_connection,
    decide_page_viewer_auth,
    decide_space_viewer_auth,
};

// Re-export delegation functions
pub use delegation::{
    decide_delegation,
    extract_issue_template,
    extract_facts_from_token,
};

// Re-export resource token decisions
pub use resource::{
    decide_space_owner_token,
    decide_space_node_to_owner_token,
    decide_page_owner_token,
};

// Re-export sync decisions
pub use sync::{
    SyncContext,
    should_send_updates,
    can_receive_updates,
    should_request_updates,
};

// Re-export consent decisions
pub use consent::{
    decide_sync_space_consent,
    decide_sync_page_consent,
};

// Re-export layer authorization
pub use layer::can_access_layer;
