//! Event Bus - Unified event system for UI interactions
//!
//! Provides a central event bus where all UI interactions flow through,
//! regardless of source (human, AI, peer, replay).
//!
//! ## Features
//! - Subscribe to events with filtering and sampling
//! - Emit events (AI/test can inject events)
//! - Record and replay event streams
//! - High-frequency event throttling

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Event source - where the event originated
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventSource {
    /// Human interaction via UI
    Human,
    /// AI/automation emitted
    Ai,
    /// Received from peer
    Peer,
    /// Replayed from recording
    Replay,
}

/// A UI event flowing through the bus
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    /// Event type (e.g., "click", "mouse_move", "field_changed")
    pub event_type: String,

    /// Where the event originated
    pub source: EventSource,

    /// Monotonic timestamp (milliseconds since bus creation)
    pub timestamp: u64,

    /// Target element ID (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,

    /// Event-specific data
    #[serde(flatten)]
    pub data: JsonValue,
}

/// Options for subscribing to events
#[derive(Debug, Clone, Default)]
pub struct SubscribeOptions {
    /// Filter by target element ID
    pub target: Option<String>,

    /// Filter by target prefix (e.g., "product_" matches "product_name", "product_price")
    pub target_prefix: Option<String>,

    /// Maximum events per second (for high-frequency events like mouse_move)
    pub sample_rate: Option<u32>,

    /// Batch events and deliver together after this many milliseconds
    pub batch_ms: Option<u32>,
}

/// Internal subscriber state
struct Subscriber {
    /// Unique subscriber ID
    id: u64,

    /// Event type to subscribe to
    _event_type: String,

    /// Filtering and sampling options
    options: SubscribeOptions,

    /// Callback ID in Lua registry
    callback_key: usize,

    /// Last event timestamp for sampling
    last_event_time: Option<Instant>,

    /// Batched events (when batch_ms is set)
    batch: Vec<Event>,

    /// Last batch flush time
    last_batch_time: Option<Instant>,
}

/// Recording of events for replay
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recording {
    /// Start timestamp
    pub started_at: u64,

    /// Recorded events
    pub events: Vec<Event>,
}

/// The Event Bus - central hub for all UI events
pub struct EventBus {
    /// Monotonic counter for event timestamps
    start_time: Instant,

    /// Next subscriber ID
    next_subscriber_id: AtomicU64,

    /// Subscribers by event type
    subscribers: HashMap<String, Vec<Subscriber>>,

    /// Active recording (if any)
    recording: Option<Recording>,

    /// Event history (for debugging, limited size)
    history: Vec<Event>,

    /// Maximum history size
    max_history: usize,
}

impl EventBus {
    /// Create a new Event Bus
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
            next_subscriber_id: AtomicU64::new(1),
            subscribers: HashMap::new(),
            recording: None,
            history: Vec::new(),
            max_history: 1000,
        }
    }

    /// Get current timestamp (milliseconds since bus creation)
    pub fn now(&self) -> u64 {
        self.start_time.elapsed().as_millis() as u64
    }

    /// Emit an event into the bus
    ///
    /// Returns list of (subscriber_id, callback_key, event_or_batch) for subscribers that should be notified
    pub fn emit(&mut self, mut event: Event) -> Vec<(u64, usize, EventDelivery)> {
        // Set timestamp if not already set
        if event.timestamp == 0 {
            event.timestamp = self.now();
        }

        // Record if recording is active
        if let Some(ref mut recording) = self.recording {
            recording.events.push(event.clone());
        }

        // Add to history
        self.history.push(event.clone());
        if self.history.len() > self.max_history {
            self.history.remove(0);
        }

        // Find matching subscribers
        let mut deliveries = Vec::new();

        if let Some(subscribers) = self.subscribers.get_mut(&event.event_type) {
            let now = Instant::now();

            for sub in subscribers.iter_mut() {
                // Check target filter
                if let Some(ref target_filter) = sub.options.target {
                    if event.target.as_ref() != Some(target_filter) {
                        continue;
                    }
                }

                // Check target prefix filter
                if let Some(ref prefix) = sub.options.target_prefix {
                    match &event.target {
                        Some(t) if t.starts_with(prefix) => {}
                        _ => continue,
                    }
                }

                // Check sample rate
                if let Some(rate) = sub.options.sample_rate {
                    let min_interval = Duration::from_secs_f64(1.0 / rate as f64);
                    if let Some(last) = sub.last_event_time {
                        if now.duration_since(last) < min_interval {
                            continue; // Skip, too soon
                        }
                    }
                    sub.last_event_time = Some(now);
                }

                // Handle batching
                if let Some(batch_ms) = sub.options.batch_ms {
                    sub.batch.push(event.clone());

                    let should_flush = match sub.last_batch_time {
                        None => {
                            sub.last_batch_time = Some(now);
                            false
                        }
                        Some(last) => {
                            now.duration_since(last) >= Duration::from_millis(batch_ms as u64)
                        }
                    };

                    if should_flush {
                        let batch = std::mem::take(&mut sub.batch);
                        sub.last_batch_time = Some(now);
                        deliveries.push((sub.id, sub.callback_key, EventDelivery::Batch(batch)));
                    }
                } else {
                    // Immediate delivery
                    deliveries.push((sub.id, sub.callback_key, EventDelivery::Single(event.clone())));
                }
            }
        }

        deliveries
    }

    /// Subscribe to events of a specific type
    ///
    /// Returns subscriber ID for later unsubscription
    pub fn subscribe(
        &mut self,
        event_type: &str,
        options: SubscribeOptions,
        callback_key: usize,
    ) -> u64 {
        let id = self.next_subscriber_id.fetch_add(1, Ordering::SeqCst);

        let subscriber = Subscriber {
            id,
            _event_type: event_type.to_string(),
            options,
            callback_key,
            last_event_time: None,
            batch: Vec::new(),
            last_batch_time: None,
        };

        self.subscribers
            .entry(event_type.to_string())
            .or_default()
            .push(subscriber);

        id
    }

    /// Unsubscribe by subscriber ID
    pub fn unsubscribe(&mut self, subscriber_id: u64) -> bool {
        for subscribers in self.subscribers.values_mut() {
            if let Some(pos) = subscribers.iter().position(|s| s.id == subscriber_id) {
                subscribers.remove(pos);
                return true;
            }
        }
        false
    }

    /// Start recording events
    pub fn start_recording(&mut self) {
        self.recording = Some(Recording {
            started_at: self.now(),
            events: Vec::new(),
        });
    }

    /// Stop recording and return the recording
    pub fn stop_recording(&mut self) -> Option<Recording> {
        self.recording.take()
    }

    /// Check if recording is active
    pub fn is_recording(&self) -> bool {
        self.recording.is_some()
    }

    /// Get event history
    pub fn history(&self) -> &[Event] {
        &self.history
    }

    /// Clear event history
    pub fn clear_history(&mut self) {
        self.history.clear();
    }

    /// Flush any pending batched events
    ///
    /// Should be called periodically (e.g., every 16ms)
    pub fn flush_batches(&mut self) -> Vec<(u64, usize, EventDelivery)> {
        let mut deliveries = Vec::new();
        let now = Instant::now();

        for subscribers in self.subscribers.values_mut() {
            for sub in subscribers.iter_mut() {
                if let Some(batch_ms) = sub.options.batch_ms {
                    if !sub.batch.is_empty() {
                        let should_flush = match sub.last_batch_time {
                            None => true,
                            Some(last) => {
                                now.duration_since(last) >= Duration::from_millis(batch_ms as u64)
                            }
                        };

                        if should_flush {
                            let batch = std::mem::take(&mut sub.batch);
                            sub.last_batch_time = Some(now);
                            deliveries.push((sub.id, sub.callback_key, EventDelivery::Batch(batch)));
                        }
                    }
                }
            }
        }

        deliveries
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

/// How events are delivered to subscribers
#[derive(Debug, Clone)]
pub enum EventDelivery {
    /// Single event (immediate delivery)
    Single(Event),
    /// Batch of events (delayed delivery)
    Batch(Vec<Event>),
}

/// Helper to create events easily
impl Event {
    /// Create a new event
    pub fn new(event_type: impl Into<String>, source: EventSource) -> Self {
        Self {
            event_type: event_type.into(),
            source,
            timestamp: 0, // Will be set by EventBus
            target: None,
            data: JsonValue::Object(serde_json::Map::new()),
        }
    }

    /// Set target element
    pub fn with_target(mut self, target: impl Into<String>) -> Self {
        self.target = Some(target.into());
        self
    }

    /// Set event data
    pub fn with_data(mut self, data: JsonValue) -> Self {
        self.data = data;
        self
    }

    /// Set a single data field
    pub fn with_field(mut self, key: impl Into<String>, value: JsonValue) -> Self {
        if let JsonValue::Object(ref mut map) = self.data {
            map.insert(key.into(), value);
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subscribe_and_filter() {
        let mut bus = EventBus::new();

        // Basic subscribe + emit
        let sub_id = bus.subscribe("click", SubscribeOptions::default(), 42);
        assert!(sub_id > 0);
        let event = Event::new("click", EventSource::Human).with_target("button1");
        let deliveries = bus.emit(event);
        assert_eq!(deliveries.len(), 1);
        assert_eq!(deliveries[0].1, 42);

        // Target filter: subscribe to specific target
        let mut bus = EventBus::new();
        bus.subscribe("click", SubscribeOptions { target: Some("submit".to_string()), ..Default::default() }, 42);

        // Different target → no match
        let d1 = bus.emit(Event::new("click", EventSource::Human).with_target("cancel"));
        assert_eq!(d1.len(), 0);

        // Matching target → match
        let d2 = bus.emit(Event::new("click", EventSource::Human).with_target("submit"));
        assert_eq!(d2.len(), 1);
    }

    #[test]
    fn test_sample_rate() {
        let mut bus = EventBus::new();

        // Subscribe with 10 events/sec max
        bus.subscribe(
            "mouse_move",
            SubscribeOptions {
                sample_rate: Some(10),
                ..Default::default()
            },
            42,
        );

        // First event should go through
        let event1 = Event::new("mouse_move", EventSource::Human);
        let deliveries1 = bus.emit(event1);
        assert_eq!(deliveries1.len(), 1);

        // Immediate second event should be filtered
        let event2 = Event::new("mouse_move", EventSource::Human);
        let deliveries2 = bus.emit(event2);
        assert_eq!(deliveries2.len(), 0);
    }

    #[test]
    fn test_recording() {
        let mut bus = EventBus::new();

        bus.start_recording();
        assert!(bus.is_recording());

        bus.emit(Event::new("click", EventSource::Human));
        bus.emit(Event::new("key_press", EventSource::Human));

        let recording = bus.stop_recording().unwrap();
        assert_eq!(recording.events.len(), 2);
        assert!(!bus.is_recording());
    }
}
