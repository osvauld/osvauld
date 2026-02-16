use crate::{AppTab, AssetPickRequest, SlintRuntime};
use domains::AppManifest;
use std::collections::HashMap;
use std::path::PathBuf;

pub type Manifest = AppManifest;

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
    pub lua_tx: tokio::sync::mpsc::Sender<lua_runtime::LuaCommand>,
    pub page_update_rx: tokio::sync::mpsc::Receiver<butler::PageUpdate>,
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
    pub lua_tx: tokio::sync::mpsc::Sender<lua_runtime::LuaCommand>,
}
