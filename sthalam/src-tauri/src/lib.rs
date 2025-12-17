//! Sthalam - Peer-to-peer collaborative website and document publisher
//!
//! Uses the new architecture:
//! - Butler for storage (redb) and identity
//! - Herald for identity (via Butler)
//! - Transport + Courier for P2P
//! - Gurkha for Permit permissions (stateless functions)

use std::path::PathBuf;
use std::sync::Arc;
use tracing::{debug, error, info, warn};
use tauri::{AppHandle, Manager};
pub mod asset_protocol;
mod types;

// HUML plugin for native Vello rendering
use tauri_huml_plugin::AppHandleExt;
use huml_renderer::ParsedTemplate;
use cel_runtime::CelEvaluator;
use huml_parser::HumlParser;
use serde::Deserialize;

// Import shared handlers from tauri_handlers crate
use tauri_handlers::handlers::auth::*;
use tauri_handlers::handlers::p2p::*;
use tauri_handlers::handlers::user::*;
use tauri_handlers::handlers::node::*;
use tauri_handlers::handlers::page::*;
use tauri_handlers::handlers::space::*;
use tauri_handlers::handlers::window::*;
use tauri_handlers::P2PState;

use butler::{Butler, RedbStore, LayerCache};
use query_bridge;

use clap::Parser;
use std::fs;
use tokio::sync::RwLock;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long, default_value = "desktop")]
    db_name: String,
}

// ============================================================================
// HUML Commands
// ============================================================================

/// Input for opening a HUML window with template content.
#[derive(Debug, Deserialize)]
pub struct OpenHumlInput {
    pub huml_content: String,
    pub title: Option<String>,
    pub initial_values: Option<serde_json::Value>,
    /// Optional page_id for Scribe persistence. If provided, messages persist to DB.
    pub page_id: Option<String>,
}

/// Find the HUML parser executable in known locations.
fn find_huml_parser() -> Option<PathBuf> {
    let paths = [
        // Development paths
        "sthalam/template-transpiler/_build/default/parser-bin/huml_native.exe",
        "../template-transpiler/_build/default/parser-bin/huml_native.exe",
        "_build/default/parser-bin/huml_native.exe",
    ];

    for path in paths {
        let path = PathBuf::from(path);
        if path.exists() {
            debug!(path = %path.display(), "Found HUML parser");
            return Some(path);
        }
    }

    // Check environment variable
    if let Ok(path) = std::env::var("HUML_PARSER") {
        let path = PathBuf::from(path);
        if path.exists() {
            return Some(path);
        }
    }

    None
}

/// Find the CEL evaluator executable in known locations.
fn find_cel_evaluator() -> Option<PathBuf> {
    let paths = [
        // Development paths
        "sthalam/template-transpiler/_build/default/eval-bin/cel_wasi.exe",
        "../template-transpiler/_build/default/eval-bin/cel_wasi.exe",
        "_build/default/eval-bin/cel_wasi.exe",
    ];

    for path in paths {
        let path = PathBuf::from(path);
        if path.exists() {
            debug!(path = %path.display(), "Found CEL evaluator");
            return Some(path);
        }
    }

    // Check environment variable
    if let Ok(path) = std::env::var("CEL_EXECUTABLE") {
        let path = PathBuf::from(path);
        if path.exists() {
            return Some(path);
        }
    }

    None
}

/// Open a HUML preview window with the given template content.
///
/// Parses the HUML content using the native OCaml parser and opens
/// a Vello-rendered window.
///
/// **Persistence**: If `page_id` is provided, connects to Scribe for data persistence.
/// Messages will be saved to the database and reloaded on next open.
#[tauri::command]
async fn open_huml_with_template(
    app: AppHandle,
    input: OpenHumlInput,
) -> Result<String, String> {
    info!(
        content_len = input.huml_content.len(),
        page_id = ?input.page_id,
        "Opening HUML window"
    );

    // Find parser executable
    let parser_path = find_huml_parser()
        .ok_or_else(|| "HUML parser not found. Build it with: cd template-transpiler && dune build".to_string())?;

    // Find CEL evaluator
    let cel_path = find_cel_evaluator()
        .ok_or_else(|| "CEL evaluator not found. Build it with: cd template-transpiler && dune build".to_string())?;

    // Parse HUML content
    let parser = HumlParser::new(parser_path)
        .map_err(|e| format!("Failed to create HUML parser: {}", e))?;

    let template: ParsedTemplate = parser
        .parse_as(&input.huml_content)
        .map_err(|e| format!("Failed to parse HUML: {}", e))?;

    info!(
        name = %template.name,
        screens = template.ui.publisher.len(),
        "Parsed HUML template"
    );

    // Create CEL evaluator
    let cel = Arc::new(
        CelEvaluator::new(cel_path)
            .map_err(|e| format!("Failed to create CEL evaluator: {}", e))?
    );

    // Generate window label
    let label = format!("huml_preview_{}", std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis());

    let title = input.title.unwrap_or_else(|| template.name.clone());

    // Set up QueryBridge and fetch initial data if page_id is provided
    let (query_bridge, initial_query_data) = if let Some(page_id) = input.page_id {
        info!(page_id = %page_id, "Connecting to Scribe for persistence");

        // Get Butler from app state
        let butler = app.state::<Arc<Butler>>();

        // Open the page to get Scribe actor
        let scribe_ref = butler.open_page(&page_id).await
            .map_err(|e| format!("Failed to open page: {}", e))?;

        // Fetch initial data for each query defined in template
        let mut initial_data: std::collections::HashMap<String, Vec<serde_json::Value>> = std::collections::HashMap::new();

        for (query_name, query_def) in &template.queries {
            info!(query = %query_name, layer = %query_def.layer, path = %query_def.path, "Fetching initial query data");

            // Create query spec - convert sort_order string to enum
            let sort_order = query_def.sort_order.as_ref().map(|s| {
                match s.to_lowercase().as_str() {
                    "desc" => butler::models::SortOrder::Desc,
                    _ => butler::models::SortOrder::Asc,
                }
            });
            let spec = butler::models::QuerySpec {
                query_id: query_name.clone(),
                layer_name: query_def.layer.clone(),
                path: query_def.path.clone(),
                filter: query_def.filter.clone(),
                sort_by: query_def.sort_by.clone(),
                sort_order,
                offset: 0,
                limit: query_def.limit,
            };

            // Send query to Scribe
            let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
            scribe_ref.cast(butler::scribe::ScribeMessage::Query {
                spec,
                reply: reply_tx,
            }).map_err(|e| format!("Failed to send query to Scribe: {}", e))?;

            // Wait for result
            match reply_rx.await {
                Ok(Ok(result)) => {
                    info!(query = %query_name, items = result.items.len(), "Loaded initial data");
                    initial_data.insert(query_name.clone(), result.items);
                }
                Ok(Err(e)) => {
                    warn!(query = %query_name, error = %e, "Failed to fetch initial data");
                    initial_data.insert(query_name.clone(), Vec::new());
                }
                Err(e) => {
                    warn!(query = %query_name, error = %e, "Query channel closed");
                    initial_data.insert(query_name.clone(), Vec::new());
                }
            }
        }

        // Spawn the Scribe bridge (converts ScribeRequest → ScribeMessage)
        let (scribe_tx, delta_rx) = query_bridge::spawn_scribe_bridge(scribe_ref);

        // Spawn QueryBridge actor
        let (handle, _renderer_delta_rx) = query_bridge::spawn_query_bridge(scribe_tx, delta_rx);

        info!(page_id = %page_id, "QueryBridge connected to Scribe");
        (Some(handle), initial_data)
    } else {
        debug!("No page_id provided, running in local-only mode");
        (None, std::collections::HashMap::new())
    };

    // Create the HUML window with optional Scribe connection and initial data
    app.create_huml_window(&label, &title, 800, 600, template, cel, query_bridge, initial_query_data)
        .map_err(|e| format!("Failed to create HUML window: {}", e))?;

    info!(label = %label, "Created HUML window");
    Ok(label)
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

            // Initialize HUML plugin for native Vello-rendered windows
            // This enables creating HUML windows via AppHandleExt::create_huml_window
            tauri_huml_plugin::init(app.handle());

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
            // Space handlers (new terminology)
            handle_create_space,
            handle_list_spaces,
            handle_delete_space,
            handle_share_space,
            // Page handlers (new terminology)
            handle_create_page,
            handle_open_page,
            handle_close_page,
            handle_apply_update,
            handle_list_pages,
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
            // Window handlers (HUML)
            handle_open_huml_window,
            handle_close_huml_window,
            // HUML template preview
            open_huml_with_template,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
