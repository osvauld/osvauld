use crate::{AppTab, AssetPickRequest, SlintRuntime};
use domains::AppManifest;
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::sync::oneshot;

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

/// Mouse button for synthetic pointer events.
///
/// Maps onto `slint::platform::PointerEventButton`. We keep our own
/// enum so the control server / sthalam crate doesn't need to depend
/// on `slint::platform` types directly.
#[derive(Debug, Clone, Copy)]
pub enum UiMouseButton {
    Left,
    Right,
    Middle,
}

/// UI automation command, processed on the slint event-loop thread by
/// the per-app timer in `launch.rs`.
///
/// **Context**: external test harness drives the live winit window via
/// the control socket; commands are sent to the per-app receiver and
/// drained one-by-one each timer tick.
/// **Threading**: variants must not block; long-running motion (Drag)
/// is stepped across multiple ticks via `RunningSlintApp.active_drag`.
#[derive(Debug)]
pub enum AppUiCommand {
    /// Synthesize a `PointerMoved` at logical (x, y) in window coords.
    MouseMove {
        x: f32,
        y: f32,
        response_tx: oneshot::Sender<Result<(), String>>,
    },
    /// Synthesize a `PointerPressed` at logical (x, y) for `button`.
    MousePress {
        x: f32,
        y: f32,
        button: UiMouseButton,
        response_tx: oneshot::Sender<Result<(), String>>,
    },
    /// Synthesize a `PointerReleased` at logical (x, y) for `button`.
    MouseRelease {
        x: f32,
        y: f32,
        button: UiMouseButton,
        response_tx: oneshot::Sender<Result<(), String>>,
    },
    /// Press at `from`, interpolate over `steps` ticks to `to`, release.
    /// Reply fires when the press is queued; completion is observable
    /// via `WindowSize`/`Screenshot` polling or via state recording.
    Drag {
        from: (f32, f32),
        to: (f32, f32),
        steps: u32,
        button: UiMouseButton,
        response_tx: oneshot::Sender<Result<(), String>>,
    },
    /// Snapshot the window and write a PNG to `path`. Returns absolute path.
    Screenshot {
        path: PathBuf,
        response_tx: oneshot::Sender<Result<PathBuf, String>>,
    },
    /// Read current window logical size.
    WindowSize {
        response_tx: oneshot::Sender<Result<(f32, f32), String>>,
    },
    /// Begin a recording: snapshot the window once per tick + capture
    /// the listed global properties. `gif_path` and `states_path` are
    /// where artefacts get written when `RecordStop` arrives.
    RecordStart {
        gif_path: PathBuf,
        states_path: PathBuf,
        captures: Vec<GlobalCapture>,
        response_tx: oneshot::Sender<Result<(), String>>,
    },
    /// End the in-flight recording: flush frames to GIF + states to JSONL.
    /// Returns the two written paths.
    RecordStop {
        response_tx: oneshot::Sender<Result<(PathBuf, PathBuf), String>>,
    },
}

/// One global property to capture each tick during a recording.
///
/// `global` and `prop` are passed straight through to
/// `ComponentInstance::get_global_property(global, prop)`. `name` is
/// the JSON key used in the per-frame state object.
#[derive(Debug, Clone)]
pub struct GlobalCapture {
    pub name: String,
    pub global: String,
    pub prop: String,
}

/// One captured frame: pixel buffer + state JSON + relative timestamp.
pub struct RecordedFrame {
    pub t_ms: u64,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub state: serde_json::Value,
}

/// In-flight recording session, spanning `RecordStart` to `RecordStop`.
///
/// Capture is throttled to `target_fps` because `take_snapshot` does a full
/// glReadPixels + ~8 MB RGBA clone per frame — running every tick starves
/// the slint thread. Hard cap at `MAX_RECORDING_FRAMES` to bound memory.
pub struct RecordingState {
    pub gif_path: PathBuf,
    pub states_path: PathBuf,
    pub captures: Vec<GlobalCapture>,
    pub started_at: std::time::Instant,
    pub frames: Vec<RecordedFrame>,
    pub frames_dropped: u32,
    /// Target capture rate. Frames are pushed at most this fast.
    pub target_fps: u32,
    /// `t_ms` of the most recent captured frame; used to gate the
    /// next capture against the target_fps interval.
    pub last_capture_t_ms: u64,
}

/// Hard cap on captured frames to bound memory.
pub const MAX_RECORDING_FRAMES: usize = 600;

/// In-flight drag motion, advanced one step per timer tick.
///
/// Created when an `AppUiCommand::Drag` arrives; on each tick the timer
/// dispatches a `PointerMoved` at the interpolated position, then on
/// the final step dispatches `PointerReleased` and clears the slot.
#[derive(Debug, Clone, Copy)]
pub struct DragState {
    pub from: (f32, f32),
    pub to: (f32, f32),
    pub total_steps: u32,
    pub current_step: u32,
    pub button: UiMouseButton,
}

impl DragState {
    pub fn position(&self) -> (f32, f32) {
        let t = if self.total_steps == 0 {
            1.0
        } else {
            (self.current_step as f32) / (self.total_steps as f32)
        };
        (
            self.from.0 + (self.to.0 - self.from.0) * t,
            self.from.1 + (self.to.1 - self.from.1) * t,
        )
    }
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
    /// Channel to send UI automation commands; consumed by the per-app
    /// timer on the slint event-loop thread.
    pub app_ui_tx: tokio::sync::mpsc::Sender<AppUiCommand>,
}
