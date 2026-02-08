use std::fmt::{Display, Formatter};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};
use tracing_tree::HierarchicalLayer;

// ID Shortening Utilities

/// Wrapper for shortening long IDs in logs
///
/// Turns `00c02ee56d990e0a0a1ce0f12f963df121efb6efc51096ae324f5b9bf4bfa4c9`
/// into `00c02e..a4c9` for cleaner log output.
///
/// # Example
/// ```rust
/// use tracing::info;
/// use logging_utils::Short;
///
/// let node_id = "00c02ee56d990e0a0a1ce0f12f963df121efb6efc51096ae324f5b9bf4bfa4c9";
/// info!(node = %Short(node_id), "Connected");
/// // Output: node=00c02e..a4c9 Connected
/// ```
pub struct Short<'a>(pub &'a str);

impl Display for Short<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let s = self.0;
        if s.len() > 16 {
            // Show first 6 and last 4 characters
            write!(f, "{}..{}", &s[..6], &s[s.len()-4..])
        } else {
            write!(f, "{}", s)
        }
    }
}

/// Shorten a DID for logging
///
/// Turns `did:key:z6MkpYKu1RW6oXaT95YYXM2W9jvEU4PsRp9wGGGxMXfZcvwU`
/// into `did:..vwU` for cleaner log output.
///
/// # Example
/// ```rust
/// use tracing::info;
/// use logging_utils::ShortDid;
///
/// let did = "did:key:z6MkpYKu1RW6oXaT95YYXM2W9jvEU4PsRp9wGGGxMXfZcvwU";
/// info!(peer = %ShortDid(did), "Peer connected");
/// // Output: peer=did:..vwU Peer connected
/// ```
pub struct ShortDid<'a>(pub &'a str);

impl Display for ShortDid<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let s = self.0;
        if s.starts_with("did:") && s.len() > 12 {
            // Show "did:.." + last 4 characters
            write!(f, "did:..{}", &s[s.len()-4..])
        } else if s.len() > 16 {
            write!(f, "{}..{}", &s[..6], &s[s.len()-4..])
        } else {
            write!(f, "{}", s)
        }
    }
}

/// Shorten a layer name for logging
///
/// Turns `abc123/orders/did:key:z6MkpYKu1RW6oXaT95YYXM2W9jvEU4PsRp9wGGGxMXfZcvwU`
/// into `abc123/orders/did:..vwU` for cleaner log output.
pub struct ShortLayer<'a>(pub &'a str);

impl Display for ShortLayer<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let s = self.0;
        // If layer contains a DID segment, shorten it
        if let Some(did_pos) = s.find("did:") {
            let prefix = &s[..did_pos];
            let did_part = &s[did_pos..];
            if did_part.len() > 12 {
                write!(f, "{}did:..{}", prefix, &did_part[did_part.len()-4..])
            } else {
                write!(f, "{}", s)
            }
        } else if s.len() > 24 {
            // Just truncate long layer names
            write!(f, "{}..{}", &s[..12], &s[s.len()-8..])
        } else {
            write!(f, "{}", s)
        }
    }
}

/// Shorten any Display-able value for logging
///
/// Convenience function for use in tracing macros.
///
/// # Example
/// ```rust
/// use tracing::info;
/// use logging_utils::short;
///
/// let node_id = some_node_id; // implements Display
/// info!(node = %short(&node_id), "Connected");
/// ```
pub fn short<T: Display>(value: &T) -> String {
    let s = value.to_string();
    if s.len() > 16 {
        format!("{}..{}", &s[..6], &s[s.len()-4..])
    } else {
        s
    }
}

/// Shorten a DID string
pub fn short_did(did: &str) -> String {
    if did.starts_with("did:") && did.len() > 12 {
        format!("did:..{}", &did[did.len()-4..])
    } else if did.len() > 16 {
        format!("{}..{}", &did[..6], &did[did.len()-4..])
    } else {
        did.to_string()
    }
}

/// A log entry for debug capture
#[derive(Debug, Clone)]
pub struct DebugLogEntry {
    pub ts: String,
    pub level: String,
    pub target: String,
    pub msg: String,
    pub instance: Option<String>,
}

// Debug Capture Layer

use tracing::{Event, Subscriber};
use tracing_subscriber::layer::Context;
use tracing_subscriber::Layer;

/// A tracing layer that captures log events and sends them to a broadcast channel
///
/// This layer intercepts all log events and sends them as `DebugLogEntry` to
/// a tokio broadcast channel, allowing external consumers (like ai_interface)
/// to access protocol-level logs from all libs.
pub struct DebugCaptureLayer {
    tx: tokio::sync::broadcast::Sender<DebugLogEntry>,
    instance_name: Option<String>,
}

impl DebugCaptureLayer {
    pub fn new(
        tx: tokio::sync::broadcast::Sender<DebugLogEntry>,
        instance_name: Option<String>,
    ) -> Self {
        Self { tx, instance_name }
    }
}

impl<S> Layer<S> for DebugCaptureLayer
where
    S: Subscriber,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let metadata = event.metadata();
        let level = metadata.level().to_string();
        let target = metadata.target().to_string();

        // Extract message from event fields
        let mut msg = String::new();
        let mut visitor = MessageVisitor(&mut msg);
        event.record(&mut visitor);

        let entry = DebugLogEntry {
            ts: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
            level,
            target,
            msg,
            instance: self.instance_name.clone(),
        };

        // Send to channel - ignore errors (no receivers)
        let _ = self.tx.send(entry);
    }
}

/// Visitor to extract message field from tracing events
struct MessageVisitor<'a>(&'a mut String);

impl<'a> tracing::field::Visit for MessageVisitor<'a> {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            *self.0 = format!("{:?}", value);
        } else if self.0.is_empty() {
            // Capture first field if no message
            self.0.push_str(&format!("{}={:?}", field.name(), value));
        } else {
            self.0.push_str(&format!(" {}={:?}", field.name(), value));
        }
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "message" {
            *self.0 = value.to_string();
        } else if self.0.is_empty() {
            self.0.push_str(&format!("{}={}", field.name(), value));
        } else {
            self.0.push_str(&format!(" {}={}", field.name(), value));
        }
    }
}

/// Configuration for the logging system
#[derive(Clone)]
pub struct LogConfig {
    /// Log level (debug, info, warn, error)
    pub level: String,

    /// Enable file logging
    pub log_to_file: bool,
    pub log_dir: Option<String>,
    pub file_prefix: Option<String>,

    /// Enable stdout logging
    pub log_to_stdout: bool,

    /// Use hierarchical tree format (recommended for development)
    pub use_tree_format: bool,

    /// Show detailed span events (enter/exit) - creates more verbose output
    pub show_span_events: bool,

    /// Show line numbers and file locations in tree mode
    pub show_source_location: bool,

    /// Show thread information
    pub show_thread_info: bool,

    /// Output JSON to stdout instead of tree/pretty format
    /// Useful for machine parsing (ai_interface reads JSON log lines)
    pub stdout_json: bool,

    /// Optional broadcast sender for debug log capture
    /// When set, all logs are also sent to this channel for debug server access
    pub debug_tx: Option<tokio::sync::broadcast::Sender<DebugLogEntry>>,

    /// Instance name for debug logs
    pub instance_name: Option<String>,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            level: "debug".to_string(),
            log_to_file: false,
            log_dir: None,
            file_prefix: None,
            log_to_stdout: true,
            use_tree_format: true,       // Enable tree format by default
            show_span_events: false,     // Less noise by default
            show_source_location: false, // Available in tree mode when needed
            show_thread_info: false,     // Show when debugging multi-threading
            stdout_json: false,          // Tree format by default, not JSON
            debug_tx: None,              // No debug capture by default
            instance_name: None,         // No instance name by default
        }
    }
}

/// Initialize rich tracing with tree formatting
///
/// This sets up a hierarchical logging system with optional file output.
/// The tree format shows nested function calls with indentation, making it
/// easy to trace execution flow through async code and multiple services.
///
/// When `debug_tx` is provided in config, all logs are also sent to that
/// broadcast channel for external consumers (like ai_interface).
///
/// # Example Output
/// ```text
/// 🎯 handle_add_folder name="My Folder"
///   ├─📁 create_folder folder_id="f_abc123"
///   │ ├─🔐 issue_folder_owner_token
///   │ │ └─✓ 8.7ms
///   │ └─✓ 45.6ms
///   └─✓ 50.1ms
/// ```
pub fn init_rich_tracing(
    config: LogConfig,
) -> Result<Option<WorkerGuard>, Box<dyn std::error::Error>> {
    let mut guard = None;

    // Create filter with suppressed third-party noise
    let filter = EnvFilter::try_from_default_env().or_else(|_| {
        EnvFilter::try_new(&format!(
            "{},\
             hyper=warn,\
             hyper_util=warn,\
             rustls=error,\
             iroh=warn,\
             iroh::net_report=off,\
             iroh::net_report::report=off,\
             quinn=warn,\
             netlink_proto=error,\
             netlink_sys=error,\
             netlink_packet_route=error,\
             hickory_proto=error,\
             hickory_resolver=error,\
             hickory_client=error,\
             portmapper=warn,\
             tokio_tungstenite=warn,\
             tungstenite=warn,\
             netwatch=warn,\
             iroh_quinn_proto::connection=warn,\
             iroh_relay=warn,\
             igd_next::aio::tokio=warn,\
             tantivy=warn,\
             reqwest=warn,\
             asset_protocol=warn,\
             loro_internal=off,\
             loro_internal::oplog=off,\
             loro_internal::oplog::change_store=off,\
             loro_internal::oplog::change_store::block_encode=off,\
             loro_kv_store=off,\
             wgpu_core=warn,\
             wgpu_hal=off,\
             wgpu_hal::gles=off,\
             winit=warn,\
             naga=warn",
            config.level
        ))
    })?;

    // Create optional debug capture layer (sends logs to broadcast channel)
    let debug_layer = config.debug_tx.map(|tx| {
        DebugCaptureLayer::new(tx, config.instance_name.clone())
    });

    let registry = tracing_subscriber::registry().with(filter).with(debug_layer);

    // JSON stdout mode (for ai_interface to parse)
    if config.log_to_stdout && config.stdout_json {
        let json_layer = fmt::Layer::new()
            .json()
            .with_ansi(false);

        // Also add file layer if configured
        if config.log_to_file {
            if let (Some(dir), Some(prefix)) = (config.log_dir.as_ref(), config.file_prefix.as_ref())
            {
                let file_appender = tracing_appender::rolling::daily(dir, prefix);
                let (file_writer, file_guard) = tracing_appender::non_blocking(file_appender);

                let file_layer = fmt::Layer::new()
                    .with_writer(file_writer)
                    .json()
                    .with_ansi(false);

                let _ = registry.with(json_layer).with(file_layer).try_init();
                guard = Some(file_guard);
            } else {
                return Err("Log directory and file prefix required when log_to_file is true".into());
            }
        } else {
            let _ = registry.with(json_layer).try_init();
        }
    }
    // Tree layer for stdout (hierarchical, beautiful)
    else if config.log_to_stdout && config.use_tree_format {
        let tree_layer = HierarchicalLayer::new(2)
            .with_targets(true)  // Always show targets (lib/module prefix like "gurkha::parser")
            .with_bracketed_fields(true)
            .with_indent_lines(true)
            .with_indent_amount(2)
            .with_thread_ids(config.show_thread_info)
            .with_thread_names(config.show_thread_info)
            .with_verbose_exit(config.show_span_events)
            .with_verbose_entry(config.show_span_events)
            .with_ansi(true);

        // File layer (JSON structured for parsing)
        if config.log_to_file {
            if let (Some(dir), Some(prefix)) = (config.log_dir.as_ref(), config.file_prefix.as_ref())
            {
                let file_appender = tracing_appender::rolling::daily(dir, prefix);
                let (file_writer, file_guard) = tracing_appender::non_blocking(file_appender);

                let file_layer = fmt::Layer::new()
                    .with_writer(file_writer)
                    .json() // Structured JSON for log analysis
                    .with_ansi(false);

                let _ = registry.with(tree_layer).with(file_layer).try_init();
                guard = Some(file_guard);
            } else {
                return Err("Log directory and file prefix required when log_to_file is true".into());
            }
        } else {
            let _ = registry.with(tree_layer).try_init();
        }
    }
    // Fallback: pretty format without tree
    else if config.log_to_stdout {
        let stdout_layer = fmt::Layer::new()
            .pretty()
            .with_ansi(true)
            .with_line_number(config.show_source_location)
            .with_thread_ids(config.show_thread_info)
            .with_thread_names(config.show_thread_info);

        if config.log_to_file {
            if let (Some(dir), Some(prefix)) = (config.log_dir.as_ref(), config.file_prefix.as_ref())
            {
                let file_appender = tracing_appender::rolling::daily(dir, prefix);
                let (file_writer, file_guard) = tracing_appender::non_blocking(file_appender);

                let file_layer = fmt::Layer::new()
                    .with_writer(file_writer)
                    .json()
                    .with_ansi(false);

                let _ = registry.with(stdout_layer).with(file_layer).try_init();
                guard = Some(file_guard);
            } else {
                return Err("Log directory and file prefix required when log_to_file is true".into());
            }
        } else {
            let _ = registry.with(stdout_layer).try_init();
        }
    } else if config.log_to_file {
        if let (Some(dir), Some(prefix)) = (config.log_dir.as_ref(), config.file_prefix.as_ref()) {
            let file_appender = tracing_appender::rolling::daily(dir, prefix);
            let (file_writer, file_guard) = tracing_appender::non_blocking(file_appender);

            let file_layer = fmt::Layer::new()
                .with_writer(file_writer)
                .json()
                .with_ansi(false);

            let _ = registry.with(file_layer).try_init();
            guard = Some(file_guard);
        } else {
            return Err("Log directory and file prefix required when log_to_file is true".into());
        }
    } else {
        return Err("At least one logging destination must be enabled".into());
    }

    // Forward log crate to tracing (for legacy log:: calls)
    // Use try_init to allow multiple initializations (won't error if already set)
    let _ = tracing_log::LogTracer::init();

    Ok(guard)
}

/// Quick init for development (tree format, debug level, stdout only)
///
/// Use this in kunki and sthalam during development for beautiful hierarchical logs.
pub fn init_dev() -> Result<Option<WorkerGuard>, Box<dyn std::error::Error>> {
    init_rich_tracing(LogConfig::default())
}

/// Production config (file + stdout, info level, tree format)
///
/// Logs to both file (JSON structured) and stdout (tree format) for production use.
pub fn init_prod(log_dir: &str) -> Result<Option<WorkerGuard>, Box<dyn std::error::Error>> {
    init_rich_tracing(LogConfig {
        level: "info".to_string(),
        log_to_file: true,
        log_dir: Some(log_dir.to_string()),
        file_prefix: Some("osvauld".to_string()),
        use_tree_format: true,
        ..Default::default()
    })
}
