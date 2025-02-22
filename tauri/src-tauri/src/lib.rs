use log::{error, info};
use tauri::Manager;
pub mod application;
mod database;
pub mod domains;
pub mod persistence;
use database::{initialize_database, DbConnection};
pub mod handlers;
mod types;
use crate::application::services::AuthService;
use crate::application::services::CredentialService;
use crate::application::services::FolderService;
use crate::application::services::P2PService;
use crate::application::services::SyncService;
use crate::handlers::auth_handler::{
    check_private_key_loaded, check_signup_status, handle_add_device, handle_change_passphrase,
    handle_export_certificate, handle_hash_and_sign, handle_sign_challenge, handle_sign_up, login,
};
use crate::handlers::credential_handler::{
    get_all_credentials, handle_add_credential, handle_get_credentials_for_folder,
    soft_delete_credential, toggle_fav, update_last_accessed,
};
use crate::handlers::folder_handler::{handle_add_folder, handle_get_folders, soft_delete_folder};
use crate::handlers::p2p_handlers::{
    connect_with_ticket, get_system_locale, get_ticket, send_message, start_p2p_listener,
};
use crate::persistence::repositories::{
    SqliteCredentialRepository, SqliteDeviceRepository, SqliteFolderRepository,
    SqliteSyncRepository, TauriStoreRepository,
};
use crypto_utils::CryptoUtils;
use log::LevelFilter;
use std::fs;
use std::sync::Arc;
use tokio::runtime::Runtime;
use tokio::sync::Mutex;
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default();

    #[cfg(any(target_os = "android", target_os = "ios"))]
    let builder = builder.plugin(tauri_plugin_barcode_scanner::init());

    builder
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(
            tauri_plugin_log::Builder::new()
                .filter(|metadata| {
                    !metadata.target().contains("tracing::span")
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
        .plugin(tauri_plugin_store::Builder::default().build())
        .setup(|app| {
            let handle = app.handle();
            let app_dir = app.path().app_data_dir().unwrap();

            if !app_dir.exists() {
                if let Err(e) = fs::create_dir_all(&app_dir) {
                    error!("Failed to create app data directory: {}", e);
                    panic!("Cannot continue without app data directory");
                }
            }

            let db_path = app_dir.join("desktop100.db").to_str().unwrap().to_string();

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

                    let folder_repo = Arc::new(SqliteFolderRepository::new(connection.clone()));
                    let sync_repo = Arc::new(SqliteSyncRepository::new(connection.clone()));
                    let credential_repo =
                        Arc::new(SqliteCredentialRepository::new(connection.clone()));
                    let device_repo = Arc::new(SqliteDeviceRepository::new(connection.clone()));
                    // Initialize folder service with cloned repositories
                    let store_repository = Arc::new(TauriStoreRepository::new(handle.clone()));
                    let folder_service = Arc::new(FolderService::new(folder_repo.clone()));

                    let crypto_utils = Arc::new(Mutex::new(CryptoUtils::new()));
                    let auth_service = Arc::new(AuthService::new(
                        store_repository.clone(),
                        crypto_utils.clone(),
                        device_repo.clone(),
                        sync_repo.clone(),
                    ));
                    let sync_service = Arc::new(SyncService::new(
                        sync_repo.clone(),
                        folder_repo.clone(),
                        credential_repo.clone(),
                        device_repo,
                        store_repository,
                    ));
                    let p2p_service = Arc::new(P2PService::new(
                        handle.clone(),
                        sync_service.clone(),
                        auth_service.clone(),
                    ));
                    let credential_service = Arc::new(CredentialService::new(
                        credential_repo.clone(),
                        crypto_utils,
                    ));

                    // Manage all services
                    app.manage(folder_service);
                    app.manage(auth_service);
                    app.manage(credential_service);
                    app.manage(sync_service);
                    app.manage(p2p_service.clone());
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
            handle_sign_challenge,
            handle_add_credential,
            handle_hash_and_sign,
            handle_add_device,
            handle_export_certificate,
            handle_change_passphrase,
            handle_add_folder,
            handle_get_folders,
            handle_get_credentials_for_folder,
            send_message,
            get_ticket,
            connect_with_ticket,
            start_p2p_listener,
            soft_delete_credential,
            soft_delete_folder,
            toggle_fav,
            update_last_accessed,
            get_all_credentials,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
