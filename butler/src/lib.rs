//! Butler: Storage and service layer for Osvauld
//!
//! Butler provides:
//! - redb-based persistent storage
//! - Layer cache for in-memory CRDT operations
//! - Filesystem storage for encrypted static assets
//! - SpaceService for Space/Page/Layer operations
//! - NodeService for node and contact management
//! - Auth service (signup, login, recovery) using Herald
//!
//! ## Terminology
//! - Space: Container for Pages (like a project or workspace)
//! - Page: A sub-application instance with its own template
//! - Layer: CRDT data containers (the actual collaborative state)

pub mod error;
pub mod models;
pub mod storage;
pub mod services;

pub use error::{ButlerError, Result};
pub use models::*;
pub use storage::{RedbStore, LayerCache, CachedLayer, LayerCacheStats, AssetStore};
pub use services::{SpaceService, LayerService, NodeService};
pub use services::{signup, login, is_signed_up, recover, change_passphrase, SignupResult, get_identity_data};
