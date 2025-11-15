/// UCAN Module - Unified User Controlled Authorization Networks
///
/// This module provides a clean, type-safe API for working with UCAN tokens.
/// It consolidates all UCAN-related code into a single, cohesive module.

// Public submodules
pub mod types;
pub mod parser;
pub mod token;
pub mod uri;

// Re-exports for convenient access
pub use types::{
    Capability, Role, DocType, ConnectionTokenType, ResourceTokenType,
    ResourceAction, SyncFacts, SyncDecision, DocMetadata,
};

pub use parser::{
    ResourceUcan, DelegationTemplate,
    ResourceUcanError, ResourceUcanResult,
};

pub use token::{
    // Connection tokens
    ConnectionToken, OneTimeConnectionToken, OwnerConnectionToken,
    NodeConnectionToken, UserConnectionToken, ViewerAuthToken,
    ViewerConnectionToken,
    ConnectionTokenError, ConnectionTokenResult,

    // Resource tokens
    ResourceOwnerToken, ResourceShareToken, ResourceViewerToken,
    FolderOwnerToken, FolderShareToken, FolderViewerToken,
};

// Trait re-exports
pub use token::{
    UcanToken, HasId, CanDelegate, ResourceOps, FolderOps,
};

/// Prelude for convenient importing
pub mod prelude {
    pub use crate::ucan::types::*;
    pub use crate::ucan::parser::{ResourceUcan, DelegationTemplate};
    pub use crate::ucan::token::*;
    pub use crate::ucan::{UcanToken, HasId, CanDelegate, ResourceOps, FolderOps};
    pub use crate::ucan::uri;
}
