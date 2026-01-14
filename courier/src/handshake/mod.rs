//! Handshake module - P2P authentication protocol
//!
//! Contains pure decision logic for the handshake state machine.
//! The peer_actor module executes these decisions.
//!
//! ## Design (Capability-Based)
//!
//! The protocol is completely role-agnostic. All decisions are based on
//! capabilities derived directly from the permit structure:
//!
//! - `accept_publish: true` → Peer can publish spaces to this node
//! - `first_connection: true` → First time connecting

pub mod decision;

pub use decision::{
    HelloDecision, WelcomeDecision, PermitGrantDecision,
    HelloContext, WelcomeContext, PermitGrantContext,
    decide_hello_response, decide_welcome_response, decide_permit_grant_response,
    extract_capabilities, is_first_connection, can_publish,
};
