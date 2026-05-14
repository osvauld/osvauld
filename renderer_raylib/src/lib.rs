//! Raylib Renderer for Games
//!
//! Renders games using Raylib with immediate-mode drawing.
//! This crate provides the Raylib-specific rendering layer for games.

mod canvas_bindings;
mod game_loop;
mod raylib_runtime;
pub mod synth;

pub use raylib_runtime::RaylibRuntime;

use butler::{Butler, ScribeMessage};
use domains::AppManifest;
use ractor::ActorRef;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

// Raylib can only be initialized once per process — guard against re-opens
static RAYLIB_RUNNING: AtomicBool = AtomicBool::new(false);

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

    // Raylib panics if initialized twice in the same process — reject re-opens
    if RAYLIB_RUNNING
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        tracing::warn!(app_name = %app_name, "Raylib already running in this process — ignoring re-open request");
        return Ok(());
    }

    std::thread::spawn(move || {
        let mut runtime =
            match RaylibRuntime::new(&page_id, &app_name, butler, scribe_ref, manifest, lua_code) {
                Ok(r) => r,
                Err(e) => {
                    tracing::error!(error = %e, "Failed to create Raylib runtime");
                    RAYLIB_RUNNING.store(false, Ordering::SeqCst);
                    return;
                }
            };

        if let Err(e) = runtime.run() {
            tracing::error!(error = %e, "Raylib runtime error");
        }
        RAYLIB_RUNNING.store(false, Ordering::SeqCst);
    });

    Ok(())
}
