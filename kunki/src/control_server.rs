//! Control Server for Kunki Node
//!
//! Uses shared control_server infrastructure with node-specific commands.

use butler::{Butler, ValidationHandle, JsonOp};
use control_server::{
    async_trait, CommandHandler, ControlServer, Response, error_codes,
    commands::butler as butler_cmds,
};
use logging_utils::CaptureHandle;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

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

/// Kunki-specific command handler
pub struct KunkiHandler {
    state: Arc<RwLock<Option<NodeState>>>,
    butler: Option<Arc<Butler>>,
    validation_handle: Arc<RwLock<Option<ValidationHandle>>>,
    capture_handle: Arc<RwLock<Option<CaptureHandle>>>,
    node_runtime: Arc<RwLock<Option<Arc<crate::node_runtime::NodeRuntimeManager>>>>,
}

impl KunkiHandler {
    pub fn new(butler: Option<Arc<Butler>>) -> Self {
        Self {
            state: Arc::new(RwLock::new(None)),
            butler,
            validation_handle: Arc::new(RwLock::new(None)),
            capture_handle: Arc::new(RwLock::new(None)),
            node_runtime: Arc::new(RwLock::new(None)),
        }
    }

    /// Set node runtime manager (for test-time controls)
    pub async fn set_node_runtime(&self, runtime: Arc<crate::node_runtime::NodeRuntimeManager>) {
        *self.node_runtime.write().await = Some(runtime);
    }

    /// Set capture handle (for event capture)
    pub async fn set_capture_handle(&self, handle: CaptureHandle) {
        *self.capture_handle.write().await = Some(handle);
    }

    /// Handle capture_start command
    async fn handle_capture_start(&self, params: Option<serde_json::Value>, id: u64) -> Response {
        let handle_guard = self.capture_handle.read().await;
        let Some(ref handle) = *handle_guard else {
            return Response::err(id, error_codes::INTERNAL_ERROR, "CaptureHandle not available");
        };

        let Some(params_obj) = params.as_ref().and_then(|p| p.as_object()) else {
            return Response::err(id, error_codes::INVALID_PARAMS, "Invalid params object");
        };

        let Some(file_path) = params_obj.get("file_path").and_then(|v| v.as_str()) else {
            return Response::err(id, error_codes::INVALID_PARAMS, "Missing file_path");
        };

        let include_logs = params_obj.get("include_logs").and_then(|v| v.as_bool()).unwrap_or(false);

        match handle.start_capture(PathBuf::from(file_path), include_logs).await {
            Ok(()) => Response::ok(id, serde_json::json!({"status": "capturing", "file_path": file_path})),
            Err(e) => Response::err(id, error_codes::INTERNAL_ERROR, &e),
        }
    }

    /// Handle capture_end command
    async fn handle_capture_end(&self, id: u64) -> Response {
        let handle_guard = self.capture_handle.read().await;
        let Some(ref handle) = *handle_guard else {
            return Response::err(id, error_codes::INTERNAL_ERROR, "CaptureHandle not available");
        };

        match handle.stop_capture().await {
            Ok(()) => Response::ok(id, serde_json::json!({"status": "stopped"})),
            Err(e) => Response::err(id, error_codes::INTERNAL_ERROR, &e),
        }
    }

    /// Set validation handle (for testing validation RPC)
    pub async fn set_validation_handle(&self, handle: ValidationHandle) {
        *self.validation_handle.write().await = Some(handle);
    }

    /// Update node state
    pub async fn set_state(&self, state: NodeState) {
        *self.state.write().await = Some(state);
    }

    /// Get the state reference for external updates
    pub fn state(&self) -> Arc<RwLock<Option<NodeState>>> {
        self.state.clone()
    }

    async fn handle_state(&self, id: u64) -> Response {
        let state_guard = self.state.read().await;
        match &*state_guard {
            Some(s) => Response::ok(id, s),
            None => Response::ok(id, serde_json::json!({"status": "not_initialized"})),
        }
    }

    /// Handle validate_ops command (for testing validation RPC)
    ///
    /// **Params**: page_id, layer_name, ops (array of JsonOp), from_did, role
    /// **Returns**: {passed: bool, error?: string}
    async fn handle_validate_ops(&self, params: Option<serde_json::Value>, id: u64) -> Response {
        let handle_guard = self.validation_handle.read().await;
        let Some(ref handle) = *handle_guard else {
            return Response::err(id, error_codes::INTERNAL_ERROR, "ValidationHandle not available");
        };

        // Extract parameters
        let Some(params_obj) = params.as_ref().and_then(|p| p.as_object()) else {
            return Response::err(id, error_codes::INVALID_PARAMS, "Invalid params object");
        };

        let Some(page_id) = params_obj.get("page_id").and_then(|v| v.as_str()) else {
            return Response::err(id, error_codes::INVALID_PARAMS, "Missing page_id");
        };

        let Some(layer_name) = params_obj.get("layer_name").and_then(|v| v.as_str()) else {
            return Response::err(id, error_codes::INVALID_PARAMS, "Missing layer_name");
        };

        let Some(from_did) = params_obj.get("from_did").and_then(|v| v.as_str()) else {
            return Response::err(id, error_codes::INVALID_PARAMS, "Missing from_did");
        };

        let Some(role) = params_obj.get("role").and_then(|v| v.as_str()) else {
            return Response::err(id, error_codes::INVALID_PARAMS, "Missing role");
        };

        let Some(ops_array) = params_obj.get("ops").and_then(|v| v.as_array()) else {
            return Response::err(id, error_codes::INVALID_PARAMS, "Missing ops array");
        };

        // Parse ops
        let ops: Vec<JsonOp> = match ops_array.iter().map(|v| serde_json::from_value(v.clone())).collect() {
            Ok(ops) => ops,
            Err(e) => return Response::err(id, error_codes::INVALID_PARAMS, &format!("Failed to parse ops: {}", e)),
        };

        // Call validation
        match handle.validate_ops(page_id, layer_name, &ops, from_did, role).await {
            Ok((passed, error_msg)) => {
                let result = if passed {
                    serde_json::json!({
                        "passed": true
                    })
                } else {
                    serde_json::json!({
                        "passed": false,
                        "error": error_msg.unwrap_or_else(|| "Validation failed".to_string())
                    })
                };
                Response::ok(id, result)
            }
            Err(e) => Response::err(id, error_codes::INTERNAL_ERROR, &format!("Validation error: {}", e)),
        }
    }
}

#[async_trait]
impl CommandHandler for KunkiHandler {
    async fn handle(&self, method: &str, params: Option<serde_json::Value>, id: u64) -> Option<Response> {
        match method {
            "state" => Some(self.handle_state(id).await),

            "validate_ops" => Some(self.handle_validate_ops(params, id).await),

            "capture_start" => Some(self.handle_capture_start(params, id).await),
            "capture_end" => Some(self.handle_capture_end(id).await),

            // Butler commands - delegate to shared implementations
            "list_spaces" => {
                if let Some(ref butler) = self.butler {
                    Some(butler_cmds::list_spaces(butler, id).await)
                } else {
                    Some(Response::err(id, error_codes::NOT_AUTHENTICATED, "Butler not available"))
                }
            }

            "list_pages" => {
                let space_id = butler_cmds::get_string_param(&params, "space_id");
                match (self.butler.as_ref(), space_id) {
                    (Some(butler), Some(sid)) => Some(butler_cmds::list_pages(butler, &sid, id).await),
                    (None, _) => Some(Response::err(id, error_codes::NOT_AUTHENTICATED, "Butler not available")),
                    (_, None) => Some(Response::err(id, error_codes::INVALID_PARAMS, "Missing space_id parameter")),
                }
            }

            "list_layers" => {
                let page_id = butler_cmds::get_string_param(&params, "page_id");
                match (self.butler.as_ref(), page_id) {
                    (Some(butler), Some(pid)) => Some(butler_cmds::list_layers(butler, &pid, id).await),
                    (None, _) => Some(Response::err(id, error_codes::NOT_AUTHENTICATED, "Butler not available")),
                    (_, None) => Some(Response::err(id, error_codes::INVALID_PARAMS, "Missing page_id parameter")),
                }
            }

            // Test-time controls
            "set_time" => {
                let Some(params_obj) = params.as_ref().and_then(|p| p.as_object()) else {
                    return Some(Response::err(id, error_codes::INVALID_PARAMS, "Invalid params object"));
                };

                let Some(unix_seconds) = params_obj.get("unix_seconds").and_then(|v| v.as_i64()) else {
                    return Some(Response::err(id, error_codes::INVALID_PARAMS, "Missing unix_seconds parameter"));
                };

                let Some(page_id) = params_obj.get("page_id").and_then(|v| v.as_str()) else {
                    return Some(Response::err(id, error_codes::INVALID_PARAMS, "Missing page_id parameter"));
                };

                let runtime_guard = self.node_runtime.read().await;
                let Some(ref runtime) = *runtime_guard else {
                    return Some(Response::err(id, error_codes::INTERNAL_ERROR, "Node runtime not available"));
                };

                let Some(cmd_tx) = runtime.get_cmd_tx(page_id).await else {
                    return Some(Response::err(id, error_codes::NOT_FOUND, format!("No node runtime for page {}", page_id)));
                };

                let (response_tx, response_rx) = tokio::sync::oneshot::channel();
                if cmd_tx.send(lua_runtime::LuaCommand::SetTime { unix_seconds, response_tx }).await.is_err() {
                    return Some(Response::err(id, error_codes::INTERNAL_ERROR, "Failed to send set_time command"));
                }

                match response_rx.await {
                    Ok(Ok(new_time)) => Some(Response::ok(id, serde_json::json!({"unix_seconds": new_time}))),
                    Ok(Err(e)) => Some(Response::err(id, error_codes::OPERATION_FAILED, e)),
                    Err(_) => Some(Response::err(id, error_codes::INTERNAL_ERROR, "SetTime command dropped")),
                }
            }

            "advance_time" => {
                let Some(params_obj) = params.as_ref().and_then(|p| p.as_object()) else {
                    return Some(Response::err(id, error_codes::INVALID_PARAMS, "Invalid params object"));
                };

                let Some(seconds) = params_obj.get("seconds").and_then(|v| v.as_u64()) else {
                    return Some(Response::err(id, error_codes::INVALID_PARAMS, "Missing seconds parameter"));
                };

                let Some(page_id) = params_obj.get("page_id").and_then(|v| v.as_str()) else {
                    return Some(Response::err(id, error_codes::INVALID_PARAMS, "Missing page_id parameter"));
                };

                let runtime_guard = self.node_runtime.read().await;
                let Some(ref runtime) = *runtime_guard else {
                    return Some(Response::err(id, error_codes::INTERNAL_ERROR, "Node runtime not available"));
                };

                let Some(cmd_tx) = runtime.get_cmd_tx(page_id).await else {
                    return Some(Response::err(id, error_codes::NOT_FOUND, format!("No node runtime for page {}", page_id)));
                };

                let (response_tx, response_rx) = tokio::sync::oneshot::channel();
                if cmd_tx.send(lua_runtime::LuaCommand::AdvanceTime { seconds, response_tx }).await.is_err() {
                    return Some(Response::err(id, error_codes::INTERNAL_ERROR, "Failed to send advance_time command"));
                }

                match response_rx.await {
                    Ok(Ok(new_time)) => Some(Response::ok(id, serde_json::json!({"unix_seconds": new_time}))),
                    Ok(Err(e)) => Some(Response::err(id, error_codes::OPERATION_FAILED, e)),
                    Err(_) => Some(Response::err(id, error_codes::INTERNAL_ERROR, "AdvanceTime command dropped")),
                }
            }

            _ => None, // Let base server handle or return method not found
        }
    }

    async fn get_connection_string(&self) -> Option<String> {
        let state_guard = self.state.read().await;
        state_guard.as_ref().and_then(|s| s.connection_string.clone())
    }
}

/// Kunki control server (wrapper for convenience)
pub struct KunkiControlServer {
    server: ControlServer<KunkiHandler>,
    handler: Arc<KunkiHandler>,
}

impl KunkiControlServer {
    /// Create a new control server for kunki
    pub fn new(socket_path: PathBuf, instance_name: String, butler: Option<Arc<Butler>>) -> Self {
        let handler = Arc::new(KunkiHandler::new(butler));
        // Clone handler Arc before moving into ControlServer
        let handler_clone = (*handler).clone();
        let server = ControlServer::new(socket_path, instance_name, handler_clone);
        Self { server, handler }
    }

    /// Set validation handle (for testing validation RPC)
    pub async fn set_validation_handle(&self, handle: ValidationHandle) {
        self.handler.set_validation_handle(handle).await;
    }

    /// Set capture handle (for event capture)
    pub async fn set_capture_handle(&self, handle: CaptureHandle) {
        self.handler.set_capture_handle(handle).await;
    }

    /// Set node runtime manager (for test-time controls)
    pub async fn set_node_runtime(&self, runtime: Arc<crate::node_runtime::NodeRuntimeManager>) {
        self.handler.set_node_runtime(runtime).await;
    }

    /// Update node state
    pub async fn set_state(&self, state: NodeState) {
        self.handler.set_state(state).await;
    }

    /// Start the control server
    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.server.start().await
    }
}

// Implement Clone for KunkiHandler to allow wrapping in ControlServer
impl Clone for KunkiHandler {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
            butler: self.butler.clone(),
            validation_handle: self.validation_handle.clone(),
            capture_handle: self.capture_handle.clone(),
            node_runtime: self.node_runtime.clone(),
        }
    }
}
