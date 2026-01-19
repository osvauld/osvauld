//! App runtime management
//!
//! Handles running apps with parallel Lua workers.
//! Apps are components (not Windows) - browser provides the window with tabs.

use butler::Butler;
use app_runtime::{generate_page_shell, write_shell_slint, AppTab};

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
    /// Temp directory (must keep alive)
    pub temp_dir: std::path::PathBuf,
}

/// Extract app and generate browser shell with tabs
///
/// **Architecture:**
/// - Apps are components (not Windows)
/// - Browser generates shell with Window + TabBar + imports App
///
/// **Returns:** PreparedPage with paths to shell and lua files
pub async fn prepare_page(
    butler: &Butler,
    page_id: &str,
    app_name: Option<&str>,
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

    // Get all files from the app layer
    let files = butler.get_app_files(page_id, &app_name).await?;

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

    // Build tabs for all apps
    let tabs: Vec<AppTab> = all_apps
        .iter()
        .map(|name| AppTab {
            name: name.clone(),
            display_name: name.clone(), // Could parse from manifest for prettier names
        })
        .collect();

    // Generate shell that imports the app
    let app_slint_path = temp_path.join("app.slint");
    let shell_source = generate_page_shell(&app_slint_path, &tabs, &app_name, &page_name);

    // Write shell to temp directory
    let shell_path = write_shell_slint(&temp_path, &shell_source)?;

    let lua_path = temp_path.join("app.lua");

    // Read models from manifest.json (optional field)
    let manifest_path = temp_path.join("manifest.json");
    let models = if manifest_path.exists() {
        match fs::read_to_string(&manifest_path) {
            Ok(content) => {
                match serde_json::from_str::<app_runtime::Manifest>(&content) {
                    Ok(manifest) => manifest.models,
                    Err(e) => {
                        tracing::warn!(
                            page_id = %page_id,
                            app_name = %app_name,
                            error = %e,
                            "Failed to parse manifest.json, using empty models"
                        );
                        vec![]
                    }
                }
            }
            Err(e) => {
                tracing::warn!(
                    page_id = %page_id,
                    app_name = %app_name,
                    error = %e,
                    "Failed to read manifest.json, using empty models"
                );
                vec![]
            }
        }
    } else {
        vec![]
    };

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
        temp_dir: temp_path,
    })
}

/// Extract app files from Butler to temporary directory (legacy, use prepare_page instead)
#[deprecated(note = "Use prepare_page instead for tabbed pages")]
pub async fn extract_app_to_temp(
    butler: &Butler,
    page_id: &str,
    app_name: Option<&str>,
) -> Result<(std::path::PathBuf, String), Box<dyn std::error::Error>> {
    let prepared = prepare_page(butler, page_id, app_name).await?;
    Ok((prepared.temp_dir, prepared.app_name))
}
