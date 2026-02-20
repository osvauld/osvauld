//! Protocol clock primitives for deterministic time handling.

use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use chrono::{DateTime, Datelike, Duration as ChronoDuration, Months, TimeZone, Utc};

/// Pluggable clock source for protocol/runtime time operations.
pub trait ClockSource: Send + Sync + 'static {
    /// Current Unix timestamp in seconds.
    fn now_unix(&self) -> i64;

    /// Current monotonic time for scheduling.
    fn now_monotonic(&self) -> Instant;

    /// Downcast support for test-time controls.
    fn as_any(&self) -> &dyn std::any::Any;
}

/// Real system clock (production).
#[derive(Debug, Clone, Copy)]
pub struct RealClock;

impl ClockSource for RealClock {
    fn now_unix(&self) -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("System time before UNIX epoch")
            .as_secs() as i64
    }

    fn now_monotonic(&self) -> Instant {
        Instant::now()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Manual clock for deterministic tests.
#[derive(Debug, Clone)]
pub struct ManualClock {
    unix_seconds: Arc<AtomicI64>,
    monotonic_nanos: Arc<AtomicU64>,
    monotonic_base: Instant,
}

impl ManualClock {
    /// Create a manual clock starting at the given Unix timestamp.
    pub fn new(start_unix: i64) -> Self {
        Self {
            unix_seconds: Arc::new(AtomicI64::new(start_unix)),
            monotonic_nanos: Arc::new(AtomicU64::new(0)),
            monotonic_base: Instant::now(),
        }
    }

    /// Advance both wall-clock and monotonic time by `duration`.
    pub fn advance(&self, duration: Duration) {
        let seconds = duration.as_secs() as i64;
        self.unix_seconds.fetch_add(seconds, Ordering::SeqCst);

        let nanos = duration.as_nanos() as u64;
        self.monotonic_nanos.fetch_add(nanos, Ordering::SeqCst);
    }

    /// Set wall-clock Unix time to a specific value.
    pub fn set_unix(&self, unix_seconds: i64) {
        self.unix_seconds.store(unix_seconds, Ordering::SeqCst);
    }
}

impl ClockSource for ManualClock {
    fn now_unix(&self) -> i64 {
        self.unix_seconds.load(Ordering::SeqCst)
    }

    fn now_monotonic(&self) -> Instant {
        let nanos = self.monotonic_nanos.load(Ordering::SeqCst);
        self.monotonic_base + Duration::from_nanos(nanos)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

fn dt_from_unix(unix_seconds: i64) -> Result<DateTime<Utc>, String> {
    Utc.timestamp_opt(unix_seconds, 0)
        .single()
        .ok_or_else(|| format!("Invalid Unix timestamp: {}", unix_seconds))
}

fn shift_months(base: DateTime<Utc>, offset: i32) -> Result<DateTime<Utc>, String> {
    if offset == 0 {
        return Ok(base);
    }

    let months = Months::new(offset.unsigned_abs());
    if offset > 0 {
        base.checked_add_months(months)
            .ok_or_else(|| format!("Month overflow for offset {}", offset))
    } else {
        base.checked_sub_months(months)
            .ok_or_else(|| format!("Month underflow for offset {}", offset))
    }
}

/// Format a period key from Unix time with calendar-relative period offset.
///
/// Period keys:
/// - minute: `YYYY-MM-DDTHH:MM`
/// - hour: `YYYY-MM-DDTHH`
/// - day: `YYYY-MM-DD`
/// - week: `YYYY-Www` (ISO week)
/// - month: `YYYY-MM`
pub fn format_period(period: &str, unix_seconds: i64, offset: i32) -> Result<String, String> {
    let base = dt_from_unix(unix_seconds)?;

    let shifted = match period {
        "minute" => base + ChronoDuration::minutes(offset as i64),
        "hour" => base + ChronoDuration::hours(offset as i64),
        "day" => base + ChronoDuration::days(offset as i64),
        "week" => base + ChronoDuration::weeks(offset as i64),
        "month" => shift_months(base, offset)?,
        _ => return Err(format!("Unknown period type: {}", period)),
    };

    match period {
        "minute" => Ok(shifted.format("%Y-%m-%dT%H:%M").to_string()),
        "hour" => Ok(shifted.format("%Y-%m-%dT%H").to_string()),
        "day" => Ok(shifted.format("%Y-%m-%d").to_string()),
        "week" => {
            let iso_week = shifted.iso_week();
            Ok(format!("{}-W{:02}", iso_week.year(), iso_week.week()))
        }
        "month" => Ok(shifted.format("%Y-%m").to_string()),
        _ => Err(format!("Unknown period type: {}", period)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manual_clock_advances_monotonic_in_subseconds() {
        let clock = ManualClock::new(1_708_444_800);
        let before = clock.now_monotonic();

        clock.advance(Duration::from_millis(15));

        assert_eq!(clock.now_unix(), 1_708_444_800);
        assert!(clock.now_monotonic() > before);

        clock.advance(Duration::from_secs(2));
        assert_eq!(clock.now_unix(), 1_708_444_802);
    }

    #[test]
    fn format_period_with_offsets() {
        let unix = 1_708_444_800; // 2024-02-20 16:00:00 UTC

        assert_eq!(
            format_period("minute", unix, 0).unwrap(),
            "2024-02-20T16:00"
        );
        assert_eq!(
            format_period("minute", unix, -1).unwrap(),
            "2024-02-20T15:59"
        );

        assert_eq!(format_period("hour", unix, 0).unwrap(), "2024-02-20T16");
        assert_eq!(format_period("hour", unix, 1).unwrap(), "2024-02-20T17");

        assert_eq!(format_period("day", unix, 0).unwrap(), "2024-02-20");
        assert_eq!(format_period("day", unix, 1).unwrap(), "2024-02-21");

        assert_eq!(format_period("week", unix, 0).unwrap(), "2024-W08");
        assert_eq!(format_period("week", unix, -1).unwrap(), "2024-W07");

        assert_eq!(format_period("month", unix, 0).unwrap(), "2024-02");
        assert_eq!(format_period("month", unix, 1).unwrap(), "2024-03");
    }

    #[test]
    fn month_offset_is_calendar_relative_with_year_rollover() {
        let jan_15_2025 = 1_736_899_200; // 2025-01-15 00:00:00 UTC

        assert_eq!(format_period("month", jan_15_2025, -1).unwrap(), "2024-12");
        assert_eq!(format_period("month", jan_15_2025, 1).unwrap(), "2025-02");
    }
}
