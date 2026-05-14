//! Sthalam - Main Application
//!
//! Platform for running Lua apps with CRDT sync.
//! This binary orchestrates:
//! - sthalam_shell: Platform shell UI (auth, spaces, pages, nodes, publish, viewer)
//! - renderer_slint: Self-managing Slint app windows
//! - renderer_raylib: Raylib game windows (optional)

mod control_server;
mod events;
mod setup;
mod ui_automation;

#[cfg(feature = "profiling")]
use std::fs::File;

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use butler::{Butler, LayerCache, RedbStore, ScribeMessage, SyncEvent};
use clap::Parser;
use courier::CourierHandle;
#[cfg(any(feature = "raylib", feature = "egui"))]
use domains::AppManifest;
use renderer_slint::{AppStatus, DebugEvalRequest, LaunchedApp, PreparedPage};
use slint::ComponentHandle;
use sthalam_shell::Shell;
use tokio::sync::{mpsc, RwLock};

use crate::control_server::{ControlServer, UiCommand};

#[derive(Parser)]
#[command(name = "sthalam")]
#[command(about = "Sthalam - Sovereign browsing and hosting")]
struct Args {
    /// Database name (without .db extension)
    #[arg(short, long, default_value = "sthalam")]
    db_name: String,

    /// Unix socket path for debug/control server (enables programmatic control)
    #[arg(long)]
    debug_socket: Option<String>,

    /// Enable test mode (use ManualClock for deterministic time control)
    #[arg(long)]
    test_mode: bool,
}

fn main() {
    let use_testing_backend = std::env::var("SLINT_BACKEND")
        .map(|v| v == "testing")
        .unwrap_or(false);

    if use_testing_backend {
        i_slint_backend_testing::init_integration_test_with_system_time();
    }

    let args = Args::parse();

    tracing::info!("Sthalam starting with raylib feature: {}", cfg!(feature = "raylib"));

    // Suppress Qt/Wayland text input warnings
    std::env::set_var("QT_LOGGING_RULES", "qt.qpa.wayland.textinput=false");

    #[cfg(feature = "profiling")]
    let _flame_guard = setup_profiling();

    // Profiling owns the subscriber, so skip logging init in that case.
    #[cfg(not(feature = "profiling"))]
    let use_json = std::env::var("OSVAULD_LOG_FORMAT")
        .map(|v| v == "json")
        .unwrap_or(false);

    #[cfg(not(feature = "profiling"))]
    let (_log_guard, capture_handle) =
        logging_utils::init_rich_tracing_with_capture(logging_utils::LogConfig {
            level: "info".to_string(),
            log_to_stdout: true,
            use_tree_format: !use_json,
            stdout_json: use_json,
            instance_name: Some(args.db_name.clone()),
            ..Default::default()
        })
        .expect("Failed to initialize logging");

    tracing::info!("Sthalam starting...");

    let clock: Arc<dyn domains::ClockSource> = if args.test_mode {
        let unix_now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("System time before UNIX epoch")
            .as_secs() as i64;
        tracing::info!(unix_now, "Test mode enabled: using ManualClock");
        Arc::new(domains::ManualClock::new(unix_now))
    } else {
        Arc::new(domains::RealClock)
    };

    let tokio_rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed to create tokio runtime");
    let tokio_handle = tokio_rt.handle().clone();

    let data_dir = std::env::var("STHALAM_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            dirs::data_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("sthalam")
        });

    std::fs::create_dir_all(&data_dir).expect("Failed to create data directory");

    let db_path = data_dir.join(format!("{}.db", args.db_name));
    tracing::info!(path = %db_path.display(), "Using database");

    let store = Arc::new(RedbStore::open(&db_path).expect("Failed to open database"));
    let layer_cache = Arc::new(RwLock::new(LayerCache::new(store.clone(), 100)));
    let assets_path = data_dir.join("assets");
    let asset_store =
        Arc::new(butler::AssetStore::new(&assets_path).expect("Failed to create asset store"));

    // Scribe -> Coordinator sync events.
    let (sync_tx, sync_rx) = tokio::sync::mpsc::channel::<SyncEvent>(32);
    let sync_event_rx = Arc::new(std::sync::Mutex::new(Some(sync_rx)));

    let butler = Arc::new(Butler::new(
        store.clone(),
        layer_cache,
        asset_store,
        Some(sync_tx),
    ));

    // P2P state - initialized after login.
    let courier_handle: Arc<RwLock<Option<CourierHandle>>> = Arc::new(RwLock::new(None));

    let mut ui_rx: Option<mpsc::Receiver<UiCommand>> = None;
    let debug_server: Option<Arc<ControlServer>> = if let Some(ref socket_path) = args.debug_socket
    {
        let socket_path = PathBuf::from(socket_path);
        let instance_name = args.db_name.clone();

        let (ui_tx, rx) = mpsc::channel::<UiCommand>(100);
        ui_rx = Some(rx);

        let server = ControlServer::new(socket_path.clone(), instance_name, courier_handle.clone());
        server.set_butler(butler.clone());
        server.set_ui_channel(ui_tx);

        #[cfg(not(feature = "profiling"))]
        server.set_capture_handle(capture_handle.clone());
        let server = Arc::new(server);

        let server_clone = server.clone();
        tokio_handle.spawn(async move {
            if let Err(e) = server_clone.start().await {
                tracing::error!(error = %e, "Debug server error");
            }
        });

        tracing::info!(socket = %socket_path.display(), "Debug server enabled");
        Some(server)
    } else {
        None
    };

    let is_signed_up = butler.is_signed_up().unwrap_or(false);
    tracing::info!(is_signed_up = is_signed_up, "Signup status");

    let shell = Shell::new().expect("Failed to create shell");

    shell.set_initialized(is_signed_up);
    shell.set_authenticated(false);

    if is_signed_up {
        shell.set_current_screen("login".into());
    } else {
        shell.set_current_screen("initiation".into());
    }

    #[cfg(not(feature = "profiling"))]
    let capture_tx = {
        let (tx, _) = tokio::sync::broadcast::channel::<String>(1024);
        tokio_handle.block_on(capture_handle.register_source(tx.clone()));
        tokio_handle.block_on(butler.set_capture_tx(tx.clone()));
        Some(tx)
    };
    #[cfg(feature = "profiling")]
    let capture_tx: Option<tokio::sync::broadcast::Sender<String>> = None;

    // Auth callback: on login success, initialize P2P and event listener.
    let butler_for_login = butler.clone();
    let courier_handle_for_login = courier_handle.clone();
    let sync_event_rx_for_login = sync_event_rx.clone();
    let shell_weak_for_login = shell.as_weak();
    let tokio_handle_for_login = tokio_handle.clone();
    let capture_tx_for_login = capture_tx.clone();

    let on_login_success: sthalam_shell::callbacks::auth::OnLoginSuccess =
        Arc::new(move |identity| {
            let butler = butler_for_login.clone();
            let courier_handle = courier_handle_for_login.clone();
            let sync_rx = sync_event_rx_for_login.clone();
            let shell_weak = shell_weak_for_login.clone();
            let tokio_handle = tokio_handle_for_login.clone();
            let capture_tx = capture_tx_for_login.clone();

            tokio_handle.spawn(async move {
                butler.set_identity(identity.clone()).await;

                // sync_rx can only be taken once.
                let sync_rx = sync_rx.lock().unwrap().take();

                tracing::info!("Initializing P2P...");
                if let Some(sync_rx) = sync_rx {
                    match setup::init_p2p(butler.clone(), sync_rx, capture_tx).await {
                        Ok((handle, event_rx)) => {
                            tracing::info!("P2P initialized successfully");
                            *courier_handle.write().await = Some(handle.clone());

                            spawn_auto_reconnect(butler.clone(), handle.clone());

                            events::spawn_event_listener(
                                event_rx,
                                shell_weak,
                                butler,
                                handle.clone(),
                            );
                        }
                        Err(e) => {
                            tracing::error!(error = %e, "Failed to initialize P2P");
                        }
                    }
                } else {
                    tracing::error!("Sync event receiver already taken - P2P already initialized?");
                }
            });
        });

    sthalam_shell::callbacks::auth::register(&shell, butler.clone(), on_login_success);
    sthalam_shell::callbacks::spaces::register(&shell, butler.clone());
    sthalam_shell::callbacks::pages::register(&shell, butler.clone());
    sthalam_shell::callbacks::nodes::register(
        &shell,
        butler.clone(),
        courier_handle.clone(),
        tokio_handle.clone(),
    );
    sthalam_shell::callbacks::publish::register(
        &shell,
        butler.clone(),
        courier_handle.clone(),
        tokio_handle.clone(),
    );
    sthalam_shell::callbacks::viewer::register(
        &shell,
        butler.clone(),
        courier_handle.clone(),
        tokio_handle.clone(),
    );

    // Channel for sending prepared apps from tokio to the Slint thread.
    let (app_ready_tx, app_ready_rx) =
        std::sync::mpsc::channel::<(PreparedPage, ractor::ActorRef<ScribeMessage>)>();

    // Drop = window stops processing.
    let app_timers: Rc<RefCell<Vec<LaunchedApp>>> = Rc::new(RefCell::new(vec![]));

    let app_status: Option<Arc<RwLock<AppStatus>>> = debug_server.as_ref().map(|s| s.app_status());

    {
        let butler = butler.clone();
        let tokio_handle = tokio_handle.clone();
        let app_ready_tx = app_ready_tx.clone();
        let shell_weak = shell.as_weak();
        // launch_egui_app needs the control-server to wire the eval channel into
        // the per-app Lua VM; clone for move-capture below.
        let debug_server_for_egui = debug_server.clone();

        shell.on_select_app(move |app_name| {
            let page_id = if let Some(shell) = shell_weak.upgrade() {
                shell.get_current_page_id().to_string()
            } else {
                return;
            };

            if page_id.is_empty() {
                tracing::warn!("No page selected");
                return;
            }

            let app_name = app_name.to_string();
            tracing::info!(page_id = %page_id, app_name = %app_name, "Selecting app");

            let butler = butler.clone();
            let tx = app_ready_tx.clone();
            let debug_server_for_egui = debug_server_for_egui.clone();
            let tokio_for_egui = tokio_handle.clone();

            tokio_handle.spawn(async move {
                let scribe_ref = match butler.open_page(&page_id).await {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::error!(error = %e, "Failed to open page");
                        return;
                    }
                };

                match renderer_slint::get_app_manifest(&scribe_ref, &app_name).await {
                    Ok(manifest) => {
                        tracing::info!(
                            app_name = %app_name,
                            renderer = %manifest.renderer,
                            "Manifest loaded, checking renderer type"
                        );
                        if manifest.renderer == "raylib" {
                            #[cfg(feature = "raylib")]
                            {
                                launch_raylib_app(
                                    &page_id,
                                    &app_name,
                                    butler,
                                    scribe_ref,
                                    manifest,
                                )
                                .await;
                            }
                            #[cfg(not(feature = "raylib"))]
                            {
                                tracing::warn!("Raylib renderer not enabled (compile with --features raylib)");
                            }
                            return;
                        }
                        if manifest.renderer == "egui" {
                            #[cfg(feature = "egui")]
                            {
                                launch_egui_app(
                                    &page_id,
                                    &app_name,
                                    butler,
                                    scribe_ref,
                                    manifest,
                                    debug_server_for_egui.clone(),
                                    tokio_for_egui.clone(),
                                )
                                .await;
                            }
                            #[cfg(not(feature = "egui"))]
                            {
                                tracing::warn!("egui renderer not enabled (compile with --features egui)");
                            }
                            return;
                        }
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "Failed to get manifest, falling back to Slint renderer");
                    }
                }

                match renderer_slint::prepare_page(&butler, &page_id, Some(&app_name), &scribe_ref)
                    .await
                {
                    Ok(prepared) => {
                        if tx.send((prepared, scribe_ref)).is_err() {
                            tracing::error!("Failed to send prepared app to Slint thread");
                        }
                        // Wake the Slint event loop so the collector timer drains the channel.
                        slint::invoke_from_event_loop(|| {}).ok();
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "Failed to prepare page");
                    }
                }
            });
        });
    }

    // Collector timer: drains app_ready, launches windows, wires the debug eval bridge.
    let collector_timer = {
        let app_timers = app_timers.clone();
        let butler = butler.clone();
        let tokio_handle = tokio_handle.clone();
        let app_status = app_status.clone();
        let debug_server = debug_server.clone();
        let clock = clock.clone();

        let timer = slint::Timer::default();
        timer.start(
            slint::TimerMode::Repeated,
            std::time::Duration::from_millis(100),
            move || {
                while let Ok((prepared, scribe_ref)) = app_ready_rx.try_recv() {
                    let app_name_for_log = prepared.app_name.clone();
                    tracing::info!(
                        page_id = %prepared.page_id,
                        app_name = %app_name_for_log,
                        "Launching app window"
                    );

                    let launch_result =
                        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            renderer_slint::launch_slint_app(
                                prepared,
                                scribe_ref,
                                butler.clone(),
                                tokio_handle.clone(),
                                app_status.clone(),
                                clock.clone(),
                            )
                        }));

                    let launched = match launch_result {
                        Ok(opt) => opt,
                        Err(panic_payload) => {
                            let msg = if let Some(s) = panic_payload.downcast_ref::<&str>() {
                                s.to_string()
                            } else if let Some(s) = panic_payload.downcast_ref::<String>() {
                                s.clone()
                            } else {
                                "unknown panic".to_string()
                            };
                            tracing::error!(
                                app = %app_name_for_log,
                                panic = %msg,
                                "App panicked during launch — skipping"
                            );
                            None
                        }
                    };

                    if let Some(launched) = launched {
                        if let Some(ref server) = debug_server {
                            let lua_tx = launched.lua_tx.clone();

                            // ControlServer -> forwarding task -> LuaCommand::DebugEval.
                            let (eval_tx, mut eval_rx) =
                                tokio::sync::mpsc::channel::<DebugEvalRequest>(32);
                            server.set_eval_channel(eval_tx);

                            let lua_tx_for_eval = lua_tx.clone();
                            tokio_handle.spawn(async move {
                                while let Some(req) = eval_rx.recv().await {
                                    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
                                    if lua_tx_for_eval
                                        .send(lua_runtime::LuaCommand::DebugEval {
                                            code: req.code,
                                            response_tx: reply_tx,
                                        })
                                        .await
                                        .is_ok()
                                    {
                                        let result = reply_rx.await.unwrap_or_else(|_| {
                                            Err("Eval reply channel closed".to_string())
                                        });
                                        let _ = req.response_tx.send(result);
                                    } else {
                                        let _ = req
                                            .response_tx
                                            .send(Err("Lua worker channel closed".to_string()));
                                    }
                                }
                            });

                            server.set_lua_worker_channel(lua_tx);

                            // Synthetic pointer events / screenshots.
                            server.set_app_ui_channel(launched.app_ui_tx.clone());
                        }

                        app_timers.borrow_mut().push(launched);
                    }
                }
            },
        );
        timer
    };

    let _ui_timer = if let Some(mut ui_rx) = ui_rx {
        let shell_weak = shell.as_weak();
        let timer = slint::Timer::default();
        timer.start(
            slint::TimerMode::Repeated,
            std::time::Duration::from_millis(16), // ~60fps
            move || {
                if let Some(shell) = shell_weak.upgrade() {
                    ui_automation::process_ui_commands(&shell, &mut ui_rx);
                }
            },
        );
        Some(timer)
    } else {
        None
    };

    // Keep timers alive until shell exits.
    let _collector_timer = collector_timer;
    let _app_timers = app_timers;

    shell.run().expect("Failed to run shell");
}

/// Spawn background task to auto-reconnect to stored nodes
fn spawn_auto_reconnect(butler: Arc<Butler>, handle: CourierHandle) {
    tokio::spawn(async move {
        // Owner-side: sovereign nodes.
        let nodes = butler.nodes().list().unwrap_or_default();
        for node in nodes {
            tracing::info!(name = %node.name, node_id = %node.node_id, "Auto-connecting to sovereign node");
            if let Some(permit) = &node.permit {
                if let Err(e) = handle.connect(&node.node_id, permit) {
                    tracing::warn!(name = %node.name, error = %e, "Failed to initiate connection");
                    let _ = butler.nodes().set_connected(&node.node_id, false);
                }
            } else {
                tracing::info!(name = %node.name, "No permit stored, skipping");
                let _ = butler.nodes().set_connected(&node.node_id, false);
            }
        }

        // Viewer-side: node contacts.
        let node_contacts = butler.contacts().list_nodes().unwrap_or_default();
        for contact in node_contacts {
            if let (Some(node_id), Some(permit)) = (&contact.node_id, &contact.permit) {
                tracing::info!(name = %contact.username, node_id = %node_id, "Auto-connecting to node contact");
                if let Err(e) = handle.connect(node_id, permit) {
                    tracing::warn!(name = %contact.username, error = %e, "Failed to initiate viewer connection");
                }
            }
        }
    });
}

/// Launch a Raylib app (when manifest.renderer == "raylib")
#[cfg(feature = "raylib")]
async fn launch_raylib_app(
    page_id: &str,
    app_name: &str,
    butler: Arc<Butler>,
    scribe_ref: ractor::ActorRef<ScribeMessage>,
    manifest: AppManifest,
) {
    let files = match renderer_slint::get_app_files_from_scribe(&scribe_ref, app_name).await {
        Ok(f) => f,
        Err(e) => {
            tracing::error!(error = %e, "Failed to get app files for Raylib app");
            return;
        }
    };

    let lua_code = match files
        .get("app.lua")
        .or_else(|| files.get(&manifest.entry_logic))
    {
        Some(code) => code.clone(),
        None => {
            tracing::error!("No Lua entry file found for Raylib app");
            return;
        }
    };

    if let Err(e) =
        renderer_raylib::spawn_app(page_id, app_name, butler, scribe_ref, manifest, lua_code)
    {
        tracing::error!(error = %e, "Failed to spawn Raylib app");
    }
}

/// Launch an egui app (when manifest.renderer == "egui").
///
/// Spawns the per-app egui window + Lua VM via `renderer_egui::spawn_app` and,
/// if the debug server is active, wires a `DebugEvalRequest -> LuaCommand::DebugEval`
/// forwarding task (mirrors the Slint collector's eval bridge).
#[cfg(feature = "egui")]
async fn launch_egui_app(
    page_id: &str,
    app_name: &str,
    butler: Arc<Butler>,
    scribe_ref: ractor::ActorRef<ScribeMessage>,
    manifest: AppManifest,
    debug_server: Option<Arc<ControlServer>>,
    tokio_handle: tokio::runtime::Handle,
) {
    let files = match renderer_slint::get_app_files_from_scribe(&scribe_ref, app_name).await {
        Ok(f) => f,
        Err(e) => {
            tracing::error!(error = %e, "Failed to get app files for egui app");
            return;
        }
    };

    let lua_code = match files
        .get("app.lua")
        .or_else(|| files.get(&manifest.entry_logic))
    {
        Some(code) => code.clone(),
        None => {
            tracing::error!("No Lua entry file found for egui app");
            return;
        }
    };

    tracing::info!(page_id = %page_id, app_name = %app_name, "launch_egui_app: spawning");
    let handle = match renderer_egui::spawn_app(
        page_id, app_name, butler, scribe_ref, manifest, lua_code,
    ) {
        Ok(h) => h,
        Err(e) => {
            tracing::error!(error = %e, "Failed to spawn egui app");
            return;
        }
    };
    tracing::info!(
        page_id = %page_id,
        app_name = %app_name,
        debug_server_some = %debug_server.is_some(),
        "launch_egui_app: spawned, wiring eval bridge",
    );

    // Eval bridge — mirrors the Slint collector: hand the control_server a
    // DebugEvalRequest sender and forward each into the egui app's LuaCommand channel.
    if let Some(server) = debug_server {
        let (eval_tx, mut eval_rx) =
            tokio::sync::mpsc::channel::<DebugEvalRequest>(32);
        server.set_eval_channel_async(eval_tx).await;
        tracing::info!("launch_egui_app: eval channel registered on control_server");
        let lua_tx = handle.lua_tx.clone();
        tokio_handle.spawn(async move {
            while let Some(req) = eval_rx.recv().await {
                tracing::debug!(code_len = req.code.len(), "egui eval bridge: received request");
                let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
                if lua_tx
                    .send(lua_runtime::LuaCommand::DebugEval {
                        code: req.code,
                        response_tx: reply_tx,
                    })
                    .await
                    .is_ok()
                {
                    let result = reply_rx.await.unwrap_or_else(|_| {
                        Err("egui eval reply channel closed".to_string())
                    });
                    let _ = req.response_tx.send(result);
                } else {
                    let _ = req
                        .response_tx
                        .send(Err("egui app lua channel closed".to_string()));
                }
            }
        });
    }
}

/// Setup profiling tools (tokio-console and/or tracing-flame).
///
/// Env vars: `TOKIO_CONSOLE_PORT` enables console layer, `FLAME_OUTPUT` enables flame layer.
#[cfg(feature = "profiling")]
fn setup_profiling() -> Option<tracing_flame::FlushGuard<std::io::BufWriter<File>>> {
    use tracing_subscriber::prelude::*;

    let console_port: Option<u16> = std::env::var("TOKIO_CONSOLE_PORT")
        .ok()
        .and_then(|p| p.parse().ok());
    let flame_path = std::env::var("FLAME_OUTPUT").ok();

    match (&flame_path, console_port) {
        (Some(path), Some(port)) => {
            let (flame_layer, guard) = match tracing_flame::FlameLayer::with_file(path) {
                Ok((layer, guard)) => (Some(layer), Some(guard)),
                Err(e) => {
                    eprintln!("Failed to create flame layer: {}", e);
                    (None, None)
                }
            };

            let console_layer = console_subscriber::ConsoleLayer::builder()
                .server_addr(([127, 0, 0, 1], port))
                .spawn();

            tracing_subscriber::registry()
                .with(console_layer)
                .with(flame_layer)
                .with(
                    tracing_subscriber::fmt::layer()
                        .with_target(true)
                        .with_level(true),
                )
                .init();

            tracing::info!(port = port, path = %path, "Profiling: tokio-console + flame");
            guard
        }
        // Flame only — filter out tokio runtime TRACE spans normally consumed by console-subscriber.
        (Some(path), None) => {
            let (flame_layer, guard) = match tracing_flame::FlameLayer::with_file(path) {
                Ok((layer, guard)) => (Some(layer), Some(guard)),
                Err(e) => {
                    eprintln!("Failed to create flame layer: {}", e);
                    (None, None)
                }
            };

            let filter = tracing_subscriber::EnvFilter::new("info,tokio=off,runtime=off");

            tracing_subscriber::registry()
                .with(filter)
                .with(flame_layer)
                .with(
                    tracing_subscriber::fmt::layer()
                        .with_target(true)
                        .with_level(true),
                )
                .init();

            tracing::info!(path = %path, "Profiling: flame only (no console overhead)");
            guard
        }
        (None, Some(port)) => {
            let console_layer = console_subscriber::ConsoleLayer::builder()
                .server_addr(([127, 0, 0, 1], port))
                .spawn();

            tracing_subscriber::registry()
                .with(console_layer)
                .with(
                    tracing_subscriber::fmt::layer()
                        .with_target(true)
                        .with_level(true),
                )
                .init();

            tracing::info!(port = port, "Profiling: tokio-console only");
            None
        }
        (None, None) => {
            let filter = tracing_subscriber::EnvFilter::new("info,tokio=off,runtime=off");

            tracing_subscriber::registry()
                .with(filter)
                .with(
                    tracing_subscriber::fmt::layer()
                        .with_target(true)
                        .with_level(true),
                )
                .init();

            tracing::info!(
                "Profiling feature enabled but no FLAME_OUTPUT or TOKIO_CONSOLE_PORT set"
            );
            None
        }
    }
}
