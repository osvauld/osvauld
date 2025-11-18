use tracing::{error, info};
use persistance::{DbConnection, database::initialize_repositories, initialize_database};
use tauri::Manager;
pub mod asset_protocol;
mod event_manager;
mod types;

// Import all shared handlers from tauri_handlers crate
use tauri_handlers::handlers::auth::*;
use tauri_handlers::handlers::folder::*;
use tauri_handlers::handlers::p2p::*;
use tauri_handlers::handlers::resource::*;
use tauri_handlers::handlers::user::*;
use tauri_handlers::UserState;
use clap::Parser;
use crypto_utils::CryptoUtils;
use network::P2PService;
use search_indexer::SearchIndexManager;

use std::fs;
use std::sync::Arc;
use tokio::runtime::Runtime;
use tokio::sync::{Mutex, RwLock};
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
        // Removed tauri_plugin_log - now using logging_utils with tracing-tree
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
            let search_manager = Arc::new(Mutex::new(SearchIndexManager::new(
                &app_dir,
                "main_doc".to_string(),
            )?));
            app.manage(search_manager);
            let db_path = app_dir
                .join(format!("{}.db", args.db_name))
                .to_str()
                .unwrap()
                .to_string();
            // Create a new Tokio runtime
            let rt = Arc::new(Runtime::new().expect("Failed to create Tokio runtime"));

            // Initialize database
            let rt_clone = Arc::clone(&rt);
            let db_connection: Result<DbConnection, String> = rt_clone.block_on(async {
                initialize_database(&db_path)
                    .await
                    .map_err(|e| format!("Failed to initialize database: {}", e))
            });

            match db_connection {
                Ok(connection) => {
                    app.manage(connection.clone());
                    let repo_ctx = Arc::new(initialize_repositories(connection.clone()));
                    let crypto_utils = Arc::new(RwLock::new(CryptoUtils::new()));
                    let ucan_service = Arc::new(RwLock::new(gurkha::UcanService::new()));
                    let (p2p_service, p2p_receiver) =
                        P2PService::new(repo_ctx.clone(), crypto_utils.clone(), ucan_service.clone());
                    let p2p_service = Arc::new(p2p_service);

                    // Create user state for handlers
                    let user_state = UserState::new();

                    // Create and start EventManager for bidirectional event communication
                    let event_manager = event_manager::EventManager::new(
                        app.handle().clone(),
                        p2p_service.clone(),
                        repo_ctx.clone(),
                        crypto_utils.clone(),
                        ucan_service.clone(),
                    );
                    event_manager.start(p2p_receiver, rt.handle());

                    app.manage(user_state);
                    app.manage(crypto_utils);
                    app.manage(ucan_service);
                    app.manage(p2p_service.clone());
                    app.manage(repo_ctx.clone());
                }
                Err(e) => {
                    error!("Failed to set up database: {}", e);
                    panic!("Cannot continue without database connection");
                }
            }

            // Manage the runtime
            app.manage(rt);

            #[cfg(debug_assertions)]
            {
                let window = app.get_webview_window("main").unwrap();
                window.open_devtools();
                window.close_devtools();
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Shared auth handlers
            check_signup_status,
            get_user_details,
            handle_sign_up,
            check_private_key_loaded,
            login,
            handle_add_device,
            handle_export_certificate,
            handle_change_passphrase,
            handle_logout,
            get_one_time_ucan_token,
            // Shared folder handlers
            handle_add_folder,
            handle_get_folders,
            handle_soft_delete_folder,
            handle_get_shared_folder_users,
            handle_share_folder,
            // Shared user handlers
            handle_add_user,
            handle_get_known_users,
            get_system_locale,
            // Shared resource handlers
            handle_add_resource,
            handle_get_resource,
            handle_get_all_resources_metadata,
            handle_update_resource,
            handle_sync_resource,
            // P2P handlers
            start_p2p_listener,
            handle_add_sovereign_node,
            handle_connect_to_website,
            // TODO: Implement remaining resource handlers:
            // handle_delete_resource,
            // etc.
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}