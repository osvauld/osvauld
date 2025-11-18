// lib.rs
mod auth_service;
mod errors;
mod folder_service;
// merge_service removed - all merge logic now in gurkha::MergeService
mod resource_service;
mod share_service;
mod user_service;
// website_service removed - all operations consolidated into resource_service

pub use auth_service::*;
pub use errors::*;
pub use folder_service::*;
// merge logic accessed via gurkha::MergeService (UCAN-aware CRDT operations)
pub use resource_service::*;
pub use share_service::*;
pub use user_service::*;
