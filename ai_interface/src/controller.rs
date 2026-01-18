//! Test Controller - Instance management and command routing
//!
//! Spawns app instances in tmux, connects to debug servers, routes commands.
//! All paths derived from session name: /tmp/sthalam/{name}/

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command as StdCommand;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};

use crate::protocol::{Request, Response};

const BASE_DIR: &str = "/tmp/sthalam";

// =============================================================================
// tmux Helpers
// =============================================================================

fn tmux_session_exists(session: &str) -> bool {
    StdCommand::new("tmux")
        .args(["has-session", "-t", session])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn tmux_kill_session(session: &str) {
    if tmux_session_exists(session) {
        let _ = StdCommand::new("tmux")
            .args(["kill-session", "-t", session])
            .output();
        tracing::info!(session = %session, "Killed existing tmux session");
    }
}

fn tmux_new_session(session: &str, window_name: &str) -> Result<(), String> {
    let output = StdCommand::new("tmux")
        .args(["new-session", "-d", "-s", session, "-n", window_name])
        .output()
        .map_err(|e| format!("Failed to create tmux session: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "tmux new-session failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}

fn tmux_new_window(session: &str, window_name: &str) -> Result<(), String> {
    let output = StdCommand::new("tmux")
        .args(["new-window", "-t", session, "-n", window_name])
        .output()
        .map_err(|e| format!("Failed to create tmux window: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "tmux new-window failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}

fn tmux_send_keys(session: &str, window: &str, command: &str) -> Result<(), String> {
    let target = format!("{}:{}", session, window);
    let output = StdCommand::new("tmux")
        .args(["send-keys", "-t", &target, command, "Enter"])
        .output()
        .map_err(|e| format!("Failed to send keys to tmux: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "tmux send-keys failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}

// =============================================================================
// Session Configuration
// =============================================================================

/// Session configuration - everything derived from name
#[derive(Debug, Clone)]
pub struct SessionConfig {
    /// Session name (used for tmux, directory, socket names)
    pub name: String,
    /// Instance names (e.g., ["owner", "customer1", "customer2"])
    pub instances: Vec<String>,
    /// Whether to spawn kunki node
    pub spawn_node: bool,
    /// Path to slint_shell binary
    pub shell_binary: PathBuf,
    /// Path to kunki binary (optional)
    pub node_binary: Option<PathBuf>,
    /// Show UI windows (instead of headless testing backend)
    pub show_ui: bool,
}

impl SessionConfig {
    /// Base directory for this session: /tmp/sthalam/{name}/
    pub fn base_dir(&self) -> PathBuf {
        PathBuf::from(BASE_DIR).join(&self.name)
    }

    /// AI interface socket: /tmp/sthalam/{name}/ai.sock
    pub fn ai_socket(&self) -> PathBuf {
        self.base_dir().join("ai.sock")
    }

    /// Instance socket: /tmp/sthalam/{name}/{instance}.sock
    pub fn instance_socket(&self, instance: &str) -> PathBuf {
        self.base_dir().join(format!("{}.sock", instance))
    }

    /// Instance DB: /tmp/sthalam/{name}/{instance}.db
    pub fn instance_db(&self, instance: &str) -> PathBuf {
        self.base_dir().join(format!("{}.db", instance))
    }
}

// =============================================================================
// Debug Connection
// =============================================================================

struct DebugConnection {
    stream: UnixStream,
}

impl DebugConnection {
    async fn connect(name: &str, socket_path: &PathBuf) -> Result<Self, std::io::Error> {
        let mut attempts = 0;
        let stream = loop {
            match UnixStream::connect(socket_path).await {
                Ok(s) => break s,
                Err(_) if attempts < 30 => {
                    attempts += 1;
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                    if attempts % 5 == 0 {
                        tracing::debug!(
                            instance = %name,
                            attempt = attempts,
                            "Retrying debug server connection"
                        );
                    }
                }
                Err(e) => return Err(e),
            }
        };

        tracing::info!(
            instance = %name,
            socket = %socket_path.display(),
            "Connected to debug server"
        );

        Ok(Self { stream })
    }

    async fn send_command(&mut self, cmd: &str) -> Result<String, std::io::Error> {
        self.stream.write_all(cmd.as_bytes()).await?;
        self.stream.write_all(b"\n").await?;
        self.stream.flush().await?;

        let mut buf_reader = BufReader::new(&mut self.stream);
        let mut response = String::new();
        buf_reader.read_line(&mut response).await?;

        Ok(response.trim().to_string())
    }
}

// =============================================================================
// Instance Metadata
// =============================================================================

struct Instance {
    socket_path: PathBuf,
}

// =============================================================================
// Test Controller
// =============================================================================

pub struct TestController {
    config: SessionConfig,
    instances: HashMap<String, Instance>,
    connections: HashMap<String, DebugConnection>,
}

impl TestController {
    pub fn new(config: SessionConfig) -> Self {
        Self {
            config,
            instances: HashMap::new(),
            connections: HashMap::new(),
        }
    }

    /// Spawn all configured instances in tmux
    pub async fn spawn_instances(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Create base directory
        let base_dir = self.config.base_dir();
        std::fs::create_dir_all(&base_dir)?;

        // Kill existing session for clean start
        tmux_kill_session(&self.config.name);

        let mut session_created = false;

        // Spawn shell instances
        for name in &self.config.instances.clone() {
            self.spawn_shell_instance(name.clone(), &mut session_created)?;
        }

        // Spawn kunki node if configured
        if self.config.spawn_node {
            self.spawn_kunki_node(&mut session_created)?;
        }

        tracing::info!(
            session = %self.config.name,
            base_dir = %base_dir.display(),
            instances = ?self.instances.keys().collect::<Vec<_>>(),
            "All instances spawned in tmux"
        );

        // Wait for all debug sockets to be ready
        for name in self.instances.keys().cloned().collect::<Vec<_>>() {
            let socket_path = self.instances.get(&name).unwrap().socket_path.clone();
            tracing::info!(instance = %name, "Waiting for debug socket...");
            let conn = DebugConnection::connect(&name, &socket_path).await?;
            self.connections.insert(name, conn);
        }

        Ok(())
    }

    fn spawn_shell_instance(
        &mut self,
        name: String,
        session_created: &mut bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let socket_path = self.config.instance_socket(&name);
        let base_dir = self.config.base_dir();

        // Remove old socket if exists
        let _ = std::fs::remove_file(&socket_path);

        // Build the command string
        let cmd_parts = vec![
            self.config.shell_binary.display().to_string(),
            "-d".to_string(),
            name.clone(),
            "--debug-socket".to_string(),
            socket_path.display().to_string(),
        ];

        // Environment setup
        let env_setup = format!("export STHALAM_DATA_DIR='{}'; ", base_dir.display());

        let backend_env = if !self.config.show_ui {
            "export SLINT_BACKEND=testing; "
        } else {
            ""
        };

        let full_command = format!("{}{}{}", env_setup, backend_env, cmd_parts.join(" "));

        // Create session or window
        if !*session_created {
            tmux_new_session(&self.config.name, &name)?;
            *session_created = true;
        } else {
            tmux_new_window(&self.config.name, &name)?;
        }

        tmux_send_keys(&self.config.name, &name, &full_command)?;

        tracing::info!(
            instance = %name,
            socket = %socket_path.display(),
            "Spawned shell instance in tmux"
        );

        self.instances.insert(name, Instance { socket_path });

        Ok(())
    }

    fn spawn_kunki_node(
        &mut self,
        session_created: &mut bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let node_binary = self
            .config
            .node_binary
            .clone()
            .ok_or("Node binary path not configured")?;

        let socket_path = self.config.instance_socket("kunki");
        let base_dir = self.config.base_dir();

        // Remove old socket if exists
        let _ = std::fs::remove_file(&socket_path);

        let env_setup = format!("export STHALAM_DATA_DIR='{}'; ", base_dir.display());

        // Check if kunki database exists, if not, init first
        let kunki_db = base_dir.join("kunki.db");
        let needs_init = !kunki_db.exists();

        // Build init + start command
        let init_cmd = if needs_init {
            format!(
                "{} -d kunki init -p test123 -u kunki && ",
                node_binary.display()
            )
        } else {
            String::new()
        };

        let start_cmd = format!(
            "{} -d kunki start --debug-socket {} --passphrase test123",
            node_binary.display(),
            socket_path.display()
        );

        let full_command = format!("{}{}{}", env_setup, init_cmd, start_cmd);

        if !*session_created {
            tmux_new_session(&self.config.name, "kunki")?;
            *session_created = true;
        } else {
            tmux_new_window(&self.config.name, "kunki")?;
        }

        tmux_send_keys(&self.config.name, "kunki", &full_command)?;

        tracing::info!(
            socket = %socket_path.display(),
            needs_init = needs_init,
            "Spawned kunki node in tmux"
        );

        self.instances
            .insert("kunki".to_string(), Instance { socket_path });

        Ok(())
    }

    /// Route a command to target instance(s)
    pub async fn route_command(&mut self, request: Request) -> Vec<Response> {
        let targets: Vec<String> = match request.target.as_deref() {
            Some("all") => self.connections.keys().cloned().collect(),
            Some(target) => vec![target.to_string()],
            None => {
                return vec![self.handle_meta_command(&request).await];
            }
        };

        let mut responses = Vec::new();

        for target in targets {
            let response = if let Some(conn) = self.connections.get_mut(&target) {
                let cmd = serde_json::json!({
                    "method": request.action,
                    "params": request.params,
                    "id": request.id
                });

                match conn.send_command(&cmd.to_string()).await {
                    Ok(resp) => match serde_json::from_str::<serde_json::Value>(&resp) {
                        Ok(v) => Response::success(v, Some(target), request.id),
                        Err(e) => Response::error(
                            format!("Invalid JSON response: {}", e),
                            Some(target),
                            request.id,
                        ),
                    },
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
                Response::success(serde_json::json!({"instances": instances}), None, request.id)
            }
            "status" => {
                let mut status = serde_json::Map::new();
                for (name, _) in &self.instances {
                    let connected = self.connections.contains_key(name);
                    status.insert(
                        name.clone(),
                        serde_json::json!(if connected { "connected" } else { "disconnected" }),
                    );
                }
                status.insert(
                    "session".to_string(),
                    serde_json::json!(self.config.name),
                );
                status.insert(
                    "base_dir".to_string(),
                    serde_json::json!(self.config.base_dir().display().to_string()),
                );
                Response::success(serde_json::json!(status), None, request.id)
            }
            "get_logs" => {
                let target = request
                    .params
                    .get("instance")
                    .and_then(|v| v.as_str())
                    .or_else(|| self.instances.keys().next().map(|s| s.as_str()));

                if let Some(target) = target {
                    if let Some(conn) = self.connections.get_mut(target) {
                        let cmd = serde_json::json!({
                            "method": "logs",
                            "params": request.params,
                            "id": request.id
                        });
                        match conn.send_command(&cmd.to_string()).await {
                            Ok(resp) => match serde_json::from_str::<serde_json::Value>(&resp) {
                                Ok(v) => Response::success(v, Some(target.to_string()), request.id),
                                Err(e) => Response::error(
                                    format!("Invalid JSON response: {}", e),
                                    Some(target.to_string()),
                                    request.id,
                                ),
                            },
                            Err(e) => Response::error(
                                format!("Failed to get logs: {}", e),
                                Some(target.to_string()),
                                request.id,
                            ),
                        }
                    } else {
                        Response::error(format!("Unknown instance: {}", target), None, request.id)
                    }
                } else {
                    Response::error("No instances available".to_string(), None, request.id)
                }
            }
            _ => Response::error(
                format!("Unknown meta command: {}", request.action),
                None,
                request.id,
            ),
        }
    }

    /// Run as Unix socket server (daemon mode)
    pub async fn run_socket_server(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let socket_path = self.config.ai_socket();

        // Remove old socket if exists
        let _ = std::fs::remove_file(&socket_path);

        let listener = UnixListener::bind(&socket_path)?;
        tracing::info!(
            socket = %socket_path.display(),
            "AI interface listening on Unix socket"
        );

        // Print ready message to stdout for scripts to capture
        let ready = serde_json::json!({
            "status": "ready",
            "session": self.config.name,
            "socket": socket_path.display().to_string(),
            "instances": self.instances.keys().collect::<Vec<_>>(),
        });
        println!("{}", ready);

        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    self.handle_socket_client(stream).await;
                }
                Err(e) => {
                    tracing::error!(error = %e, "Failed to accept connection");
                }
            }
        }
    }

    async fn handle_socket_client(&mut self, mut stream: UnixStream) {
        let mut buf_reader = BufReader::new(&mut stream);
        let mut line = String::new();

        // Read single command
        match buf_reader.read_line(&mut line).await {
            Ok(0) => return, // EOF
            Ok(_) => {
                let line_trimmed = line.trim();
                if line_trimmed.is_empty() {
                    return;
                }

                let response = match serde_json::from_str::<Request>(line_trimmed) {
                    Ok(request) => {
                        let responses = self.route_command(request).await;
                        // Return first response (usually there's only one)
                        responses.into_iter().next().unwrap_or_else(|| {
                            Response::error("No response".to_string(), None, None)
                        })
                    }
                    Err(e) => Response::error(format!("Invalid JSON: {}", e), None, None),
                };

                if let Ok(json) = serde_json::to_string(&response) {
                    let _ = stream.write_all(json.as_bytes()).await;
                    let _ = stream.write_all(b"\n").await;
                    let _ = stream.flush().await;
                }
            }
            Err(e) => {
                tracing::error!(error = %e, "Failed to read from client");
            }
        }
    }

    /// Run JSON REPL on stdin/stdout (for testing)
    pub async fn run_stdin_repl(&mut self) {
        let stdin = tokio::io::stdin();
        let mut stdout = tokio::io::stdout();
        let mut reader = BufReader::new(stdin);

        async fn write_line(stdout: &mut tokio::io::Stdout, msg: &str) -> std::io::Result<()> {
            stdout.write_all(msg.as_bytes()).await?;
            stdout.write_all(b"\n").await?;
            stdout.flush().await
        }

        let ready = serde_json::json!({
            "status": "ready",
            "session": self.config.name,
            "socket": self.config.ai_socket().display().to_string(),
            "instances": self.instances.keys().collect::<Vec<_>>(),
        });
        let _ = write_line(&mut stdout, &ready.to_string()).await;

        let mut line = String::new();
        loop {
            line.clear();

            match reader.read_line(&mut line).await {
                Ok(0) => break,
                Ok(_) => {
                    let line_trimmed = line.trim();
                    if line_trimmed.is_empty() {
                        continue;
                    }

                    let request: Request = match serde_json::from_str(line_trimmed) {
                        Ok(r) => r,
                        Err(e) => {
                            let error = Response::error(format!("Invalid JSON: {}", e), None, None);
                            let _ =
                                write_line(&mut stdout, &serde_json::to_string(&error).unwrap())
                                    .await;
                            continue;
                        }
                    };

                    let responses = self.route_command(request).await;
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

    pub async fn shutdown(&mut self) {
        tracing::info!(
            session = %self.config.name,
            "Shutting down (tmux session left running)"
        );
        tracing::info!("To kill: tmux kill-session -t {}", self.config.name);
        tracing::info!("To attach: tmux attach -t {}", self.config.name);
        self.connections.clear();
    }
}
