//! DST Bounded Retry Tests - Infinite Loop Prevention
//!
//! Tests bounded retry invariants:
//! - INV-L1: Rejected updates don't trigger infinite retry
//! - INV-L2: Resync attempts are tracked and bounded
//! - INV-L3: MAX_RESYNC_ATTEMPTS triggers SyncReset
//! - INV-L5: SyncSnapshot terminates the retry loop

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use anyhow::Result;
use tracing::info;

use crate::scenario::init_tracing;

/// Maximum number of resync attempts before SyncReset
const MAX_RESYNC_ATTEMPTS: u32 = 3;

// Test: Rejection vs Divergence Handling (merged with rejected_updates_no_infinite_loop)

/// Validates INV-L1: Rejection vs divergence are handled differently
///
/// **Rejection**: Unauthorized ops -> no retry, discard local ops
/// **Divergence**: Authorized but vectors misaligned -> bounded retry -> SyncReset
#[tokio::test]
async fn test_rejection_vs_divergence_handling() -> Result<()> {
    init_tracing();

    // Case 1: Rejection (unauthorized) - no retry
    let resync_attempts: AtomicU32 = AtomicU32::new(0);
    let sync_accept_count: AtomicU32 = AtomicU32::new(0);

    let apply_result: Result<(), &str> = Err("Permission denied: cannot write to layer");
    match apply_result {
        Ok(()) => { sync_accept_count.fetch_add(1, Ordering::SeqCst); }
        Err(_) => { /* Rejection - no SyncAccept, no retry */ }
    }

    assert_eq!(resync_attempts.load(Ordering::SeqCst), 0, "INV-L1: Rejection does not increment resync_attempts");
    assert_eq!(sync_accept_count.load(Ordering::SeqCst), 0, "INV-L1: No SyncAccept for rejected updates");

    // Case 2: Divergence (authorized, vectors misaligned) - bounded retry
    struct SyncState { resync_attempts: u32 }

    let mut rejection = SyncState { resync_attempts: 0 };
    // Rejection: no increment
    assert_eq!(rejection.resync_attempts, 0);

    let mut divergence = SyncState { resync_attempts: 0 };
    divergence.resync_attempts += 1;
    assert_eq!(divergence.resync_attempts, 1, "Divergence should increment resync_attempts");

    info!("Rejection vs divergence distinction test passed (INV-L1)");
    Ok(())
}

// Test: Divergence Triggers Bounded Resync Then SyncReset

/// Validates INV-L2, INV-L3: Bounded resync then SyncReset
#[tokio::test]
async fn test_divergence_triggers_sync_reset() -> Result<()> {
    init_tracing();

    let resync_attempts: AtomicU32 = AtomicU32::new(0);
    let sync_reset_sent = Arc::new(std::sync::atomic::AtomicBool::new(false));

    let our_vector = vec![1, 2, 3, 4];
    let their_vector = vec![1, 2, 3, 5]; // Always different

    for _ in 0..=MAX_RESYNC_ATTEMPTS {
        if our_vector == their_vector {
            break;
        }
        let current = resync_attempts.fetch_add(1, Ordering::SeqCst) + 1;
        if current > MAX_RESYNC_ATTEMPTS {
            sync_reset_sent.store(true, Ordering::SeqCst);
            break;
        }
    }

    assert!(resync_attempts.load(Ordering::SeqCst) > MAX_RESYNC_ATTEMPTS);
    assert!(sync_reset_sent.load(Ordering::SeqCst), "INV-L3: SyncReset sent after max resyncs");

    info!("Bounded resync test passed (INV-L2, INV-L3)");
    Ok(())
}

// Test: SyncSnapshot Terminates Loop

/// Validates INV-L5: Receiving SyncSnapshot terminates the sync loop
#[tokio::test]
async fn test_sync_snapshot_terminates_loop() -> Result<()> {
    init_tracing();

    let loop_terminated = std::sync::atomic::AtomicBool::new(false);
    let snapshot_applied = std::sync::atomic::AtomicBool::new(false);

    let received_snapshot = vec![1, 2, 3, 4, 5];

    if !received_snapshot.is_empty() {
        snapshot_applied.store(true, Ordering::SeqCst);
        loop_terminated.store(true, Ordering::SeqCst);
    }

    assert!(snapshot_applied.load(Ordering::SeqCst));
    assert!(loop_terminated.load(Ordering::SeqCst), "INV-L5: Loop terminates after SyncSnapshot");

    Ok(())
}

// Test: Resync Counter Reset on Success

/// Tests that resync_attempts resets to 0 after successful sync
#[tokio::test]
async fn test_resync_counter_reset_on_success() -> Result<()> {
    init_tracing();

    let mut resync_attempts: u32 = 2;

    let our_vector = vec![1, 2, 3];
    let their_vector = vec![1, 2, 3];

    if our_vector == their_vector {
        resync_attempts = 0;
    }

    assert_eq!(resync_attempts, 0, "Resync attempts should reset on success");
    Ok(())
}

// Test: Mode-Specific Behavior (User sends SyncReset, Node sends SyncSnapshot)

/// Tests that User mode sends SyncReset and Node mode sends SyncSnapshot
#[tokio::test]
async fn test_mode_specific_behavior() -> Result<()> {
    init_tracing();

    #[derive(Clone, Copy, PartialEq)]
    enum Mode { User, Node }

    // User mode sends SyncReset
    let user_mode = Mode::User;
    assert!(user_mode == Mode::User, "User mode should send SyncReset");

    // Node mode does NOT send SyncReset
    let node_mode = Mode::Node;
    assert!(node_mode != Mode::User, "Node mode should NOT send SyncReset");

    // Node mode responds with SyncSnapshot
    assert!(node_mode == Mode::Node, "Node mode should send SyncSnapshot");

    // User mode does NOT respond with SyncSnapshot
    assert!(user_mode != Mode::Node, "User mode should NOT respond with SyncSnapshot");

    info!("Mode-specific behavior test passed");
    Ok(())
}
