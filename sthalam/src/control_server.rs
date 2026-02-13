//! Control Server for UI Testing Automation
//!
//! Simplified version using sthalam_shell. Removed unused commands:
//! ui_click, ui_type, ui_get_text, ui_get_screen, ui_list_elements,
//! upload_asset, update_node_address, state.

use butler::Butler;
use control_server::{
    async_trait, CommandHandler, ControlServer as BaseControlServer, Response, error_codes,
    commands::butler as butler_cmds,
};
use courier::CourierHandle;
use logging_utils::CaptureHandle;
use renderer_slint::{AppStatus, DebugEvalRequest};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, RwLock};

// UI Command (simplified — no element-level interaction)

/// UI automation command (processed on Slint main thread)
#[derive(Debug)]
pub enum UiCommand {
    DirectLogin {
        passphrase: String,
        response_tx: oneshot::Sender<Result<String, String>>,
    },
    SignUp {
        username: String,
        passphrase: String,
        response_tx: oneshot::Sender<Result<String, String>>,
    },
    OpenApp {
        page_id: String,
        app_name: String,
        response_tx: oneshot::Sender<Result<String, String>>,
    },
    CreateSpace {
        name: String,
        template_path: String,
        response_tx: oneshot::Sender<Result<(String, String), String>>,
    },
    AddNode {
        connection_string: String,
        response_tx: oneshot::Sender<Result<String, String>>,
    },
    AddWebsite {
        connection_string: String,
        response_tx: oneshot::Sender<Result<String, String>>,
    },
    RefreshApp {
        app_name: String,
        app_dir: String,
        response_tx: oneshot::Sender<Result<Vec<String>, String>>,
    },
    RefreshPage {
        page_dir: String,
        response_tx: oneshot::Sender<Result<Vec<String>, String>>,
    },
}

// Shell Handler

/// Shell-specific command handler
pub struct ShellHandler {
    butler: Arc<RwLock<Option<Arc<Butler>>>>,
    courier_handle: Arc<RwLock<Option<CourierHandle>>>,
    ui_tx: Arc<RwLock<Option<mpsc::Sender<UiCommand>>>>,
    eval_tx: Arc<RwLock<Option<mpsc::Sender<DebugEvalRequest>>>>,
    lua_worker_tx: Arc<RwLock<Option<mpsc::Sender<lua_runtime::LuaCommand>>>>,
    app_status: Arc<RwLock<AppStatus>>,
    capture_handle: Arc<RwLock<Option<CaptureHandle>>>,
}

impl ShellHandler {
    pub fn new(courier_handle: Arc<RwLock<Option<CourierHandle>>>) -> Self {
        Self {
            butler: Arc::new(RwLock::new(None)),
            courier_handle,
            ui_tx: Arc::new(RwLock::new(None)),
            eval_tx: Arc::new(RwLock::new(None)),
            lua_worker_tx: Arc::new(RwLock::new(None)),
            app_status: Arc::new(RwLock::new(AppStatus::new())),
            capture_handle: Arc::new(RwLock::new(None)),
        }
    }

    pub fn set_capture_handle_blocking(&self, handle: CaptureHandle) {
        *self.capture_handle.blocking_write() = Some(handle);
    }

    pub fn app_status(&self) -> Arc<RwLock<AppStatus>> {
        self.app_status.clone()
    }

    pub fn set_butler_blocking(&self, butler: Arc<Butler>) {
        *self.butler.blocking_write() = Some(butler);
    }

    pub fn set_ui_channel_blocking(&self, tx: mpsc::Sender<UiCommand>) {
        *self.ui_tx.blocking_write() = Some(tx);
    }

    pub fn set_eval_channel_blocking(&self, tx: mpsc::Sender<DebugEvalRequest>) {
        *self.eval_tx.blocking_write() = Some(tx);
    }

    pub fn set_lua_worker_channel_blocking(&self, tx: mpsc::Sender<lua_runtime::LuaCommand>) {
        *self.lua_worker_tx.blocking_write() = Some(tx);
    }
}

impl Clone for ShellHandler {
    fn clone(&self) -> Self {
        Self {
            butler: self.butler.clone(),
            courier_handle: self.courier_handle.clone(),
            ui_tx: self.ui_tx.clone(),
            eval_tx: self.eval_tx.clone(),
            lua_worker_tx: self.lua_worker_tx.clone(),
            app_status: self.app_status.clone(),
            capture_handle: self.capture_handle.clone(),
        }
    }
}

/// Helper to get string param
fn get_param(params: &Option<serde_json::Value>, key: &str) -> Option<String> {
    params.as_ref()?.get(key)?.as_str().map(|s| s.to_string())
}

#[async_trait]
impl CommandHandler for ShellHandler {
    async fn handle(&self, method: &str, params: Option<serde_json::Value>, id: u64) -> Option<Response> {
        match method {
            // Lua Eval
            "eval" => {
                let code = get_param(&params, "code")?;
                let eval_tx = self.eval_tx.read().await;
                let Some(tx) = eval_tx.as_ref() else {
                    return Some(Response::err(id, error_codes::INTERNAL_ERROR, "Lua eval not available - no app is open"));
                };
                let (response_tx, response_rx) = oneshot::channel();
                if tx.send(DebugEvalRequest { code, response_tx }).await.is_err() {
                    return Some(Response::err(id, error_codes::INTERNAL_ERROR, "Failed to send eval request"));
                }
                match response_rx.await {
                    Ok(Ok(value)) => Some(Response::ok(id, value)),
                    Ok(Err(e)) => Some(Response::err(id, error_codes::OPERATION_FAILED, e)),
                    Err(_) => Some(Response::err(id, error_codes::INTERNAL_ERROR, "Eval request dropped")),
                }
            }

            // Butler Commands (shared)
            "list_spaces" => {
                let butler = self.butler.read().await;
                match butler.as_ref() {
                    Some(b) => Some(butler_cmds::list_spaces(b, id).await),
                    None => Some(Response::err(id, error_codes::NOT_AUTHENTICATED, "Butler not available")),
                }
            }

            "list_pages" => {
                let space_id = get_param(&params, "space_id")?;
                let butler = self.butler.read().await;
                match butler.as_ref() {
                    Some(b) => Some(butler_cmds::list_pages(b, &space_id, id).await),
                    None => Some(Response::err(id, error_codes::NOT_AUTHENTICATED, "Butler not available")),
                }
            }

            "list_layers" => {
                let page_id = get_param(&params, "page_id")?;
                let butler = self.butler.read().await;
                match butler.as_ref() {
                    Some(b) => Some(butler_cmds::list_layers(b, &page_id, id).await),
                    None => Some(Response::err(id, error_codes::NOT_AUTHENTICATED, "Butler not available")),
                }
            }

            "list_apps" => {
                let page_id = get_param(&params, "page_id")?;
                let butler = self.butler.read().await;
                let Some(b) = butler.as_ref() else {
                    return Some(Response::err(id, error_codes::NOT_AUTHENTICATED, "Butler not available"));
                };
                match b.apps().list(&page_id) {
                    Ok(apps) => {
                        let apps_json: Vec<serde_json::Value> = apps.iter()
                            .map(|name| serde_json::json!({"name": name}))
                            .collect();
                        Some(Response::ok(id, serde_json::json!({
                            "page_id": page_id,
                            "apps": apps_json,
                            "count": apps_json.len()
                        })))
                    }
                    Err(e) => Some(Response::err(id, error_codes::OPERATION_FAILED, format!("Failed to list apps: {}", e))),
                }
            }

            "list_nodes" => {
                let butler = self.butler.read().await;
                let Some(b) = butler.as_ref() else {
                    return Some(Response::err(id, error_codes::NOT_AUTHENTICATED, "Butler not available"));
                };
                match b.nodes().list() {
                    Ok(nodes) => {
                        let nodes_json: Vec<serde_json::Value> = nodes.iter()
                            .map(|n| serde_json::json!({
                                "node_id": n.node_id,
                                "name": n.name,
                                "connected": n.is_connected,
                            }))
                            .collect();
                        Some(Response::ok(id, serde_json::json!({
                            "nodes": nodes_json,
                            "count": nodes_json.len()
                        })))
                    }
                    Err(e) => Some(Response::err(id, error_codes::OPERATION_FAILED, format!("Failed to list nodes: {}", e))),
                }
            }

            "is_node_authenticated" => {
                let node_id = get_param(&params, "node_id")?;

                let courier = self.courier_handle.read().await;
                let Some(courier_handle) = courier.as_ref() else {
                    return Some(Response::err(id, error_codes::INTERNAL_ERROR, "P2P not connected"));
                };

                match courier_handle.is_node_authenticated(&node_id).await {
                    Ok(is_auth) => Some(Response::ok(id, serde_json::json!({
                        "node_id": node_id,
                        "authenticated": is_auth
                    }))),
                    Err(e) => Some(Response::err(id, error_codes::OPERATION_FAILED, format!("Failed to check auth: {}", e))),
                }
            }

            // Auth Commands
            "login" => {
                let passphrase = get_param(&params, "passphrase")?;
                let ui_tx = self.ui_tx.read().await;
                let Some(tx) = ui_tx.as_ref() else {
                    return Some(Response::err(id, error_codes::INTERNAL_ERROR, "UI automation not available"));
                };
                let (response_tx, response_rx) = oneshot::channel();
                if tx.send(UiCommand::DirectLogin { passphrase, response_tx }).await.is_err() {
                    return Some(Response::err(id, error_codes::INTERNAL_ERROR, "Failed to send login command"));
                }
                match response_rx.await {
                    Ok(Ok(did)) => Some(Response::ok(id, serde_json::json!({"success": true, "did": did}))),
                    Ok(Err(e)) => Some(Response::err(id, error_codes::OPERATION_FAILED, e)),
                    Err(_) => Some(Response::err(id, error_codes::INTERNAL_ERROR, "Login command dropped")),
                }
            }

            "sign_up" => {
                let username = get_param(&params, "username")?;
                let passphrase = get_param(&params, "passphrase")?;
                let ui_tx = self.ui_tx.read().await;
                let Some(tx) = ui_tx.as_ref() else {
                    return Some(Response::err(id, error_codes::INTERNAL_ERROR, "UI automation not available"));
                };
                let (response_tx, response_rx) = oneshot::channel();
                if tx.send(UiCommand::SignUp { username, passphrase, response_tx }).await.is_err() {
                    return Some(Response::err(id, error_codes::INTERNAL_ERROR, "Failed to send signup command"));
                }
                match response_rx.await {
                    Ok(Ok(did)) => Some(Response::ok(id, serde_json::json!({"success": true, "did": did}))),
                    Ok(Err(e)) => Some(Response::err(id, error_codes::OPERATION_FAILED, e)),
                    Err(_) => Some(Response::err(id, error_codes::INTERNAL_ERROR, "Signup command dropped")),
                }
            }

            // App Commands
            "open_app" => {
                let page_id = get_param(&params, "page_id")?;
                let app_name = get_param(&params, "app_name")?;

                {
                    let mut status = self.app_status.write().await;
                    status.page_id = Some(page_id.clone());
                    status.app_name = Some(app_name.clone());
                    status.status = "loading".to_string();
                    status.error = None;
                    status.loaded_at = None;
                }

                let ui_tx = self.ui_tx.read().await;
                let Some(tx) = ui_tx.as_ref() else {
                    return Some(Response::err(id, error_codes::INTERNAL_ERROR, "UI automation not available"));
                };
                let (response_tx, response_rx) = oneshot::channel();
                if tx.send(UiCommand::OpenApp { page_id: page_id.clone(), app_name: app_name.clone(), response_tx }).await.is_err() {
                    return Some(Response::err(id, error_codes::INTERNAL_ERROR, "Failed to send open_app command"));
                }
                match response_rx.await {
                    Ok(Ok(msg)) => Some(Response::ok(id, serde_json::json!({
                        "success": true,
                        "page_id": page_id,
                        "app_name": app_name,
                        "message": msg
                    }))),
                    Ok(Err(e)) => Some(Response::err(id, error_codes::OPERATION_FAILED, e)),
                    Err(_) => Some(Response::err(id, error_codes::INTERNAL_ERROR, "Open app command dropped")),
                }
            }

            "get_app_status" => {
                let status = self.app_status.read().await.clone();
                Some(Response::ok(id, serde_json::to_value(status).unwrap_or(serde_json::Value::Null)))
            }

            "refresh_app" => {
                let app_name = get_param(&params, "app_name")?;
                let app_dir = get_param(&params, "app_dir")?;
                let ui_tx = self.ui_tx.read().await;
                let Some(tx) = ui_tx.as_ref() else {
                    return Some(Response::err(id, error_codes::INTERNAL_ERROR, "UI automation not available"));
                };
                let (response_tx, response_rx) = oneshot::channel();
                if tx.send(UiCommand::RefreshApp { app_name: app_name.clone(), app_dir: app_dir.clone(), response_tx }).await.is_err() {
                    return Some(Response::err(id, error_codes::INTERNAL_ERROR, "Failed to send refresh_app command"));
                }
                match response_rx.await {
                    Ok(Ok(changed_files)) => Some(Response::ok(id, serde_json::json!({
                        "success": true,
                        "app_name": app_name,
                        "app_dir": app_dir,
                        "changed_files": changed_files,
                        "changed_count": changed_files.len()
                    }))),
                    Ok(Err(e)) => Some(Response::err(id, error_codes::OPERATION_FAILED, e)),
                    Err(_) => Some(Response::err(id, error_codes::INTERNAL_ERROR, "RefreshApp command dropped")),
                }
            }

            "refresh_page" => {
                let page_dir = get_param(&params, "page_dir")?;
                let ui_tx = self.ui_tx.read().await;
                let Some(tx) = ui_tx.as_ref() else {
                    return Some(Response::err(id, error_codes::INTERNAL_ERROR, "UI automation not available"));
                };
                let (response_tx, response_rx) = oneshot::channel();
                if tx.send(UiCommand::RefreshPage { page_dir: page_dir.clone(), response_tx }).await.is_err() {
                    return Some(Response::err(id, error_codes::INTERNAL_ERROR, "Failed to send refresh_page command"));
                }
                match response_rx.await {
                    Ok(Ok(updated_apps)) => Some(Response::ok(id, serde_json::json!({
                        "success": true,
                        "page_dir": page_dir,
                        "updated_apps": updated_apps,
                        "updated_count": updated_apps.len()
                    }))),
                    Ok(Err(e)) => Some(Response::err(id, error_codes::OPERATION_FAILED, e)),
                    Err(_) => Some(Response::err(id, error_codes::INTERNAL_ERROR, "RefreshPage command dropped")),
                }
            }

            // Space/Page Management
            "create_space" => {
                let name = get_param(&params, "name")?;
                let template_path = get_param(&params, "template_path")?;

                // Validate Slint files before creating
                let template_dir = PathBuf::from(&template_path);
                if template_dir.exists() {
                    if let Err(e) = renderer_slint::validate_slint_files(&template_dir).await {
                        return Some(Response::err(id, error_codes::OPERATION_FAILED, e));
                    }
                }

                let ui_tx = self.ui_tx.read().await;
                let Some(tx) = ui_tx.as_ref() else {
                    return Some(Response::err(id, error_codes::INTERNAL_ERROR, "UI automation not available"));
                };
                let (response_tx, response_rx) = oneshot::channel();
                if tx.send(UiCommand::CreateSpace { name, template_path, response_tx }).await.is_err() {
                    return Some(Response::err(id, error_codes::INTERNAL_ERROR, "Failed to send create_space command"));
                }
                match response_rx.await {
                    Ok(Ok((space_id, space_name))) => Some(Response::ok(id, serde_json::json!({
                        "id": space_id,
                        "name": space_name,
                    }))),
                    Ok(Err(e)) => Some(Response::err(id, error_codes::OPERATION_FAILED, e)),
                    Err(_) => Some(Response::err(id, error_codes::INTERNAL_ERROR, "Create space command dropped")),
                }
            }

            "import_page" => {
                let space_id = get_param(&params, "space_id")?;
                let page_dir = get_param(&params, "page_dir")?;

                let butler = self.butler.read().await;
                let Some(b) = butler.as_ref() else {
                    return Some(Response::err(id, error_codes::NOT_AUTHENTICATED, "Butler not available"));
                };

                let page_path = PathBuf::from(&page_dir);
                if !page_path.exists() {
                    return Some(Response::err(id, error_codes::OPERATION_FAILED, format!("Page directory not found: {}", page_dir)));
                }

                // Validate Slint files before importing
                if let Err(e) = renderer_slint::validate_slint_files(&page_path).await {
                    return Some(Response::err(id, error_codes::OPERATION_FAILED, e));
                }

                match b.apps().import_page(&space_id, &page_path).await {
                    Ok(page) => {
                        let apps = b.apps().list(&page.id).unwrap_or_default();
                        Some(Response::ok(id, serde_json::json!({
                            "page_id": page.id,
                            "page_name": page.name,
                            "apps": apps,
                        })))
                    }
                    Err(e) => Some(Response::err(id, error_codes::OPERATION_FAILED, format!("Failed to import page: {}", e))),
                }
            }

            // Node/P2P Commands
            "add_node" => {
                let connection_string = get_param(&params, "connection_string")?;
                let ui_tx = self.ui_tx.read().await;
                let Some(tx) = ui_tx.as_ref() else {
                    return Some(Response::err(id, error_codes::INTERNAL_ERROR, "UI automation not available"));
                };
                let (response_tx, response_rx) = oneshot::channel();
                if tx.send(UiCommand::AddNode { connection_string, response_tx }).await.is_err() {
                    return Some(Response::err(id, error_codes::INTERNAL_ERROR, "Failed to send add_node command"));
                }
                match response_rx.await {
                    Ok(Ok(node_id)) => Some(Response::ok(id, serde_json::json!({
                        "success": true,
                        "node_id": node_id,
                        "triggered": true
                    }))),
                    Ok(Err(e)) => Some(Response::err(id, error_codes::OPERATION_FAILED, e)),
                    Err(_) => Some(Response::err(id, error_codes::INTERNAL_ERROR, "Add node command dropped")),
                }
            }

            "add_website" => {
                let connection_string = get_param(&params, "connection_string")?;
                let ui_tx = self.ui_tx.read().await;
                let Some(tx) = ui_tx.as_ref() else {
                    return Some(Response::err(id, error_codes::INTERNAL_ERROR, "UI automation not available"));
                };
                let (response_tx, response_rx) = oneshot::channel();
                if tx.send(UiCommand::AddWebsite { connection_string, response_tx }).await.is_err() {
                    return Some(Response::err(id, error_codes::INTERNAL_ERROR, "Failed to send add_website command"));
                }
                match response_rx.await {
                    Ok(Ok(space_id)) => Some(Response::ok(id, serde_json::json!({
                        "success": true,
                        "space_id": space_id,
                        "triggered": true
                    }))),
                    Ok(Err(e)) => Some(Response::err(id, error_codes::OPERATION_FAILED, e)),
                    Err(_) => Some(Response::err(id, error_codes::INTERNAL_ERROR, "Add website command dropped")),
                }
            }

            "publish_space" => {
                let space_id = get_param(&params, "space_id")?;
                let node_id = get_param(&params, "node_id")?;

                let butler = self.butler.read().await;
                let Some(b) = butler.as_ref() else {
                    return Some(Response::err(id, error_codes::NOT_AUTHENTICATED, "Butler not available"));
                };

                let node = match b.nodes().get(&node_id) {
                    Ok(Some(n)) => n,
                    Ok(None) => return Some(Response::err(id, error_codes::NOT_FOUND, format!("Node not found: {}", node_id))),
                    Err(e) => return Some(Response::err(id, error_codes::OPERATION_FAILED, format!("Failed to get node: {}", e))),
                };

                let courier = self.courier_handle.read().await;
                let Some(courier_handle) = courier.as_ref() else {
                    return Some(Response::err(id, error_codes::INTERNAL_ERROR, "P2P not connected"));
                };

                match courier_handle.publish_space(&space_id, &node_id).await {
                    Ok(_) => {
                        let _ = b.publish().mark_space_published(&space_id, &node_id);
                        Some(Response::ok(id, serde_json::json!({
                            "success": true,
                            "space_id": space_id,
                            "node_id": node_id,
                            "node_name": node.name
                        })))
                    }
                    Err(e) => Some(Response::err(id, error_codes::OPERATION_FAILED, format!("Failed to publish space: {}", e))),
                }
            }

            "get_shareable_link" => {
                let space_id = get_param(&params, "space_id")?;
                let node_id = get_param(&params, "node_id")?;

                let courier = self.courier_handle.read().await;
                let Some(courier_handle) = courier.as_ref() else {
                    return Some(Response::err(id, error_codes::INTERNAL_ERROR, "P2P not connected"));
                };

                match courier_handle.get_shareable_link(&space_id, &node_id).await {
                    Ok(connection_string) => Some(Response::ok(id, serde_json::json!({
                        "connection_string": connection_string,
                        "space_id": space_id,
                        "node_id": node_id
                    }))),
                    Err(e) => Some(Response::err(id, error_codes::OPERATION_FAILED, format!("Failed to get shareable link: {}", e))),
                }
            }

            "p2p_status" => {
                let butler_ready = self.butler.read().await.is_some();
                let courier_ready = self.courier_handle.read().await.is_some();
                Some(Response::ok(id, serde_json::json!({
                    "butler_ready": butler_ready,
                    "courier_ready": courier_ready,
                    "p2p_ready": butler_ready && courier_ready
                })))
            }

            // Capture commands
            "capture_start" => {
                let handle_guard = self.capture_handle.read().await;
                let Some(ref handle) = *handle_guard else {
                    return Some(Response::err(id, error_codes::INTERNAL_ERROR, "CaptureHandle not available"));
                };

                let Some(params_obj) = params.as_ref().and_then(|p| p.as_object()) else {
                    return Some(Response::err(id, error_codes::INVALID_PARAMS, "Invalid params object"));
                };

                let Some(file_path) = params_obj.get("file_path").and_then(|v| v.as_str()) else {
                    return Some(Response::err(id, error_codes::INVALID_PARAMS, "Missing file_path"));
                };

                let include_logs = params_obj.get("include_logs").and_then(|v| v.as_bool()).unwrap_or(false);

                match handle.start_capture(PathBuf::from(file_path), include_logs).await {
                    Ok(()) => Some(Response::ok(id, serde_json::json!({"status": "capturing", "file_path": file_path}))),
                    Err(e) => Some(Response::err(id, error_codes::INTERNAL_ERROR, &e)),
                }
            }

            "capture_end" => {
                let handle_guard = self.capture_handle.read().await;
                let Some(ref handle) = *handle_guard else {
                    return Some(Response::err(id, error_codes::INTERNAL_ERROR, "CaptureHandle not available"));
                };

                match handle.stop_capture().await {
                    Ok(()) => Some(Response::ok(id, serde_json::json!({"status": "stopped"}))),
                    Err(e) => Some(Response::err(id, error_codes::INTERNAL_ERROR, &e)),
                }
            }

            _ => None,
        }
    }

    async fn get_connection_string(&self) -> Option<String> {
        let butler = self.butler.read().await;
        let b = butler.as_ref()?;
        b.nodes().generate_connection_string(None).await.ok()
    }
}

// Public API

/// Shell control server wrapper
pub struct ControlServer {
    inner: BaseControlServer<ShellHandler>,
    handler: Arc<ShellHandler>,
}

impl ControlServer {
    /// Create a new control server
    pub fn new(
        socket_path: PathBuf,
        instance_name: String,
        courier_handle: Arc<RwLock<Option<CourierHandle>>>,
    ) -> Self {
        let handler = Arc::new(ShellHandler::new(courier_handle));
        let inner = BaseControlServer::new(socket_path, instance_name, (*handler).clone());
        Self { inner, handler }
    }

    pub fn app_status(&self) -> Arc<RwLock<AppStatus>> {
        self.handler.app_status()
    }

    pub fn set_butler(&self, butler: Arc<Butler>) {
        self.handler.set_butler_blocking(butler);
    }

    pub fn set_ui_channel(&self, tx: mpsc::Sender<UiCommand>) {
        self.handler.set_ui_channel_blocking(tx);
    }

    pub fn set_eval_channel(&self, tx: mpsc::Sender<DebugEvalRequest>) {
        self.handler.set_eval_channel_blocking(tx);
    }

    pub fn set_capture_handle(&self, handle: CaptureHandle) {
        self.handler.set_capture_handle_blocking(handle);
    }

    pub fn set_lua_worker_channel(&self, tx: mpsc::Sender<lua_runtime::LuaCommand>) {
        self.handler.set_lua_worker_channel_blocking(tx);
    }

    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.inner.start().await
    }
}
