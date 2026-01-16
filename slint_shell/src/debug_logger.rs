//! Debug Logger - Tracing Layer for Log Broadcasting
//!
//! Captures tracing events and broadcasts them to debug server clients.

use crate::debug_server::LogEntry;
use tokio::sync::broadcast;
use tracing::field::{Field, Visit};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::layer::Context;
use tracing_subscriber::Layer;

/// A tracing Layer that broadcasts log entries to debug clients
pub struct DebugLogLayer {
    log_tx: broadcast::Sender<LogEntry>,
    instance_name: String,
}

impl DebugLogLayer {
    /// Create a new debug log layer
    pub fn new(log_tx: broadcast::Sender<LogEntry>, instance_name: String) -> Self {
        Self {
            log_tx,
            instance_name,
        }
    }
}

impl<S> Layer<S> for DebugLogLayer
where
    S: Subscriber,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let metadata = event.metadata();

        // Extract message from event
        let mut visitor = MessageVisitor::default();
        event.record(&mut visitor);

        let entry = LogEntry {
            ts: chrono::Utc::now().to_rfc3339(),
            level: level_to_string(metadata.level()),
            target: metadata.target().to_string(),
            msg: visitor.message.unwrap_or_default(),
            instance: Some(self.instance_name.clone()),
        };

        // Broadcast - ignore errors if no receivers
        let _ = self.log_tx.send(entry);
    }
}

/// Visitor to extract message field from tracing events
#[derive(Default)]
struct MessageVisitor {
    message: Option<String>,
}

impl Visit for MessageVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.message = Some(format!("{:?}", value));
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.message = Some(value.to_string());
        }
    }
}

fn level_to_string(level: &Level) -> String {
    match *level {
        Level::ERROR => "error".to_string(),
        Level::WARN => "warn".to_string(),
        Level::INFO => "info".to_string(),
        Level::DEBUG => "debug".to_string(),
        Level::TRACE => "trace".to_string(),
    }
}

/// Create a debug log layer connected to a debug server
pub fn create_debug_layer(
    log_tx: broadcast::Sender<LogEntry>,
    instance_name: String,
) -> DebugLogLayer {
    DebugLogLayer::new(log_tx, instance_name)
}
