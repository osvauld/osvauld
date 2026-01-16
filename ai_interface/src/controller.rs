//! Test Controller - Instance management and command routing
//!
//! Spawns app instances, connects to debug servers, routes commands.
//! Aggregates logs from all instances with regex filtering.

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::process::Stdio;

use regex::Regex;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, RwLock};
use std::sync::Arc;

use crate::protocol::{LogEntry, Request, Response};

// =============================================================================
// Log Filter and Aggregator
// =============================================================================

/// JSON log entry as output by tracing-subscriber
#[derive(Debug, serde::Deserialize)]
struct TracingLogEntry {
    timestamp: String,
    level: String,
    target: String,
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    fields: Option<serde_json::Value>,
}

impl TracingLogEntry {
    /// Convert to our LogEntry format
    fn to_log_entry(self, instance: String) -> LogEntry {
        let msg = self.message.unwrap_or_else(|| {
            // Try to extract message from fields
            self.fields
                .and_then(|f| f.get("message").and_then(|m| m.as_str()).map(|s| s.to_string()))
                .unwrap_or_default()
        });

        LogEntry {
            ts: self.timestamp,
            level: self.level,
            target: self.target,
            msg,
            instance: Some(instance),
        }
    }
}

/// Log filter for subscription
#[derive(Debug, Clone)]
pub struct LogFilter {
    /// Filter by log level (info, warn, error, etc.)
    pub level: Option<String>,
    /// Filter by target module (regex)
    pub target: Option<Regex>,
    /// Filter by message content (regex)
    pub message: Option<Regex>,
    /// Filter by instance names
    pub instances: Vec<String>,
}

impl LogFilter {
    /// Check if a log entry matches this filter
    pub fn matches(&self, entry: &LogEntry) -> bool {
        // Check instance
        if !self.instances.is_empty() {
            let instance = entry.instance.as_deref().unwrap_or("");
            if !self.instances.iter().any(|i| i == instance) {
                return false;
            }
        }

        // Check level
        if let Some(ref level) = self.level {
            if !entry.level.eq_ignore_ascii_case(level) {
                return false;
            }
        }

        // Check target regex
        if let Some(ref re) = self.target {
            if !re.is_match(&entry.target) {
                return false;
            }
        }

        // Check message regex
        if let Some(ref re) = self.message {
            if !re.is_match(&entry.msg) {
                return false;
            }
        }

        true
    }

    /// Create filter from request params
    pub fn from_params(params: &serde_json::Value) -> Result<Self, String> {
        let obj = params.as_object();

        let level = obj
            .and_then(|p| p.get("level"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let target = obj
            .and_then(|p| p.get("target"))
            .and_then(|v| v.as_str())
            .map(|s| Regex::new(s))
            .transpose()
            .map_err(|e| format!("Invalid target regex: {}", e))?;

        let message = obj
            .and_then(|p| p.get("message"))
            .and_then(|v| v.as_str())
            .map(|s| Regex::new(s))
            .transpose()
            .map_err(|e| format!("Invalid message regex: {}", e))?;

        let instances = obj
            .and_then(|p| p.get("instances"))
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        Ok(Self {
            level,
            target,
            message,
            instances,
        })
    }
}

/// Log aggregator - collects logs from all instances
pub struct LogAggregator {
    /// Ring buffer of recent logs
    buffer: VecDeque<LogEntry>,
    /// Maximum buffer size
    max_size: usize,
    /// Active log filter for subscription
    filter: Option<LogFilter>,
    /// Subscription active flag
    subscribed: bool,
}

impl LogAggregator {
    pub fn new(max_size: usize) -> Self {
        Self {
            buffer: VecDeque::with_capacity(max_size),
            max_size,
            filter: None,
            subscribed: false,
        }
    }

    /// Add a log entry to the buffer
    pub fn push(&mut self, entry: LogEntry) {
        if self.buffer.len() >= self.max_size {
            self.buffer.pop_front();
        }
        self.buffer.push_back(entry);
    }

    /// Get recent logs with optional filter
    pub fn get_logs(&self, count: usize, filter: Option<&LogFilter>) -> Vec<&LogEntry> {
        self.buffer
            .iter()
            .rev()
            .filter(|e| filter.map_or(true, |f| f.matches(e)))
            .take(count)
            .collect()
    }

    /// Set subscription filter
    pub fn subscribe(&mut self, filter: Option<LogFilter>) {
        self.filter = filter;
        self.subscribed = true;
    }

    /// Clear subscription
    pub fn unsubscribe(&mut self) {
        self.filter = None;
        self.subscribed = false;
    }

    /// Check if subscribed
    pub fn is_subscribed(&self) -> bool {
        self.subscribed
    }

    /// Check if a log entry matches the current subscription filter
    pub fn matches_subscription(&self, entry: &LogEntry) -> bool {
        if !self.subscribed {
            return false;
        }
        self.filter.as_ref().map_or(true, |f| f.matches(entry))
    }
}

// =============================================================================
// Test Configuration and Instance Management
// =============================================================================

/// Configuration for test controller
#[derive(Debug, Clone)]
pub struct TestConfig {
    /// Instance names (e.g., ["owner", "customer1", "customer2"])
    pub instances: Vec<String>,
    /// Whether to spawn kunki node
    pub spawn_node: bool,
    /// Directory for database files
    pub db_dir: PathBuf,
    /// Path to slint_shell binary
    pub shell_binary: PathBuf,
    /// Path to kunki binary (optional)
    pub node_binary: Option<PathBuf>,
    /// Show UI windows (instead of headless testing backend)
    pub show_ui: bool,
}

/// Connection to a debug server
struct DebugConnection {
    /// Instance name
    name: String,
    /// Unix socket stream
    stream: UnixStream,
    /// Socket path
    socket_path: PathBuf,
}

impl DebugConnection {
    async fn connect(name: String, socket_path: PathBuf) -> Result<Self, std::io::Error> {
        // Retry connection with backoff
        let mut attempts = 0;
        let stream = loop {
            match UnixStream::connect(&socket_path).await {
                Ok(s) => break s,
                Err(_) if attempts < 10 => {
                    attempts += 1;
                    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
                    tracing::debug!(
                        instance = %name,
                        attempt = attempts,
                        "Retrying debug server connection"
                    );
                }
                Err(e) => return Err(e),
            }
        };

        tracing::info!(
            instance = %name,
            socket = %socket_path.display(),
            "Connected to debug server"
        );

        Ok(Self {
            name,
            stream,
            socket_path,
        })
    }

    async fn send_command(&mut self, cmd: &str) -> Result<String, std::io::Error> {
        // Send command with newline
        self.stream.write_all(cmd.as_bytes()).await?;
        self.stream.write_all(b"\n").await?;
        self.stream.flush().await?;

        // Read response (single line)
        let mut buf_reader = BufReader::new(&mut self.stream);
        let mut response = String::new();
        buf_reader.read_line(&mut response).await?;

        Ok(response.trim().to_string())
    }
}

/// Spawned instance info
struct Instance {
    name: String,
    process: Child,
    socket_path: PathBuf,
    db_path: PathBuf,
}

/// Test controller - orchestrates instances and routes commands
pub struct TestController {
    /// Instance configurations
    config: TestConfig,
    /// Spawned instances
    instances: HashMap<String, Instance>,
    /// Debug server connections
    connections: HashMap<String, DebugConnection>,
    /// Log aggregator (receives logs from all instance stdouts)
    log_aggregator: Arc<RwLock<LogAggregator>>,
    /// Channel to receive logs from stdout reader tasks
    log_rx: mpsc::Receiver<LogEntry>,
    /// Sender cloned to stdout reader tasks
    log_tx: mpsc::Sender<LogEntry>,
}

impl TestController {
    /// Create a new test controller
    pub fn new(config: TestConfig) -> Self {
        let (log_tx, log_rx) = mpsc::channel(10000);
        Self {
            config,
            instances: HashMap::new(),
            connections: HashMap::new(),
            log_aggregator: Arc::new(RwLock::new(LogAggregator::new(10000))),
            log_rx,
            log_tx,
        }
    }

    /// Spawn all configured instances
    pub async fn spawn_instances(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Create DB directory
        std::fs::create_dir_all(&self.config.db_dir)?;

        // Spawn shell instances
        for name in &self.config.instances.clone() {
            self.spawn_shell_instance(name.clone()).await?;
        }

        // Spawn kunki node if configured
        if self.config.spawn_node {
            self.spawn_kunki_node().await?;
        }

        // Connect to all debug servers
        for name in self.instances.keys().cloned().collect::<Vec<_>>() {
            let socket_path = self.instances.get(&name).unwrap().socket_path.clone();
            let conn = DebugConnection::connect(name.clone(), socket_path).await?;
            self.connections.insert(name, conn);
        }

        Ok(())
    }

    async fn spawn_shell_instance(&mut self, name: String) -> Result<(), Box<dyn std::error::Error>> {
        let socket_path = self.config.db_dir.join(format!("{}.sock", name));
        let db_path = self.config.db_dir.join(format!("{}.db", name));

        // Remove old socket if exists
        let _ = std::fs::remove_file(&socket_path);

        // Set environment for JSON log format and data directory
        let mut cmd = Command::new(&self.config.shell_binary);
        cmd.arg("-d")
            .arg(&name)
            .arg("--debug-socket")
            .arg(&socket_path)
            .env("STHALAM_DATA_DIR", &self.config.db_dir)
            .env("OSVAULD_LOG_FORMAT", "json")  // Output JSON logs for parsing
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Use headless testing backend unless --show-ui is set
        if !self.config.show_ui {
            cmd.env("SLINT_BACKEND", "testing");
        }

        let mut process = cmd.spawn()?;

        tracing::info!(
            instance = %name,
            pid = ?process.id(),
            socket = %socket_path.display(),
            "Spawned shell instance"
        );

        // Take stdout and spawn reader task to collect logs
        if let Some(stdout) = process.stdout.take() {
            let instance_name = name.clone();
            let log_tx = self.log_tx.clone();
            tokio::spawn(async move {
                let reader = BufReader::new(stdout);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    // Try to parse as JSON log entry
                    if let Ok(tracing_entry) = serde_json::from_str::<TracingLogEntry>(&line) {
                        let entry = tracing_entry.to_log_entry(instance_name.clone());
                        let _ = log_tx.send(entry).await;
                    }
                }
                tracing::debug!(instance = %instance_name, "Stdout reader task ended");
            });
        }

        self.instances.insert(
            name.clone(),
            Instance {
                name,
                process,
                socket_path,
                db_path,
            },
        );

        // Give the instance time to start
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        Ok(())
    }

    async fn spawn_kunki_node(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let node_binary = self.config.node_binary.clone()
            .ok_or("Node binary path not configured")?;

        let socket_path = self.config.db_dir.join("kunki.sock");
        let db_path = self.config.db_dir.join("kunki.db");  // For Instance struct

        // Remove old socket if exists
        let _ = std::fs::remove_file(&socket_path);

        let mut cmd = Command::new(&node_binary);
        // kunki CLI structure: kunki -d <db_name> start --debug-socket <socket> --passphrase <pass>
        // kunki uses STHALAM_DATA_DIR env var to find the database (like slint_shell)
        cmd.arg("-d")
            .arg("kunki")  // Just the db name, kunki adds .db extension
            .arg("start")  // Subcommand
            .arg("--debug-socket")
            .arg(&socket_path)
            .arg("--passphrase")
            .arg("test123")  // Default passphrase for test databases
            .env("STHALAM_DATA_DIR", &self.config.db_dir)  // Same as slint_shell
            .env("OSVAULD_LOG_FORMAT", "json")  // Output JSON logs for parsing
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut process = cmd.spawn()?;

        tracing::info!(
            pid = ?process.id(),
            socket = %socket_path.display(),
            "Spawned kunki node"
        );

        // Take stdout and spawn reader task to collect logs
        if let Some(stdout) = process.stdout.take() {
            let log_tx = self.log_tx.clone();
            tokio::spawn(async move {
                let reader = BufReader::new(stdout);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    // Try to parse as JSON log entry
                    if let Ok(tracing_entry) = serde_json::from_str::<TracingLogEntry>(&line) {
                        let entry = tracing_entry.to_log_entry("kunki".to_string());
                        let _ = log_tx.send(entry).await;
                    }
                }
                tracing::debug!("Kunki stdout reader task ended");
            });
        }

        self.instances.insert(
            "kunki".to_string(),
            Instance {
                name: "kunki".to_string(),
                process,
                socket_path,
                db_path,
            },
        );

        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        Ok(())
    }

    /// Route a command to target instance(s)
    pub async fn route_command(&mut self, request: Request) -> Vec<Response> {
        let targets: Vec<String> = match request.target.as_deref() {
            Some("all") => self.connections.keys().cloned().collect(),
            Some(target) => vec![target.to_string()],
            None => {
                // No target specified - handle meta commands
                return vec![self.handle_meta_command(&request).await];
            }
        };

        let mut responses = Vec::new();

        for target in targets {
            let response = if let Some(conn) = self.connections.get_mut(&target) {
                // Build command JSON
                let cmd = serde_json::json!({
                    "method": request.action,
                    "params": request.params,
                    "id": request.id
                });

                match conn.send_command(&cmd.to_string()).await {
                    Ok(resp) => {
                        match serde_json::from_str::<serde_json::Value>(&resp) {
                            Ok(v) => Response::success(v, Some(target), request.id),
                            Err(e) => Response::error(
                                format!("Invalid JSON response: {}", e),
                                Some(target),
                                request.id,
                            ),
                        }
                    }
                    Err(e) => Response::error(
                        format!("Command failed: {}", e),
                        Some(target),
                        request.id,
                    ),
                }
            } else {
                Response::error(
                    format!("Unknown instance: {}", target),
                    Some(target),
                    request.id,
                )
            };

            responses.push(response);
        }

        responses
    }

    async fn handle_meta_command(&mut self, request: &Request) -> Response {
        match request.action.as_str() {
            "list_instances" => {
                let instances: Vec<&str> = self.instances.keys().map(|s| s.as_str()).collect();
                Response::success(
                    serde_json::json!({"instances": instances}),
                    None,
                    request.id,
                )
            }
            "get_logs" => {
                // Get recent logs from aggregator with optional filter
                let aggregator = self.log_aggregator.read().await;

                let count = request.params
                    .as_object()
                    .and_then(|p| p.get("count"))
                    .and_then(|v| v.as_u64())
                    .unwrap_or(50) as usize;

                // Build filter from params if provided
                let filter = LogFilter::from_params(&request.params).ok();
                let entries: Vec<&LogEntry> = aggregator.get_logs(count, filter.as_ref());

                Response::success(
                    serde_json::to_value(&entries).unwrap_or_default(),
                    None,
                    request.id,
                )
            }
            "subscribe_logs" => {
                // Subscribe to log stream with optional filter
                let filter = match LogFilter::from_params(&request.params) {
                    Ok(f) => Some(f),
                    Err(e) => {
                        return Response::error(e, None, request.id);
                    }
                };

                let mut aggregator = self.log_aggregator.write().await;
                aggregator.subscribe(filter);

                Response::success(
                    serde_json::json!({"subscribed": true}),
                    None,
                    request.id,
                )
            }
            "unsubscribe_logs" => {
                // Unsubscribe from log stream
                let mut aggregator = self.log_aggregator.write().await;
                aggregator.unsubscribe();

                Response::success(
                    serde_json::json!({"subscribed": false}),
                    None,
                    request.id,
                )
            }
            _ => Response::error(
                format!("Unknown meta command: {}", request.action),
                None,
                request.id,
            ),
        }
    }

    /// Run JSON REPL (stdin → route → stdout)
    /// Uses select! to handle both stdin commands and incoming logs
    pub async fn run_json_repl(&mut self) {
        let stdin = tokio::io::stdin();
        let mut stdout = tokio::io::stdout();
        let mut reader = BufReader::new(stdin);

        tracing::info!("Starting JSON REPL (stdin/stdout)");

        // Helper to write a line
        async fn write_line(stdout: &mut tokio::io::Stdout, msg: &str) -> std::io::Result<()> {
            stdout.write_all(msg.as_bytes()).await?;
            stdout.write_all(b"\n").await?;
            stdout.flush().await
        }

        // Print ready message
        let ready = serde_json::json!({"status": "ready", "instances": self.instances.keys().collect::<Vec<_>>()});
        if let Err(e) = write_line(&mut stdout, &ready.to_string()).await {
            tracing::error!(error = %e, "Failed to write ready message");
            return;
        }

        let mut line = String::new();
        loop {
            line.clear();

            tokio::select! {
                // Handle incoming logs from instance stdouts
                Some(entry) = self.log_rx.recv() => {
                    // Always push to aggregator buffer
                    let mut aggregator = self.log_aggregator.write().await;
                    aggregator.push(entry.clone());

                    // If subscribed and matches filter, output to stdout
                    if aggregator.matches_subscription(&entry) {
                        drop(aggregator); // Release lock before writing
                        let log_output = serde_json::json!({
                            "log": entry,
                        });
                        let _ = write_line(&mut stdout, &log_output.to_string()).await;
                    }
                }

                // Handle stdin commands
                result = reader.read_line(&mut line) => {
                    match result {
                        Ok(0) => {
                            // EOF
                            tracing::info!("EOF on stdin, shutting down");
                            break;
                        }
                        Ok(_) => {
                            let line_trimmed = line.trim();
                            if line_trimmed.is_empty() {
                                continue;
                            }

                            // Parse request
                            let request: Request = match serde_json::from_str(line_trimmed) {
                                Ok(r) => r,
                                Err(e) => {
                                    let error = Response::error(
                                        format!("Invalid JSON: {}", e),
                                        None,
                                        None,
                                    );
                                    let _ = write_line(&mut stdout, &serde_json::to_string(&error).unwrap()).await;
                                    continue;
                                }
                            };

                            // Route command
                            let responses = self.route_command(request).await;

                            // Output responses
                            for response in responses {
                                if let Ok(json) = serde_json::to_string(&response) {
                                    let _ = write_line(&mut stdout, &json).await;
                                }
                            }
                        }
                        Err(e) => {
                            tracing::error!(error = %e, "Failed to read from stdin");
                            break;
                        }
                    }
                }
            }
        }
    }

    /// Shutdown all instances
    pub async fn shutdown(&mut self) {
        tracing::info!("Shutting down all instances");

        for (name, instance) in &mut self.instances {
            tracing::info!(instance = %name, "Killing instance");
            let _ = instance.process.kill().await;
        }

        self.instances.clear();
        self.connections.clear();
    }
}

impl Drop for TestController {
    fn drop(&mut self) {
        // Try to kill processes synchronously in drop
        for (name, instance) in &mut self.instances {
            tracing::debug!(instance = %name, "Killing instance in drop");
            // Can't use async kill in drop, so just start the kill
            let _ = instance.process.start_kill();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_response_serialization() {
        let resp = Response::success(
            serde_json::json!({"value": 42}),
            Some("owner".to_string()),
            Some(1),
        );
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("value"));
        assert!(json.contains("owner"));
    }
}
