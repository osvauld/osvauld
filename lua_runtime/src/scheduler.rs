//! Scheduler - Timer and coroutine scheduling for Lua runtime
//!
//! This module provides JS-style event loop scheduling:
//! - `setTimeout` / `setInterval` style timers
//! - Future: coroutine scheduling with `await`
//!
//! ## Architecture
//!
//! The scheduler tracks pending work and integrates with the runtime's event loop:
//!
//! ```text
//! tokio::select! {
//!     cmd = cmd_rx.recv() => handle_command(cmd),
//!     _ = scheduler.wait_next_timer() => scheduler.fire_due_timers(),
//! }
//! ```
//!
//! ## Timer Storage
//!
//! - Rust side: tracks timing (id, deadline, repeat interval)
//! - Lua side: stores callbacks in `_G._timers[id]`
//!
//! When a timer fires, we send the id to Lua which looks up and calls the callback.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};
use std::sync::atomic::{AtomicU64, Ordering};

use mlua::{Function, Lua, Result as LuaResult, Table, Value};
use tracing::{debug, trace, warn};

// Timer Entry

/// A registered timer
#[derive(Debug, Clone)]
pub struct TimerEntry {
    /// Unique timer ID
    pub id: u64,

    /// When this timer should fire
    pub deadline: Instant,

    /// For setInterval: repeat interval (None for setTimeout)
    pub interval: Option<Duration>,

    /// Whether this timer has been cancelled
    pub cancelled: bool,
}

// Scheduler

/// Timer and coroutine scheduler
///
/// Integrates with the Lua runtime's event loop to provide:
/// - setTimeout / setInterval style timers
/// - Future: coroutine resume scheduling
pub struct Scheduler {
    /// Active timers, keyed by ID for O(1) lookup/cancel
    pub(crate) timers: BTreeMap<u64, TimerEntry>,

    /// Timers sorted by deadline for efficient next-deadline lookup
    /// Maps deadline -> list of timer IDs firing at that time
    by_deadline: BTreeMap<Instant, Vec<u64>>,

    /// Next timer ID
    next_id: AtomicU64,
}

impl Default for Scheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl Scheduler {
    /// Create a new scheduler
    pub fn new() -> Self {
        Self {
            timers: BTreeMap::new(),
            by_deadline: BTreeMap::new(),
            next_id: AtomicU64::new(1),
        }
    }

    /// Register a new timer
    ///
    /// Returns the timer ID for later cancellation
    pub fn register_timer(&mut self, delay_ms: u64, repeat: bool) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let now = Instant::now();
        let delay = Duration::from_millis(delay_ms);
        let deadline = now + delay;

        let entry = TimerEntry {
            id,
            deadline,
            interval: if repeat { Some(delay) } else { None },
            cancelled: false,
        };

        // Add to timers map
        self.timers.insert(id, entry);

        // Add to deadline index
        self.by_deadline
            .entry(deadline)
            .or_insert_with(Vec::new)
            .push(id);

        debug!(timer_id = id, delay_ms, repeat, "Timer registered");
        id
    }

    /// Cancel a timer
    ///
    /// Returns true if the timer was found and cancelled
    pub fn clear_timer(&mut self, id: u64) -> bool {
        if let Some(entry) = self.timers.get_mut(&id) {
            entry.cancelled = true;
            debug!(timer_id = id, "Timer cancelled");
            true
        } else {
            debug!(timer_id = id, "Timer not found for cancellation");
            false
        }
    }

    /// Get the deadline of the next timer to fire
    ///
    /// Returns None if no timers are pending
    pub fn next_deadline(&self) -> Option<Instant> {
        // Find first non-cancelled timer
        for (deadline, ids) in &self.by_deadline {
            for id in ids {
                if let Some(entry) = self.timers.get(id) {
                    if !entry.cancelled {
                        return Some(*deadline);
                    }
                }
            }
        }
        None
    }

    /// Get duration until next timer fires
    ///
    /// Returns a very long duration if no timers pending (for select! timeout)
    pub fn time_until_next(&self) -> Duration {
        match self.next_deadline() {
            Some(deadline) => {
                let now = Instant::now();
                if deadline <= now {
                    Duration::ZERO
                } else {
                    deadline - now
                }
            }
            None => Duration::from_secs(86400), // 24 hours - effectively infinite
        }
    }

    /// Fire all timers that are due
    ///
    /// Returns list of timer IDs that fired (for Lua to call callbacks)
    pub fn fire_due_timers(&mut self) -> Vec<u64> {
        let now = Instant::now();
        let mut fired = Vec::new();
        let mut to_reschedule = Vec::new();
        let mut deadlines_to_remove = Vec::new();

        // Collect all timers that are due
        for (&deadline, ids) in &self.by_deadline {
            if deadline > now {
                break; // BTreeMap is sorted, no more due timers
            }

            deadlines_to_remove.push(deadline);

            for &id in ids {
                if let Some(entry) = self.timers.get(&id) {
                    if entry.cancelled {
                        continue;
                    }

                    fired.push(id);

                    // If repeating, schedule next occurrence
                    if let Some(interval) = entry.interval {
                        to_reschedule.push((id, interval));
                    }
                }
            }
        }

        // Remove processed deadlines
        for deadline in deadlines_to_remove {
            self.by_deadline.remove(&deadline);
        }

        // Reschedule repeating timers
        for (id, interval) in to_reschedule {
            if let Some(entry) = self.timers.get_mut(&id) {
                let new_deadline = Instant::now() + interval;
                entry.deadline = new_deadline;

                self.by_deadline
                    .entry(new_deadline)
                    .or_insert_with(Vec::new)
                    .push(id);
            }
        }

        // Clean up one-shot timers that fired
        for &id in &fired {
            if let Some(entry) = self.timers.get(&id) {
                if entry.interval.is_none() {
                    self.timers.remove(&id);
                }
            }
        }

        if !fired.is_empty() {
            trace!(count = fired.len(), "Timers fired");
        }

        fired
    }

    /// Check if there are any pending timers
    pub fn has_pending_timers(&self) -> bool {
        self.timers.values().any(|t| !t.cancelled)
    }

    /// Get count of active (non-cancelled) timers
    pub fn active_timer_count(&self) -> usize {
        self.timers.values().filter(|t| !t.cancelled).count()
    }

    /// Clean up cancelled timers (call periodically to free memory)
    pub fn gc(&mut self) {
        // Remove cancelled timers
        self.timers.retain(|_, entry| !entry.cancelled);

        // Clean up deadline index
        self.by_deadline.retain(|_, ids| {
            ids.retain(|id| self.timers.contains_key(id));
            !ids.is_empty()
        });
    }
}

// Lua Integration

/// Register timer functions in Lua
///
/// Creates the `timer` global with:
/// - `timer.setTimeout(ms, callback)` -> id
/// - `timer.setInterval(ms, callback)` -> id
/// - `timer.clear(id)`
pub fn register_timer_api(lua: &Lua) -> LuaResult<()> {
    // Create _timers table for storing callbacks
    let timers_table: Table = lua.create_table()?;
    lua.globals().set("_timers", timers_table)?;

    // Create timer API table
    let timer_table: Table = lua.create_table()?;

    // Note: The actual setTimeout/setInterval functions need access to the
    // Scheduler, which is owned by the runtime. We'll set these up when
    // creating the runtime, using lua.scope() or by passing a channel.
    //
    // For now, we just create the structure. The runtime will fill in the
    // actual implementations.

    lua.globals().set("timer", timer_table)?;

    debug!("Timer API registered");
    Ok(())
}

/// Fire a timer callback in Lua
///
/// Looks up the callback in `_G._timers[id]` and calls it
pub fn fire_timer_callback(lua: &Lua, timer_id: u64) -> LuaResult<()> {
    let timers: Table = lua.globals().get("_timers")?;

    let callback: Option<Function> = timers.get(timer_id)?;

    match callback {
        Some(func) => {
            trace!(timer_id, "Calling timer callback");
            if let Err(e) = func.call::<()>(()) {
                warn!(timer_id, error = %e, "Timer callback error");
            }
        }
        None => {
            warn!(timer_id, "Timer callback not found");
        }
    }

    Ok(())
}

/// Remove a timer callback from Lua (for one-shot timers)
pub fn remove_timer_callback(lua: &Lua, timer_id: u64) -> LuaResult<()> {
    let timers: Table = lua.globals().get("_timers")?;
    timers.set(timer_id, Value::Nil)?;
    Ok(())
}

// Tests

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;

    #[test]
    fn test_register_and_fire() {
        let mut scheduler = Scheduler::new();

        // Timeout (one-shot)
        let id = scheduler.register_timer(10, false);
        assert_eq!(id, 1);
        assert!(scheduler.has_pending_timers());
        assert!(scheduler.fire_due_timers().is_empty(), "Should not fire immediately");
        sleep(Duration::from_millis(15));
        assert_eq!(scheduler.fire_due_timers(), vec![1]);
        assert!(!scheduler.has_pending_timers(), "One-shot removed after firing");

        // Interval (repeating)
        let id = scheduler.register_timer(10, true);
        sleep(Duration::from_millis(15));
        assert_eq!(scheduler.fire_due_timers(), vec![id]);
        assert!(scheduler.has_pending_timers(), "Interval still pending");
        sleep(Duration::from_millis(15));
        assert_eq!(scheduler.fire_due_timers(), vec![id], "Fires again");
    }

    #[test]
    fn test_cancel_timer() {
        let mut scheduler = Scheduler::new();

        let id = scheduler.register_timer(10, false);
        assert!(scheduler.has_pending_timers());
        assert!(scheduler.clear_timer(id));

        sleep(Duration::from_millis(15));
        assert!(scheduler.fire_due_timers().is_empty());
    }

    #[test]
    fn test_multiple_timers_and_deadlines() {
        let mut scheduler = Scheduler::new();

        // No timers → no deadline
        assert!(scheduler.next_deadline().is_none());

        let id1 = scheduler.register_timer(10, false);
        let id2 = scheduler.register_timer(20, false);
        let id3 = scheduler.register_timer(10, false);

        assert_eq!(scheduler.active_timer_count(), 3);
        assert!(scheduler.next_deadline().is_some());
        let until = scheduler.time_until_next();
        assert!(until <= Duration::from_millis(10));

        // First batch: id1 and id3 fire
        sleep(Duration::from_millis(15));
        let fired = scheduler.fire_due_timers();
        assert!(fired.contains(&id1));
        assert!(fired.contains(&id3));
        assert!(!fired.contains(&id2));

        // Second batch: id2 fires
        sleep(Duration::from_millis(10));
        assert_eq!(scheduler.fire_due_timers(), vec![id2]);
    }
}
