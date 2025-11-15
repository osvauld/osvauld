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
//! ## Module Structure
//!
//! - `types` - Domain types (Capability, Role, DocType, TokenTypes)
//! - `uri` - URI parsing and capability URIs
//! - `token` - Typed token wrappers (11 types)
//! - `parser` - GenericUcan, DelegationTemplate
//! - `decision` - Decision/inference logic (pure logic, no crypto)
//! - `crypto` - Crypto operations (signing, verification)
//! - `extractors` - UCAN extraction utilities
//! - `service` - Public API (orchestrates decision + crypto)
//! - `errors` - GurkhaError types

pub mod types;
pub mod uri;
pub mod token;
pub mod parser;
pub mod decision;
pub mod crypto;
pub mod extractors;
pub mod service;
pub mod errors;

// Re-export commonly used items
pub use types::*;
pub use token::*;
pub use parser::{GenericUcan, DelegationTemplate};
pub use decision::{TokenDecision, DelegationDecision, SyncContext, should_send_updates, can_receive_updates};
pub use service::UcanService;

// Factory function
pub fn create_ucan_service(signing_key: ed25519_dalek::SigningKey, verifying_key: ed25519_dalek::VerifyingKey) -> std::sync::Arc<std::sync::RwLock<UcanService>> {
    let mut service = UcanService::new();
    service.load_keys(signing_key, verifying_key);
    std::sync::Arc::new(std::sync::RwLock::new(service))
}
