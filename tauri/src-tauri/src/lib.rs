use log::error;
use osvauld_db::{DbConnection, database::initialize_repositories, initialize_database};
use tauri::Manager;
pub mod current_note_state;
pub mod handlers;
pub mod listners;
mod types;
pub mod user_state;
use crate::handlers::auth_handler::{
    check_private_key_loaded, check_signup_status, get_user_details, handle_add_device,
    handle_change_passphrase, handle_export_certificate, handle_sign_up, login,
};
use crate::handlers::folder_handler::{
    handle_add_folder, handle_get_folders, handle_soft_delete_folder,
};
use crate::handlers::resource_handler::{
    emit_all_resources, handle_add_resource, handle_get_all_resources, handle_get_resource,
    handle_get_resources_for_folder, handle_share_resource, handle_toggle_fav,
    handle_update_last_accessed, handle_update_resource, soft_delete_resource,
};
use crate::handlers::user_handler::{get_system_locale, handle_add_user, handle_get_known_users};
use crate::user_state::UserState;
use clap::Parser;
use crypto_utils::CryptoUtils;
use p2p_service::P2PService;

use listners::EventManager;
use std::fs;
use std::sync::Arc;
use tokio::runtime::Runtime;
use tokio::sync::Mutex;
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
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(
            tauri_plugin_log::Builder::new()
                .filter(|metadata| {
                    !metadata.target().contains("tracing::span")
                        && !metadata.target().contains("tokio_tungstenite")
                        && !metadata.target().contains("tungstenite")
                        && !metadata.target().contains("iroh")
                        && !metadata.target().contains("hyper_util")
                        && !metadata.target().contains("netwatch")
                        && !metadata.target().contains("iroh_net_report")
                        && !metadata.target().contains("iroh_quinn_proto::connection")
                        && !metadata.target().contains("hickory_")
                        && !metadata.target().contains("iroh_relay")
                        && !metadata.target().contains("portmapper")
                        && !metadata.target().contains("igd_next::aio::tokio")
                        && !metadata.target().contains("igd_next::aio::tokio")
                        && !metadata.target().contains("rustls::client")
                })
                .build(),
        )
        .setup(move |app| {
            let handle = app.handle();
            let app_dir = app.path().app_data_dir().unwrap();

            if !app_dir.exists() {
                if let Err(e) = fs::create_dir_all(&app_dir) {
                    error!("Failed to create app data directory: {}", e);
                    panic!("Cannot continue without app data directory");
                }
            }

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
                    let repo_ctx = initialize_repositories(connection.clone());

                    let crypto_utils = Arc::new(Mutex::new(CryptoUtils::new()));

                    let (p2p_service, p2p_receiver, p2p_sender, incoming_receiver) =
                        P2PService::new(repo_ctx.clone(), crypto_utils.clone());
                    let p2p_service_clone = p2p_service.clone();
                    let p2p_service = Arc::new(p2p_service);
                    rt.spawn(async move {
                        P2PService::start_processing_incoming_events(
                            p2p_service_clone,
                            incoming_receiver,
                        );
                    });
                    let user_state = UserState::new();
                    // Initialize event manager and start listening
                    let event_manager = EventManager::new(
                        handle.clone(),
                        p2p_receiver,
                        p2p_sender,
                        repo_ctx.clone(),
                        crypto_utils.clone(),
                    );
                    rt.spawn(async move {
                        event_manager.start_listening();
                    });

                    // Manage all services

                    app.manage(user_state);
                    app.manage(crypto_utils);
                    app.manage(p2p_service.clone());
                    app.manage(repo_ctx);
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
            get_system_locale,
            check_signup_status,
            handle_sign_up,
            check_private_key_loaded,
            login,
            handle_add_resource,
            handle_add_device,
            handle_export_certificate,
            handle_change_passphrase,
            handle_add_folder,
            handle_get_folders,
            handle_get_resources_for_folder,
            soft_delete_resource,
            handle_soft_delete_folder,
            handle_toggle_fav,
            handle_update_last_accessed,
            handle_get_all_resources,
            handle_update_resource,
            handle_get_resource,
            handle_add_user,
            handle_get_known_users,
            handle_share_resource,
            get_user_details,
            emit_all_resources,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
