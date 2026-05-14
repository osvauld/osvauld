//! egui renderer for user Lua apps.
//!
//! In-process via GLFW + `egui_glow`, mirroring `renderer_raylib`. Each app
//! gets its own OS thread, GLFW window, Lua VM, and scribe `ActorRef`.
//! GLFW (rather than winit) so we coexist with Slint's main-thread event loop.

mod egui_runtime;
mod glfw_host;
mod ui_bindings;
mod types;

pub use egui_runtime::EguiApp;
pub use glfw_host::GlfwEguiHost;
pub use types::Manifest;

use butler::{Butler, ScribeMessage};
use domains::AppManifest;
use lua_runtime::LuaCommand;
use ractor::ActorRef;
use std::sync::Arc;
use tokio::sync::mpsc;

/// Handle returned by `spawn_app`.
///
/// Carries the `LuaCommand` sender so sthalam's control-server bridge can
/// route `DebugEval` to our Lua VM. Other variants are accepted-and-logged.
pub struct EguiAppHandle {
    pub lua_tx: mpsc::Sender<LuaCommand>,
}

/// Launch an egui window for an uploaded Lua app.
///
/// Spawns a new OS thread owning the GLFW window, GL context, egui `Context`,
/// and Lua VM. Linux/Wayland tolerates a GLFW window on a worker thread; macOS
/// is stricter and not yet supported.
pub fn spawn_app(
    page_id: &str,
    app_name: &str,
    butler: Arc<Butler>,
    scribe_ref: ActorRef<ScribeMessage>,
    _manifest: AppManifest,
    lua_code: String,
) -> Result<EguiAppHandle, Box<dyn std::error::Error + Send + Sync>> {
    let page_id = page_id.to_string();
    let app_name = app_name.to_string();
    let title = app_name.clone();

    // Subscribe BEFORE handing `scribe_ref` to the app thread. The tokio rx
    // feeds the wake-up thread; the std channel is what the app drains in `frame()`.
    let (scribe_tx, mut scribe_rx) = mpsc::channel(256);
    scribe_ref
        .cast(ScribeMessage::SubscribeToPageUpdates { tx: scribe_tx })
        .map_err(|e| format!("subscribe to page updates: {}", e))?;
    let (page_update_tx, page_update_rx) = std::sync::mpsc::channel();

    let (lua_tx, lua_rx) = mpsc::channel::<LuaCommand>(32);
    let handle = EguiAppHandle {
        lua_tx: lua_tx.clone(),
    };

    std::thread::Builder::new()
        .name(format!("egui-app:{}", app_name))
        .spawn(move || {
            let mut app = match EguiApp::new(
                page_id.clone(),
                app_name.clone(),
                butler,
                scribe_ref,
                lua_code,
                page_update_rx,
                lua_rx,
            ) {
                Ok(a) => a,
                Err(e) => {
                    tracing::error!(page_id = %page_id, error = %e, "EguiApp::new failed");
                    return;
                }
            };

            let host = match GlfwEguiHost::new(&title, (900, 700)) {
                Ok(h) => h,
                Err(e) => {
                    tracing::error!(page_id = %page_id, error = %e, "GlfwEguiHost::new failed");
                    return;
                }
            };

            // Wake-up thread: bridges scribe broadcasts to the egui frame loop.
            // egui is reactive — without an explicit `request_repaint()` here,
            // peer-driven CRDT changes wouldn't redraw the window.
            let ctx_clone = host.egui_ctx().clone();
            let wakeup_page_id = page_id.clone();
            std::thread::Builder::new()
                .name(format!("egui-wakeup:{}", app_name))
                .spawn(move || {
                    while let Some(update) = scribe_rx.blocking_recv() {
                        if page_update_tx.send(update).is_err() {
                            break; // EguiApp dropped — nothing more to forward.
                        }
                        ctx_clone.request_repaint();
                    }
                    tracing::debug!(page_id = %wakeup_page_id, "egui wake-up thread exiting");
                })
                .expect("spawn wake-up thread");

            // Repaint nudge for control-socket eval commands — without this,
            // queued `lua_rx` messages would only drain on the next user-input
            // repaint. 50 ms balances interactivity vs CPU.
            let eval_wake_ctx = host.egui_ctx().clone();
            let eval_wake_tx = lua_tx.clone();
            // Drop the local sender so the handle + this thread control liveness.
            drop(lua_tx);
            std::thread::Builder::new()
                .name(format!("egui-eval-wake:{}", app_name))
                .spawn(move || {
                    while !eval_wake_tx.is_closed() {
                        eval_wake_ctx.request_repaint();
                        std::thread::sleep(std::time::Duration::from_millis(50));
                    }
                })
                .expect("spawn eval-wake thread");

            if let Err(e) = host.run(|ctx| app.frame(ctx)) {
                tracing::error!(page_id = %page_id, error = %e, "egui host run error");
            }
            tracing::info!(page_id = %page_id, "egui app exited");
        })?;

    Ok(handle)
}
