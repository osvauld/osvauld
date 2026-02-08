//! Message tracing for protocol debugging
//!
//! Provides infrastructure for recording and asserting on protocol message sequences
//! during integration tests.
//!
//! ## Usage
//!
//! ```ignore
//! let tracer = MessageTracer::new();
//!
//! // ... run protocol operations ...
//!
//! // Assert exact message sequence
//! tracer.assert_sequence(&["Hello", "Welcome", "PermitGrant", "Ack"]);
//!
//! // Get messages of specific type
//! let sync_offers = tracer.messages_of_type("SyncOffer");
//! assert_eq!(sync_offers.len(), 1);
//!
//! // Debug output
//! tracer.dump();
//!
//! // Latency metrics
//! let metrics = tracer.latency_report();
//! println!("{}", metrics.summary());
//! ```

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use courier::Message;
use transport::NodeId;

// Ephemeral Types

/// A captured ephemeral datagram
#[derive(Debug, Clone)]
pub struct TracedEphemeral {
    /// When the ephemeral was captured
    pub timestamp: Instant,
    /// Direction (sent or received)
    pub direction: Direction,
    /// Peer name (human-readable)
    pub peer_name: String,
    /// Page ID
    pub page_id: String,
    /// The payload as JSON (if parseable)
    pub payload_json: Option<serde_json::Value>,
    /// Raw payload bytes
    pub payload_raw: Vec<u8>,
}

// Types

/// Direction of a traced message
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// Message sent by a peer
    Sent,
    /// Message received by a peer
    Received,
}

impl std::fmt::Display for Direction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Direction::Sent => write!(f, "->"),
            Direction::Received => write!(f, "<-"),
        }
    }
}

/// A captured protocol message with metadata
#[derive(Debug, Clone)]
pub struct TracedMessage {
    /// When the message was captured
    pub timestamp: Instant,
    /// Direction (sent or received)
    pub direction: Direction,
    /// Node that originated the message
    pub from: NodeId,
    /// Node that received the message
    pub to: NodeId,
    /// The actual protocol message
    pub message: Message,
    /// Peer name (human-readable, for debugging)
    pub peer_name: String,
}

impl TracedMessage {
    /// Get the message type name
    pub fn name(&self) -> &'static str {
        self.message.name()
    }
}

// MessageTracer

/// Message tracer for protocol debugging
///
/// Thread-safe collector of protocol messages. Can be shared across peers
/// to capture the full conversation.
#[derive(Clone, Default)]
pub struct MessageTracer {
    messages: Arc<Mutex<Vec<TracedMessage>>>,
    ephemerals: Arc<Mutex<Vec<TracedEphemeral>>>,
}

impl MessageTracer {
    /// Create a new message tracer
    pub fn new() -> Self {
        Self {
            messages: Arc::new(Mutex::new(Vec::new())),
            ephemerals: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Record a message
    ///
    /// **Context**: Called by PeerActor hooks (send/receive) to capture messages
    pub fn trace(
        &self,
        direction: Direction,
        from: NodeId,
        to: NodeId,
        message: Message,
        peer_name: &str,
    ) {
        let traced = TracedMessage {
            timestamp: Instant::now(),
            direction,
            from,
            to,
            message,
            peer_name: peer_name.to_string(),
        };

        let mut messages = self.messages.lock().expect("tracer lock poisoned");
        messages.push(traced);
    }

    /// Record a sent message
    pub fn trace_sent(&self, from: NodeId, to: NodeId, message: Message, peer_name: &str) {
        self.trace(Direction::Sent, from, to, message, peer_name);
    }

    /// Record a received message
    pub fn trace_received(&self, from: NodeId, to: NodeId, message: Message, peer_name: &str) {
        self.trace(Direction::Received, from, to, message, peer_name);
    }

    /// Get all traced messages
    pub fn messages(&self) -> Vec<TracedMessage> {
        let messages = self.messages.lock().expect("tracer lock poisoned");
        messages.clone()
    }

    /// Get message count
    pub fn len(&self) -> usize {
        let messages = self.messages.lock().expect("tracer lock poisoned");
        messages.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get messages of a specific type (by name)
    pub fn messages_of_type(&self, type_name: &str) -> Vec<TracedMessage> {
        let messages = self.messages.lock().expect("tracer lock poisoned");
        messages
            .iter()
            .filter(|m| m.name() == type_name)
            .cloned()
            .collect()
    }

    /// Get messages matching a filter
    pub fn messages_matching<F>(&self, filter: F) -> Vec<TracedMessage>
    where
        F: Fn(&TracedMessage) -> bool,
    {
        let messages = self.messages.lock().expect("tracer lock poisoned");
        messages.iter().filter(|m| filter(m)).cloned().collect()
    }

    /// Get messages from a specific peer (by name)
    pub fn messages_from_peer(&self, peer_name: &str) -> Vec<TracedMessage> {
        self.messages_matching(|m| m.peer_name == peer_name && m.direction == Direction::Sent)
    }

    /// Get messages received by a specific peer (by name)
    pub fn messages_to_peer(&self, peer_name: &str) -> Vec<TracedMessage> {
        self.messages_matching(|m| m.peer_name == peer_name && m.direction == Direction::Received)
    }

    /// Get the sequence of message type names
    pub fn sequence(&self) -> Vec<&'static str> {
        let messages = self.messages.lock().expect("tracer lock poisoned");
        messages.iter().map(|m| m.name()).collect()
    }

    /// Assert that the message sequence matches expected
    ///
    /// **Panics**: If sequence doesn't match (for test assertions)
    pub fn assert_sequence(&self, expected: &[&str]) {
        let actual = self.sequence();
        assert_eq!(
            actual, expected,
            "\nMessage sequence mismatch!\nExpected: {:?}\nActual:   {:?}\n\nFull trace:\n{}",
            expected,
            actual,
            self.format_trace()
        );
    }

    /// Assert that specific messages appear in order (allows other messages in between)
    ///
    /// **Example**: `assert_contains_sequence(&["Hello", "Welcome"])` passes if
    /// Hello appears before Welcome, even if other messages are in between.
    pub fn assert_contains_sequence(&self, expected: &[&str]) {
        let messages = self.messages.lock().expect("tracer lock poisoned");
        let mut expected_iter = expected.iter();
        let mut current_expected = expected_iter.next();

        for msg in messages.iter() {
            if let Some(exp) = current_expected {
                if msg.name() == *exp {
                    current_expected = expected_iter.next();
                }
            }
        }

        if current_expected.is_some() {
            let actual = self.sequence();
            panic!(
                "\nMessage sequence missing expected messages!\nExpected to contain: {:?}\nActual sequence:      {:?}\n\nFull trace:\n{}",
                expected,
                actual,
                self.format_trace()
            );
        }
    }

    /// Assert that a message type appears exactly N times
    pub fn assert_count(&self, type_name: &str, expected: usize) {
        let actual = self.messages_of_type(type_name).len();
        assert_eq!(
            actual, expected,
            "\nMessage count mismatch for '{}'!\nExpected: {}\nActual:   {}\n\nFull trace:\n{}",
            type_name,
            expected,
            actual,
            self.format_trace()
        );
    }

    /// Assert that no messages of a specific type were sent
    pub fn assert_no_message(&self, type_name: &str) {
        self.assert_count(type_name, 0);
    }

    /// Clear all traced messages
    pub fn clear(&self) {
        let mut messages = self.messages.lock().expect("tracer lock poisoned");
        messages.clear();
    }

    /// Format trace for debugging output
    pub fn format_trace(&self) -> String {
        let messages = self.messages.lock().expect("tracer lock poisoned");
        let start = messages.first().map(|m| m.timestamp);

        messages
            .iter()
            .map(|m| {
                let elapsed = start.map(|s| m.timestamp.duration_since(s)).unwrap_or_default();
                format!(
                    "[{:>6.3}s] {} {} {} {}",
                    elapsed.as_secs_f64(),
                    m.peer_name,
                    m.direction,
                    m.name(),
                    format_message_summary(&m.message)
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Print trace to stdout (for debugging)
    pub fn dump(&self) {
        println!("\n=== Message Trace ===\n{}\n=====================", self.format_trace());
    }

    // Ephemeral Tracing

    /// Record an ephemeral datagram
    pub fn trace_ephemeral(
        &self,
        direction: Direction,
        peer_name: &str,
        page_id: &str,
        payload: &[u8],
    ) {
        let payload_json = serde_json::from_slice(payload).ok();
        let traced = TracedEphemeral {
            timestamp: Instant::now(),
            direction,
            peer_name: peer_name.to_string(),
            page_id: page_id.to_string(),
            payload_json,
            payload_raw: payload.to_vec(),
        };

        let mut ephemerals = self.ephemerals.lock().expect("tracer lock poisoned");
        ephemerals.push(traced);
    }

    /// Record a sent ephemeral
    pub fn trace_ephemeral_sent(&self, peer_name: &str, page_id: &str, payload: &[u8]) {
        self.trace_ephemeral(Direction::Sent, peer_name, page_id, payload);
    }

    /// Record a received ephemeral
    pub fn trace_ephemeral_received(&self, peer_name: &str, page_id: &str, payload: &[u8]) {
        self.trace_ephemeral(Direction::Received, peer_name, page_id, payload);
    }

    /// Get all traced ephemerals
    pub fn ephemerals(&self) -> Vec<TracedEphemeral> {
        let ephemerals = self.ephemerals.lock().expect("tracer lock poisoned");
        ephemerals.clone()
    }

    /// Get ephemerals received by a specific peer
    pub fn ephemerals_for_peer(&self, peer_name: &str) -> Vec<TracedEphemeral> {
        let ephemerals = self.ephemerals.lock().expect("tracer lock poisoned");
        ephemerals
            .iter()
            .filter(|e| e.peer_name == peer_name && e.direction == Direction::Received)
            .cloned()
            .collect()
    }

    /// Get ephemerals of a specific type (by JSON "type" field)
    pub fn ephemerals_of_type(&self, type_name: &str) -> Vec<TracedEphemeral> {
        let ephemerals = self.ephemerals.lock().expect("tracer lock poisoned");
        ephemerals
            .iter()
            .filter(|e| {
                e.payload_json
                    .as_ref()
                    .and_then(|j| j.get("type"))
                    .and_then(|t| t.as_str())
                    == Some(type_name)
            })
            .cloned()
            .collect()
    }

    /// Format ephemeral trace for debugging
    pub fn format_ephemeral_trace(&self) -> String {
        let ephemerals = self.ephemerals.lock().expect("tracer lock poisoned");
        let start = ephemerals.first().map(|e| e.timestamp);

        ephemerals
            .iter()
            .map(|e| {
                let elapsed = start.map(|s| e.timestamp.duration_since(s)).unwrap_or_default();
                let type_str = e.payload_json
                    .as_ref()
                    .and_then(|j| j.get("type"))
                    .and_then(|t| t.as_str())
                    .unwrap_or("unknown");
                format!(
                    "[{:>6.3}s] {} {} ephemeral type={} page={}",
                    elapsed.as_secs_f64(),
                    e.peer_name,
                    e.direction,
                    type_str,
                    e.page_id,
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Print ephemeral trace to stdout
    pub fn dump_ephemerals(&self) {
        println!("\n=== Ephemeral Trace ===\n{}\n=======================", self.format_ephemeral_trace());
    }
}

// Latency Metrics

/// Latency measurements for P2P operations
///
/// Collects timing data for various protocol operations to enable
/// performance analysis and regression detection.
#[derive(Debug, Default)]
pub struct LatencyMetrics {
    /// Handshake latency (Hello -> Welcome) in milliseconds
    pub handshake_ms: Vec<f64>,
    /// Sync offer to accept latency in milliseconds
    pub sync_offer_to_accept_ms: Vec<f64>,
    /// Full sync round-trip (SyncOffer -> SyncAck) in milliseconds
    pub sync_full_round_trip_ms: Vec<f64>,
    /// Message propagation latency in milliseconds
    pub message_propagation_ms: Vec<f64>,
}

impl LatencyMetrics {
    /// Create new empty metrics
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a handshake duration
    pub fn record_handshake(&mut self, duration: Duration) {
        self.handshake_ms.push(duration.as_secs_f64() * 1000.0);
    }

    /// Record a sync round-trip duration
    pub fn record_sync_round_trip(&mut self, duration: Duration) {
        self.sync_full_round_trip_ms.push(duration.as_secs_f64() * 1000.0);
    }

    /// Record a message propagation duration
    pub fn record_message_propagation(&mut self, duration: Duration) {
        self.message_propagation_ms.push(duration.as_secs_f64() * 1000.0);
    }

    /// Calculate statistics for a set of samples
    fn stats(samples: &[f64]) -> (f64, f64, f64) {
        if samples.is_empty() {
            return (0.0, 0.0, 0.0);
        }
        let avg = samples.iter().sum::<f64>() / samples.len() as f64;
        let min = samples.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = samples.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        (avg, min, max)
    }

    /// Format statistics as string
    fn format_stats(samples: &[f64]) -> String {
        if samples.is_empty() {
            return "no data".to_string();
        }
        let (avg, min, max) = Self::stats(samples);
        format!("{:.1}ms avg ({:.1}-{:.1}ms, n={})", avg, min, max, samples.len())
    }

    /// Generate formatted summary report
    pub fn summary(&self) -> String {
        format!(
            r#"
+------------------------------------------------------------+
|  Latency Report                                            |
+------------------------------------------------------------+
|  Handshake:           {:38} |
|  Sync round-trip:     {:38} |
|  Message propagation: {:38} |
+------------------------------------------------------------+"#,
            Self::format_stats(&self.handshake_ms),
            Self::format_stats(&self.sync_full_round_trip_ms),
            Self::format_stats(&self.message_propagation_ms),
        )
    }

    /// Convert to JSON-serializable format
    pub fn to_json(&self) -> serde_json::Value {
        let handshake = Self::stats(&self.handshake_ms);
        let sync = Self::stats(&self.sync_full_round_trip_ms);
        let propagation = Self::stats(&self.message_propagation_ms);

        serde_json::json!({
            "handshake_ms": {
                "avg": handshake.0,
                "min": handshake.1,
                "max": handshake.2,
                "samples": self.handshake_ms.len()
            },
            "sync_round_trip_ms": {
                "avg": sync.0,
                "min": sync.1,
                "max": sync.2,
                "samples": self.sync_full_round_trip_ms.len()
            },
            "message_propagation_ms": {
                "avg": propagation.0,
                "min": propagation.1,
                "max": propagation.2,
                "samples": self.message_propagation_ms.len()
            }
        })
    }
}

impl MessageTracer {
    /// Measure time between two message types
    ///
    /// Finds the first occurrence of `from_msg` and the last occurrence of `to_msg`,
    /// returning the duration between them.
    pub fn measure_latency(&self, from_msg: &str, to_msg: &str) -> Option<Duration> {
        let messages = self.messages.lock().expect("tracer lock poisoned");

        let from_time = messages.iter()
            .find(|m| m.name().contains(from_msg))?
            .timestamp;

        let to_time = messages.iter()
            .rev()
            .find(|m| m.name().contains(to_msg))?
            .timestamp;

        if to_time > from_time {
            Some(to_time.duration_since(from_time))
        } else {
            None
        }
    }

    /// Generate latency report from traced messages
    ///
    /// Analyzes the message trace to extract timing metrics for
    /// handshake, sync, and other protocol operations.
    pub fn latency_report(&self) -> LatencyMetrics {
        let mut metrics = LatencyMetrics::new();

        // Measure handshake latency (Hello -> Welcome)
        if let Some(d) = self.measure_latency("Hello", "Welcome") {
            metrics.record_handshake(d);
        }

        // Measure sync round-trip (SyncOffer -> SyncAck)
        if let Some(d) = self.measure_latency("SyncOffer", "SyncAck") {
            metrics.record_sync_round_trip(d);
        }

        metrics
    }
}

/// Format a message for trace output (short summary)
fn format_message_summary(msg: &Message) -> String {
    match msg {
        Message::Hello { username, .. } => format!("(user={})", username),
        Message::Welcome { node_id, .. } => format!("(node={}...)", &node_id[..8.min(node_id.len())]),
        Message::SyncOffer { page_id, layer_name, .. } => {
            format!("(page={}..., layer={})", &page_id[..8.min(page_id.len())], layer_name)
        }
        Message::SyncAccept { page_id, layer_name, .. } => {
            format!("(page={}..., layer={})", &page_id[..8.min(page_id.len())], layer_name)
        }
        Message::SyncAck { page_id, layer_name, .. } => {
            format!("(page={}..., layer={})", &page_id[..8.min(page_id.len())], layer_name)
        }
        Message::SyncReset { page_id, layer_name } => {
            format!("(page={}..., layer={})", &page_id[..8.min(page_id.len())], layer_name)
        }
        Message::SyncSnapshot { page_id, layer_name, .. } => {
            format!("(page={}..., layer={})", &page_id[..8.min(page_id.len())], layer_name)
        }
        Message::PublishSpace { space, .. } => format!("(space={})", space.name),
        Message::PublishPage { page, .. } => format!("(page={})", page.name),
        Message::Rejected { reason } => format!("(reason={})", reason),
        Message::Error { message, .. } => format!("(msg={})", message),
        _ => String::new(),
    }
}

// Tests

#[cfg(test)]
mod tests {
    use super::*;
    use transport::mock_node_id;

    fn make_hello() -> Message {
        Message::Hello {
            did: "did:key:test".to_string(),
            username: "alice".to_string(),
            public_key: [1u8; 32],
            encryption_key: [2u8; 32],
            signature: vec![],
            timestamp: 0,
            permit: "permit".to_string(),
        }
    }

    fn make_welcome() -> Message {
        Message::Welcome {
            node_id: "node123".to_string(),
            node_public_key: [1u8; 32],
            node_encryption_key: [2u8; 32],
            signature: vec![],
            timestamp: 0,
            permit_for_peer: "permit".to_string(),
        }
    }

    #[test]
    fn test_trace_and_retrieve() {
        let tracer = MessageTracer::new();
        let from = mock_node_id("alice");
        let to = mock_node_id("bob");

        tracer.trace_sent(from, to, make_hello(), "alice");
        tracer.trace_received(to, from, make_welcome(), "alice");

        assert_eq!(tracer.len(), 2);
        assert_eq!(tracer.sequence(), vec!["Hello", "Welcome"]);
    }

    #[test]
    fn test_assert_sequence() {
        let tracer = MessageTracer::new();
        let from = mock_node_id("alice");
        let to = mock_node_id("bob");

        tracer.trace_sent(from, to, make_hello(), "alice");
        tracer.trace_received(to, from, make_welcome(), "alice");

        // Should pass
        tracer.assert_sequence(&["Hello", "Welcome"]);
    }

    #[test]
    #[should_panic(expected = "Message sequence mismatch")]
    fn test_assert_sequence_fails() {
        let tracer = MessageTracer::new();
        let from = mock_node_id("alice");
        let to = mock_node_id("bob");

        tracer.trace_sent(from, to, make_hello(), "alice");

        // Should fail - missing Welcome
        tracer.assert_sequence(&["Hello", "Welcome"]);
    }

    #[test]
    fn test_assert_contains_sequence() {
        let tracer = MessageTracer::new();
        let from = mock_node_id("alice");
        let to = mock_node_id("bob");

        tracer.trace_sent(from, to, make_hello(), "alice");
        tracer.trace_sent(from, to, Message::Ack, "alice");
        tracer.trace_received(to, from, make_welcome(), "alice");

        // Should pass - Hello before Welcome, even with Ack in between
        tracer.assert_contains_sequence(&["Hello", "Welcome"]);
    }

    #[test]
    fn test_messages_of_type() {
        let tracer = MessageTracer::new();
        let from = mock_node_id("alice");
        let to = mock_node_id("bob");

        tracer.trace_sent(from, to, make_hello(), "alice");
        tracer.trace_sent(from, to, Message::Ack, "alice");
        tracer.trace_sent(from, to, Message::Ack, "alice");

        let acks = tracer.messages_of_type("Ack");
        assert_eq!(acks.len(), 2);

        let hellos = tracer.messages_of_type("Hello");
        assert_eq!(hellos.len(), 1);
    }

    #[test]
    fn test_clear() {
        let tracer = MessageTracer::new();
        let from = mock_node_id("alice");
        let to = mock_node_id("bob");

        tracer.trace_sent(from, to, make_hello(), "alice");
        assert_eq!(tracer.len(), 1);

        tracer.clear();
        assert!(tracer.is_empty());
    }

    #[test]
    fn test_format_trace() {
        let tracer = MessageTracer::new();
        let from = mock_node_id("alice");
        let to = mock_node_id("bob");

        tracer.trace_sent(from, to, make_hello(), "alice");

        let trace = tracer.format_trace();
        assert!(trace.contains("alice"));
        assert!(trace.contains("Hello"));
    }
}
