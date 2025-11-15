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
// These are convenience re-exports for backward compatibility
pub use gurkha::{
    // Types
    Capability, Role, DocType, ConnectionTokenType, ResourceTokenType,
    ResourceAction, SyncFacts, SyncDecision, DocMetadata,
    // Parser
    GenericUcan, DelegationTemplate,
    // Tokens
    ConnectionToken, OneTimeConnectionToken, OwnerConnectionToken,
    NodeConnectionToken, UserConnectionToken, ViewerAuthToken,
    ViewerConnectionToken, ResourceOwnerToken, ResourceShareToken,
    ResourceViewerToken, FolderOwnerToken, FolderShareToken, FolderViewerToken,
    // Sync context and decision functions
    SyncContext, should_send_updates, can_receive_updates,
};
