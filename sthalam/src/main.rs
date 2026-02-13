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
}

fn main() {
    // Initialize testing backend if requested via env var
    let use_testing_backend = std::env::var("SLINT_BACKEND")
        .map(|v| v == "testing")
        .unwrap_or(false);

    if use_testing_backend {
        i_slint_backend_testing::init_integration_test_with_system_time();
    }

    let args = Args::parse();

    // Suppress Qt/Wayland text input warnings
    std::env::set_var("QT_LOGGING_RULES", "qt.qpa.wayland.textinput=false");

    // Initialize profiling tools when feature is enabled
    #[cfg(feature = "profiling")]
    let _flame_guard = setup_profiling();

    // Initialize logging with capture support (skip if profiling takes over subscriber)
    #[cfg(not(feature = "profiling"))]
    let use_json = std::env::var("OSVAULD_LOG_FORMAT")
        .map(|v| v == "json")
        .unwrap_or(false);

    #[cfg(not(feature = "profiling"))]
    let (_log_guard, capture_handle) = logging_utils::init_rich_tracing_with_capture(logging_utils::LogConfig {
        level: "info".to_string(),
        log_to_stdout: true,
        use_tree_format: !use_json,
        stdout_json: use_json,
        instance_name: Some(args.db_name.clone()),
        ..Default::default()
    })
    .expect("Failed to initialize logging");

    tracing::info!("Sthalam starting...");

    // Create tokio runtime for async operations
    let tokio_rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed to create tokio runtime");
    let tokio_handle = tokio_rt.handle().clone();

    // Initialize data directory
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

    // Initialize Butler
    let store = Arc::new(RedbStore::open(&db_path).expect("Failed to open database"));
    let layer_cache = Arc::new(RwLock::new(LayerCache::new(store.clone(), 100)));
    let assets_path = data_dir.join("assets");
    let asset_store =
        Arc::new(butler::AssetStore::new(&assets_path).expect("Failed to create asset store"));

    // Create sync event channel for Scribe -> Coordinator communication
    let (sync_tx, sync_rx) = tokio::sync::mpsc::channel::<SyncEvent>(32);
    let sync_event_rx = Arc::new(std::sync::Mutex::new(Some(sync_rx)));

    let butler = Arc::new(Butler::new(store.clone(), layer_cache, asset_store, Some(sync_tx)));

    // P2P state - initialized after login
    let courier_handle: Arc<RwLock<Option<CourierHandle>>> = Arc::new(RwLock::new(None));

    // Start debug server if requested
    let mut ui_rx: Option<mpsc::Receiver<UiCommand>> = None;
    let debug_server: Option<Arc<ControlServer>> = if let Some(ref socket_path) = args.debug_socket
    {
        let socket_path = PathBuf::from(socket_path);
        let instance_name = args.db_name.clone();

        // Create UI command channel
        let (ui_tx, rx) = mpsc::channel::<UiCommand>(100);
        ui_rx = Some(rx);

        // Create debug server and set UI channel + butler + capture
        let server =
            ControlServer::new(socket_path.clone(), instance_name, courier_handle.clone());
        server.set_butler(butler.clone());
        server.set_ui_channel(ui_tx);

        // Set capture handle for event capture commands
        #[cfg(not(feature = "profiling"))]
        server.set_capture_handle(capture_handle.clone());
        let server = Arc::new(server);

        // Spawn debug server in background
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

    // Check signup status
    let is_signed_up = butler.is_signed_up().unwrap_or(false);
    tracing::info!(is_signed_up = is_signed_up, "Signup status");

    // Create the shell window
    let shell = Shell::new().expect("Failed to create shell");

    // Set initial state
    shell.set_initialized(is_signed_up);
    shell.set_authenticated(false);

    if is_signed_up {
        shell.set_current_screen("login".into());
    } else {
        shell.set_current_screen("initiation".into());
    }

    // Create capture broadcast channel for event capture
    #[cfg(not(feature = "profiling"))]
    let capture_tx = {
        let (tx, _) = tokio::sync::broadcast::channel::<String>(1024);
        tokio_handle.block_on(capture_handle.register_source(tx.clone()));
        tokio_handle.block_on(butler.set_capture_tx(tx.clone()));
        Some(tx)
    };
    #[cfg(feature = "profiling")]
    let capture_tx: Option<tokio::sync::broadcast::Sender<String>> = None;

    // Register callbacks

    // Auth callback: on login success, initialize P2P and event listener
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

                // Take the sync receiver (can only be done once)
                let sync_rx = sync_rx.lock().unwrap().take();

                tracing::info!("Initializing P2P...");
                if let Some(sync_rx) = sync_rx {
                    match setup::init_p2p(butler.clone(), sync_rx, capture_tx).await {
                        Ok((handle, event_rx)) => {
                            tracing::info!("P2P initialized successfully");
                            *courier_handle.write().await = Some(handle.clone());

                            // Auto-reconnect to stored sovereign nodes
                            spawn_auto_reconnect(butler.clone(), handle.clone());

                            // Spawn event listener
                            events::spawn_event_listener(event_rx, shell_weak, butler);
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

    // App launching (select_app callback)

    // Channel for sending prepared apps from tokio to Slint thread
    let (app_ready_tx, app_ready_rx) = std::sync::mpsc::channel::<(
        PreparedPage,
        ractor::ActorRef<ScribeMessage>,
    )>();

    // Store launched app timers (drop = window stops processing)
    let app_timers: Rc<RefCell<Vec<LaunchedApp>>> = Rc::new(RefCell::new(vec![]));

    // App status for debug server
    let app_status: Option<Arc<RwLock<AppStatus>>> =
        debug_server.as_ref().map(|s| s.app_status());

    {
        let butler = butler.clone();
        let tokio_handle = tokio_handle.clone();
        let app_ready_tx = app_ready_tx.clone();
        let shell_weak = shell.as_weak();

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

            tokio_handle.spawn(async move {
                // Get scribe for this page
                let scribe_ref = match butler.open_page(&page_id).await {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::error!(error = %e, "Failed to open page");
                        return;
                    }
                };

                // Check manifest to determine renderer
                match renderer_slint::get_app_manifest(&scribe_ref, &app_name).await {
                    Ok(manifest) if manifest.renderer == "raylib" => {
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
                    Ok(_) => {} // Slint renderer - continue below
                    Err(e) => {
                        tracing::warn!(error = %e, "Failed to get manifest, assuming Slint renderer");
                    }
                }

                // Prepare Slint app
                match renderer_slint::prepare_page(&butler, &page_id, Some(&app_name), &scribe_ref)
                    .await
                {
                    Ok(prepared) => {
                        if tx.send((prepared, scribe_ref)).is_err() {
                            tracing::error!("Failed to send prepared app to Slint thread");
                        }
                        // Wake up Slint event loop to process the channel
                        slint::invoke_from_event_loop(|| {}).ok();
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "Failed to prepare page");
                    }
                }
            });
        });
    }

    // Refresh app callback

    {
        let butler = butler.clone();
        let tokio_handle = tokio_handle.clone();

        shell.on_refresh_app(move |app_name, app_dir| {
            let app_name = app_name.to_string();
            let app_dir = PathBuf::from(app_dir.to_string());
            tracing::info!(app_name = %app_name, app_dir = %app_dir.display(), "Refresh app");

            let butler = butler.clone();

            tokio_handle.spawn(async move {
                // Find page containing this app by searching Butler's spaces/pages
                let page_id = find_page_for_app(&butler, &app_name);

                if let Some(page_id) = page_id {
                    match butler.open_page(&page_id).await {
                        Ok(scribe_ref) => {
                            let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
                            if let Err(e) = scribe_ref.cast(ScribeMessage::RefreshApp {
                                app_name: app_name.clone(),
                                app_dir,
                                reply: reply_tx,
                            }) {
                                tracing::error!(error = %e, "Failed to send RefreshApp to Scribe");
                                return;
                            }

                            match reply_rx.await {
                                Ok(Ok(changed_files)) => {
                                    tracing::info!(
                                        app_name = %app_name,
                                        changed_count = changed_files.len(),
                                        "App refreshed"
                                    );
                                }
                                Ok(Err(e)) => {
                                    tracing::error!(error = %e, "RefreshApp failed");
                                }
                                Err(_) => {
                                    tracing::error!("RefreshApp reply channel closed");
                                }
                            }
                        }
                        Err(e) => {
                            tracing::error!(error = %e, page_id = %page_id, "Failed to open page for refresh");
                        }
                    }
                } else {
                    tracing::warn!(app_name = %app_name, "Could not find page containing app");
                }
            });
        });
    }

    // Collector timer: poll app_ready channel, launch windows, connect debug eval

    let collector_timer = {
        let app_timers = app_timers.clone();
        let butler = butler.clone();
        let tokio_handle = tokio_handle.clone();
        let app_status = app_status.clone();
        let debug_server = debug_server.clone();

        let timer = slint::Timer::default();
        timer.start(
            slint::TimerMode::Repeated,
            std::time::Duration::from_millis(100),
            move || {
                while let Ok((prepared, scribe_ref)) = app_ready_rx.try_recv() {
                    tracing::info!(
                        page_id = %prepared.page_id,
                        app_name = %prepared.app_name,
                        "Launching app window"
                    );

                    if let Some(launched) = renderer_slint::launch_slint_app(
                        prepared,
                        scribe_ref,
                        butler.clone(),
                        tokio_handle.clone(),
                        app_status.clone(),
                    ) {
                        // Connect debug eval bridge if debug server is active
                        if let Some(ref server) = debug_server {
                            let lua_tx = launched.lua_tx.clone();

                            // Create eval channel: ControlServer -> forwarding task -> LuaCommand
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

                            // Also set direct lua_worker_tx for the control server
                            server.set_lua_worker_channel(lua_tx);
                        }

                        app_timers.borrow_mut().push(launched);
                    }
                }
            },
        );
        timer
    };

    // UI automation timer (if debug server is enabled)

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

    // Keep timers alive
    let _collector_timer = collector_timer;
    let _app_timers = app_timers;

    // Run the shell
    shell.run().expect("Failed to run shell");
}

/// Spawn background task to auto-reconnect to stored nodes
fn spawn_auto_reconnect(butler: Arc<Butler>, handle: CourierHandle) {
    tokio::spawn(async move {
        // Owner-side: Reconnect to sovereign nodes
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

        // Viewer-side: Reconnect to node contacts
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

/// Find the page that contains a given app by searching all spaces/pages
fn find_page_for_app(butler: &Butler, app_name: &str) -> Option<String> {
    let spaces = butler.spaces().list().ok()?;
    for space in &spaces {
        let pages = butler.pages().list(&space.id).ok()?;
        for page in &pages {
            let apps = butler.apps().list(&page.id).ok()?;
            if apps.iter().any(|a| a == app_name) {
                return Some(page.id.clone());
            }
        }
    }
    None
}

/// Launch a Raylib app (when manifest.renderer == "raylib")
#[cfg(feature = "raylib")]
async fn launch_raylib_app(
    page_id: &str,
    app_name: &str,
    butler: Arc<Butler>,
    scribe_ref: ractor::ActorRef<ScribeMessage>,
    manifest: renderer_slint::Manifest,
) {
    let files = match renderer_slint::get_app_files_from_scribe(&scribe_ref, app_name).await {
        Ok(f) => f,
        Err(e) => {
            tracing::error!(error = %e, "Failed to get app files for Raylib app");
            return;
        }
    };

    let lua_code = match files.get("app.lua").or_else(|| files.get(&manifest.entry_logic)) {
        Some(code) => code.clone(),
        None => {
            tracing::error!("No Lua entry file found for Raylib app");
            return;
        }
    };

    let game_manifest = renderer_raylib::GameManifest {
        name: manifest.name.clone(),
        version: manifest.version.clone(),
        entry_logic: manifest.entry_logic.clone(),
        width: manifest.width.unwrap_or(800),
        height: manifest.height.unwrap_or(600),
        target_fps: manifest.target_fps.unwrap_or(60),
    };

    if let Err(e) = renderer_raylib::spawn_app(
        page_id,
        app_name,
        butler,
        scribe_ref,
        game_manifest,
        lua_code,
    ) {
        tracing::error!(error = %e, "Failed to spawn Raylib app");
    }
}

/// Setup profiling tools (tokio-console and/or tracing-flame)
///
/// **Modes** (based on environment variables):
/// - `FLAME_OUTPUT` + `TOKIO_CONSOLE_PORT` → both layers
/// - `FLAME_OUTPUT` only → flame layer only (no console overhead)
/// - `TOKIO_CONSOLE_PORT` only → console layer only
///
/// **Environment variables**:
/// - `TOKIO_CONSOLE_PORT`: Port for tokio-console (enables console layer)
/// - `FLAME_OUTPUT`: Path for flame graph output file (enables flame layer)
#[cfg(feature = "profiling")]
fn setup_profiling() -> Option<tracing_flame::FlushGuard<std::io::BufWriter<File>>> {
    use tracing_subscriber::prelude::*;

    let console_port: Option<u16> = std::env::var("TOKIO_CONSOLE_PORT")
        .ok()
        .and_then(|p| p.parse().ok());
    let flame_path = std::env::var("FLAME_OUTPUT").ok();

    match (&flame_path, console_port) {
        // Both flame + console
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
                .with(tracing_subscriber::fmt::layer().with_target(true).with_level(true))
                .init();

            tracing::info!(port = port, path = %path, "Profiling: tokio-console + flame");
            guard
        }
        // Flame only (no console overhead)
        // Filter out tokio runtime TRACE spans (normally consumed by console-subscriber)
        (Some(path), None) => {
            let (flame_layer, guard) = match tracing_flame::FlameLayer::with_file(path) {
                Ok((layer, guard)) => (Some(layer), Some(guard)),
                Err(e) => {
                    eprintln!("Failed to create flame layer: {}", e);
                    (None, None)
                }
            };

            let filter = tracing_subscriber::EnvFilter::new(
                "info,tokio=off,runtime=off",
            );

            tracing_subscriber::registry()
                .with(filter)
                .with(flame_layer)
                .with(tracing_subscriber::fmt::layer().with_target(true).with_level(true))
                .init();

            tracing::info!(path = %path, "Profiling: flame only (no console overhead)");
            guard
        }
        // Console only
        (None, Some(port)) => {
            let console_layer = console_subscriber::ConsoleLayer::builder()
                .server_addr(([127, 0, 0, 1], port))
                .spawn();

            tracing_subscriber::registry()
                .with(console_layer)
                .with(tracing_subscriber::fmt::layer().with_target(true).with_level(true))
                .init();

            tracing::info!(port = port, "Profiling: tokio-console only");
            None
        }
        // Neither - just fmt logging
        (None, None) => {
            let filter = tracing_subscriber::EnvFilter::new(
                "info,tokio=off,runtime=off",
            );

            tracing_subscriber::registry()
                .with(filter)
                .with(tracing_subscriber::fmt::layer().with_target(true).with_level(true))
                .init();

            tracing::info!("Profiling feature enabled but no FLAME_OUTPUT or TOKIO_CONSOLE_PORT set");
            None
        }
    }
}
