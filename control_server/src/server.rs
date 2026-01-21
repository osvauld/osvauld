//! Base control server implementation
//!
//! Provides a Unix socket server that handles JSON-RPC style requests.
//! Implementors provide a CommandHandler to process application-specific commands.

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tracing::{debug, error, info, warn};

use crate::types::{Request, Response, error_codes};

/// Trait for handling commands
///
/// Implement this trait to add application-specific commands to the control server.
#[async_trait]
pub trait CommandHandler: Send + Sync {
    /// Handle a command request
    ///
    /// Returns Some(Response) if the command was handled, None if not recognized.
    /// The base server handles common commands (ping) automatically.
    async fn handle(&self, method: &str, params: Option<serde_json::Value>, id: u64) -> Option<Response>;

    /// Get the connection string for this instance (if applicable)
    async fn get_connection_string(&self) -> Option<String> {
        None
    }
}

/// Base control server
pub struct ControlServer<H: CommandHandler> {
    socket_path: PathBuf,
    instance_name: String,
    handler: Arc<H>,
}

impl<H: CommandHandler + 'static> ControlServer<H> {
    /// Create a new control server
    pub fn new(socket_path: PathBuf, instance_name: String, handler: H) -> Self {
        Self {
            socket_path,
            instance_name,
            handler: Arc::new(handler),
        }
    }

    /// Get the socket path
    pub fn socket_path(&self) -> &PathBuf {
        &self.socket_path
    }

    /// Start the control server
    ///
    /// This runs the server loop, accepting connections and handling requests.
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
            instance = %self.instance_name,
            "Control server listening"
        );

        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    let handler = self.handler.clone();
                    let instance_name = self.instance_name.clone();

                    tokio::spawn(async move {
                        if let Err(e) = handle_connection(
                            stream,
                            handler,
                            instance_name,
                        ).await {
                            debug!(error = %e, "Connection handler error");
                        }
                    });
                }
                Err(e) => {
                    error!(error = %e, "Failed to accept connection");
                }
            }
        }
    }
}

/// Handle a single client connection
async fn handle_connection<H: CommandHandler>(
    stream: UnixStream,
    handler: Arc<H>,
    instance_name: String,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    while reader.read_line(&mut line).await? > 0 {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            line.clear();
            continue;
        }

        debug!(request = %trimmed, "Received request");

        // Parse request
        let response = match serde_json::from_str::<Request>(trimmed) {
            Ok(request) => {
                process_request(
                    &request,
                    &handler,
                    &instance_name,
                ).await
            }
            Err(e) => {
                warn!(error = %e, "Failed to parse request");
                Response::err(0, error_codes::PARSE_ERROR, format!("Parse error: {}", e))
            }
        };

        // Send response
        let response_json = serde_json::to_string(&response)?;
        debug!(response = %response_json, "Sending response");
        writer.write_all(response_json.as_bytes()).await?;
        writer.write_all(b"\n").await?;
        writer.flush().await?;

        line.clear();
    }

    Ok(())
}

/// Process a parsed request
async fn process_request<H: CommandHandler>(
    request: &Request,
    handler: &Arc<H>,
    instance_name: &str,
) -> Response {
    let id = request.id;
    let method = request.method.as_str();

    // Handle common commands first
    match method {
        "ping" => {
            return Response::ok(id, serde_json::json!({
                "status": "ok",
                "instance": instance_name
            }));
        }
        "get_connection_string" => {
            if let Some(conn_str) = handler.get_connection_string().await {
                return Response::ok(id, serde_json::json!({"connection_string": conn_str}));
            } else {
                return Response::err(id, error_codes::NOT_FOUND, "Connection string not available");
            }
        }
        _ => {}
    }

    // Try application-specific handler
    if let Some(response) = handler.handle(method, request.params.clone(), id).await {
        return response;
    }

    // Method not found
    Response::err(id, error_codes::METHOD_NOT_FOUND, format!("Unknown method: {}", method))
}
