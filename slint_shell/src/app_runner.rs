//! App runtime management
//!
//! Handles running apps with parallel Lua workers.
//! Apps are components (not Windows) - browser provides the window with tabs.

use std::collections::HashMap;
use butler::Butler;
use butler::scribe::ScribeMessage;
use ractor::ActorRef;
use app_runtime::{generate_page_shell, write_shell_slint, AppTab};

/// Window geometry for in-place reload
///
/// **Context**: When an app restarts (layer update), we capture the window position/size
/// before closing, then restore it after showing the new window. This preserves
/// the user's window placement.
#[derive(Debug, Clone, Default)]
pub struct WindowGeometry {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// Running app instance with parallel Lua worker
pub struct RunningApp {
    /// App name (for tracking which app is running)
    pub app_name: String,
    /// Page ID (for reloading different apps in same page)
    pub page_id: String,
    /// Page name (for display)
    pub page_name: String,
    /// All apps in this page (for tab switching)
    pub all_apps: Vec<String>,
    /// Slint runtime (main thread, owns UI and VecModels)
    pub slint_runtime: app_runtime::SlintRuntime,
    /// Lua worker thread handle
    #[allow(dead_code)]
    pub lua_thread: std::thread::JoinHandle<()>,
    /// Channel to send commands to Lua worker
    pub lua_tx: tokio::sync::mpsc::Sender<app_runtime::LuaWorkerCommand>,
    /// Unified page event receiver (replaces loro_rx + scribe_event_rx)
    /// Receives all layer changes (Created/Updated) from Scribe
    pub page_event_rx: tokio::sync::mpsc::Receiver<butler::PageEvent>,
    /// Ephemeral event receiver (cursor, typing, presence from peers)
    /// Received via datagrams, forwarded to Lua for rendering
    pub ephemeral_event_rx: tokio::sync::mpsc::Receiver<butler::EphemeralEvent>,
    // Note: ephemeral_broadcast_rx removed - outbound ephemeral now goes directly via
    // Scribe → PeerActor channels (SubscriberInfo.ephemeral_tx)
    /// Receiver for tab switch requests
    pub tab_switch_rx: std::sync::mpsc::Receiver<String>,
    /// Receiver for asset pick requests (triggered by Slint button click)
    pub asset_pick_rx: std::sync::mpsc::Receiver<app_runtime::AssetPickRequest>,
    /// App version (semantic + content hash)
    pub version: app_runtime::AppVersion,
}

/// Result of preparing a page for loading
pub struct PreparedPage {
    /// Path to the generated shell.slint
    pub shell_path: std::path::PathBuf,
    /// Path to the app's Lua file
    pub lua_path: std::path::PathBuf,
    /// Name of the current app
    pub app_name: String,
    /// Page ID (for page-level operations)
    pub page_id: String,
    /// Name of the page (for window title)
    pub page_name: String,
    /// All apps in this page (for tabs)
    pub all_apps: Vec<String>,
    /// Data layers from permit template (for Loro subscriptions)
    pub data_layers: Vec<String>,
    /// Model names from manifest (for VecModel initialization)
    pub models: Vec<String>,
    /// Enable game loop tick() calls at ~60fps (from manifest)
    pub tick_enabled: bool,
    /// Temp directory (must keep alive)
    pub temp_dir: std::path::PathBuf,
    /// Window geometry to restore after reload (for in-place window reload)
    pub restore_geometry: Option<WindowGeometry>,
    /// App version (semantic + content hash)
    pub version: app_runtime::AppVersion,
}

/// Get app files from Scribe's in-memory layer
///
/// **Why**: Scribe has the latest synced content; storage may be stale
async fn get_app_files_from_scribe(
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
    let page = butler.get_page(page_id)?
        .ok_or_else(|| format!("Page {} not found", page_id))?;
    let page_name = page.meta.name.clone();

    // Get all apps in this page (for tabs)
    let all_apps = butler.list_apps(page_id)?;
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
    let data_layers = butler.list_data_layers(page_id).unwrap_or_default();
    tracing::debug!(
        page_id = %page_id,
        data_layers = ?data_layers,
        "Retrieved data layers from page"
    );

    let lua_path = temp_path.join("app.lua");

    // Read models, tick_enabled, and version from manifest.json
    let manifest_path = temp_path.join("manifest.json");
    let (models, tick_enabled, semantic_version) = if manifest_path.exists() {
        match fs::read_to_string(&manifest_path) {
            Ok(content) => {
                match serde_json::from_str::<app_runtime::Manifest>(&content) {
                    Ok(manifest) => (manifest.models, manifest.tick_enabled, manifest.version),
                    Err(e) => {
                        tracing::warn!(
                            page_id = %page_id,
                            app_name = %app_name,
                            error = %e,
                            "Failed to parse manifest.json, using defaults"
                        );
                        (vec![], false, "0.0.0".to_string())
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
                (vec![], false, "0.0.0".to_string())
            }
        }
    } else {
        (vec![], false, "0.0.0".to_string())
    };

    // Compute app version (semantic + content hash)
    let version = app_runtime::AppVersion::new(&semantic_version, &files);
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
            display_name: name.clone(), // Could parse from manifest for prettier names
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
        all_apps,
        data_layers,
        models,
        tick_enabled,
        temp_dir: temp_path,
        restore_geometry: None,
        version,
    })
}
