pub mod certificate;
pub mod device;
pub mod document;
pub mod folder;
pub mod folder_share_record;
pub mod p2p;
pub mod resource;
pub mod share_record;
pub mod user;

pub use certificate::*;
pub use device::*;
pub use document::*;
pub use folder::*;
pub use folder_share_record::*;
pub use p2p::*;
pub use resource::*;
pub use share_record::*;
pub use user::*;

// Re-export commonly used UCAN types from gurkha
// V3 Migration: Simplified to just Permit + essential types
pub use gurkha::{
    // Types (domain concepts)
    Capability, DocType, ResourceAction, SyncFacts, SyncDecision, DocMetadata,
    // Parser (single unified token type)
    Permit, DelegationTemplate, UcanCore,
    // Token errors
    UcanTokenError, UcanTokenResult,
};

// Note: Role, ConnectionTokenType, ResourceTokenType enums removed in V3
// Note: All typed token wrappers removed in V3 - use Permit directly
