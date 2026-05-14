//! Dev launcher for renderer_egui.
//!
//! Loads a Lua file from disk and runs it through the real `EguiApp` runtime
//! (same code path as `spawn_app` from sthalam), minus the scribe/butler
//! wiring. Use this to eyeball a sample app during development without
//! publishing through the full peer flow.
//!
//! Usage:
//!     cargo run -p renderer_egui --example dev_launch -- path/to/app.lua
//!     cargo run -p renderer_egui --example dev_launch   # defaults to hello-egui

use renderer_egui::{EguiApp, GlfwEguiHost};
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let arg = std::env::args().nth(1);
    let path: PathBuf = match arg {
        Some(p) => PathBuf::from(p),
        None => PathBuf::from("sample_apps/hello-egui/hello-egui/app.lua"),
    };

    let lua_code = std::fs::read_to_string(&path)
        .map_err(|e| format!("failed to read {}: {}", path.display(), e))?;
    let app_name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("dev_app")
        .to_string();

    println!("dev_launch: loading {}", path.display());

    let mut app = EguiApp::new_headless(app_name.clone(), lua_code)?;
    let host = GlfwEguiHost::new(
        &format!("renderer_egui dev: {}", app_name),
        (900, 700),
    )?;
    host.run(|ctx| app.frame(ctx))
}
