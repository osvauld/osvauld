//! Slint Renderer for User Apps
//!
//! Renders UI apps (chat, survey, ecomm) using Slint interpreter.
//! This crate provides the Slint-specific rendering layer, separate from
//! the platform shell (sthalam_shell) and other renderers (renderer_raylib).
//!
//! ## Key Functions
//!
//! - `prepare_page()`: Extracts app files from Scribe and generates browser shell
//! - `launch_slint_app()`: Creates a self-managing app window with its own timer
//! - `create_slint_app()`: Lower-level function to create a running app instance
//! - `validate_slint_files()`: Validates Slint files before importing

mod slint_runtime;
mod page_runtime;
mod slint_model_bindings;

pub use slint_runtime::{SlintRuntime, AssetPickRequest};
pub use slint_model_bindings::LuaSlintModel;
pub use page_runtime::{generate_page_shell, write_shell_slint, AppTab, parse_exported_types, ExportedTypes};

use butler::{Butler, PageUpdate, ScribeMessage};
use slint::ComponentHandle;
use lua_runtime::{LuaCommand, LuaRuntime, LuaRuntimeConfig, ActorScribeHandle, ScribeHandle, UiMutation, UiQuery};
use ractor::ActorRef;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

/// App manifest (manifest.json)
#[derive(Debug, Clone, serde::Deserialize)]
pub struct Manifest {
    pub name: String,
    pub version: String,
    /// Slint UI entry point (required for Slint apps, None for Raylib)
    #[serde(default)]
    pub entry_ui: Option<String>,
    pub entry_logic: String,
    /// Renderer type: "slint" (default) or "raylib"
    #[serde(default = "default_renderer")]
    pub renderer: String,
    /// Array properties that need VecModel tracking for incremental updates
    #[serde(default)]
    pub models: Vec<String>,
    /// Window width (for raylib apps)
    #[serde(default)]
    pub width: Option<u32>,
    /// Window height (for raylib apps)
    #[serde(default)]
    pub height: Option<u32>,
    /// Target FPS (for raylib apps)
    #[serde(default)]
    pub target_fps: Option<u32>,
}

fn default_renderer() -> String {
    "slint".to_string()
}

/// App version combining semantic version and content hash
#[derive(Debug, Clone, serde::Serialize)]
pub struct AppVersion {
    pub semantic: String,
    pub content_hash: String,
    pub display: String,
}

impl AppVersion {
    pub fn new(semantic: &str, files: &HashMap<String, String>) -> Self {
        use sha2::{Digest, Sha256};
        use std::collections::BTreeMap;

        let mut hasher = Sha256::new();
        let sorted: BTreeMap<_, _> = files.iter().collect();
        for (path, content) in sorted {
            hasher.update(path.as_bytes());
            hasher.update(content.as_bytes());
        }
        let hash = hasher.finalize();
        let content_hash = hex::encode(&hash[..4]);

        Self {
            semantic: semantic.to_string(),
            content_hash: content_hash.clone(),
            display: format!("{}-{}", semantic, content_hash),
        }
    }
}

/// Window geometry for in-place reload
#[derive(Debug, Clone)]
pub struct WindowGeometry {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// Prepared page data for launching an app
pub struct PreparedPage {
    pub page_id: String,
    pub page_name: String,
    pub app_name: String,
    pub lua_path: PathBuf,
    pub shell_path: PathBuf,
    pub all_apps: Vec<AppTab>,
    pub data_layers: Vec<String>,
    pub models: Vec<String>,
    pub version: AppVersion,
    pub restore_geometry: Option<WindowGeometry>,
    /// Temp directory (must keep alive while app runs)
    pub temp_dir: PathBuf,
}

/// Running Slint app instance
pub struct RunningSlintApp {
    pub app_name: String,
    pub page_id: String,
    pub page_name: String,
    pub all_apps: Vec<AppTab>,
    pub slint_runtime: SlintRuntime,
    pub lua_thread: std::thread::JoinHandle<()>,
    pub lua_tx: tokio::sync::mpsc::Sender<LuaCommand>,
    pub page_update_rx: tokio::sync::mpsc::Receiver<PageUpdate>,
    pub tab_switch_rx: std::sync::mpsc::Receiver<String>,
    pub asset_pick_rx: std::sync::mpsc::Receiver<AssetPickRequest>,
    pub version: AppVersion,
}

/// Application status for debug server
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct AppStatus {
    pub page_id: Option<String>,
    pub app_name: Option<String>,
    pub status: String,
    pub error: Option<String>,
    pub loaded_at: Option<String>,
    pub version: Option<String>,
}

impl AppStatus {
    pub fn new() -> Self {
        Self {
            status: "idle".to_string(),
            ..Default::default()
        }
    }
}

/// Command to send to LuaWorker for debug evaluation
pub struct DebugEvalRequest {
    pub code: String,
    pub response_tx: tokio::sync::oneshot::Sender<Result<serde_json::Value, String>>,
}

/// Result of launching a self-managing Slint app window
pub struct LaunchedApp {
    /// Timer that must be kept alive (drop = window stops processing)
    pub timer: slint::Timer,
    /// Channel to send commands to this app's Lua worker
    pub lua_tx: tokio::sync::mpsc::Sender<LuaCommand>,
}

// Scribe Helpers

/// Get app files from Scribe's in-memory layer
///
/// **Why**: Scribe has the latest synced content; storage may be stale
pub async fn get_app_files_from_scribe(
    scribe: &ActorRef<ScribeMessage>,
    app_name: &str,
) -> Result<HashMap<String, String>, Box<dyn std::error::Error>> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    scribe.cast(ScribeMessage::GetAppFiles {
        app_name: app_name.to_string(),
        reply: tx,
    })?;
    let result = rx.await??;
    Ok(result)
}

/// Get app manifest from Scribe
///
/// **Why**: Need to check renderer field before deciding which runtime to use
pub async fn get_app_manifest(
    scribe: &ActorRef<ScribeMessage>,
    app_name: &str,
) -> Result<Manifest, Box<dyn std::error::Error + Send + Sync>> {
    let files = get_app_files_from_scribe(scribe, app_name).await
        .map_err(|e| format!("Failed to get app files: {}", e))?;

    let manifest_content = files.get("manifest.json")
        .ok_or_else(|| format!("manifest.json not found for app {}", app_name))?;

    let manifest: Manifest = serde_json::from_str(manifest_content)
        .map_err(|e| format!("Failed to parse manifest.json: {}", e))?;

    Ok(manifest)
}

// Page Preparation

/// Extract app and generate browser shell with tabs
///
/// **Architecture:**
/// - Apps are components (not Windows)
/// - Browser generates shell with Window + TabBar + imports App
///
/// **Returns:** PreparedPage with paths to shell and lua files
///
/// **scribe**: Scribe reference to read files from in-memory layer (latest synced content)
pub async fn prepare_page(
    butler: &Butler,
    page_id: &str,
    app_name: Option<&str>,
    scribe: &ActorRef<ScribeMessage>,
) -> Result<PreparedPage, Box<dyn std::error::Error>> {
    use std::fs;

    // Get page info for title
    let page = butler.pages().get(page_id)?
        .ok_or_else(|| format!("Page {} not found", page_id))?;
    let page_name = page.meta.name.clone();

    // Get all apps in this page (for tabs)
    let all_apps = butler.apps().list(page_id)?;
    if all_apps.is_empty() {
        return Err(format!("No apps found in page {}", page_id).into());
    }

    // Determine which app to load
    let app_name = match app_name {
        Some(name) => name.to_string(),
        None => all_apps.first().unwrap().clone(),
    };

    // Get all files from Scribe's in-memory layer (has latest synced content)
    let files = get_app_files_from_scribe(scribe, &app_name).await?;

    tracing::debug!(
        page_id = %page_id,
        app_name = %app_name,
        file_count = files.len(),
        all_apps = ?all_apps,
        "Preparing page with tabs"
    );

    // Create temporary directory
    let temp_dir = tempfile::tempdir()?;
    let temp_path = temp_dir.path().to_path_buf();

    // Extract app files
    for (file_path, content) in &files {
        let full_path = temp_path.join(file_path);

        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(&full_path, content)?;
    }

    // Get data layers from page (not from app files - permit is at page level)
    let data_layers = butler.apps().list_data_layers(page_id).unwrap_or_default();
    tracing::debug!(
        page_id = %page_id,
        data_layers = ?data_layers,
        "Retrieved data layers from page"
    );

    let lua_path = temp_path.join("app.lua");

    // Read models and version from manifest.json
    let manifest_path = temp_path.join("manifest.json");
    let (models, semantic_version) = if manifest_path.exists() {
        match fs::read_to_string(&manifest_path) {
            Ok(content) => {
                match serde_json::from_str::<Manifest>(&content) {
                    Ok(manifest) => (manifest.models, manifest.version),
                    Err(e) => {
                        tracing::warn!(
                            page_id = %page_id,
                            app_name = %app_name,
                            error = %e,
                            "Failed to parse manifest.json, using defaults"
                        );
                        (vec![], "0.0.0".to_string())
                    }
                }
            }
            Err(e) => {
                tracing::warn!(
                    page_id = %page_id,
                    app_name = %app_name,
                    error = %e,
                    "Failed to read manifest.json, using defaults"
                );
                (vec![], "0.0.0".to_string())
            }
        }
    } else {
        (vec![], "0.0.0".to_string())
    };

    // Compute app version (semantic + content hash)
    let version = AppVersion::new(&semantic_version, &files);
    tracing::info!(
        app_name = %app_name,
        version = %version.display,
        "App version computed"
    );

    // Build tabs for all apps
    let tabs: Vec<AppTab> = all_apps
        .iter()
        .map(|name| AppTab {
            name: name.clone(),
            display_name: name.clone(),
        })
        .collect();

    // Generate shell that imports the app (with version footer)
    let app_slint_path = temp_path.join("app.slint");
    let shell_source = generate_page_shell(&app_slint_path, &tabs, &app_name, &page_name, Some(&version.display));

    // Write shell to temp directory
    let shell_path = write_shell_slint(&temp_path, &shell_source)?;

    tracing::info!(
        page_id = %page_id,
        page_name = %page_name,
        app_name = %app_name,
        shell_path = %shell_path.display(),
        tabs_count = tabs.len(),
        models = ?models,
        "Generated browser shell with tabs"
    );

    // Prevent auto-cleanup
    let _ = temp_dir.keep();

    Ok(PreparedPage {
        shell_path,
        lua_path,
        app_name,
        page_id: page_id.to_string(),
        page_name,
        all_apps: tabs,
        data_layers,
        models,
        temp_dir: temp_path,
        restore_geometry: None,
        version,
    })
}

// App Creation

/// Create a running Slint app from prepared page data
///
/// **Context**: Called when an app needs to be loaded with Slint renderer
pub fn create_slint_app(
    prepared: PreparedPage,
    scribe_ref: ActorRef<ScribeMessage>,
    butler: Arc<Butler>,
    app_status: Option<Arc<RwLock<AppStatus>>>,
) -> Option<RunningSlintApp> {
    tracing::info!(
        page_id = %prepared.page_id,
        page_name = %prepared.page_name,
        app_name = %prepared.app_name,
        "Creating Slint app runtime"
    );

    // Get user identity and role from Butler
    let app_ctx = butler.app_context(&prepared.page_id);
    let user_did = app_ctx.user_did;
    let user_name = app_ctx.user_name;
    let user_role = app_ctx.user_role;

    let raw_lua_code = match std::fs::read_to_string(&prepared.lua_path) {
        Ok(code) => code,
        Err(e) => {
            let error_msg = format!("Failed to read Lua code: {}", e);
            tracing::error!(%error_msg);
            update_status_failed(&app_status, error_msg);
            return None;
        }
    };

    // Set package.path so require("module") finds .lua files in the app's temp directory
    let lua_code = format!(
        "package.path = '{}/?.lua;' .. package.path\n{}",
        prepared.temp_dir.display(),
        raw_lua_code
    );

    let slint_path = prepared.shell_path.clone();

    // Create channels
    let (ui_tx, ui_rx) = tokio::sync::mpsc::channel::<UiMutation>(256);
    let (query_tx, query_rx) = tokio::sync::mpsc::channel::<UiQuery>(32);
    let (page_update_tx, page_update_rx) = tokio::sync::mpsc::channel::<PageUpdate>(256);
    let (tab_switch_tx, tab_switch_rx) = std::sync::mpsc::channel::<String>();
    let (asset_pick_tx, asset_pick_rx) = std::sync::mpsc::channel::<AssetPickRequest>();

    let page_id = prepared.page_id.clone();
    let app_name = prepared.app_name.clone();

    // Spawn Lua runtime
    let navigate_tx = tab_switch_tx.clone();

    let config = LuaRuntimeConfig {
        page_id: page_id.clone(),
        app_name: app_name.clone(),
        scribe: ActorScribeHandle::new(scribe_ref.clone()),
        user_did: user_did.clone(),
        user_name: user_name.clone(),
        user_role: user_role.clone(),
        lua_code,
        ui_enabled: true,
        ui_tx: Some(ui_tx),
        query_tx: Some(query_tx),
        navigate_tx: Some(navigate_tx),
    };

    let (lua_thread, lua_tx) = match LuaRuntime::spawn(config) {
        Ok((thread, tx)) => (thread, tx),
        Err(e) => {
            let error_msg = format!("Failed to spawn Lua worker: {}", e);
            tracing::error!(%error_msg);
            update_status_failed(&app_status, error_msg);
            return None;
        }
    };

    // Create Slint runtime
    let mut slint_runtime = match SlintRuntime::load(
        slint_path,
        app_name.clone(),
        ui_rx,
        query_rx,
        lua_tx.clone(),
    ) {
        Ok(runtime) => runtime,
        Err(e) => {
            let error_msg = format!("Failed to create SlintRuntime: {}", e);
            tracing::error!(%error_msg);
            update_status_failed(&app_status, error_msg);
            return None;
        }
    };

    slint_runtime.set_tab_switch_channel(tab_switch_tx);
    slint_runtime.set_asset_pick_channel(asset_pick_tx);

    if let Err(e) = slint_runtime.slint_instance().show() {
        let error_msg = format!("Failed to show app window: {:?}", e);
        tracing::error!(%error_msg);
        update_status_failed(&app_status, error_msg);
        return None;
    }

    // Restore window geometry if this is an in-place reload
    if let Some(ref geom) = prepared.restore_geometry {
        let window = slint_runtime.slint_instance().window();
        window.set_position(slint::WindowPosition::Physical(slint::PhysicalPosition::new(
            geom.x, geom.y,
        )));
        window.set_size(slint::WindowSize::Physical(slint::PhysicalSize::new(
            geom.width,
            geom.height,
        )));
    }

    if let Err(e) = slint_runtime.setup_callbacks() {
        let error_msg = format!("Failed to setup shell callbacks: {}", e);
        tracing::error!(%error_msg);
        update_status_failed(&app_status, error_msg);
        return None;
    }

    // Initialize VecModels
    if let Err(e) = slint_runtime.init_models(&prepared.models) {
        tracing::warn!("Failed to init models from manifest: {}", e);
    }

    // Setup AppAPI global callbacks
    if let Err(e) = slint_runtime.setup_global_callbacks() {
        let error_msg = format!("Failed to setup global callbacks: {}", e);
        tracing::error!(%error_msg);
        update_status_failed(&app_status, error_msg);
        return None;
    }

    // Subscribe to page updates
    if let Err(e) = scribe_ref.cast(ScribeMessage::SubscribeToPageUpdates { tx: page_update_tx }) {
        tracing::warn!("Failed to subscribe to page updates: {}", e);
    }

    // Note: Initial data loading is handled by scribe:bind() in on_init()
    // which calls fetch_layer_data() to get the real layer snapshot from Scribe.
    // We no longer send empty LoroChanged events here as they would override
    // the real data fetched by scribe:bind().

    // Update status to loaded
    if let Some(ref status) = app_status {
        if let Ok(mut s) = status.try_write() {
            s.page_id = Some(prepared.page_id.clone());
            s.app_name = Some(prepared.app_name.clone());
            s.status = "loaded".to_string();
            s.error = None;
            s.loaded_at = Some(chrono::Utc::now().to_rfc3339());
            s.version = Some(prepared.version.display.clone());
        }
    }

    Some(RunningSlintApp {
        app_name: prepared.app_name,
        page_id: prepared.page_id,
        page_name: prepared.page_name,
        all_apps: prepared.all_apps,
        slint_runtime,
        lua_thread,
        lua_tx,
        page_update_rx,
        tab_switch_rx,
        asset_pick_rx,
        version: prepared.version,
    })
}

// Self-Managing App Window

/// Launch a Slint app in a self-managing window
///
/// Creates the app, shows the window, and starts its own timer that processes
/// page updates, UI mutations, asset picks, tab switches, and app restarts.
///
/// **Returns**: LaunchedApp with timer (must keep alive) and lua_tx for debug eval.
/// The window itself is owned by Slint's event loop.
pub fn launch_slint_app(
    prepared: PreparedPage,
    scribe_ref: ActorRef<ScribeMessage>,
    butler: Arc<Butler>,
    tokio_handle: tokio::runtime::Handle,
    app_status: Option<Arc<RwLock<AppStatus>>>,
) -> Option<LaunchedApp> {
    // Create the running app
    let running = create_slint_app(prepared, scribe_ref, butler.clone(), app_status.clone())?;
    let lua_tx_out = running.lua_tx.clone();

    // Internal state wrapped for timer closure (Rc is fine - Slint main thread only)
    let running: Rc<RefCell<Option<RunningSlintApp>>> = Rc::new(RefCell::new(Some(running)));
    let ready_rx: Rc<RefCell<Option<std::sync::mpsc::Receiver<(PreparedPage, ActorRef<ScribeMessage>)>>>> =
        Rc::new(RefCell::new(None));
    let pending_geometry: Rc<RefCell<Option<WindowGeometry>>> = Rc::new(RefCell::new(None));

    // Self-managing timer - processes all events for this window
    let timer = slint::Timer::default();
    timer.start(slint::TimerMode::Repeated, Duration::from_millis(100), move || {
        // Check for ready apps (restart/tab-switch completion)
        if let Some(ref rx) = *ready_rx.borrow() {
            while let Ok((mut new_prepared, new_scribe)) = rx.try_recv() {
                // Restore geometry from before restart
                if let Some(geom) = pending_geometry.borrow_mut().take() {
                    new_prepared.restore_geometry = Some(geom);
                }
                if let Some(new_app) = create_slint_app(new_prepared, new_scribe, butler.clone(), app_status.clone()) {
                    *running.borrow_mut() = Some(new_app);
                }
            }
        }

        let mut running_ref = running.borrow_mut();
        let Some(running_app) = running_ref.as_mut() else { return };

        // Forward page updates to Lua
        while let Ok(update) = running_app.page_update_rx.try_recv() {
            match update {
                PageUpdate::LayerChanged { layer, ops, delta, full_data, created, .. } => {
                    // Check if this is an app layer update (triggers restart)
                    // Use bare layer name (Scribe uses bare names, no page_id/ prefix)
                    // Only restart if there are actual operations (changes) - empty ops means
                    // sync protocol sent update but content is identical
                    let expected_app_layer = format!("app:{}", running_app.app_name);
                    let has_ops = ops.as_ref().map(|o| !o.is_empty()).unwrap_or(false);
                    if layer == expected_app_layer && !created && has_ops {
                        tracing::info!(layer = %layer, "App layer updated - restarting");

                        // Capture geometry, shutdown, schedule re-preparation
                        let geom = capture_window_geometry(running_app);
                        *pending_geometry.borrow_mut() = Some(geom);

                        let _ = running_app.slint_runtime.slint_instance().hide();
                        let _ = running_app.lua_tx.try_send(LuaCommand::Shutdown);

                        let (tx, rx) = std::sync::mpsc::channel();
                        *ready_rx.borrow_mut() = Some(rx);

                        let butler = butler.clone();
                        let page_id = running_app.page_id.clone();
                        let app_name = running_app.app_name.clone();

                        *running_ref = None;

                        tokio_handle.spawn(async move {
                            let scribe_ref = match butler.open_page(&page_id).await {
                                Ok(s) => s,
                                Err(e) => {
                                    tracing::error!(error = %e, "Failed to open page for restart");
                                    return;
                                }
                            };
                            let prepared = match prepare_page(&butler, &page_id, Some(&app_name), &scribe_ref).await {
                                Ok(p) => p,
                                Err(e) => {
                                    tracing::error!(error = %e, "Failed to prepare page for restart");
                                    return;
                                }
                            };
                            let _ = tx.send((prepared, scribe_ref));
                            slint::invoke_from_event_loop(|| {}).ok();
                        });
                        return;
                    }

                    if created {
                        let _ = running_app.lua_tx.try_send(LuaCommand::LayerDiscovered {
                            layer_name: layer,
                        });
                    } else {
                        let _ = running_app.lua_tx.try_send(LuaCommand::LoroChanged {
                            layer_name: layer,
                            ops,
                            delta,
                            full_data,  // Already Option<JsonValue> from PageUpdate
                        });
                    }
                }

                PageUpdate::Ephemeral { user_did, payload, .. } => {
                    let _ = running_app.lua_tx.try_send(LuaCommand::Ephemeral { user_did, payload });
                }

                PageUpdate::PeerSubscribed { did, .. } => {
                    let _ = running_app.lua_tx.try_send(LuaCommand::PeerJoined { user_did: did });
                }

                PageUpdate::PeerUnsubscribed { did } => {
                    let _ = running_app.lua_tx.try_send(LuaCommand::PeerLeft { user_did: did });
                }

                PageUpdate::QueryUpdated { .. } => {}

                PageUpdate::StructuredEphemeral { from_did, func, args } => {
                    let _ = running_app.lua_tx.try_send(LuaCommand::StructuredEphemeral { from_did, func, args });
                }
            }
        }

        // Handle tab switch requests
        while let Ok(new_app_name) = running_app.tab_switch_rx.try_recv() {
            if new_app_name != running_app.app_name {
                tracing::info!(
                    from = %running_app.app_name,
                    to = %new_app_name,
                    "Tab switch requested"
                );

                let geom = capture_window_geometry(running_app);
                *pending_geometry.borrow_mut() = Some(geom);

                let _ = running_app.slint_runtime.slint_instance().hide();
                let _ = running_app.lua_tx.try_send(LuaCommand::Shutdown);

                let (tx, rx) = std::sync::mpsc::channel();
                *ready_rx.borrow_mut() = Some(rx);

                let butler = butler.clone();
                let page_id = running_app.page_id.clone();

                *running_ref = None;

                tokio_handle.spawn(async move {
                    let scribe_ref = match butler.open_page(&page_id).await {
                        Ok(s) => s,
                        Err(e) => {
                            tracing::error!(error = %e, "Failed to open page for tab switch");
                            return;
                        }
                    };
                    let prepared = match prepare_page(&butler, &page_id, Some(&new_app_name), &scribe_ref).await {
                        Ok(p) => p,
                        Err(e) => {
                            tracing::error!(error = %e, "Failed to prepare page for tab switch");
                            return;
                        }
                    };
                    let _ = tx.send((prepared, scribe_ref));
                    slint::invoke_from_event_loop(|| {}).ok();
                });
                return;
            }
        }

        // Process asset pick requests
        while let Ok(request) = running_app.asset_pick_rx.try_recv() {
            handle_asset_pick(
                &request,
                &running_app.page_id,
                &running_app.lua_tx,
                &butler,
                &tokio_handle,
            );
        }

        // Process UI mutations (Lua -> Slint)
        if let Err(e) = running_app.slint_runtime.process_ui_mutations() {
            tracing::warn!(error = %e, "Failed to process UI mutations");
        }

        // Process UI queries (Slint -> Lua)
        running_app.slint_runtime.process_ui_queries();
    });

    Some(LaunchedApp {
        timer,
        lua_tx: lua_tx_out,
    })
}

// Asset Handling

/// Handle asset pick request - opens native file dialog and uploads selected file
pub fn handle_asset_pick(
    request: &AssetPickRequest,
    page_id: &str,
    lua_tx: &tokio::sync::mpsc::Sender<LuaCommand>,
    butler: &Arc<Butler>,
    tokio_handle: &tokio::runtime::Handle,
) {
    let filter_extensions: &[&str] = match request.filter.as_str() {
        "images" => &["png", "jpg", "jpeg", "gif", "webp", "bmp", "svg"],
        _ => &["*"],
    };

    let filter_name = match request.filter.as_str() {
        "images" => "Images",
        _ => "All Files",
    };

    let file = rfd::FileDialog::new()
        .set_title("Select Asset")
        .add_filter(filter_name, filter_extensions)
        .pick_file();

    let path = match file {
        Some(p) => p,
        None => {
            tracing::info!("Asset pick cancelled by user");
            return;
        }
    };

    tracing::info!(path = %path.display(), "Asset selected");

    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) => {
            tracing::error!(path = %path.display(), error = %e, "Failed to read asset file");
            return;
        }
    };

    let filename = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let mime_type = mime_guess::from_path(&path)
        .first_or_octet_stream()
        .to_string();

    let size = bytes.len() as u64;

    let butler = butler.clone();
    let page_id = page_id.to_string();
    let lua_tx = lua_tx.clone();

    tokio_handle.spawn(async move {
        match butler
            .assets()
            .upload(&page_id, &bytes, &filename, &mime_type)
            .await
        {
            Ok(hash) => {
                tracing::info!(
                    filename = %filename,
                    mime_type = %mime_type,
                    size = size,
                    hash = %hash,
                    "Asset uploaded"
                );

                let _ = lua_tx.try_send(LuaCommand::AssetUploaded {
                    hash,
                    filename,
                    mime_type,
                    size,
                });
            }
            Err(e) => {
                tracing::error!(error = %e, "Failed to upload asset");
            }
        }
    });
}

// Test App Creation

/// Create a Slint app for testing — no Butler, no Scribe actor
///
/// **Context**: Reads app files from disk, uses any ScribeHandle impl (mock or real)
/// **app_dir**: Directory containing manifest.json, app.slint, app.lua
pub fn create_test_slint_app(
    app_dir: &Path,
    page_id: &str,
    user_did: &str,
    user_name: &str,
    user_role: &str,
    scribe: Arc<dyn ScribeHandle>,
) -> Option<RunningSlintApp> {
    use std::fs;

    // Read manifest
    let manifest_path = app_dir.join("manifest.json");
    let manifest_content = match fs::read_to_string(&manifest_path) {
        Ok(c) => c,
        Err(e) => {
            tracing::error!(path = %manifest_path.display(), error = %e, "Failed to read manifest.json");
            return None;
        }
    };
    let manifest: Manifest = match serde_json::from_str(&manifest_content) {
        Ok(m) => m,
        Err(e) => {
            tracing::error!(error = %e, "Failed to parse manifest.json");
            return None;
        }
    };

    let app_name = manifest.name.clone();

    // Read Lua code
    let lua_path = app_dir.join(&manifest.entry_logic);
    let raw_lua_code = match fs::read_to_string(&lua_path) {
        Ok(code) => code,
        Err(e) => {
            tracing::error!(path = %lua_path.display(), error = %e, "Failed to read Lua code");
            return None;
        }
    };

    // Copy app files to temp dir
    let temp_dir = match tempfile::tempdir() {
        Ok(d) => d,
        Err(e) => {
            tracing::error!(error = %e, "Failed to create temp dir");
            return None;
        }
    };
    let temp_path = temp_dir.path().to_path_buf();

    for entry in walkdir::WalkDir::new(app_dir).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            let rel = entry.path().strip_prefix(app_dir).unwrap_or(entry.path());
            let dest = temp_path.join(rel);
            if let Some(parent) = dest.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let _ = fs::copy(entry.path(), &dest);
        }
    }

    // Generate shell .slint (single tab, no page switching)
    let app_slint_path = temp_path.join(manifest.entry_ui.as_deref().unwrap_or("app.slint"));
    let tab = AppTab { name: app_name.clone(), display_name: app_name.clone() };
    let shell_source = generate_page_shell(&app_slint_path, &[tab.clone()], &app_name, &app_name, None);
    let shell_path = match write_shell_slint(&temp_path, &shell_source) {
        Ok(p) => p,
        Err(e) => {
            tracing::error!(error = %e, "Failed to write shell slint");
            return None;
        }
    };

    // Create channels
    let (ui_tx, ui_rx) = tokio::sync::mpsc::channel::<UiMutation>(256);
    let (query_tx, query_rx) = tokio::sync::mpsc::channel::<UiQuery>(32);
    let (_page_update_tx, page_update_rx) = tokio::sync::mpsc::channel::<PageUpdate>(256);
    let (tab_switch_tx, tab_switch_rx) = std::sync::mpsc::channel::<String>();
    let (asset_pick_tx, asset_pick_rx) = std::sync::mpsc::channel::<AssetPickRequest>();

    // Set package.path so require("module") finds .lua files in the app's temp directory
    let lua_code = format!(
        "package.path = '{}/?.lua;' .. package.path\n{}",
        temp_path.display(),
        raw_lua_code
    );

    // Spawn Lua runtime
    let config = LuaRuntimeConfig {
        page_id: page_id.to_string(),
        app_name: app_name.clone(),
        scribe,
        user_did: user_did.to_string(),
        user_name: user_name.to_string(),
        user_role: user_role.to_string(),
        lua_code,
        ui_enabled: true,
        ui_tx: Some(ui_tx),
        query_tx: Some(query_tx),
        navigate_tx: None,
    };

    let (lua_thread, lua_tx) = match LuaRuntime::spawn(config) {
        Ok((thread, tx)) => (thread, tx),
        Err(e) => {
            tracing::error!(error = %e, "Failed to spawn Lua worker");
            return None;
        }
    };

    // Create Slint runtime
    let mut slint_runtime = match SlintRuntime::load(
        shell_path,
        app_name.clone(),
        ui_rx,
        query_rx,
        lua_tx.clone(),
    ) {
        Ok(runtime) => runtime,
        Err(e) => {
            tracing::error!(error = %e, "Failed to create SlintRuntime");
            return None;
        }
    };

    slint_runtime.set_tab_switch_channel(tab_switch_tx);
    slint_runtime.set_asset_pick_channel(asset_pick_tx);

    if let Err(e) = slint_runtime.slint_instance().show() {
        tracing::error!(error = ?e, "Failed to show app window");
        return None;
    }

    if let Err(e) = slint_runtime.setup_callbacks() {
        tracing::error!(error = %e, "Failed to setup shell callbacks");
        return None;
    }

    if let Err(e) = slint_runtime.init_models(&manifest.models) {
        tracing::warn!(error = %e, "Failed to init models from manifest");
    }

    if let Err(e) = slint_runtime.setup_global_callbacks() {
        tracing::error!(error = %e, "Failed to setup global callbacks");
        return None;
    }

    // Keep temp dir alive
    let _ = temp_dir.keep();

    let files: HashMap<String, String> = HashMap::new();
    let version = AppVersion::new(&manifest.version, &files);

    Some(RunningSlintApp {
        app_name,
        page_id: page_id.to_string(),
        page_name: page_id.to_string(),
        all_apps: vec![tab],
        slint_runtime,
        lua_thread,
        lua_tx,
        page_update_rx,
        tab_switch_rx,
        asset_pick_rx,
        version,
    })
}

/// Launch a self-managing test Slint app window
///
/// **Context**: Wraps create_test_slint_app() with a 100ms timer to pump UI mutations.
/// No page-update/restart logic since test apps don't hot-reload.
///
/// **Returns**: LaunchedApp with timer (must keep alive) and lua_tx for commands.
pub fn launch_test_slint_app(
    app_dir: &Path,
    page_id: &str,
    user_did: &str,
    user_name: &str,
    user_role: &str,
    scribe: Arc<dyn ScribeHandle>,
) -> Option<LaunchedApp> {
    let running = create_test_slint_app(app_dir, page_id, user_did, user_name, user_role, scribe)?;
    let lua_tx_out = running.lua_tx.clone();

    let running = Rc::new(RefCell::new(Some(running)));

    let timer = slint::Timer::default();
    timer.start(slint::TimerMode::Repeated, Duration::from_millis(100), move || {
        let mut running_ref = running.borrow_mut();
        let Some(running_app) = running_ref.as_mut() else { return };

        // Process UI mutations (Lua -> Slint)
        if let Err(e) = running_app.slint_runtime.process_ui_mutations() {
            tracing::warn!(error = %e, "Failed to process UI mutations");
        }

        // Process UI queries (Slint -> Lua)
        running_app.slint_runtime.process_ui_queries();
    });

    Some(LaunchedApp {
        timer,
        lua_tx: lua_tx_out,
    })
}

// Slint Validation

/// Validate all Slint files in a directory using the Slint compiler
///
/// **Context**: Called before importing pages/creating spaces to catch syntax errors early
pub async fn validate_slint_files(page_dir: &std::path::Path) -> Result<(), String> {
    use slint_interpreter::Compiler;
    use walkdir::WalkDir;

    // Only validate app.slint entry points, not helper files like theme.slint
    // which may only contain globals and can't be compiled standalone.
    let slint_files: Vec<PathBuf> = WalkDir::new(page_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().file_name().map_or(false, |name| name == "app.slint"))
        .map(|e| e.path().to_path_buf())
        .collect();

    if slint_files.is_empty() {
        return Ok(());
    }

    tokio::task::spawn_blocking(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| format!("Failed to create runtime: {}", e))?;

        let mut errors = Vec::new();

        for slint_path in slint_files {
            tracing::info!("Validating Slint file: {}", slint_path.display());
            let compiler = Compiler::default();
            let result = rt.block_on(compiler.build_from_path(&slint_path));

            let compile_errors: Vec<_> = result
                .diagnostics()
                .filter(|d| d.level() == slint_interpreter::DiagnosticLevel::Error)
                .map(|d| d.to_string())
                .collect();

            if !compile_errors.is_empty() {
                errors.push(format!("{}:\n  {}", slint_path.display(), compile_errors.join("\n  ")));
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(format!("Slint compilation errors:\n{}", errors.join("\n")))
        }
    })
    .await
    .map_err(|e| format!("Validation task failed: {}", e))?
}

// Internal Helpers

/// Capture window geometry for in-place reload
fn capture_window_geometry(app: &RunningSlintApp) -> WindowGeometry {
    let window = app.slint_runtime.slint_instance().window();
    let win_pos = window.position();
    let win_size = window.size();
    WindowGeometry {
        x: win_pos.x,
        y: win_pos.y,
        width: win_size.width,
        height: win_size.height,
    }
}

/// Update app status to failed
fn update_status_failed(app_status: &Option<Arc<RwLock<AppStatus>>>, error_msg: String) {
    if let Some(ref status) = app_status {
        if let Ok(mut s) = status.try_write() {
            s.status = "failed".to_string();
            s.error = Some(error_msg);
        }
    }
}
