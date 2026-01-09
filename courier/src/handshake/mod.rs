//! Handshake module - P2P authentication protocol
//!
//! Contains pure decision logic for the handshake state machine.
//! The peer_actor module executes these decisions.

pub mod decision;

pub use decision::{
    HandshakeRole,
    HelloDecision, WelcomeDecision, PermitGrantDecision,
    HelloContext, WelcomeContext, PermitGrantContext,
    decide_hello_response, decide_welcome_response, decide_permit_grant_response,
    permit_type_for_role, relationship_for_user,
};
