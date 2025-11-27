pub mod certificate;
pub mod device;
pub mod document;
pub mod folder;
pub mod folder_share_record;
pub mod p2p;
pub mod resource;
pub mod share_record;
pub mod user;

// New models for Butler/redb
pub mod identity;
pub mod device2;
pub mod contact;
pub mod node;

// Core domain: Space → Page → Layer
pub mod space;
pub mod page;
pub mod layer;

pub use certificate::*;
pub use device::*;
pub use document::*;
pub use folder::*;
pub use folder_share_record::*;
pub use p2p::*;
pub use resource::*;
pub use share_record::*;
pub use user::*;

// New model exports (Butler/redb)
pub use identity::*;
pub use device2::*;
pub use contact::*;
pub use node::*;

// Core domain exports: Space → Page → Layer
pub use space::*;
pub use page::*;
pub use layer::*;

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
