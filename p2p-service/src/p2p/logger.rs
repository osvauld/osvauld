use std::str::FromStr;
use thiserror::Error;
use tracing::Level;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

/// Errors that can occur during logging setup
#[derive(Error, Debug)]
pub enum LoggingError {
    #[error("Log directory and file prefix must be specified when log_to_file is true")]
    MissingFileConfig,

    #[error("At least one logging destination must be enabled")]
    NoDestinationEnabled,

    #[error("Failed to set global subscriber: {0}")]
    SubscriberError(String),

    #[error("Failed to initialize log tracer: {0}")]
    LogTracerError(#[from] log::SetLoggerError),
}

/// Logging configuration for the P2P service
pub struct LogConfig {
    /// Log level to use for the P2P service
    pub level: Level,
    /// Whether to log to a file
    pub log_to_file: bool,
    /// Directory to store log files
    pub log_dir: Option<String>,
    /// Prefix for log files
    pub file_prefix: Option<String>,
    /// Whether to log to stdout
    pub log_to_stdout: bool,
    /// Whether to show line numbers in logs
    pub show_line_numbers: bool,
    /// Whether to include spans in logs
    pub log_spans: bool,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            level: Level::DEBUG,
            log_to_file: false,
            log_dir: None,
            file_prefix: None,
            log_to_stdout: true,
            show_line_numbers: true,
            log_spans: true,
        }
    }
}

/// Initialize the tracing subscriber for the P2P service
pub fn init_tracing(config: LogConfig) -> Result<Option<WorkerGuard>, LoggingError> {
    // Determine span events format
    let span_events = if config.log_spans {
        fmt::format::FmtSpan::NEW | fmt::format::FmtSpan::CLOSE
    } else {
        fmt::format::FmtSpan::NONE
    };

    // Configure separate subscribers for file and stdout if needed
    let mut guard = None;

    // Helper function to create a filter based on the config
    let create_filter = || {
        let filter_string = format!(
            "p2p_service={},osvauld_services={}",
            config.level, config.level
        );
        EnvFilter::from_str(&filter_string).unwrap_or_else(|_| EnvFilter::new(filter_string))
    };

    // If we're logging to file and stdout
    if config.log_to_file && config.log_to_stdout {
        if let (Some(dir), Some(prefix)) = (config.log_dir.as_ref(), config.file_prefix.as_ref()) {
            let file_appender = tracing_appender::rolling::daily(dir, prefix);
            let (file_writer, file_guard) = tracing_appender::non_blocking(file_appender);

            // Create a registry with multiple layers
            let subscriber = tracing_subscriber::registry()
                // File layer
                .with(
                    fmt::Layer::new()
                        .with_writer(file_writer)
                        .with_ansi(false)
                        .with_span_events(span_events.clone())
                        .with_line_number(config.show_line_numbers)
                        .with_thread_names(true)
                        .with_thread_ids(true)
                        .with_filter(create_filter()),
                )
                // Stdout layer
                .with(
                    fmt::Layer::new()
                        .with_ansi(true)
                        .with_span_events(span_events.clone())
                        .with_line_number(config.show_line_numbers)
                        .with_thread_names(true)
                        .with_thread_ids(true)
                        .pretty()
                        .with_filter(create_filter()),
                );

            tracing::subscriber::set_global_default(subscriber)
                .map_err(|e| LoggingError::SubscriberError(e.to_string()))?;

            guard = Some(file_guard);
        } else {
            return Err(LoggingError::MissingFileConfig);
        }
    }
    // Only file logging
    else if config.log_to_file {
        if let (Some(dir), Some(prefix)) = (config.log_dir.as_ref(), config.file_prefix.as_ref()) {
            let file_appender = tracing_appender::rolling::daily(dir, prefix);
            let (file_writer, file_guard) = tracing_appender::non_blocking(file_appender);

            let subscriber = fmt::Subscriber::builder()
                .with_env_filter(create_filter())
                .with_ansi(false)
                .with_span_events(span_events)
                .with_line_number(config.show_line_numbers)
                .with_thread_names(true)
                .with_thread_ids(true)
                .with_writer(file_writer)
                .finish();

            tracing::subscriber::set_global_default(subscriber)
                .map_err(|e| LoggingError::SubscriberError(e.to_string()))?;

            guard = Some(file_guard);
        } else {
            return Err(LoggingError::MissingFileConfig);
        }
    }
    // Only stdout logging
    else if config.log_to_stdout {
        let subscriber = fmt::Subscriber::builder()
            .with_env_filter(create_filter())
            .with_ansi(true)
            .with_span_events(span_events)
            .with_line_number(config.show_line_numbers)
            .with_thread_names(true)
            .with_thread_ids(true)
            .pretty()
            .finish();

        tracing::subscriber::set_global_default(subscriber)
            .map_err(|e| LoggingError::SubscriberError(e.to_string()))?;
    } else {
        return Err(LoggingError::NoDestinationEnabled);
    }

    // Initialize the global logger to forward log events to tracing
    tracing_log::LogTracer::init()?;

    Ok(guard)
}

/// Simple function to initialize logging with default settings
pub fn init_default_tracing() -> Result<Option<WorkerGuard>, LoggingError> {
    init_tracing(LogConfig::default())
}
