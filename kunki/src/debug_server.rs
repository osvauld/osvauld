//! Debug Server for Kunki Node
//!
//! Provides debug access via Unix socket for testing and monitoring.
//! Includes layer inspection for debugging sync and derivation.

use butler::Butler;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{broadcast, RwLock};
use tracing::{debug, error, info, warn};

/// Log entry for broadcasting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub ts: String,
    pub level: String,
    pub target: String,
    pub msg: String,
    #[serde(default)]
    pub instance: Option<String>,
}

/// Node state snapshot
#[derive(Debug, Clone, Serialize)]
pub struct NodeState {
    pub node_id: String,
    pub did: String,
    pub username: String,
    pub connected_peers: Vec<PeerInfo>,
    pub relay_url: Option<String>,
    /// Connection string for others to connect to this node
    pub connection_string: Option<String>,
}

/// Connected peer info
#[derive(Debug, Clone, Serialize)]
pub struct PeerInfo {
    pub node_id: String,
    pub did: Option<String>,
    pub username: Option<String>,
}

/// Debug server for kunki node
pub struct KunkiDebugServer {
    socket_path: PathBuf,
    log_tx: broadcast::Sender<LogEntry>,
    log_buffer: Arc<RwLock<Vec<LogEntry>>>,
    state: Arc<RwLock<Option<NodeState>>>,
    butler: Option<Arc<Butler>>,
    instance_name: String,
}

impl KunkiDebugServer {
    /// Create a new debug server
    pub fn new(socket_path: PathBuf, instance_name: String, butler: Option<Arc<Butler>>) -> Self {
        let (log_tx, _) = broadcast::channel(1000);
        Self {
            socket_path,
            log_tx,
            log_buffer: Arc::new(RwLock::new(Vec::new())),
            state: Arc::new(RwLock::new(None)),
            butler,
            instance_name,
        }
    }

    /// Get log sender for broadcasting
    pub fn log_sender(&self) -> broadcast::Sender<LogEntry> {
        self.log_tx.clone()
    }

    /// Update node state
    pub async fn set_state(&self, state: NodeState) {
        *self.state.write().await = Some(state);
    }

    /// Start the debug server
    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Remove existing socket file
        let _ = std::fs::remove_file(&self.socket_path);

        // Ensure parent directory exists
        if let Some(parent) = self.socket_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let listener = UnixListener::bind(&self.socket_path)?;
        info!(
            socket = %self.socket_path.display(),
            "Kunki debug server listening"
        );

        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    let log_buffer = self.log_buffer.clone();
                    let log_tx = self.log_tx.clone();
                    let state = self.state.clone();
                    let butler = self.butler.clone();
                    let instance_name = self.instance_name.clone();

                    tokio::spawn(async move {
                        if let Err(e) = handle_connection(
                            stream,
                            log_buffer,
                            log_tx,
                            state,
                            butler,
                            instance_name,
                        ).await {
                            warn!(error = %e, "Debug connection error");
                        }
                    });
                }
                Err(e) => {
                    error!(error = %e, "Failed to accept debug connection");
                }
            }
        }
    }
}

async fn handle_connection(
    stream: UnixStream,
    log_buffer: Arc<RwLock<Vec<LogEntry>>>,
    log_tx: broadcast::Sender<LogEntry>,
    state: Arc<RwLock<Option<NodeState>>>,
    butler: Option<Arc<Butler>>,
    instance_name: String,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    debug!(instance = %instance_name, "Debug client connected");

    loop {
        line.clear();
        let n = reader.read_line(&mut line).await?;
        if n == 0 {
            debug!(instance = %instance_name, "Debug client disconnected");
            break;
        }

        let response = handle_command(&line, &log_buffer, &log_tx, &state, &butler, &instance_name).await;
        let response_json = serde_json::to_string(&response)?;

        writer.write_all(response_json.as_bytes()).await?;
        writer.write_all(b"\n").await?;
        writer.flush().await?;
    }

    Ok(())
}

async fn handle_command(
    line: &str,
    log_buffer: &Arc<RwLock<Vec<LogEntry>>>,
    _log_tx: &broadcast::Sender<LogEntry>,
    state: &Arc<RwLock<Option<NodeState>>>,
    butler: &Option<Arc<Butler>>,
    instance_name: &str,
) -> JsonValue {
    // Parse command JSON
    let cmd: JsonValue = match serde_json::from_str(line.trim()) {
        Ok(v) => v,
        Err(e) => {
            return serde_json::json!({
                "error": format!("Invalid JSON: {}", e),
                "instance": instance_name
            });
        }
    };

    let method = cmd.get("method").and_then(|v| v.as_str()).unwrap_or("");
    let id = cmd.get("id").and_then(|v| v.as_u64());

    match method {
        "ping" => {
            serde_json::json!({
                "result": {"status": "ok", "instance": instance_name},
                "id": id
            })
        }

        "state" => {
            let state_guard = state.read().await;
            match &*state_guard {
                Some(s) => serde_json::json!({
                    "result": s,
                    "id": id
                }),
                None => serde_json::json!({
                    "result": {"status": "not_initialized"},
                    "id": id
                }),
            }
        }

        "logs" => {
            let params = cmd.get("params");
            let last = params
                .and_then(|p| p.get("last"))
                .and_then(|v| v.as_u64())
                .unwrap_or(50) as usize;

            let logs = log_buffer.read().await;
            let entries: Vec<&LogEntry> = logs.iter().rev().take(last).collect();

            serde_json::json!({
                "result": {"logs": entries},
                "id": id
            })
        }

        "get_connection_string" => {
            let state_guard = state.read().await;
            match &*state_guard {
                Some(s) => {
                    if let Some(ref conn_str) = s.connection_string {
                        serde_json::json!({
                            "result": {"connection_string": conn_str},
                            "id": id
                        })
                    } else {
                        serde_json::json!({
                            "error": "Connection string not available",
                            "id": id
                        })
                    }
                }
                None => serde_json::json!({
                    "error": "Node not initialized",
                    "id": id
                }),
            }
        }

        // === Layer inspection methods ===

        "list_spaces" => {
            match butler {
                Some(b) => {
                    match b.list_spaces() {
                        Ok(spaces) => {
                            let space_list: Vec<_> = spaces.iter().map(|s| {
                                serde_json::json!({
                                    "id": s.id,
                                    "name": s.name,
                                })
                            }).collect();
                            serde_json::json!({
                                "result": {"spaces": space_list},
                                "id": id
                            })
                        }
                        Err(e) => serde_json::json!({
                            "error": format!("Failed to list spaces: {:?}", e),
                            "id": id
                        }),
                    }
                }
                None => serde_json::json!({
                    "error": "Butler not available",
                    "id": id
                }),
            }
        }

        "list_pages" => {
            let params = cmd.get("params");
            let space_id = params
                .and_then(|p| p.get("space_id"))
                .and_then(|v| v.as_str());

            match (butler, space_id) {
                (Some(b), Some(sid)) => {
                    match b.list_pages(sid) {
                        Ok(pages) => {
                            let page_list: Vec<_> = pages.iter().map(|p| {
                                serde_json::json!({
                                    "id": p.id,
                                    "name": p.name,
                                    "space_id": p.space_id,
                                })
                            }).collect();
                            serde_json::json!({
                                "result": {"pages": page_list},
                                "id": id
                            })
                        }
                        Err(e) => serde_json::json!({
                            "error": format!("Failed to list pages: {:?}", e),
                            "id": id
                        }),
                    }
                }
                (None, _) => serde_json::json!({
                    "error": "Butler not available",
                    "id": id
                }),
                (_, None) => serde_json::json!({
                    "error": "Missing space_id parameter",
                    "id": id
                }),
            }
        }

        "list_layers" => {
            let params = cmd.get("params");
            let page_id = params
                .and_then(|p| p.get("page_id"))
                .and_then(|v| v.as_str());

            match (butler, page_id) {
                (Some(b), Some(pid)) => {
                    match b.list_data_layers(pid) {
                        Ok(layers) => {
                            serde_json::json!({
                                "result": {"layers": layers},
                                "id": id
                            })
                        }
                        Err(e) => serde_json::json!({
                            "error": format!("Failed to list layers: {:?}", e),
                            "id": id
                        }),
                    }
                }
                (None, _) => serde_json::json!({
                    "error": "Butler not available",
                    "id": id
                }),
                (_, None) => serde_json::json!({
                    "error": "Missing page_id parameter",
                    "id": id
                }),
            }
        }

        "get_layer" => {
            let params = cmd.get("params");
            let page_id = params.and_then(|p| p.get("page_id")).and_then(|v| v.as_str());
            let layer_name = params.and_then(|p| p.get("layer_name")).and_then(|v| v.as_str());

            match (butler, page_id, layer_name) {
                (Some(b), Some(pid), Some(lname)) => {
                    // Get decrypted page and extract layer
                    match b.get_decrypted_page(pid).await {
                        Ok((decrypted, _aes_key)) => {
                            if let Some(layer_bytes) = decrypted.docs.get(lname) {
                                if layer_bytes.is_empty() {
                                    serde_json::json!({
                                        "result": {"data": null, "empty": true},
                                        "id": id
                                    })
                                } else {
                                    match butler::models::Layer::from_snapshot(layer_bytes) {
                                        Ok(layer) => {
                                            let json_data = layer.to_json_value();
                                            serde_json::json!({
                                                "result": {"data": json_data},
                                                "id": id
                                            })
                                        }
                                        Err(e) => serde_json::json!({
                                            "error": format!("Failed to parse layer: {}", e),
                                            "id": id
                                        }),
                                    }
                                }
                            } else {
                                serde_json::json!({
                                    "error": format!("Layer '{}' not found in page", lname),
                                    "id": id
                                })
                            }
                        }
                        Err(e) => serde_json::json!({
                            "error": format!("Failed to get page: {:?}", e),
                            "id": id
                        }),
                    }
                }
                (None, _, _) => serde_json::json!({
                    "error": "Butler not available",
                    "id": id
                }),
                (_, None, _) => serde_json::json!({
                    "error": "Missing page_id parameter",
                    "id": id
                }),
                (_, _, None) => serde_json::json!({
                    "error": "Missing layer_name parameter",
                    "id": id
                }),
            }
        }

        _ => {
            serde_json::json!({
                "error": format!("Unknown method: {}", method),
                "id": id
            })
        }
    }
}

/// Tracing layer for capturing logs to debug server
pub struct KunkiLogLayer {
    log_tx: broadcast::Sender<LogEntry>,
    log_buffer: Arc<RwLock<Vec<LogEntry>>>,
    instance_name: String,
}

impl KunkiLogLayer {
    pub fn new(
        log_tx: broadcast::Sender<LogEntry>,
        log_buffer: Arc<RwLock<Vec<LogEntry>>>,
        instance_name: String,
    ) -> Self {
        Self {
            log_tx,
            log_buffer,
            instance_name,
        }
    }
}

impl<S> tracing_subscriber::Layer<S> for KunkiLogLayer
where
    S: tracing::Subscriber,
{
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: tracing_subscriber::layer::Context<'_, S>) {
        let metadata = event.metadata();

        // Extract message
        let mut visitor = MessageVisitor::default();
        event.record(&mut visitor);

        let entry = LogEntry {
            ts: chrono::Utc::now().to_rfc3339(),
            level: level_to_string(metadata.level()),
            target: metadata.target().to_string(),
            msg: visitor.message.unwrap_or_default(),
            instance: Some(self.instance_name.clone()),
        };

        // Store in buffer (non-blocking)
        let buffer = self.log_buffer.clone();
        let entry_clone = entry.clone();
        tokio::spawn(async move {
            let mut buf = buffer.write().await;
            if buf.len() > 10000 {
                buf.drain(0..5000); // Keep last 5000
            }
            buf.push(entry_clone);
        });

        // Broadcast (ignore if no receivers)
        let _ = self.log_tx.send(entry);
    }
}

#[derive(Default)]
struct MessageVisitor {
    message: Option<String>,
}

impl tracing::field::Visit for MessageVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.message = Some(format!("{:?}", value));
        }
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "message" {
            self.message = Some(value.to_string());
        }
    }
}

fn level_to_string(level: &tracing::Level) -> String {
    match *level {
        tracing::Level::ERROR => "error".to_string(),
        tracing::Level::WARN => "warn".to_string(),
        tracing::Level::INFO => "info".to_string(),
        tracing::Level::DEBUG => "debug".to_string(),
        tracing::Level::TRACE => "trace".to_string(),
    }
}
