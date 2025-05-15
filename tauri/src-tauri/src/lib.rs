use log::error;
use osvauld_db::{DbConnection, initialize_database};
use tauri::Manager;
pub mod current_note_state;
pub mod handlers;
pub mod listners;
mod types;
pub mod user_state;
use crate::handlers::auth_handler::{
    check_private_key_loaded, check_signup_status, get_public_key, get_user_details, get_user_id,
    handle_add_device, handle_change_passphrase, handle_export_certificate, handle_hash_and_sign,
    handle_sign_challenge, handle_sign_up, login,
};
use crate::handlers::folder_handler::{handle_add_folder, handle_get_folders, soft_delete_folder};
use crate::handlers::p2p_handlers::{
    connect_with_device, get_system_locale, get_ticket, initiate_first_connection, send_message,
    start_p2p_listener,
};
use crate::handlers::resource_handler::{
    get_all_resources, get_resource, handle_add_resource, handle_get_resources_for_folder,
    share_resource, soft_delete_resource, toggle_fav, update_last_accessed, update_resource,
};
use crate::handlers::user_handler::{add_known_user, get_details_for_share, get_known_users};
use crate::user_state::UserState;
use crypto_utils::CryptoUtils;
use osvauld_db::repositories::{
    SqliteDeviceRepository, SqliteFolderRepository, SqliteResourceKeyRepository,
    SqliteResourceRepository, SqliteShareRepository, SqliteStoreRepository, SqliteSyncRepository,
    SqliteUserRepository, SqliteVectorClockRepository,
};
use osvauld_services::{
    AuthService, FolderService, ResourceService, SyncService, TransactionService, UserService,
};
use p2p_service::P2PService;
use rendezvous_client::rendezvous_service::RendezvousService;

use listners::EventManager;
use std::fs;
use std::sync::Arc;
use tokio::runtime::Runtime;
use tokio::sync::Mutex;
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
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
        .setup(|app| {
            let handle = app.handle();
            let app_dir = app.path().app_data_dir().unwrap();

            if !app_dir.exists() {
                if let Err(e) = fs::create_dir_all(&app_dir) {
                    error!("Failed to create app data directory: {}", e);
                    panic!("Cannot continue without app data directory");
                }
            }

            let db_path = app_dir.join("desktop.db").to_str().unwrap().to_string();

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
                    let resource_repo = Arc::new(SqliteResourceRepository::new(connection.clone()));
                    let device_repo = Arc::new(SqliteDeviceRepository::new(connection.clone()));
                    let share_repo = Arc::new(SqliteShareRepository::new(connection.clone()));
                    let resource_key_repo =
                        Arc::new(SqliteResourceKeyRepository::new(connection.clone()));
                    let store_repository = Arc::new(SqliteStoreRepository::new(connection.clone()));
                    let user_repository = Arc::new(SqliteUserRepository::new(connection.clone()));
                    let vector_clock_repo =
                        Arc::new(SqliteVectorClockRepository::new(connection.clone()));

                    let folder_service =
                        Arc::new(FolderService::new(folder_repo.clone(), device_repo.clone()));
                    let crypto_utils = Arc::new(Mutex::new(CryptoUtils::new()));
                    let auth_service = Arc::new(AuthService::new(
                        store_repository.clone(),
                        crypto_utils.clone(),
                        device_repo.clone(),
                    ));

                    let transaction_service = Arc::new(TransactionService::new(
                        resource_repo.clone(),
                        resource_key_repo.clone(),
                        sync_repo.clone(),
                        share_repo.clone(),
                        store_repository.clone(),
                        user_repository.clone(),
                        device_repo.clone(),
                        folder_repo.clone(),
                        vector_clock_repo.clone(),
                    ));

                    let resource_service = Arc::new(ResourceService::new(
                        resource_repo.clone(),
                        crypto_utils.clone(),
                        vector_clock_repo.clone(),
                        resource_key_repo.clone(),
                        device_repo.clone(),
                        user_repository.clone(),
                        share_repo.clone(),
                        sync_repo.clone(),
                    ));
                    let sync_service = Arc::new(SyncService::new(
                        sync_repo.clone(),
                        folder_repo.clone(),
                        resource_repo.clone(),
                        device_repo.clone(),
                        store_repository.clone(),
                        vector_clock_repo.clone(),
                        user_repository.clone(),
                        share_repo.clone(),
                        resource_service.clone(),
                        transaction_service.clone(),
                    ));

                    let user_service = Arc::new(UserService::new(
                        user_repository.clone(),
                        crypto_utils.clone(),
                        sync_repo.clone(),
                        device_repo.clone(),
                        vector_clock_repo.clone(),
                        share_repo.clone(),
                    ));
                    let (p2p_service, p2p_receiver, p2p_sender, incoming_receiver) =
                        P2PService::new(
                            sync_service.clone(),
                            auth_service.clone(),
                            user_service.clone(),
                        );
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
                    let rendezvous_service = Arc::new(RendezvousService::new(
                        p2p_service.clone(),
                        "ws://0.0.0.0:3030/ws",
                        user_service.clone(),
                    ));
                    let event_manager = EventManager::new(
                        handle.clone(),
                        p2p_receiver,
                        resource_service.clone(),
                        p2p_sender,
                        user_service.clone(),
                        rendezvous_service.clone(),
                    );
                    rt.spawn(async move {
                        event_manager.start_listening();
                    });

                    // Manage all services

                    app.manage(user_state);
                    app.manage(folder_service);
                    app.manage(auth_service);
                    app.manage(resource_service);
                    app.manage(sync_service);
                    app.manage(p2p_service.clone());
                    app.manage(user_service);
                    app.manage(transaction_service);
                    app.manage(rendezvous_service);
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
            handle_add_resource,
            handle_hash_and_sign,
            handle_add_device,
            handle_export_certificate,
            handle_change_passphrase,
            handle_add_folder,
            handle_get_folders,
            handle_get_resources_for_folder,
            send_message,
            get_ticket,
            connect_with_device,
            start_p2p_listener,
            soft_delete_resource,
            soft_delete_folder,
            toggle_fav,
            update_last_accessed,
            get_all_resources,
            get_user_id,
            update_resource,
            get_resource,
            add_known_user,
            get_known_users,
            get_public_key,
            initiate_first_connection,
            share_resource,
            get_details_for_share,
            get_user_details,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
