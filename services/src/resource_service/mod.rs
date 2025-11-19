//! Resource Service - UCAN-aware resource management
//!
//! This module provides service-layer operations for encrypted resources using Loro CRDTs.
//! It is organized into three sub-modules:
//!
//! ## Module Structure
//!
//! - **core**: Reusable building blocks (load, decrypt, encrypt, filter patterns)
//! - **crud**: Resource lifecycle operations (create, read, update, delete, share)
//! - **sync**: CRDT synchronization operations (state vectors, updates, peer sync)
//!
//! All operations work through the same sync pipeline regardless of node mode (sovereign or viewer).
//! Permissions are enforced via UCAN tokens - there are no special "viewer-only" functions.
//!
//! ## Design Principles
//!
//! 1. **Typed Tokens**: All functions use typed UCAN wrappers (ResourceOwnerToken, etc.)
//! 2. **SyncContext**: Permission logic delegated to pure functions in sync_context.rs
//! 3. **DRY**: Common patterns extracted to core module (~400 lines eliminated)
//! 4. **Data-Driven**: Frontend defines permissions, backend enforces via UCAN
//! 5. **Unified Pipeline**: Same sync functions for node-to-node and node-to-viewer
//!
//! ## Usage Examples
//!
//! ```ignore
//! use crate::resource_service::{create_resource, get_resource_by_id};
//! use osvauld_core::models::ResourceOwnerToken;
//!
//! // Create a new resource
//! let token = ResourceOwnerToken::from_token(&ucan_str)?;
//! let resource = create_resource(&token, metadata, repo_ctx, crypto_utils).await?;
//!
//! // Load and sync with peer (works for both node and viewer)
//! let our_token = ResourceShareToken::from_token(&our_ucan)?;
//! let peer_token = ResourceShareToken::from_token(&peer_ucan)?;
//! let updates = generate_updates_for_peer(&our_token, &peer_token, state_vectors, ...).await?;
//! ```

mod core;
mod crud;
mod sync;

// Re-export public API
pub use crud::*;
pub use sync::*;

// Re-export specific core functions needed by other services
pub use core::delegate_and_create_share_record;

// core module is private (internal use only)
// It provides reusable building blocks used by crud and sync
