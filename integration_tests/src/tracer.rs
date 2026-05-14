//! Thin wrapper around courier::MessageTrace for test assertions
//!
//! Collects protocol messages from the courier trace channel and provides
//! assertion methods for verifying message sequences.

use std::sync::{Arc, Mutex};

use courier::trace::{MessageTrace, TraceDirection};
use tokio::sync::mpsc;

/// Protocol message tracer for integration tests
///
/// Create with `Tracer::new()`, pass the sender to Coordinator,
/// then use assertion methods after running protocol operations.
#[derive(Clone)]
pub struct Tracer {
    messages: Arc<Mutex<Vec<MessageTrace>>>,
}

impl Tracer {
    /// Create a new tracer and its sender channel
    ///
    /// Pass the sender to Coordinator's Arguments tuple.
    /// The tracer spawns a background task to collect messages.
    pub fn new() -> (Self, mpsc::UnboundedSender<MessageTrace>) {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let messages = Arc::new(Mutex::new(Vec::new()));
        let messages_clone = messages.clone();

        // Spawn collector task
        tokio::spawn(async move {
            while let Some(trace) = rx.recv().await {
                let mut msgs = messages_clone.lock().expect("tracer lock poisoned");
                msgs.push(trace);
            }
        });

        (Self { messages }, tx)
    }

    /// Get all collected messages
    pub fn messages(&self) -> Vec<MessageTrace> {
        let msgs = self.messages.lock().expect("tracer lock poisoned");
        msgs.clone()
    }

    /// Get message type name sequence
    pub fn sequence(&self) -> Vec<&'static str> {
        let msgs = self.messages.lock().expect("tracer lock poisoned");
        msgs.iter().map(|m| m.msg_name).collect()
    }

    /// Assert exact message sequence matches
    pub fn assert_sequence(&self, expected: &[&str]) {
        let actual = self.sequence();
        assert_eq!(
            actual,
            expected,
            "\nMessage sequence mismatch!\nExpected: {:?}\nActual:   {:?}\n\nFull trace:\n{}",
            expected,
            actual,
            self.format_trace()
        );
    }

    /// Assert expected messages appear in order (other messages allowed in between)
    pub fn assert_contains_sequence(&self, expected: &[&str]) {
        let msgs = self.messages.lock().expect("tracer lock poisoned");
        let mut expected_iter = expected.iter();
        let mut current = expected_iter.next();

        for msg in msgs.iter() {
            if let Some(exp) = current {
                if msg.msg_name == *exp {
                    current = expected_iter.next();
                }
            }
        }

        if current.is_some() {
            let actual = self.sequence();
            panic!(
                "\nExpected subsequence not found!\nExpected: {:?}\nActual:   {:?}\n\nFull trace:\n{}",
                expected, actual, self.format_trace()
            );
        }
    }

    /// Assert a message type appears exactly N times
    pub fn assert_count(&self, msg_type: &str, expected: usize) {
        let actual = self.messages_of_type(msg_type).len();
        assert_eq!(
            actual,
            expected,
            "\nCount mismatch for '{}'! Expected: {}, Actual: {}\n\nFull trace:\n{}",
            msg_type,
            expected,
            actual,
            self.format_trace()
        );
    }

    /// Get messages of a specific type
    pub fn messages_of_type(&self, type_name: &str) -> Vec<MessageTrace> {
        let msgs = self.messages.lock().expect("tracer lock poisoned");
        msgs.iter()
            .filter(|m| m.msg_name == type_name)
            .cloned()
            .collect()
    }

    /// Format trace for debug output
    pub fn format_trace(&self) -> String {
        let msgs = self.messages.lock().expect("tracer lock poisoned");
        let start = msgs.first().map(|m| m.timestamp);

        msgs.iter()
            .map(|m| {
                let elapsed = start
                    .map(|s| m.timestamp.duration_since(s))
                    .unwrap_or_default();
                let dir = match m.direction {
                    TraceDirection::Sent => "->",
                    TraceDirection::Received => "<-",
                };
                format!(
                    "[{:>6.3}s] {} {} (node={}, peer={})",
                    elapsed.as_secs_f64(),
                    dir,
                    m.msg_name,
                    m.node_id,
                    m.peer_node_id,
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Print trace to stdout
    pub fn dump(&self) {
        println!(
            "\n=== Message Trace ===\n{}\n=====================",
            self.format_trace()
        );
    }
}
