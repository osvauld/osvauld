//! Raylib Renderer for Games
//!
//! Renders games using Raylib with immediate-mode drawing.
//! This crate provides the Raylib-specific rendering layer for games.

mod canvas_bindings;
mod game_loop;
mod raylib_runtime;

pub use raylib_runtime::RaylibRuntime;

use butler::{Butler, ScribeMessage};
use domains::AppManifest;
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
    manifest: AppManifest,
    lua_code: String,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let page_id = page_id.to_string();
    let app_name = app_name.to_string();

    std::thread::spawn(move || {
        let mut runtime =
            match RaylibRuntime::new(&page_id, &app_name, butler, scribe_ref, manifest, lua_code) {
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
