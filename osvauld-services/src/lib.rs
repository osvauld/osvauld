// lib.rs
mod auth_service;
mod auth_service2;
mod folder_service;
mod folder_service2;
mod resource_service;
mod resource_service2;
mod sync_service;
mod sync_service2;
mod transaction_service;
mod user_service;
mod user_service2;

pub use auth_service::AuthService;
pub use auth_service2::*;
pub use folder_service::FolderService;
pub use folder_service2::*;
pub use resource_service::ResourceService;
pub use resource_service2::*;
pub use sync_service::{SyncEvent, SyncService};
pub use sync_service2::*;
pub use transaction_service::TransactionService;
pub use user_service::UserService;
pub use user_service2::*;
