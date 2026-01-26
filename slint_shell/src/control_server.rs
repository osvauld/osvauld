//! Control Server for UI Testing Automation
//!
//! Provides a Unix socket interface for:
//! - Executing Lua code via the LuaWorker
//! - Inspecting application state
//! - UI automation commands (via Slint testing backend)

use butler::Butler;
use courier::CourierHandle;
use serde::{Deserialize, Serialize};
use slint_interpreter::Compiler;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{mpsc, oneshot, RwLock};
use tracing::{debug, error, info, warn};
use walkdir::WalkDir;

/// Debug command received from client
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "method", rename_all = "snake_case")]
pub enum DebugCommand {
    /// Execute Lua code and return result
    Eval {
        params: EvalParams,
        id: u64,
    },
    /// Get current state snapshot
    State {
        id: u64,
    },
    /// UI automation: click element by accessible-label
    UiClick {
        params: UiClickParams,
        id: u64,
    },
    /// UI automation: type text into element
    UiType {
        params: UiTypeParams,
        id: u64,
    },
    /// UI automation: get text from element
    UiGetText {
        params: UiGetTextParams,
        id: u64,
    },
    /// UI automation: get current screen name
    UiGetScreen {
        id: u64,
    },
    /// Ping to check connection
    Ping {
        id: u64,
    },
    /// Reload the current app (re-read Lua code from disk)
    Reload {
        params: Option<ReloadParams>,
        id: u64,
    },
    /// Get connection string for this instance (for inter-instance workflows)
    GetConnectionString {
        id: u64,
    },
    /// Debug: list all accessible elements in the UI
    UiListElements {
        id: u64,
    },
    /// Direct login command (bypasses UI automation limitation)
    Login {
        params: LoginParams,
        id: u64,
    },
    /// Sign up a new user
    SignUp {
        params: SignUpParams,
        id: u64,
    },
    /// Open an app programmatically (bypasses UI navigation)
    OpenApp {
        params: OpenAppParams,
        id: u64,
    },
    /// List all spaces
    ListSpaces {
        id: u64,
    },
    /// List pages in a space
    ListPages {
        params: ListPagesParams,
        id: u64,
    },
    /// List apps in a page
    ListApps {
        params: ListAppsParams,
        id: u64,
    },
    /// Create a new space
    CreateSpace {
        params: CreateSpaceParams,
        id: u64,
    },
    /// Import a page from a directory
    ImportPage {
        params: ImportPageParams,
        id: u64,
    },
    /// Add a sovereign node by connection string
    AddNode {
        params: AddNodeParams,
        id: u64,
    },
    /// Publish a space to a node
    PublishSpace {
        params: PublishSpaceParams,
        id: u64,
    },
    /// Get shareable viewer link for a space
    GetShareableLink {
        params: GetShareableLinkParams,
        id: u64,
    },
    /// Connect to a space as viewer (via connection string)
    AddWebsite {
        params: AddWebsiteParams,
        id: u64,
    },
    /// List nodes
    ListNodes {
        id: u64,
    },
    /// Check P2P status
    P2pStatus {
        id: u64,
    },
    /// Update node address and reconnect
    UpdateNodeAddress {
        params: UpdateNodeAddressParams,
        id: u64,
    },
    /// Refresh app from filesystem (reload Lua/Slint from disk)
    RefreshApp {
        params: RefreshAppParams,
        id: u64,
    },
    /// Refresh entire page from filesystem (reload ALL apps from root directory)
    RefreshPage {
        params: RefreshPageParams,
        id: u64,
    },
    /// Get current app loading status (for AI feedback)
    GetAppStatus {
        id: u64,
    },
    /// Upload an asset to a page (for integration testing, bypasses UI file picker)
    UploadAsset {
        params: UploadAssetParams,
        id: u64,
    },
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoginParams {
    pub passphrase: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SignUpParams {
    pub username: String,
    pub passphrase: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OpenAppParams {
    pub page_id: String,
    pub app_name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ListPagesParams {
    pub space_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ListAppsParams {
    pub page_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateSpaceParams {
    pub name: String,
    /// Path to app folder containing space_permit_template.json (e.g., sample_apps/my-booking)
    pub template_path: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ImportPageParams {
    pub space_id: String,
    pub page_dir: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AddNodeParams {
    pub connection_string: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PublishSpaceParams {
    pub space_id: String,
    pub node_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GetShareableLinkParams {
    pub space_id: String,
    pub node_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AddWebsiteParams {
    pub connection_string: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateNodeAddressParams {
    /// Connection string with new relay URL (from running kunki)
    pub connection_string: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RefreshAppParams {
    /// Name of the app to refresh
    pub app_name: String,
    /// Directory containing app source files
    pub app_dir: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RefreshPageParams {
    /// Root directory containing the page (with multiple app subdirectories)
    pub page_dir: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UploadAssetParams {
    /// ID of the page to upload the asset to
    pub page_id: String,
    /// Path to the file to upload
    pub file_path: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ReloadParams {
    /// Optional specific app/page to reload (default: current)
    pub page_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EvalParams {
    pub code: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UiClickParams {
    pub label: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UiTypeParams {
    pub label: String,
    pub text: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UiGetTextParams {
    pub label: String,
}

/// Debug response sent to client
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum DebugResponse {
    Result {
        result: serde_json::Value,
        id: u64,
    },
    Error {
        error: DebugError,
        id: u64,
    },
    Stream {
        stream: String,
        data: serde_json::Value,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct DebugError {
    pub code: i32,
    pub message: String,
}

/// State snapshot for debugging
#[derive(Debug, Clone, Serialize)]
pub struct DebugState {
    pub layers: Vec<LayerInfo>,
    pub ui_props: serde_json::Value,
    pub permit: Option<PermitInfo>,
}

/// App loading status for AI feedback
#[derive(Debug, Clone, Serialize, Default)]
pub struct AppStatus {
    pub page_id: Option<String>,
    pub app_name: Option<String>,
    /// Status: "idle", "loading", "loaded", "failed"
    pub status: String,
    pub error: Option<String>,
    pub loaded_at: Option<String>,
    /// App version (semantic + content hash, e.g., "1.0.0-a1b2c3d4")
    pub version: Option<String>,
}

impl AppStatus {
    pub fn new() -> Self {
        Self {
            page_id: None,
            app_name: None,
            status: "idle".to_string(),
            error: None,
            loaded_at: None,
            version: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct LayerInfo {
    pub name: String,
    pub size: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct PermitInfo {
    pub role: String,
    pub page_id: String,
    pub our_did: String,
}

/// Command to send to LuaWorker for debug evaluation
pub struct DebugEvalRequest {
    pub code: String,
    pub response_tx: oneshot::Sender<Result<serde_json::Value, String>>,
}

/// UI automation command (processed on Slint main thread)
#[derive(Debug)]
pub enum UiCommand {
    Click {
        label: String,
        response_tx: oneshot::Sender<Result<(), String>>,
    },
    Type {
        label: String,
        text: String,
        response_tx: oneshot::Sender<Result<(), String>>,
    },
    GetText {
        label: String,
        response_tx: oneshot::Sender<Result<String, String>>,
    },
    GetScreen {
        response_tx: oneshot::Sender<String>,
    },
    /// Debug: list all accessible elements
    ListElements {
        response_tx: oneshot::Sender<Vec<ElementInfo>>,
    },
    /// Direct login (bypasses UI automation)
    DirectLogin {
        passphrase: String,
        response_tx: oneshot::Sender<Result<String, String>>,
    },
    /// Open app programmatically (sets page_id and invokes select_app)
    OpenApp {
        page_id: String,
        app_name: String,
        response_tx: oneshot::Sender<Result<String, String>>,
    },
    /// Sign up new user
    SignUp {
        username: String,
        passphrase: String,
        response_tx: oneshot::Sender<Result<String, String>>,
    },
    /// Refresh app from filesystem via Scribe
    RefreshApp {
        app_name: String,
        app_dir: String,
        response_tx: oneshot::Sender<Result<Vec<String>, String>>,
    },
    /// Refresh entire page from filesystem (all apps in directory)
    RefreshPage {
        page_dir: String,
        response_tx: oneshot::Sender<Result<Vec<String>, String>>,
    },
    /// Create a new space (routes through shell callback to update UI)
    CreateSpace {
        name: String,
        template_path: String,
        response_tx: oneshot::Sender<Result<(String, String), String>>,  // (space_id, name)
    },
    /// Add a website/space as viewer (routes through shell callback to update UI)
    AddWebsite {
        connection_string: String,
        response_tx: oneshot::Sender<Result<String, String>>,  // space_id
    },
    /// Add a sovereign node (routes through shell callback to update UI)
    AddNode {
        connection_string: String,
        response_tx: oneshot::Sender<Result<String, String>>,  // node_id
    },
}

/// Information about an accessible element
#[derive(Debug, Clone, Serialize)]
pub struct ElementInfo {
    pub id: String,
    pub type_name: String,
    pub accessible_label: Option<String>,
    pub accessible_role: String,
}

/// Validate all Slint files in a directory before import
///
/// **Context**: Called before importing a page to catch compilation errors early
/// **Returns**: Ok(()) if all Slint files compile, Err with details otherwise
async fn validate_slint_files(page_dir: &Path) -> Result<(), String> {
    // Collect all .slint file paths first
    let slint_files: Vec<PathBuf> = WalkDir::new(page_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "slint"))
        .map(|e| e.path().to_path_buf())
        .collect();

    if slint_files.is_empty() {
        return Ok(());
    }

    // Validate in a blocking task since Slint Compiler isn't Send
    let result = tokio::task::spawn_blocking(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| format!("Failed to create runtime: {}", e))?;

        let mut errors = Vec::new();

        for slint_path in slint_files {
            info!("Validating Slint file: {}", slint_path.display());

            let compiler = Compiler::default();
            let result = rt.block_on(compiler.build_from_path(&slint_path));

            // Check for compilation errors
            let compile_errors: Vec<_> = result
                .diagnostics()
                .filter(|d| d.level() == slint_interpreter::DiagnosticLevel::Error)
                .map(|d| d.to_string())
                .collect();

            if !compile_errors.is_empty() {
                errors.push(format!(
                    "{}:\n  {}",
                    slint_path.display(),
                    compile_errors.join("\n  ")
                ));
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(format!("Slint compilation errors:\n{}", errors.join("\n")))
        }
    })
    .await
    .map_err(|e| format!("Validation task failed: {}", e))?;

    result
}

/// Debug server handle for managing the server
pub struct ControlServer {
    socket_path: PathBuf,
    /// Channel to send eval requests to LuaWorker (runtime-settable when app opens)
    eval_tx: Arc<RwLock<Option<mpsc::Sender<DebugEvalRequest>>>>,
    /// Channel to send UI commands to Slint main thread
    ui_tx: Option<mpsc::Sender<UiCommand>>,
    /// Instance name for this debug server
    instance_name: String,
    /// Butler instance for connection string generation
    butler: Option<Arc<Butler>>,
    /// Courier handle for P2P operations
    courier_handle: Option<Arc<RwLock<Option<CourierHandle>>>>,
    /// Current app loading status (for AI feedback)
    app_status: Arc<RwLock<AppStatus>>,
    /// Channel to send commands to LuaWorker (for AssetUploaded callback)
    lua_worker_tx: Arc<RwLock<Option<mpsc::Sender<app_runtime::LuaWorkerCommand>>>>,
}

impl ControlServer {
    /// Create a new debug server
    pub fn new(socket_path: PathBuf, instance_name: String) -> Self {
        Self {
            socket_path,
            eval_tx: Arc::new(RwLock::new(None)),
            ui_tx: None,
            instance_name,
            butler: None,
            courier_handle: None,
            app_status: Arc::new(RwLock::new(AppStatus::new())),
            lua_worker_tx: Arc::new(RwLock::new(None)),
        }
    }

    /// Get a reference to the app status for external updates
    pub fn app_status(&self) -> Arc<RwLock<AppStatus>> {
        self.app_status.clone()
    }

    /// Set the Butler instance for connection string generation
    pub fn set_butler(&mut self, butler: Arc<Butler>) {
        self.butler = Some(butler);
    }

    /// Set the Courier handle for P2P operations
    pub fn set_courier_handle(&mut self, courier: Arc<RwLock<Option<CourierHandle>>>) {
        self.courier_handle = Some(courier);
    }

    /// Set the eval channel for routing Lua commands (can be called after server starts)
    pub fn set_eval_channel(&self, tx: mpsc::Sender<DebugEvalRequest>) {
        // Use blocking_write since this is typically called from sync context
        let mut guard = self.eval_tx.blocking_write();
        *guard = Some(tx);
    }

    /// Set the LuaWorker command channel (for triggering Lua callbacks like AssetUploaded)
    pub fn set_lua_worker_channel(&self, tx: mpsc::Sender<app_runtime::LuaWorkerCommand>) {
        let mut guard = self.lua_worker_tx.blocking_write();
        *guard = Some(tx);
    }

    /// Set the UI command channel for routing UI automation commands
    pub fn set_ui_channel(&mut self, tx: mpsc::Sender<UiCommand>) {
        self.ui_tx = Some(tx);
    }

    /// Start the debug server
    pub async fn start(self: Arc<Self>) -> Result<(), std::io::Error> {
        // Remove existing socket file if present
        if self.socket_path.exists() {
            std::fs::remove_file(&self.socket_path)?;
        }

        let listener = UnixListener::bind(&self.socket_path)?;
        info!(socket_path = ?self.socket_path, "Debug server listening");

        loop {
            match listener.accept().await {
                Ok((stream, _addr)) => {
                    let server = self.clone();
                    tokio::spawn(async move {
                        if let Err(e) = server.handle_connection(stream).await {
                            error!(error = %e, "Debug connection error");
                        }
                    });
                }
                Err(e) => {
                    error!(error = %e, "Failed to accept debug connection");
                }
            }
        }
    }

    /// Handle a single client connection
    async fn handle_connection(&self, stream: UnixStream) -> Result<(), std::io::Error> {
        let (reader, mut writer) = stream.into_split();
        let mut reader = BufReader::new(reader);
        let mut line = String::new();

        info!(instance = %self.instance_name, "Debug client connected");

        loop {
            match reader.read_line(&mut line).await {
                Ok(0) => {
                    // EOF - client disconnected
                    info!("Debug client disconnected");
                    break;
                }
                Ok(_) => {
                    let response = self.handle_command(&line).await;
                    let response_json = serde_json::to_string(&response)
                        .unwrap_or_else(|_| r#"{"error": "serialization failed"}"#.to_string());
                    writer.write_all(response_json.as_bytes()).await?;
                    writer.write_all(b"\n").await?;
                    writer.flush().await?;
                    line.clear();
                }
                Err(e) => {
                    error!(error = %e, "Read error");
                    break;
                }
            }
        }

        Ok(())
    }

    /// Handle a single command
    async fn handle_command(&self, line: &str) -> DebugResponse {
        let cmd: DebugCommand = match serde_json::from_str(line.trim()) {
            Ok(cmd) => cmd,
            Err(e) => {
                return DebugResponse::Error {
                    error: DebugError {
                        code: -32700,
                        message: format!("Parse error: {}", e),
                    },
                    id: 0,
                };
            }
        };

        match cmd {
            DebugCommand::Ping { id } => DebugResponse::Result {
                result: serde_json::json!({"status": "ok", "instance": self.instance_name}),
                id,
            },

            DebugCommand::Eval { params, id } => {
                self.handle_eval(params.code, id).await
            }

            DebugCommand::State { id } => {
                self.handle_state(id).await
            }

            DebugCommand::UiClick { params, id } => {
                self.handle_ui_click(params.label, id).await
            }

            DebugCommand::UiType { params, id } => {
                self.handle_ui_type(params.label, params.text, id).await
            }

            DebugCommand::UiGetText { params, id } => {
                self.handle_ui_get_text(params.label, id).await
            }

            DebugCommand::UiGetScreen { id } => {
                self.handle_ui_get_screen(id).await
            }

            DebugCommand::Reload { params, id } => {
                self.handle_reload(params, id).await
            }

            DebugCommand::GetConnectionString { id } => {
                self.handle_get_connection_string(id).await
            }

            DebugCommand::UiListElements { id } => {
                self.handle_ui_list_elements(id).await
            }

            DebugCommand::Login { params, id } => {
                self.handle_login(params.passphrase, id).await
            }

            DebugCommand::SignUp { params, id } => {
                self.handle_signup(params.username, params.passphrase, id).await
            }

            DebugCommand::OpenApp { params, id } => {
                self.handle_open_app(params.page_id, params.app_name, id).await
            }

            DebugCommand::ListSpaces { id } => {
                self.handle_list_spaces(id).await
            }

            DebugCommand::ListPages { params, id } => {
                self.handle_list_pages(params.space_id, id).await
            }

            DebugCommand::ListApps { params, id } => {
                self.handle_list_apps(params.page_id, id).await
            }

            DebugCommand::CreateSpace { params, id } => {
                self.handle_create_space(params.name, &params.template_path, id).await
            }

            DebugCommand::ImportPage { params, id } => {
                self.handle_import_page(params.space_id, params.page_dir, id).await
            }

            DebugCommand::AddNode { params, id } => {
                self.handle_add_node(params.connection_string, id).await
            }

            DebugCommand::PublishSpace { params, id } => {
                self.handle_publish_space(params.space_id, params.node_id, id).await
            }

            DebugCommand::GetShareableLink { params, id } => {
                self.handle_get_shareable_link(params.space_id, params.node_id, id).await
            }

            DebugCommand::AddWebsite { params, id } => {
                self.handle_add_website(params.connection_string, id).await
            }

            DebugCommand::ListNodes { id } => {
                self.handle_list_nodes(id).await
            }

            DebugCommand::P2pStatus { id } => {
                self.handle_p2p_status(id).await
            }

            DebugCommand::UpdateNodeAddress { params, id } => {
                self.handle_update_node_address(params.connection_string, id).await
            }

            DebugCommand::RefreshApp { params, id } => {
                self.handle_refresh_app(params.app_name, params.app_dir, id).await
            }

            DebugCommand::RefreshPage { params, id } => {
                self.handle_refresh_page(params.page_dir, id).await
            }

            DebugCommand::GetAppStatus { id } => {
                self.handle_get_app_status(id).await
            }

            DebugCommand::UploadAsset { params, id } => {
                self.handle_upload_asset(params.page_id, params.file_path, id).await
            }
        }
    }

    async fn handle_eval(&self, code: String, id: u64) -> DebugResponse {
        // Read the eval channel from RwLock
        let eval_tx = {
            let guard = self.eval_tx.read().await;
            guard.clone()
        };

        let Some(eval_tx) = eval_tx else {
            return DebugResponse::Error {
                error: DebugError {
                    code: -32603,
                    message: "Lua eval not available - no app is open".to_string(),
                },
                id,
            };
        };

        let (response_tx, response_rx) = oneshot::channel();
        let request = DebugEvalRequest { code, response_tx };

        if eval_tx.send(request).await.is_err() {
            return DebugResponse::Error {
                error: DebugError {
                    code: -32603,
                    message: "Failed to send eval request".to_string(),
                },
                id,
            };
        }

        match response_rx.await {
            Ok(Ok(value)) => DebugResponse::Result { result: value, id },
            Ok(Err(e)) => DebugResponse::Error {
                error: DebugError {
                    code: -32000,
                    message: e,
                },
                id,
            },
            Err(_) => DebugResponse::Error {
                error: DebugError {
                    code: -32603,
                    message: "Eval request dropped".to_string(),
                },
                id,
            },
        }
    }

    async fn handle_state(&self, id: u64) -> DebugResponse {
        // TODO: Implement state inspection
        // This will require access to Butler and LuaWorker state
        DebugResponse::Result {
            result: serde_json::json!({
                "layers": [],
                "ui_props": {},
                "permit": null,
                "note": "State inspection not yet implemented"
            }),
            id,
        }
    }

    async fn handle_ui_click(&self, label: String, id: u64) -> DebugResponse {
        let Some(ref ui_tx) = self.ui_tx else {
            return DebugResponse::Error {
                error: DebugError {
                    code: -32603,
                    message: "UI automation not available".to_string(),
                },
                id,
            };
        };

        let (response_tx, response_rx) = oneshot::channel();
        let cmd = UiCommand::Click {
            label: label.clone(),
            response_tx,
        };

        if ui_tx.send(cmd).await.is_err() {
            return DebugResponse::Error {
                error: DebugError {
                    code: -32603,
                    message: "Failed to send UI command".to_string(),
                },
                id,
            };
        }

        match response_rx.await {
            Ok(Ok(())) => DebugResponse::Result {
                result: serde_json::json!({"clicked": label, "success": true}),
                id,
            },
            Ok(Err(e)) => DebugResponse::Error {
                error: DebugError {
                    code: -32000,
                    message: e,
                },
                id,
            },
            Err(_) => DebugResponse::Error {
                error: DebugError {
                    code: -32603,
                    message: "UI command dropped".to_string(),
                },
                id,
            },
        }
    }

    async fn handle_ui_type(&self, label: String, text: String, id: u64) -> DebugResponse {
        let Some(ref ui_tx) = self.ui_tx else {
            return DebugResponse::Error {
                error: DebugError {
                    code: -32603,
                    message: "UI automation not available".to_string(),
                },
                id,
            };
        };

        let (response_tx, response_rx) = oneshot::channel();
        let cmd = UiCommand::Type {
            label: label.clone(),
            text: text.clone(),
            response_tx,
        };

        if ui_tx.send(cmd).await.is_err() {
            return DebugResponse::Error {
                error: DebugError {
                    code: -32603,
                    message: "Failed to send UI command".to_string(),
                },
                id,
            };
        }

        match response_rx.await {
            Ok(Ok(())) => DebugResponse::Result {
                result: serde_json::json!({"typed": text, "into": label, "success": true}),
                id,
            },
            Ok(Err(e)) => DebugResponse::Error {
                error: DebugError {
                    code: -32000,
                    message: e,
                },
                id,
            },
            Err(_) => DebugResponse::Error {
                error: DebugError {
                    code: -32603,
                    message: "UI command dropped".to_string(),
                },
                id,
            },
        }
    }

    async fn handle_ui_get_text(&self, label: String, id: u64) -> DebugResponse {
        let Some(ref ui_tx) = self.ui_tx else {
            return DebugResponse::Error {
                error: DebugError {
                    code: -32603,
                    message: "UI automation not available".to_string(),
                },
                id,
            };
        };

        let (response_tx, response_rx) = oneshot::channel();
        let cmd = UiCommand::GetText {
            label: label.clone(),
            response_tx,
        };

        if ui_tx.send(cmd).await.is_err() {
            return DebugResponse::Error {
                error: DebugError {
                    code: -32603,
                    message: "Failed to send UI command".to_string(),
                },
                id,
            };
        }

        match response_rx.await {
            Ok(Ok(text)) => DebugResponse::Result {
                result: serde_json::json!({"label": label, "text": text}),
                id,
            },
            Ok(Err(e)) => DebugResponse::Error {
                error: DebugError {
                    code: -32000,
                    message: e,
                },
                id,
            },
            Err(_) => DebugResponse::Error {
                error: DebugError {
                    code: -32603,
                    message: "UI command dropped".to_string(),
                },
                id,
            },
        }
    }

    async fn handle_ui_get_screen(&self, id: u64) -> DebugResponse {
        let Some(ref ui_tx) = self.ui_tx else {
            return DebugResponse::Error {
                error: DebugError {
                    code: -32603,
                    message: "UI automation not available".to_string(),
                },
                id,
            };
        };

        let (response_tx, response_rx) = oneshot::channel();
        let cmd = UiCommand::GetScreen { response_tx };

        if ui_tx.send(cmd).await.is_err() {
            return DebugResponse::Error {
                error: DebugError {
                    code: -32603,
                    message: "Failed to send UI command".to_string(),
                },
                id,
            };
        }

        match response_rx.await {
            Ok(screen) => DebugResponse::Result {
                result: serde_json::json!({"screen": screen}),
                id,
            },
            Err(_) => DebugResponse::Error {
                error: DebugError {
                    code: -32603,
                    message: "UI command dropped".to_string(),
                },
                id,
            },
        }
    }

    async fn handle_reload(&self, params: Option<ReloadParams>, id: u64) -> DebugResponse {
        // TODO: Implement app reload
        // This requires:
        // 1. Access to the app runner to trigger reload
        // 2. Shutdown current LuaWorker
        // 3. Re-read app code from disk
        // 4. Create new LuaWorker with fresh code
        let page_id = params.and_then(|p| p.page_id);
        debug!(page_id = ?page_id, "Reload requested");
        DebugResponse::Result {
            result: serde_json::json!({
                "reload": "requested",
                "page_id": page_id,
                "note": "Reload not yet implemented - requires app runner integration"
            }),
            id,
        }
    }

    async fn handle_get_connection_string(&self, id: u64) -> DebugResponse {
        let Some(ref butler) = self.butler else {
            return DebugResponse::Error {
                error: DebugError {
                    code: -32603,
                    message: "Butler not available - not logged in".to_string(),
                },
                id,
            };
        };

        match butler.generate_connection_string(None).await {
            Ok(connection_string) => DebugResponse::Result {
                result: serde_json::json!({
                    "connection_string": connection_string
                }),
                id,
            },
            Err(e) => DebugResponse::Error {
                error: DebugError {
                    code: -32000,
                    message: format!("Failed to generate connection string: {}", e),
                },
                id,
            },
        }
    }

    async fn handle_signup(&self, username: String, passphrase: String, id: u64) -> DebugResponse {
        let Some(ref ui_tx) = self.ui_tx else {
            return DebugResponse::Error {
                error: DebugError {
                    code: -32603,
                    message: "UI automation not available".to_string(),
                },
                id,
            };
        };

        let (response_tx, response_rx) = oneshot::channel();
        let cmd = UiCommand::SignUp {
            username,
            passphrase,
            response_tx,
        };

        if ui_tx.send(cmd).await.is_err() {
            return DebugResponse::Error {
                error: DebugError {
                    code: -32603,
                    message: "Failed to send signup command".to_string(),
                },
                id,
            };
        }

        match response_rx.await {
            Ok(Ok(did)) => DebugResponse::Result {
                result: serde_json::json!({"success": true, "did": did}),
                id,
            },
            Ok(Err(e)) => DebugResponse::Error {
                error: DebugError {
                    code: -32000,
                    message: e,
                },
                id,
            },
            Err(_) => DebugResponse::Error {
                error: DebugError {
                    code: -32603,
                    message: "Signup command dropped".to_string(),
                },
                id,
            },
        }
    }

    async fn handle_login(&self, passphrase: String, id: u64) -> DebugResponse {
        let Some(ref ui_tx) = self.ui_tx else {
            return DebugResponse::Error {
                error: DebugError {
                    code: -32603,
                    message: "UI automation not available".to_string(),
                },
                id,
            };
        };

        let (response_tx, response_rx) = oneshot::channel();
        let cmd = UiCommand::DirectLogin {
            passphrase,
            response_tx,
        };

        if ui_tx.send(cmd).await.is_err() {
            return DebugResponse::Error {
                error: DebugError {
                    code: -32603,
                    message: "Failed to send login command".to_string(),
                },
                id,
            };
        }

        match response_rx.await {
            Ok(Ok(did)) => DebugResponse::Result {
                result: serde_json::json!({"success": true, "did": did}),
                id,
            },
            Ok(Err(e)) => DebugResponse::Error {
                error: DebugError {
                    code: -32000,
                    message: e,
                },
                id,
            },
            Err(_) => DebugResponse::Error {
                error: DebugError {
                    code: -32603,
                    message: "Login command dropped".to_string(),
                },
                id,
            },
        }
    }

    async fn handle_ui_list_elements(&self, id: u64) -> DebugResponse {
        let Some(ref ui_tx) = self.ui_tx else {
            return DebugResponse::Error {
                error: DebugError {
                    code: -32603,
                    message: "UI automation not available".to_string(),
                },
                id,
            };
        };

        let (response_tx, response_rx) = oneshot::channel();
        let cmd = UiCommand::ListElements { response_tx };

        if ui_tx.send(cmd).await.is_err() {
            return DebugResponse::Error {
                error: DebugError {
                    code: -32603,
                    message: "Failed to send UI command".to_string(),
                },
                id,
            };
        }

        match response_rx.await {
            Ok(elements) => DebugResponse::Result {
                result: serde_json::json!({"elements": elements, "count": elements.len()}),
                id,
            },
            Err(_) => DebugResponse::Error {
                error: DebugError {
                    code: -32603,
                    message: "UI command dropped".to_string(),
                },
                id,
            },
        }
    }

    async fn handle_list_spaces(&self, id: u64) -> DebugResponse {
        let Some(ref butler) = self.butler else {
            return DebugResponse::Error {
                error: DebugError {
                    code: -32603,
                    message: "Butler not available - not logged in".to_string(),
                },
                id,
            };
        };

        match butler.list_spaces() {
            Ok(spaces) => {
                let spaces_json: Vec<serde_json::Value> = spaces
                    .iter()
                    .map(|s| serde_json::json!({
                        "id": s.id,
                        "name": s.name,
                    }))
                    .collect();
                DebugResponse::Result {
                    result: serde_json::json!({
                        "spaces": spaces_json,
                        "count": spaces_json.len()
                    }),
                    id,
                }
            }
            Err(e) => DebugResponse::Error {
                error: DebugError {
                    code: -32000,
                    message: format!("Failed to list spaces: {}", e),
                },
                id,
            },
        }
    }

    async fn handle_list_pages(&self, space_id: String, id: u64) -> DebugResponse {
        let Some(ref butler) = self.butler else {
            return DebugResponse::Error {
                error: DebugError {
                    code: -32603,
                    message: "Butler not available - not logged in".to_string(),
                },
                id,
            };
        };

        match butler.list_pages(&space_id) {
            Ok(pages) => {
                let pages_json: Vec<serde_json::Value> = pages
                    .iter()
                    .map(|p| serde_json::json!({
                        "id": p.id,
                        "name": p.name,
                    }))
                    .collect();
                DebugResponse::Result {
                    result: serde_json::json!({
                        "space_id": space_id,
                        "pages": pages_json,
                        "count": pages_json.len()
                    }),
                    id,
                }
            }
            Err(e) => DebugResponse::Error {
                error: DebugError {
                    code: -32000,
                    message: format!("Failed to list pages: {}", e),
                },
                id,
            },
        }
    }

    async fn handle_import_page(&self, space_id: String, page_dir: String, id: u64) -> DebugResponse {
        let Some(ref butler) = self.butler else {
            return DebugResponse::Error {
                error: DebugError {
                    code: -32603,
                    message: "Butler not available - not logged in".to_string(),
                },
                id,
            };
        };

        let page_path = std::path::PathBuf::from(&page_dir);
        if !page_path.exists() {
            return DebugResponse::Error {
                error: DebugError {
                    code: -32000,
                    message: format!("Page directory not found: {}", page_dir),
                },
                id,
            };
        }

        // Validate Slint files before importing
        if let Err(e) = validate_slint_files(&page_path).await {
            return DebugResponse::Error {
                error: DebugError {
                    code: -32000,
                    message: e,
                },
                id,
            };
        }

        match butler.import_page(&space_id, &page_path).await {
            Ok(page) => {
                // Get apps in this page
                let apps = butler.list_apps(&page.id).unwrap_or_default();
                DebugResponse::Result {
                    result: serde_json::json!({
                        "page_id": page.id,
                        "page_name": page.name,
                        "apps": apps,
                    }),
                    id,
                }
            }
            Err(e) => DebugResponse::Error {
                error: DebugError {
                    code: -32000,
                    message: format!("Failed to import page: {}", e),
                },
                id,
            },
        }
    }

    async fn handle_create_space(&self, name: String, template_path: &str, id: u64) -> DebugResponse {
        let Some(ref ui_tx) = self.ui_tx else {
            return DebugResponse::Error {
                error: DebugError {
                    code: -32603,
                    message: "UI automation not available".to_string(),
                },
                id,
            };
        };

        // Validate Slint files in template before creating space
        let template_dir = std::path::PathBuf::from(template_path);
        if template_dir.exists() {
            if let Err(e) = validate_slint_files(&template_dir).await {
                return DebugResponse::Error {
                    error: DebugError {
                        code: -32000,
                        message: e,
                    },
                    id,
                };
            }
        }

        let (response_tx, response_rx) = oneshot::channel();
        let cmd = UiCommand::CreateSpace {
            name: name.clone(),
            template_path: template_path.to_string(),
            response_tx,
        };

        if ui_tx.send(cmd).await.is_err() {
            return DebugResponse::Error {
                error: DebugError {
                    code: -32603,
                    message: "Failed to send create_space command".to_string(),
                },
                id,
            };
        }

        match response_rx.await {
            Ok(Ok((space_id, space_name))) => DebugResponse::Result {
                result: serde_json::json!({
                    "id": space_id,
                    "name": space_name,
                }),
                id,
            },
            Ok(Err(e)) => DebugResponse::Error {
                error: DebugError {
                    code: -32000,
                    message: e,
                },
                id,
            },
            Err(_) => DebugResponse::Error {
                error: DebugError {
                    code: -32603,
                    message: "Create space command dropped".to_string(),
                },
                id,
            },
        }
    }

    async fn handle_list_apps(&self, page_id: String, id: u64) -> DebugResponse {
        let Some(ref butler) = self.butler else {
            return DebugResponse::Error {
                error: DebugError {
                    code: -32603,
                    message: "Butler not available - not logged in".to_string(),
                },
                id,
            };
        };

        match butler.list_apps(&page_id) {
            Ok(apps) => {
                // list_apps returns Vec<String> of app names
                let apps_json: Vec<serde_json::Value> = apps
                    .iter()
                    .map(|name| serde_json::json!({
                        "name": name,
                    }))
                    .collect();
                DebugResponse::Result {
                    result: serde_json::json!({
                        "page_id": page_id,
                        "apps": apps_json,
                        "count": apps_json.len()
                    }),
                    id,
                }
            }
            Err(e) => DebugResponse::Error {
                error: DebugError {
                    code: -32000,
                    message: format!("Failed to list apps: {}", e),
                },
                id,
            },
        }
    }

    async fn handle_open_app(&self, page_id: String, app_name: String, id: u64) -> DebugResponse {
        let Some(ref ui_tx) = self.ui_tx else {
            return DebugResponse::Error {
                error: DebugError {
                    code: -32603,
                    message: "UI automation not available".to_string(),
                },
                id,
            };
        };

        // Set status to "loading" before sending command
        {
            let mut status = self.app_status.write().await;
            status.page_id = Some(page_id.clone());
            status.app_name = Some(app_name.clone());
            status.status = "loading".to_string();
            status.error = None;
            status.loaded_at = None;
        }

        let (response_tx, response_rx) = oneshot::channel();
        let cmd = UiCommand::OpenApp {
            page_id: page_id.clone(),
            app_name: app_name.clone(),
            response_tx,
        };

        if ui_tx.send(cmd).await.is_err() {
            return DebugResponse::Error {
                error: DebugError {
                    code: -32603,
                    message: "Failed to send open_app command".to_string(),
                },
                id,
            };
        }

        match response_rx.await {
            Ok(Ok(msg)) => DebugResponse::Result {
                result: serde_json::json!({
                    "success": true,
                    "page_id": page_id,
                    "app_name": app_name,
                    "message": msg
                }),
                id,
            },
            Ok(Err(e)) => DebugResponse::Error {
                error: DebugError {
                    code: -32000,
                    message: e,
                },
                id,
            },
            Err(_) => DebugResponse::Error {
                error: DebugError {
                    code: -32603,
                    message: "Open app command dropped".to_string(),
                },
                id,
            },
        }
    }

    async fn handle_add_node(&self, connection_string: String, id: u64) -> DebugResponse {
        let Some(ref ui_tx) = self.ui_tx else {
            return DebugResponse::Error {
                error: DebugError {
                    code: -32603,
                    message: "UI automation not available".to_string(),
                },
                id,
            };
        };

        let (response_tx, response_rx) = oneshot::channel();
        let cmd = UiCommand::AddNode {
            connection_string: connection_string.clone(),
            response_tx,
        };

        if ui_tx.send(cmd).await.is_err() {
            return DebugResponse::Error {
                error: DebugError {
                    code: -32603,
                    message: "Failed to send add_node command".to_string(),
                },
                id,
            };
        }

        match response_rx.await {
            Ok(Ok(node_id)) => DebugResponse::Result {
                result: serde_json::json!({
                    "success": true,
                    "node_id": node_id,
                    "triggered": true
                }),
                id,
            },
            Ok(Err(e)) => DebugResponse::Error {
                error: DebugError {
                    code: -32000,
                    message: e,
                },
                id,
            },
            Err(_) => DebugResponse::Error {
                error: DebugError {
                    code: -32603,
                    message: "Add node command dropped".to_string(),
                },
                id,
            },
        }
    }

    async fn handle_p2p_status(&self, id: u64) -> DebugResponse {
        let butler_ready = self.butler.is_some();

        let courier_ready = if let Some(ref courier_arc) = self.courier_handle {
            courier_arc.read().await.is_some()
        } else {
            false
        };

        DebugResponse::Result {
            result: serde_json::json!({
                "butler_ready": butler_ready,
                "courier_ready": courier_ready,
                "p2p_ready": butler_ready && courier_ready
            }),
            id,
        }
    }

    /// Update node's relay URL from new connection string and trigger reconnection
    ///
    /// **Context**: After setup_test_dbs, stored nodes may have invalid relay URLs.
    /// This command updates the relay URL with the real one from running kunki.
    async fn handle_update_node_address(&self, connection_string: String, id: u64) -> DebugResponse {
        let Some(ref butler) = self.butler else {
            return DebugResponse::Error {
                error: DebugError {
                    code: -32603,
                    message: "Butler not available - not logged in".to_string(),
                },
                id,
            };
        };

        // Parse the connection string to get the new relay URL
        let conn = match butler::models::node::ConnectionString::parse(&connection_string) {
            Ok(c) => c,
            Err(e) => {
                return DebugResponse::Error {
                    error: DebugError {
                        code: -32602,
                        message: format!("Invalid connection string: {}", e),
                    },
                    id,
                };
            }
        };

        let node_id = conn.node_id();
        let relay_url = conn.relay.clone();

        // Update the node's relay URL in storage
        match butler.update_node_relay(&node_id, relay_url.clone()) {
            Ok(_) => {
                info!("Updated node {} relay to {:?}", node_id, relay_url);
            }
            Err(e) => {
                return DebugResponse::Error {
                    error: DebugError {
                        code: -32000,
                        message: format!("Failed to update node: {}", e),
                    },
                    id,
                };
            }
        }

        // Trigger reconnection via courier
        if let Some(ref courier_arc) = self.courier_handle {
            if let Some(courier) = courier_arc.read().await.as_ref() {
                // Get permit from stored node
                let permit = match butler.get_sovereign_node(&node_id) {
                    Ok(Some(node)) => node.permit,
                    _ => None,
                };

                if let Some(permit) = permit {
                    if let Err(e) = courier.connect(&node_id, &permit) {
                        // Don't fail - relay update succeeded
                        warn!("Connect triggered but may not be immediate: {}", e);
                    }
                } else {
                    warn!("No permit stored for node {}, cannot reconnect", node_id);
                }
            }
        }

        DebugResponse::Result {
            result: serde_json::json!({
                "success": true,
                "node_id": node_id,
                "relay_url": relay_url
            }),
            id,
        }
    }

    async fn handle_list_nodes(&self, id: u64) -> DebugResponse {
        let Some(ref butler) = self.butler else {
            return DebugResponse::Error {
                error: DebugError {
                    code: -32603,
                    message: "Butler not available - not logged in".to_string(),
                },
                id,
            };
        };

        match butler.list_sovereign_nodes() {
            Ok(nodes) => {
                let nodes_json: Vec<serde_json::Value> = nodes
                    .iter()
                    .map(|n| serde_json::json!({
                        "node_id": n.node_id,
                        "name": n.name,
                        "connected": n.is_connected,
                    }))
                    .collect();
                DebugResponse::Result {
                    result: serde_json::json!({
                        "nodes": nodes_json,
                        "count": nodes_json.len()
                    }),
                    id,
                }
            }
            Err(e) => DebugResponse::Error {
                error: DebugError {
                    code: -32000,
                    message: format!("Failed to list nodes: {}", e),
                },
                id,
            },
        }
    }

    async fn handle_publish_space(&self, space_id: String, node_id: String, id: u64) -> DebugResponse {
        let Some(ref butler) = self.butler else {
            return DebugResponse::Error {
                error: DebugError {
                    code: -32603,
                    message: "Butler not available - not logged in".to_string(),
                },
                id,
            };
        };

        // Get the node's pubkey
        let node = match butler.get_sovereign_node(&node_id) {
            Ok(Some(n)) => n,
            Ok(None) => {
                return DebugResponse::Error {
                    error: DebugError {
                        code: -32000,
                        message: format!("Node not found: {}", node_id),
                    },
                    id,
                };
            }
            Err(e) => {
                return DebugResponse::Error {
                    error: DebugError {
                        code: -32000,
                        message: format!("Failed to get node: {}", e),
                    },
                    id,
                };
            }
        };

        // Get courier handle for publishing
        let Some(ref courier_arc) = self.courier_handle else {
            return DebugResponse::Error {
                error: DebugError {
                    code: -32603,
                    message: "Courier not available".to_string(),
                },
                id,
            };
        };

        let courier_guard = courier_arc.read().await;
        let courier_handle = match courier_guard.as_ref() {
            Some(h) => h.clone(),
            None => {
                return DebugResponse::Error {
                    error: DebugError {
                        code: -32603,
                        message: "P2P not initialized".to_string(),
                    },
                    id,
                };
            }
        };
        drop(courier_guard);

        // Publish via courier
        match courier_handle.publish_space(&space_id, &node_id).await {
            Ok(_) => {
                let _ = butler.mark_space_published(&space_id, &node_id);
                DebugResponse::Result {
                    result: serde_json::json!({
                        "success": true,
                        "space_id": space_id,
                        "node_id": node_id,
                        "node_name": node.name
                    }),
                    id,
                }
            }
            Err(e) => DebugResponse::Error {
                error: DebugError {
                    code: -32000,
                    message: format!("Failed to publish space: {}", e),
                },
                id,
            },
        }
    }

    async fn handle_get_shareable_link(&self, space_id: String, node_id: String, id: u64) -> DebugResponse {
        // Get courier handle for requesting link from node
        let Some(ref courier_arc) = self.courier_handle else {
            return DebugResponse::Error {
                error: DebugError {
                    code: -32603,
                    message: "Courier not available".to_string(),
                },
                id,
            };
        };

        let courier_guard = courier_arc.read().await;
        let courier_handle = match courier_guard.as_ref() {
            Some(h) => h.clone(),
            None => {
                return DebugResponse::Error {
                    error: DebugError {
                        code: -32603,
                        message: "P2P not initialized - not logged in".to_string(),
                    },
                    id,
                };
            }
        };
        drop(courier_guard);

        // Request shareable link from node (node generates viewer permit)
        match courier_handle.get_shareable_link(&space_id, &node_id).await {
            Ok(connection_string) => DebugResponse::Result {
                result: serde_json::json!({
                    "connection_string": connection_string,
                    "space_id": space_id,
                    "node_id": node_id
                }),
                id,
            },
            Err(e) => DebugResponse::Error {
                error: DebugError {
                    code: -32000,
                    message: format!("Failed to get shareable link: {}", e),
                },
                id,
            },
        }
    }

    async fn handle_add_website(&self, connection_string: String, id: u64) -> DebugResponse {
        let Some(ref ui_tx) = self.ui_tx else {
            return DebugResponse::Error {
                error: DebugError {
                    code: -32603,
                    message: "UI automation not available".to_string(),
                },
                id,
            };
        };

        let (response_tx, response_rx) = oneshot::channel();
        let cmd = UiCommand::AddWebsite {
            connection_string: connection_string.clone(),
            response_tx,
        };

        if ui_tx.send(cmd).await.is_err() {
            return DebugResponse::Error {
                error: DebugError {
                    code: -32603,
                    message: "Failed to send add_website command".to_string(),
                },
                id,
            };
        }

        match response_rx.await {
            Ok(Ok(space_id)) => DebugResponse::Result {
                result: serde_json::json!({
                    "success": true,
                    "space_id": space_id,
                    "triggered": true
                }),
                id,
            },
            Ok(Err(e)) => DebugResponse::Error {
                error: DebugError {
                    code: -32000,
                    message: e,
                },
                id,
            },
            Err(_) => DebugResponse::Error {
                error: DebugError {
                    code: -32603,
                    message: "Add website command dropped".to_string(),
                },
                id,
            },
        }
    }

    /// Refresh app from filesystem via Scribe
    ///
    /// **Context**: Developer edited files on disk, wants to reload via Scribe
    /// **Flow**: Debug command → UiCommand → main thread → Scribe.RefreshApp
    async fn handle_refresh_app(&self, app_name: String, app_dir: String, id: u64) -> DebugResponse {
        let Some(ref ui_tx) = self.ui_tx else {
            return DebugResponse::Error {
                error: DebugError {
                    code: -32603,
                    message: "UI automation not available".to_string(),
                },
                id,
            };
        };

        let (response_tx, response_rx) = oneshot::channel();
        let cmd = UiCommand::RefreshApp {
            app_name: app_name.clone(),
            app_dir: app_dir.clone(),
            response_tx,
        };

        if ui_tx.send(cmd).await.is_err() {
            return DebugResponse::Error {
                error: DebugError {
                    code: -32603,
                    message: "Failed to send refresh_app command".to_string(),
                },
                id,
            };
        }

        match response_rx.await {
            Ok(Ok(changed_files)) => DebugResponse::Result {
                result: serde_json::json!({
                    "success": true,
                    "app_name": app_name,
                    "app_dir": app_dir,
                    "changed_files": changed_files,
                    "changed_count": changed_files.len()
                }),
                id,
            },
            Ok(Err(e)) => DebugResponse::Error {
                error: DebugError {
                    code: -32000,
                    message: e,
                },
                id,
            },
            Err(_) => DebugResponse::Error {
                error: DebugError {
                    code: -32603,
                    message: "RefreshApp command dropped".to_string(),
                },
                id,
            },
        }
    }

    /// Refresh entire page from filesystem (all apps in root directory)
    ///
    /// **Context**: Developer edited files on disk, wants to reload ALL apps via Scribe
    /// **Flow**: Debug command → UiCommand → main thread → Scribe.RefreshPage
    async fn handle_refresh_page(&self, page_dir: String, id: u64) -> DebugResponse {
        let Some(ref ui_tx) = self.ui_tx else {
            return DebugResponse::Error {
                error: DebugError {
                    code: -32603,
                    message: "UI automation not available".to_string(),
                },
                id,
            };
        };

        let (response_tx, response_rx) = oneshot::channel();
        let cmd = UiCommand::RefreshPage {
            page_dir: page_dir.clone(),
            response_tx,
        };

        if ui_tx.send(cmd).await.is_err() {
            return DebugResponse::Error {
                error: DebugError {
                    code: -32603,
                    message: "Failed to send refresh_page command".to_string(),
                },
                id,
            };
        }

        match response_rx.await {
            Ok(Ok(updated_apps)) => DebugResponse::Result {
                result: serde_json::json!({
                    "success": true,
                    "page_dir": page_dir,
                    "updated_apps": updated_apps,
                    "updated_count": updated_apps.len()
                }),
                id,
            },
            Ok(Err(e)) => DebugResponse::Error {
                error: DebugError {
                    code: -32000,
                    message: e,
                },
                id,
            },
            Err(_) => DebugResponse::Error {
                error: DebugError {
                    code: -32603,
                    message: "RefreshPage command dropped".to_string(),
                },
                id,
            },
        }
    }

    /// Get current app loading status
    ///
    /// **Context**: AI wants to know if an app loaded successfully or failed
    /// **Returns**: Current status with page_id, app_name, status, and error (if any)
    async fn handle_get_app_status(&self, id: u64) -> DebugResponse {
        let status = self.app_status.read().await.clone();
        DebugResponse::Result {
            result: serde_json::to_value(status).unwrap_or(serde_json::json!({"error": "serialization failed"})),
            id,
        }
    }

    /// Upload an asset to a page (for integration testing)
    ///
    /// **Context**: Bypasses UI file picker for automated testing
    /// **Flow**:
    ///   1. Read file from filesystem
    ///   2. Detect MIME type
    ///   3. Call Butler::upload_asset_async()
    ///   4. Return hash and metadata
    async fn handle_upload_asset(&self, page_id: String, file_path: String, id: u64) -> DebugResponse {
        let Some(ref butler) = self.butler else {
            return DebugResponse::Error {
                error: DebugError {
                    code: -32603,
                    message: "Butler not available - not logged in".to_string(),
                },
                id,
            };
        };

        // Read file from disk
        let path = std::path::Path::new(&file_path);
        if !path.exists() {
            return DebugResponse::Error {
                error: DebugError {
                    code: -32000,
                    message: format!("File not found: {}", file_path),
                },
                id,
            };
        }

        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) => {
                return DebugResponse::Error {
                    error: DebugError {
                        code: -32000,
                        message: format!("Failed to read file: {}", e),
                    },
                    id,
                };
            }
        };

        // Get filename from path
        let filename = path.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        // Detect MIME type
        let mime_type = mime_guess::from_path(path)
            .first_or_octet_stream()
            .to_string();

        info!(
            page_id = %page_id,
            filename = %filename,
            mime_type = %mime_type,
            size = bytes.len(),
            "Uploading asset via debug API"
        );

        // Upload via butler
        match butler.upload_asset_async(&page_id, &bytes, &filename, &mime_type).await {
            Ok(hash) => {
                // Notify Lua of successful upload (trigger on_asset_uploaded callback)
                let lua_tx = self.lua_worker_tx.read().await;
                if let Some(tx) = lua_tx.as_ref() {
                    let _ = tx.try_send(app_runtime::LuaWorkerCommand::AssetUploaded {
                        hash: hash.clone(),
                        filename: filename.clone(),
                        mime_type: mime_type.clone(),
                        size: bytes.len() as u64,
                    });
                }

                DebugResponse::Result {
                    result: serde_json::json!({
                        "success": true,
                        "hash": hash,
                        "filename": filename,
                        "mime_type": mime_type,
                        "size": bytes.len(),
                    }),
                    id,
                }
            }
            Err(e) => DebugResponse::Error {
                error: DebugError {
                    code: -32000,
                    message: format!("Failed to upload asset: {}", e),
                },
                id,
            },
        }
    }
}

/// Helper to create socket path from instance name
pub fn socket_path_for_instance(instance_name: &str) -> PathBuf {
    let tmp_dir = std::env::temp_dir();
    tmp_dir.join(format!("osvauld-debug-{}.sock", instance_name))
}
