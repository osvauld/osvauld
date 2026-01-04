//! Sthalam - Peer-to-peer collaborative website and document publisher
//!
//! Uses the new architecture:
//! - Butler for storage (redb) and identity
//! - Herald for identity (via Butler)
//! - Transport + Courier for P2P
//! - Gurkha for Permit permissions (stateless functions)

use std::sync::Arc;
use tracing::{error, info};
use tauri::Manager;
pub mod asset_protocol;
mod types;

// Import shared handlers from tauri_handlers crate
use tauri_handlers::handlers::auth::*;
use tauri_handlers::handlers::p2p::*;
use tauri_handlers::handlers::user::*;
use tauri_handlers::handlers::page::*;
use tauri_handlers::handlers::space::*;
use tauri_handlers::P2PState;

use butler::{Butler, RedbStore, LayerCache};

use clap::Parser;
use std::fs;
use tokio::sync::RwLock;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long, default_value = "desktop")]
    db_name: String,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let args = Args::parse();
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init());

    #[cfg(any(target_os = "android", target_os = "ios"))]
    let builder = builder.plugin(tauri_plugin_barcode_scanner::init());

    builder
        .register_asynchronous_uri_scheme_protocol("asset", asset_protocol::handle_asset_request)
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_shell::init())
        .setup(move |app| {
            let app_dir = app.path().app_data_dir().unwrap();

            // Initialize rich tracing for backend logging
            #[cfg(debug_assertions)]
            let _guard = logging_utils::init_dev()
                .expect("Failed to initialize logging");

            #[cfg(not(debug_assertions))]
            let _guard = {
                let log_dir = app_dir.join("logs");
                logging_utils::init_prod(
                    log_dir.to_str().unwrap_or("./logs")
                ).expect("Failed to initialize logging")
            };

            info!("🚀 Sthalam desktop app starting");

            if !app_dir.exists() {
                if let Err(e) = fs::create_dir_all(&app_dir) {
                    error!("Failed to create app data directory: {}", e);
                    panic!("Cannot continue without app data directory");
                }
            }

            // Initialize Butler's RedbStore with db_name from CLI args
            let db_filename = format!("{}.db", args.db_name);
            let redb_path = app_dir.join(&db_filename);
            info!("Using database: {}", db_filename);
            let redb_store = match RedbStore::open(&redb_path) {
                Ok(store) => {
                    info!("Butler RedbStore initialized at {:?}", redb_path);
                    Arc::new(store)
                }
                Err(e) => {
                    error!("Failed to initialize RedbStore: {}", e);
                    panic!("Cannot continue without RedbStore");
                }
            };

            // Initialize LayerCache
            let layer_cache = Arc::new(RwLock::new(LayerCache::new(redb_store.clone(), 100)));

            // Create Butler (owns store, cache, and will hold identity after login)
            let butler = Arc::new(Butler::new(redb_store, layer_cache));

            // P2P state (initialized after login via start_p2p_listener)
            let p2p_state = P2PState::new();

            // Manage state - Butler is the single entry point for all storage/identity operations
            app.manage(butler);
            app.manage(p2p_state);

            // Initialize renderer plugin for native Vello-rendered Rune windows
            tauri_renderer_plugin::init(app.handle());

            #[cfg(debug_assertions)]
            {
                let window = app.get_webview_window("main").unwrap();
                window.open_devtools();
                window.close_devtools();
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Auth handlers
            check_signup_status,
            get_user_details,
            handle_sign_up,
            check_private_key_loaded,
            login,
            handle_add_device,
            handle_export_certificate,
            handle_change_passphrase,
            handle_logout,
            get_one_time_permit,
            // Space handlers
            handle_create_space,
            handle_list_spaces,
            handle_delete_space,
            handle_share_space,
            // Page handlers
            handle_create_page,
            handle_open_page,
            handle_close_page,
            handle_apply_update,
            handle_list_pages,
            handle_save_page_wasm,
            handle_open_page_preview,
            // User handlers
            handle_add_user,
            handle_get_known_users,
            handle_get_sovereign_nodes,
            get_system_locale,
            // P2P handlers
            start_p2p_listener,
            handle_add_sovereign_node,
            handle_connect_to_website,
            handle_publish_space,
            handle_get_share_link,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
