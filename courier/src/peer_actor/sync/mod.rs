//! Unified sync handlers - permit determines role (owner/node/viewer)
//!
//! Key insight: Sync mechanics are identical regardless of who is syncing.
//! The ONLY difference is the permit - gurkha handles authorization.
//!
//! Flows:
//! 1. Viewer requests space: SpaceRequest → SpaceData → SpaceDataAck → PageData (x N)
//! 2. 3-Step Sync Protocol: SyncOffer → SyncAccept → SyncAck (or resync SyncOffer if diverged)
//!
//! The 3-step protocol is used for ALL sync operations:
//! - Initial sync on subscribe
//! - Broadcast updates
//! - Periodic reconciliation
//!
//! Divergence detection: If state vectors don't match after applying update,
//! sender immediately sends full diff as new SyncOffer.
//!
//! ## Authorization
//!
//! **Identity-based**: `gurkha::can_access_layer()` checks if viewer can access layer
//! **Role-based**: App code checks `permit:role()` for state-dependent authorization
//!
//! Authorization logic lives in app code (signed by owner, trusted).

mod space_request;
mod protocol;
mod subscription;
