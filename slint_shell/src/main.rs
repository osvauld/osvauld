//! Sthalam Shell - Example runner
//!
//! This binary runs the Slint shell for testing and development.
//! Integrates with Butler for persistent authentication state.

use std::sync::Arc;

use butler::{Butler, LayerCache, RedbStore};
use clap::Parser;
use courier::CourierHandle;
use slint::ComponentHandle;
use slint_shell::Shell;
use tokio::sync::RwLock;

#[derive(Parser)]
#[command(name = "sthalam-shell")]
#[command(about = "Sthalam Shell - Sovereign browsing and hosting")]
struct Args {
    /// Database name (without .db extension)
    #[arg(short, long, default_value = "shell")]
    db_name: String,
}

fn main() {
    let args = Args::parse();

    // Suppress Qt/Wayland text input warnings
    std::env::set_var("QT_LOGGING_RULES", "qt.qpa.wayland.textinput=false");

    // Initialize rich tracing (hierarchical tree format)
    let _log_guard = logging_utils::init_rich_tracing(logging_utils::LogConfig {
        level: "info".to_string(),
        log_to_stdout: true,
        use_tree_format: true,
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
    let data_dir = dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("sthalam");

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
        tokio_handle.clone(),
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

    // Run the shell
    shell.run().expect("Failed to run shell");
}
