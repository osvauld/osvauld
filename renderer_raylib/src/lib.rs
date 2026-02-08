//! Raylib Renderer for Games
//!
//! Renders games using Raylib with immediate-mode drawing.
//! This crate provides the Raylib-specific rendering layer for games.

mod raylib_runtime;
mod game_loop;
mod canvas_bindings;

pub use raylib_runtime::RaylibRuntime;

use butler::{Butler, ScribeMessage};
use ractor::ActorRef;
use std::sync::Arc;

/// Launch a Raylib game window
///
/// **Context**: Called when an app with renderer="raylib" is selected
/// **Threading**: Spawns a new OS thread for the game loop
/// **Note**: Manifest and lua_code must be pre-fetched by caller (avoids block_on in async context)
pub fn spawn_app(
    page_id: &str,
    app_name: &str,
    butler: Arc<Butler>,
    scribe_ref: ActorRef<ScribeMessage>,
    manifest: GameManifest,
    lua_code: String,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let page_id = page_id.to_string();
    let app_name = app_name.to_string();

    std::thread::spawn(move || {
        let mut runtime = match RaylibRuntime::new(
            &page_id,
            &app_name,
            butler,
            scribe_ref,
            manifest,
            lua_code,
        ) {
            Ok(r) => r,
            Err(e) => {
                tracing::error!(error = %e, "Failed to create Raylib runtime");
                return;
            }
        };

        if let Err(e) = runtime.run() {
            tracing::error!(error = %e, "Raylib runtime error");
        }
    });

    Ok(())
}

/// Game manifest for Raylib apps
#[derive(Debug, Clone, serde::Deserialize)]
pub struct GameManifest {
    pub name: String,
    pub version: String,
    pub entry_logic: String,
    /// Window width
    #[serde(default = "default_width")]
    pub width: u32,
    /// Window height
    #[serde(default = "default_height")]
    pub height: u32,
    /// Target FPS
    #[serde(default = "default_fps")]
    pub target_fps: u32,
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
