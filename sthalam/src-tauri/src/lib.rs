use log::error;
use persistance::{DbConnection, database::initialize_repositories, initialize_database};
use tauri::Manager;
pub mod current_note_state;
pub mod handlers;
pub mod listners;
pub mod preview_generator;
mod types;
pub mod user_state;
pub mod website_state;
use crate::handlers::auth_handler::{
    check_private_key_loaded, check_signup_status, get_one_time_ucan_token, get_user_details,
    handle_add_device, handle_change_passphrase, handle_export_certificate, handle_logout,
    handle_sign_up, login,
};
use crate::handlers::folder_handler::{
    handle_add_folder, handle_get_folders, handle_get_shared_folder_users, handle_share_folder,
    handle_soft_delete_folder,
};
use crate::handlers::resource_handler::{
    emit_all_resources, handle_add_resource, handle_get_all_resources, handle_get_resource,
    handle_get_resources_for_folder, handle_publish_resource, handle_search_resources,
    handle_share_resource, handle_toggle_fav, handle_update_last_accessed, handle_update_resource,
    soft_delete_resource,
};
use crate::handlers::user_handler::{get_system_locale, handle_add_user, handle_get_known_users};
use crate::handlers::website_handler::{
    handle_connect_to_website, handle_folder_sync_viewer, handle_generate_share_token,
    handle_load_website_state, handle_sync_resource, handle_update_website_state,
};
use crate::user_state::UserState;
use crate::website_state::WebsiteState;
use clap::Parser;
use crypto_utils::CryptoUtils;
use network::P2PService;
use search_indexer::SearchIndexManager;

use listners::EventManager;
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
        .register_asynchronous_uri_scheme_protocol("asset", |app, request, responder| {
            use tauri::http::Response;

            let uri = request.uri();
            let path = uri.path();

            // Convert path to file path - remove leading slash
            let path_str = if path.starts_with('/') {
                path[1..].to_string()
            } else {
                path.to_string()
            };

            // Log every request
            log::info!("[Asset Protocol] Request for: {}", path_str);

            // Get app handle for resource resolution
            let app_handle = app.app_handle().clone();

            tauri::async_runtime::spawn(async move {
                // Determine file path based on mode
                let file_path = if cfg!(debug_assertions) {
                    // Dev mode: Navigate from src-tauri directory up to workspace root
                    let current_dir = std::env::current_dir().ok();

                    if let Some(src_tauri_dir) = current_dir {
                        // Go up one level from src-tauri to workspace root
                        let workspace = src_tauri_dir.parent().unwrap_or(&src_tauri_dir);
                        let dev_path = workspace.join("frontend/desktop/public").join(&path_str);
                        log::info!("[Asset Protocol] Dev mode - workspace: {}, trying path: {}",
                            workspace.display(), dev_path.display());
                        Some(dev_path)
                    } else {
                        log::error!("[Asset Protocol] Could not determine current directory");
                        None
                    }
                } else {
                    // Production: serve from Tauri's resource directory
                    let prod_path = app_handle.path().resource_dir()
                        .ok()
                        .map(|dir| dir.join(&path_str));

                    if let Some(ref p) = prod_path {
                        log::info!("[Asset Protocol] Production mode - trying path: {}", p.display());
                    }
                    prod_path
                };

                if let Some(file_path) = file_path {
                    match std::fs::read(&file_path) {
                        Ok(content) => {
                            let mime_type = if path_str.ends_with(".wasm") {
                                "application/wasm".to_string()
                            } else {
                                mime_guess::from_path(&file_path).first_or_octet_stream().to_string()
                            };

                            log::info!("[Asset Protocol] ✓ Successfully serving {} ({} bytes) with MIME: {}",
                                path_str, content.len(), mime_type);

                            responder.respond(
                                Response::builder()
                                    .status(200)
                                    .header("Content-Type", mime_type)
                                    .header("Access-Control-Allow-Origin", "*")
                                    .header("Access-Control-Allow-Methods", "GET, OPTIONS")
                                    .header("Access-Control-Allow-Headers", "*")
                                    .body(content)
                                    .unwrap()
                            );
                        }
                        Err(e) => {
                            log::error!("[Asset Protocol] ✗ File not found: {} - {}", file_path.display(), e);
                            responder.respond(
                                Response::builder()
                                    .status(404)
                                    .header("Access-Control-Allow-Origin", "*")
                                    .header("Access-Control-Allow-Methods", "GET, OPTIONS")
                                    .header("Access-Control-Allow-Headers", "*")
                                    .body(Vec::new())
                                    .unwrap()
                            );
                        }
                    }
                } else {
                    log::error!("[Asset Protocol] ✗ Could not resolve file path for: {}", path_str);
                    responder.respond(
                        Response::builder()
                            .status(404)
                            .header("Access-Control-Allow-Origin", "*")
                            .header("Access-Control-Allow-Methods", "GET, OPTIONS")
                            .header("Access-Control-Allow-Headers", "*")
                            .body(Vec::new())
                            .unwrap()
                    );
                }
            });
        })
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
                    let domain = Arc::new("sthalam".to_string());
                    let (p2p_service, p2p_receiver, p2p_sender, incoming_receiver) =
                        P2PService::new(repo_ctx.clone(), crypto_utils.clone(), domain);
                    let p2p_service_clone = p2p_service.clone();
                    let p2p_service = Arc::new(p2p_service);
                    rt.spawn(async move {
                        P2PService::start_processing_incoming_events(
                            p2p_service_clone,
                            incoming_receiver,
                        );
                    });
                    let user_state = UserState::new();

                    // Initialize WebsiteState
                    let website_state = Arc::new(RwLock::new(WebsiteState::new(repo_ctx.clone())));

                    // Initialize event manager and start listening
                    let event_manager = EventManager::new(
                        handle.clone(),
                        p2p_receiver,
                        p2p_sender,
                        repo_ctx.clone(),
                        crypto_utils.clone(),
                    );
                    rt.spawn(async move {
                        event_manager.start_listening().await;
                    });
                    app.manage(user_state);
                    app.manage(crypto_utils);
                    app.manage(p2p_service.clone());
                    app.manage(repo_ctx.clone());
                    app.manage(website_state);
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
            handle_publish_resource,
            get_user_details,
            emit_all_resources,
            get_one_time_ucan_token,
            handle_logout,
            handle_search_resources,
            handle_share_folder,
            handle_get_shared_folder_users,
            handle_generate_share_token,
            handle_update_website_state,
            handle_load_website_state,
            handle_connect_to_website,
            handle_sync_resource,
            handle_folder_sync_viewer,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}