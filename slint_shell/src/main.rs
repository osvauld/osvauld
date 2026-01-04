//! Sthalam Shell - Example runner
//!
//! This binary runs the Slint shell for testing and development.
//! Integrates with Butler for persistent authentication state.

use std::sync::Arc;
use std::rc::Rc;
use std::cell::RefCell;
use slint::ComponentHandle;
use slint_shell::Shell;
use butler::{Butler, RedbStore, LayerCache, ScribeMessage};
use tokio::sync::RwLock;
use ractor::ActorRef;

/// Space permit template (matches permissions.ts SPACE_TEMPLATE)
const SPACE_TEMPLATE: &str = r#"{
    "owner_template": {
        "operations": {
            "own": "allow",
            "get_share_link": "allow",
            "add_pages": "allow",
            "share_space": "allow"
        },
        "delegation": {
            "node": {
                "token_type": "space_share",
                "operations": {
                    "get_share_link": "allow",
                    "add_pages": "allow",
                    "share_space": "allow"
                },
                "auth_capabilities": {
                    "can_connect": true,
                    "persist_share": true,
                    "can_delegate": false,
                    "sync_enabled": true
                },
                "relationship": "node"
            },
            "viewer": {
                "token_type": "space_viewer",
                "operations": {
                    "request_pages": "allow",
                    "get_share_link": "allow"
                },
                "auth_capabilities": {
                    "can_connect": true,
                    "persist_share": false,
                    "can_delegate": false,
                    "sync_enabled": false
                },
                "relationship": "viewer"
            }
        }
    }
}"#;

/// Running app instance with parallel Lua worker
struct RunningApp {
    /// Slint runtime (main thread, owns UI and VecModels)
    slint_runtime: app_runtime::SlintRuntime,
    /// Lua worker thread handle
    #[allow(dead_code)]
    lua_thread: std::thread::JoinHandle<()>,
    /// Channel to send commands to Lua worker
    lua_tx: tokio::sync::mpsc::Sender<app_runtime::LuaWorkerCommand>,
    /// Receiver for Loro change events from Scribe
    loro_rx: tokio::sync::mpsc::Receiver<butler::LoroChangeEvent>,
}

/// Extract app files from Butler to temporary directory
///
/// **Context**: Slint compiler requires filesystem paths
/// **Returns**: TempDir path (caller must keep TempDir alive)
async fn extract_app_to_temp(
    butler: &Butler,
    page_id: &str,
) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    use std::fs;

    // Create temporary directory
    let temp_dir = tempfile::tempdir()?;

    // Get list of all files in this page
    let file_paths = butler.list_page_files(page_id)?;

    tracing::debug!(
        page_id = %page_id,
        file_count = file_paths.len(),
        "Extracting app files from Butler to temp directory"
    );

    // Extract each file
    for file_path in file_paths {
        // Get file content from Butler
        let content = butler
            .get_page_file(page_id, &file_path)
            .await?
            .ok_or_else(|| format!("Missing file: {}", file_path))?;

        // Write to temp directory
        let full_path = temp_dir.path().join(&file_path);

        // Create parent directories if needed
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(&full_path, &content)?;

        tracing::trace!(
            file_path = %file_path,
            size = content.len(),
            "Extracted file to temp directory"
        );
    }

    tracing::info!(
        page_id = %page_id,
        temp_dir = %temp_dir.path().display(),
        "Successfully extracted app files from Butler"
    );

    // Return path and prevent auto-cleanup
    let path = temp_dir.path().to_path_buf();
    let _ = temp_dir.keep(); // Prevent auto-cleanup
    Ok(path)
}

fn main() {
    // Initialize tracing for logs from Butler, Scribe, and app_runtime
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,butler=debug".into())
        )
        .init();

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

    let db_path = data_dir.join("shell.db");
    println!("Using database: {:?}", db_path);

    let store = Arc::new(
        RedbStore::open(&db_path).expect("Failed to open database")
    );

    // Initialize LayerCache
    let layer_cache = Arc::new(RwLock::new(LayerCache::new(store.clone(), 100)));

    // Create Butler instance
    let butler = Arc::new(Butler::new(store.clone(), layer_cache));

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
        // User exists, show login screen
        shell.set_current_screen("login".into());
    } else {
        // Fresh install, show initiation screen
        shell.set_current_screen("initiation".into());
    }

    // Clone shell handle for use in callbacks
    let shell_weak = shell.as_weak();

    // Wire up callbacks
    shell.on_import_key(|| {
        println!("Import key clicked");
        // TODO: Open file dialog for key import
    });

    // Login callback - verify passphrase with Butler
    let shell_weak_login = shell_weak.clone();
    let butler_login = butler.clone();
    shell.on_login(move |passphrase| {
        println!("Attempting login...");

        // Try to login with Butler (blocking call)
        match butler_login.login_sync(&passphrase) {
            Ok(identity) => {
                println!("Login successful! DID: {}", identity.did());

                // Store identity in Butler for future operations (blocking)
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(butler_login.set_identity(identity.clone()));

                if let Some(shell) = shell_weak_login.upgrade() {
                    shell.set_authenticated(true);
                    shell.set_current_screen("spaces".into()); // Go to spaces dashboard
                    // SpacesDashboard will call request-spaces on init
                }
            }
            Err(e) => {
                println!("Login failed: {}", e);
                // TODO: Set error state on shell to show error message
            }
        }
    });

    // Sign up callback - create account with Butler
    let shell_weak_signup = shell_weak.clone();
    let butler_signup = butler.clone();
    shell.on_sign_up(move |username, password| {
        println!("Sign up: username={}", username);

        // Create account with Butler (blocking call)
        match butler_signup.signup_sync(&username, &password) {
            Ok(result) => {
                println!("Signup successful!");
                println!("IMPORTANT: Save this seed phrase: {}", result.mnemonic);

                if let Some(shell) = shell_weak_signup.upgrade() {
                    shell.set_initialized(true);
                    shell.set_current_screen("login".into()); // Go to login after signup
                }
            }
            Err(e) => {
                println!("Signup failed: {}", e);
                // TODO: Set error state on shell to show error message
            }
        }
    });

    shell.on_tab_selected(|tab_id| {
        println!("Tab selected: {}", tab_id);
    });

    shell.on_tab_closed(|tab_id| {
        println!("Tab closed: {}", tab_id);
    });

    shell.on_new_tab(|| {
        println!("New tab clicked");
    });

    // Space management callbacks

    // Request spaces - called by UI when SpacesDashboard is shown
    let shell_weak_request_spaces = shell_weak.clone();
    let butler_request_spaces = butler.clone();
    shell.on_request_spaces(move || {
        println!("Requesting spaces...");

        let spaces = butler_request_spaces.list_spaces().unwrap_or_default();
        println!("Found {} spaces", spaces.len());

        if let Some(shell) = shell_weak_request_spaces.upgrade() {
            let space_infos: Vec<slint_shell::SpaceInfo> = spaces
                .iter()
                .map(|s| slint_shell::SpaceInfo {
                    id: s.id.clone().into(),
                    name: s.name.clone().into(),
                    app_count: 0, // TODO: count pages/apps
                })
                .collect();
            shell.set_spaces(slint::ModelRc::new(slint::VecModel::from(space_infos)));
        }
    });

    // Create space
    let shell_weak_create_space = shell_weak.clone();
    let butler_create_space = butler.clone();
    shell.on_create_space(move |name| {
        println!("Creating space: {}", name);

        // Create space using Butler (blocking in callback is OK for now)
        let rt = tokio::runtime::Runtime::new().unwrap();
        let butler_inner = butler_create_space.clone();
        let name_str = name.to_string();

        let result = rt.block_on(async {
            // Get user DID
            let user_info = butler_inner.user_info().await?;

            // Create space with proper permit template
            butler_inner.create_space(name_str, user_info.did, SPACE_TEMPLATE).await
        });

        match result {
            Ok(space) => {
                println!("Space created: {} ({})", space.name, space.id);

                // Refresh spaces list
                if let Some(shell) = shell_weak_create_space.upgrade() {
                    let spaces = butler_create_space.list_spaces().unwrap_or_default();
                    let space_infos: Vec<slint_shell::SpaceInfo> = spaces
                        .iter()
                        .map(|s| slint_shell::SpaceInfo {
                            id: s.id.clone().into(),
                            name: s.name.clone().into(),
                            app_count: 0, // TODO: count apps
                        })
                        .collect();
                    shell.set_spaces(slint::ModelRc::new(slint::VecModel::from(space_infos)));
                }
            }
            Err(e) => {
                println!("Failed to create space: {}", e);
            }
        }
    });

    let shell_weak_select_space = shell_weak.clone();
    let butler_select_space = butler.clone();
    shell.on_select_space(move |space_id| {
        println!("Selected space: {}", space_id);

        // Get space details and set current space name
        if let Some(shell) = shell_weak_select_space.upgrade() {
            if let Ok(Some(space)) = butler_select_space.get_space(&space_id) {
                shell.set_current_space_name(space.name.into());
            }
        }
    });

    shell.on_delete_space(|space_id| {
        println!("Delete space: {}", space_id);
        // TODO: Implement space deletion
    });

    // Request apps for a space - called by UI when SpaceView is shown
    let shell_weak_request_apps = shell_weak.clone();
    let butler_request_apps = butler.clone();
    shell.on_request_apps(move |space_id| {
        println!("Requesting apps for space: {}", space_id);

        // List pages in this space (pages are our "apps")
        let pages = butler_request_apps.list_pages(&space_id).unwrap_or_default();
        println!("Found {} apps/pages", pages.len());

        if let Some(shell) = shell_weak_request_apps.upgrade() {
            let app_infos: Vec<slint_shell::AppInfo> = pages
                .iter()
                .map(|p| slint_shell::AppInfo {
                    id: p.id.clone().into(),
                    name: p.name.clone().into(),
                    space_id: space_id.clone(),
                })
                .collect();
            shell.set_apps(slint::ModelRc::new(slint::VecModel::from(app_infos)));
        }
    });

    // Channel to send loaded app data from tokio to Slint thread
    let (app_ready_tx, app_ready_rx) = std::sync::mpsc::channel::<(
        String,  // page_id
        std::path::PathBuf,  // temp_dir path
        ActorRef<ScribeMessage>,
    )>();

    // Store running apps on Slint thread (not Send, uses Rc<RefCell>)
    let running_apps: Rc<RefCell<Vec<RunningApp>>> = Rc::new(RefCell::new(vec![]));

    let butler_select_app = butler.clone();
    let tokio_handle_select = tokio_handle.clone();
    shell.on_select_app(move |app_id| {
        println!("Loading app: {}", app_id);

        let butler = butler_select_app.clone();
        let app_id_str = app_id.to_string();
        let handle = tokio_handle_select.clone();
        let tx = app_ready_tx.clone();

        // Spawn on tokio runtime for butler/ractor operations
        handle.spawn(async move {
            // Open page to get ScribeRef for data sync
            let scribe_ref = match butler.open_page(&app_id_str).await {
                Ok(scribe) => {
                    println!("Page opened, scribe ready for sync");
                    scribe
                }
                Err(e) => {
                    println!("Failed to open page: {}", e);
                    return;
                }
            };

            // Extract app files to temp directory
            let temp_dir = match extract_app_to_temp(&butler, &app_id_str).await {
                Ok(dir) => dir,
                Err(e) => {
                    println!("Failed to extract app: {}", e);
                    return;
                }
            };

            // Send data to Slint thread to create runtime
            if tx.send((app_id_str, temp_dir, scribe_ref)).is_err() {
                println!("Failed to send app data to Slint thread");
            }

            // Signal Slint to process
            slint::invoke_from_event_loop(|| {}).ok();
        });
    });

    // Timer to check for ready apps and create runtimes on Slint thread
    let running_apps_timer = running_apps.clone();
    let timer = slint::Timer::default();
    timer.start(slint::TimerMode::Repeated, std::time::Duration::from_millis(100), move || {
        // Handle ready apps (new apps to load)
        while let Ok((page_id, temp_dir, scribe_ref)) = app_ready_rx.try_recv() {
            println!("Creating parallel app runtime for page: {}", page_id);

            // Load Lua code from temp directory
            let lua_path = temp_dir.join("app.lua");
            let lua_code = match std::fs::read_to_string(&lua_path) {
                Ok(code) => code,
                Err(e) => {
                    println!("Failed to read Lua code: {}", e);
                    continue;
                }
            };

            // Load Slint UI path
            let slint_path = temp_dir.join("app.slint");

            // Create channels
            let (ui_tx, ui_rx) = tokio::sync::mpsc::channel::<app_runtime::UiMutation>(32);
            let (loro_tx, loro_rx) = tokio::sync::mpsc::channel::<butler::LoroChangeEvent>(32);

            println!("SlintRuntime: spawning Lua worker thread first...");

            // Spawn Lua worker thread FIRST (OS thread, owns Lua VM)
            // This creates the command channel internally and returns the sender
            let (lua_thread, lua_tx) = match app_runtime::LuaWorker::spawn(
                page_id.clone(),
                lua_code,
                scribe_ref.clone(),
                ui_tx,
            ) {
                Ok((thread, tx)) => (thread, tx),
                Err(e) => {
                    println!("Failed to spawn Lua worker: {}", e);
                    continue;
                }
            };

            println!("Lua worker thread spawned, creating SlintRuntime...");

            // Create SlintRuntime on main thread
            let slint_runtime = match app_runtime::SlintRuntime::load(
                slint_path,
                page_id.clone(),
                ui_rx,
                lua_tx.clone(),
            ) {
                Ok(runtime) => runtime,
                Err(e) => {
                    println!("Failed to create SlintRuntime: {}", e);
                    continue;
                }
            };

            // Show window
            if let Err(e) = slint_runtime.slint_instance().show() {
                println!("Failed to show app window: {:?}", e);
                continue;
            }

            // Setup UI callbacks (button clicks → Lua)
            if let Err(e) = slint_runtime.setup_callbacks() {
                println!("Failed to setup callbacks: {}", e);
                continue;
            }

            println!("Lua worker thread spawned successfully");

            // Subscribe to Loro changes for the "messages" layer
            // TODO: Make this configurable based on app manifest
            if let Err(e) = scribe_ref.cast(ScribeMessage::SubscribeToLoroChanges {
                layer_name: "messages".to_string(),
                event_tx: loro_tx.clone(),
            }) {
                println!("Failed to subscribe to Loro changes: {}", e);
            } else {
                println!("Subscribed to Loro changes for 'messages' layer");
            }

            // Also subscribe to ui_state layer for persistent UI state
            if let Err(e) = scribe_ref.cast(ScribeMessage::SubscribeToLoroChanges {
                layer_name: "ui_state".to_string(),
                event_tx: loro_tx.clone(),
            }) {
                println!("Failed to subscribe to ui_state changes: {}", e);
            } else {
                println!("Subscribed to Loro changes for 'ui_state' layer");
            }

            // Send initial load event to Lua worker
            // This triggers on_loro_change() to populate the UI with existing data
            let lua_tx_init = lua_tx.clone();
            println!("Sending initial load events to Lua worker...");

            // Send empty initial events to trigger UI population
            // The Lua worker will query Loro directly to get current state
            let _ = lua_tx_init.try_send(app_runtime::LuaWorkerCommand::LoroChanged {
                layer_name: "messages".to_string(),
                full_data: serde_json::json!({}),
            });
            let _ = lua_tx_init.try_send(app_runtime::LuaWorkerCommand::LoroChanged {
                layer_name: "ui_state".to_string(),
                full_data: serde_json::json!({}),
            });

            // Store the running app
            running_apps_timer.borrow_mut().push(RunningApp {
                slint_runtime,
                lua_thread,
                lua_tx,
                loro_rx,
            });

            println!("App loaded successfully with parallel Lua worker!");
        }

        // Process updates for all running apps
        for running_app in running_apps_timer.borrow_mut().iter_mut() {
            // Forward Loro events to Lua thread
            while let Ok(event) = running_app.loro_rx.try_recv() {
                let _ = running_app.lua_tx.try_send(app_runtime::LuaWorkerCommand::LoroChanged {
                    layer_name: event.layer_name.clone(),
                    full_data: event.full_data.clone(),
                });
            }

            // Process UI mutations on Slint thread (from Lua worker)
            if let Err(e) = running_app.slint_runtime.process_ui_mutations() {
                println!("Failed to process UI mutations: {}", e);
            }
        }
    });

    shell.on_delete_app(|app_id| {
        println!("Delete app: {}", app_id);
        // TODO: Implement app deletion
    });

    // Upload app - opens folder picker, reads all files, stores in Butler
    let shell_weak_upload = shell_weak.clone();
    let butler_upload = butler.clone();
    shell.on_upload_app_clicked(move || {
        println!("Upload app clicked - opening folder picker...");

        // Open folder picker
        let folder = rfd::FileDialog::new()
            .set_title("Select App Folder")
            .pick_folder();

        if let Some(folder_path) = folder {
            println!("Selected folder: {:?}", folder_path);

            // Get current space ID from shell
            let current_space_id = if let Some(shell) = shell_weak_upload.upgrade() {
                shell.get_current_space_id().to_string()
            } else {
                println!("Failed to get current space ID");
                return;
            };

            if current_space_id.is_empty() {
                println!("No space selected");
                return;
            }

            // Import app using Butler's import service (handles all file reading, page creation, etc.)
            let rt = tokio::runtime::Runtime::new().unwrap();
            let butler_inner = butler_upload.clone();

            let page_result = rt.block_on(async {
                butler_inner.import_app(&current_space_id, &folder_path).await
            });

            let page = match page_result {
                Ok(p) => p,
                Err(e) => {
                    println!("Failed to import app: {}", e);
                    return;
                }
            };

            println!("App '{}' uploaded successfully!", page.name);

            // Refresh apps list
            if let Some(shell) = shell_weak_upload.upgrade() {
                let pages = butler_upload.list_pages(&current_space_id).unwrap_or_default();
                let app_infos: Vec<slint_shell::AppInfo> = pages
                    .iter()
                    .map(|p| slint_shell::AppInfo {
                        id: p.id.clone().into(),
                        name: p.name.clone().into(),
                        space_id: current_space_id.clone().into(),
                    })
                    .collect();
                shell.set_apps(slint::ModelRc::new(slint::VecModel::from(app_infos)));
            }
        }
    });

    // Run the shell
    shell.run().expect("Failed to run shell");
}
