//! Sthalam Shell - Example runner
//!
//! This binary runs the Slint shell for testing and development.
//! Integrates with Butler for persistent authentication state.

use std::path::PathBuf;
use std::sync::Arc;

use butler::{Butler, LayerCache, RedbStore};
use clap::Parser;
use courier::CourierHandle;
use slint::ComponentHandle;
use slint_shell::debug_logger::DebugLogLayer;
use slint_shell::debug_server::{DebugServer, LogEntry, UiCommand};
use slint_shell::Shell;
use tokio::sync::{broadcast, mpsc, RwLock};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

#[derive(Parser)]
#[command(name = "sthalam-shell")]
#[command(about = "Sthalam Shell - Sovereign browsing and hosting")]
struct Args {
    /// Database name (without .db extension)
    #[arg(short, long, default_value = "shell")]
    db_name: String,

    /// Enable debug server on specified Unix socket path
    /// If not specified, debug server is disabled
    #[arg(long)]
    debug_socket: Option<String>,
}

fn main() {
    // Initialize testing backend FIRST, before ANYTHING else
    // Check env var set by ai_interface (SLINT_BACKEND=testing)
    // This MUST be called before any Slint components are created or clap parses args
    let use_testing_backend = std::env::var("SLINT_BACKEND")
        .map(|v| v == "testing")
        .unwrap_or(false);

    if use_testing_backend {
        i_slint_backend_testing::init_integration_test_with_system_time();
    }

    let args = Args::parse();

    // Suppress Qt/Wayland text input warnings
    std::env::set_var("QT_LOGGING_RULES", "qt.qpa.wayland.textinput=false");

    // Initialize rich tracing
    // Check for OSVAULD_LOG_FORMAT=json to output JSON logs (for ai_interface)
    let use_json = std::env::var("OSVAULD_LOG_FORMAT")
        .map(|v| v == "json")
        .unwrap_or(false);

    let _log_guard = logging_utils::init_rich_tracing(logging_utils::LogConfig {
        level: "info".to_string(),
        log_to_stdout: true,
        use_tree_format: !use_json,
        stdout_json: use_json,
        ..Default::default()
    }).expect("Failed to initialize logging");

    println!("Sthalam Shell starting...");

    // Create tokio runtime for async butler/ractor operations
    // This runs in a background thread, Slint runs on main thread
    let tokio_rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed to create tokio runtime");
    let tokio_handle = tokio_rt.handle().clone();

    // Initialize Butler store
    // Check for STHALAM_DATA_DIR env var (used by ai_interface for test automation)
    let data_dir = std::env::var("STHALAM_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            dirs::data_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join("sthalam")
        });

    // Create data directory if it doesn't exist
    std::fs::create_dir_all(&data_dir).expect("Failed to create data directory");

    let db_path = data_dir.join(format!("{}.db", args.db_name));
    println!("Using database: {:?}", db_path);

    let store = Arc::new(RedbStore::open(&db_path).expect("Failed to open database"));

    // Initialize LayerCache
    let layer_cache = Arc::new(RwLock::new(LayerCache::new(store.clone(), 100)));

    // Create Butler instance
    let butler = Arc::new(Butler::new(store.clone(), layer_cache));

    // P2P state - initialized after login (matches Tauri pattern)
    // Transport requires device key from Butler identity
    let courier_handle: Arc<RwLock<Option<CourierHandle>>> = Arc::new(RwLock::new(None));

    // Start debug server if requested
    // Store UI command receiver to be set up after shell is created
    let mut ui_rx: Option<mpsc::Receiver<UiCommand>> = None;
    // Debug server reference for passing to app callbacks (for eval channel)
    let debug_server: Option<Arc<DebugServer>> = if let Some(ref socket_path) = args.debug_socket {
        let socket_path = PathBuf::from(socket_path);
        let instance_name = args.db_name.clone();

        // Create UI command channel
        let (ui_tx, rx) = mpsc::channel::<UiCommand>(100);
        ui_rx = Some(rx);

        // Create debug server and set UI channel + butler + courier
        let mut debug_server = DebugServer::new(socket_path.clone(), instance_name);
        debug_server.set_ui_channel(ui_tx);
        debug_server.set_butler(butler.clone());
        debug_server.set_courier_handle(courier_handle.clone());
        let debug_server = Arc::new(debug_server);

        // Spawn debug server in background
        let server = debug_server.clone();
        tokio_handle.spawn(async move {
            if let Err(e) = server.start().await {
                eprintln!("Debug server error: {}", e);
            }
        });

        println!("Debug server enabled on: {:?}", socket_path);
        Some(debug_server)
    } else {
        None
    };

    // Check signup status BEFORE creating shell (persistent state)
    let is_signed_up = butler.is_signed_up().unwrap_or(false);
    println!("Is signed up: {}", is_signed_up);

    // Create the shell window
    let shell = Shell::new().expect("Failed to create shell");

    // Set initial state based on Butler's persistent state
    shell.set_initialized(is_signed_up);
    shell.set_authenticated(false); // Always start logged out (need passphrase)

    // Navigate to appropriate screen
    if is_signed_up {
        shell.set_current_screen("login".into());
    } else {
        shell.set_current_screen("initiation".into());
    }

    // Register all callbacks via modules
    slint_shell::callbacks::auth::register(
        &shell,
        butler.clone(),
        courier_handle.clone(),
        tokio_handle.clone(),
    );

    slint_shell::callbacks::spaces::register(&shell, butler.clone());

    let _app_timer = slint_shell::callbacks::apps::register(
        &shell,
        butler.clone(),
        courier_handle.clone(),
        tokio_handle.clone(),
        debug_server.clone(),
    );

    slint_shell::callbacks::nodes::register(
        &shell,
        butler.clone(),
        courier_handle.clone(),
        tokio_handle.clone(),
    );

    slint_shell::callbacks::publish::register(
        &shell,
        butler.clone(),
        courier_handle.clone(),
        tokio_handle.clone(),
    );

    slint_shell::callbacks::viewer::register(
        &shell,
        butler.clone(),
        courier_handle.clone(),
        tokio_handle.clone(),
    );

    // Tab callbacks (too simple to extract)
    shell.on_tab_selected(|tab_id| {
        println!("Tab selected: {}", tab_id);
    });

    shell.on_tab_closed(|tab_id| {
        println!("Tab closed: {}", tab_id);
    });

    shell.on_new_tab(|| {
        println!("New tab clicked");
    });

    // Set up UI automation timer if debug server is enabled
    let _ui_timer = if let Some(mut ui_rx) = ui_rx {
        let shell_weak = shell.as_weak();
        let timer = slint::Timer::default();
        timer.start(
            slint::TimerMode::Repeated,
            std::time::Duration::from_millis(16), // ~60fps
            move || {
                if let Some(shell) = shell_weak.upgrade() {
                    slint_shell::ui_automation::process_ui_commands(&shell, &mut ui_rx);
                }
            },
        );
        Some(timer)
    } else {
        None
    };

    // Run the shell
    shell.run().expect("Failed to run shell");
}
