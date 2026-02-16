/// Unified app manifest (manifest.json) across renderers and node runtime.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct AppManifest {
    pub name: String,
    pub version: String,
    pub entry_logic: String,

    /// Renderer type: "slint" (default) or "raylib"
    #[serde(default = "default_renderer")]
    pub renderer: String,

    /// Slint UI entry point (used by Slint renderer)
    #[serde(default)]
    pub entry_ui: Option<String>,

    /// Array properties that need VecModel tracking for incremental updates
    #[serde(default)]
    pub models: Vec<String>,

    /// Window width (used by Raylib renderer)
    #[serde(default = "default_width")]
    pub width: u32,

    /// Window height (used by Raylib renderer)
    #[serde(default = "default_height")]
    pub height: u32,

    /// Target FPS (used by Raylib renderer)
    #[serde(default = "default_fps")]
    pub target_fps: u32,

    /// Node runtime entry script (used by kunki)
    #[serde(default)]
    pub entry_node: Option<String>,
}

fn default_renderer() -> String {
    "slint".to_string()
}

fn default_width() -> u32 {
    800
}

fn default_height() -> u32 {
    600
}

fn default_fps() -> u32 {
    60
}
