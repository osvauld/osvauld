//! App management callbacks
//!
//! Handles page/app loading, upload, and runtime management.
//! Flow: Spaces → Pages → Apps

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use butler::{Butler, ScribeMessage};
use courier::CourierHandle;
use ractor::ActorRef;
use slint::ComponentHandle;

use tokio::sync::RwLock;

use crate::app_runner::{prepare_page, PreparedPage, RunningApp};
use crate::control_server::{AppStatus, ControlServer};
use crate::Shell;

/// Request to refresh an app from filesystem via Scribe
pub struct RefreshAppRequest {
    pub app_name: String,
    pub app_dir: PathBuf,
}

/// Register app management callbacks on the shell
///
/// Returns the timer that must be kept alive for the app runtime loop.
pub fn register(
    shell: &Shell,
    butler: Arc<Butler>,
    courier_handle: Arc<RwLock<Option<CourierHandle>>>,
    tokio_handle: tokio::runtime::Handle,
    debug_server: Option<Arc<ControlServer>>,
) -> slint::Timer {
    // Page-level callbacks (SpaceView shows pages)
    register_request_pages(shell, butler.clone());
    register_select_page(shell, butler.clone());
    register_upload_page(shell, butler.clone());
    register_reload_page(shell, butler.clone());

    // App-level callbacks (PageView shows apps)
    register_request_page_apps(shell, butler.clone());
    register_delete_app(shell);

    // Setup app loading and runtime timer (returns refresh_tx for refresh_app callback)
    let (timer, refresh_tx) = setup_app_runtime(shell, butler, courier_handle, tokio_handle, debug_server);

    // Register refresh_app callback (needs refresh_tx from timer setup)
    register_refresh_app(shell, refresh_tx);

    timer
}

/// Register request_pages callback - loads pages for a space
fn register_request_pages(shell: &Shell, butler: Arc<Butler>) {
    let shell_weak = shell.as_weak();
    shell.on_request_pages(move |space_id| {
        println!("Requesting pages for space: {}", space_id);

        let pages = butler.list_pages(&space_id).unwrap_or_default();
        println!("Found {} pages", pages.len());

        if let Some(shell) = shell_weak.upgrade() {
            let page_infos: Vec<crate::PageInfo> = pages
                .iter()
                .map(|p| {
                    // Count apps in this page
                    let app_count = butler.list_apps(&p.id).map(|a| a.len() as i32).unwrap_or(0);
                    crate::PageInfo {
                        id: p.id.clone().into(),
                        name: p.name.clone().into(),
                        app_count,
                    }
                })
                .collect();
            shell.set_pages(slint::ModelRc::new(slint::VecModel::from(page_infos)));
        }
    });
}

/// Register select_page callback - navigates to page view
fn register_select_page(shell: &Shell, butler: Arc<Butler>) {
    let shell_weak = shell.as_weak();
    shell.on_select_page(move |page_id| {
        println!("Selecting page: {}", page_id);

        if let Some(shell) = shell_weak.upgrade() {
            // Get page info to set the name
            if let Ok(Some(page)) = butler.get_page(&page_id) {
                shell.set_current_page_name(page.meta.name.into());
            }

            // Load apps for this page (list_apps returns Vec<String> of app names)
            let apps = butler.list_apps(&page_id).unwrap_or_default();
            println!("Found {} apps in page {}", apps.len(), page_id);

            let app_infos: Vec<crate::AppInfo> = apps
                .iter()
                .map(|app_name| crate::AppInfo {
                    id: app_name.clone().into(),
                    name: app_name.clone().into(),
                    page_id: page_id.clone(),
                })
                .collect();
            shell.set_page_apps(slint::ModelRc::new(slint::VecModel::from(app_infos)));
        }
    });
}

/// Register upload_page callback - opens folder picker to upload page directory
fn register_upload_page(shell: &Shell, butler: Arc<Butler>) {
    let shell_weak = shell.as_weak();
    shell.on_upload_page_clicked(move || {
        println!("Upload page clicked - opening folder picker...");

        let folder = rfd::FileDialog::new()
            .set_title("Select Page Folder")
            .pick_folder();

        if let Some(folder_path) = folder {
            println!("Selected folder: {:?}", folder_path);

            let current_space_id = if let Some(shell) = shell_weak.upgrade() {
                shell.get_current_space_id().to_string()
            } else {
                println!("Failed to get current space ID");
                return;
            };

            if current_space_id.is_empty() {
                println!("No space selected");
                return;
            }

            let rt = tokio::runtime::Runtime::new().unwrap();
            let butler_inner = butler.clone();

            // Use import_page to import the entire page directory
            let page_result = rt.block_on(async {
                butler_inner.import_page(&current_space_id, &folder_path).await
            });

            let page = match page_result {
                Ok(p) => p,
                Err(e) => {
                    println!("Failed to import page: {}", e);
                    return;
                }
            };

            println!("Page '{}' uploaded successfully!", page.name);

            // Refresh pages list
            if let Some(shell) = shell_weak.upgrade() {
                let pages = butler.list_pages(&current_space_id).unwrap_or_default();
                let page_infos: Vec<crate::PageInfo> = pages
                    .iter()
                    .map(|p| {
                        let app_count = butler.list_apps(&p.id).map(|a| a.len() as i32).unwrap_or(0);
                        crate::PageInfo {
                            id: p.id.clone().into(),
                            name: p.name.clone().into(),
                            app_count,
                        }
                    })
                    .collect();
                shell.set_pages(slint::ModelRc::new(slint::VecModel::from(page_infos)));
            }
        }
    });
}

/// Register reload_page callback - reloads page from filesystem
fn register_reload_page(shell: &Shell, butler: Arc<Butler>) {
    let shell_weak = shell.as_weak();
    shell.on_reload_page(move || {
        println!("Reload page clicked");

        let current_page_id = if let Some(shell) = shell_weak.upgrade() {
            shell.get_current_page_id().to_string()
        } else {
            return;
        };

        if current_page_id.is_empty() {
            println!("No page selected");
            return;
        }

        // TODO: We need to store the page_dir path in page metadata for reload
        // For now, just refresh the apps list
        println!("Page reload requested for: {} (needs page_dir path)", current_page_id);

        // Refresh apps list
        if let Some(shell) = shell_weak.upgrade() {
            let apps = butler.list_apps(&current_page_id).unwrap_or_default();
            let app_infos: Vec<crate::AppInfo> = apps
                .iter()
                .map(|app_name| crate::AppInfo {
                    id: app_name.clone().into(),
                    name: app_name.clone().into(),
                    page_id: current_page_id.clone().into(),
                })
                .collect();
            shell.set_page_apps(slint::ModelRc::new(slint::VecModel::from(app_infos)));
        }
    });
}

/// Register request_page_apps callback - loads apps for a page
fn register_request_page_apps(shell: &Shell, butler: Arc<Butler>) {
    let shell_weak = shell.as_weak();
    shell.on_request_page_apps(move |page_id| {
        println!("Requesting apps for page: {}", page_id);

        let apps = butler.list_apps(&page_id).unwrap_or_default();
        println!("Found {} apps", apps.len());

        if let Some(shell) = shell_weak.upgrade() {
            let app_infos: Vec<crate::AppInfo> = apps
                .iter()
                .map(|app_name| crate::AppInfo {
                    id: app_name.clone().into(),
                    name: app_name.clone().into(),
                    page_id: page_id.clone(),
                })
                .collect();
            shell.set_page_apps(slint::ModelRc::new(slint::VecModel::from(app_infos)));
        }
    });
}

fn register_delete_app(shell: &Shell) {
    shell.on_delete_app(|app_id| {
        println!("Delete app: {}", app_id);
        // TODO: Implement app deletion
    });
}

/// Register refresh_app callback - sends refresh request to timer loop
fn register_refresh_app(shell: &Shell, refresh_tx: std::sync::mpsc::Sender<RefreshAppRequest>) {
    shell.on_refresh_app(move |app_name, app_dir| {
        println!("Refresh app: {} from {}", app_name, app_dir);
        let request = RefreshAppRequest {
            app_name: app_name.to_string(),
            app_dir: PathBuf::from(app_dir.to_string()),
        };
        if refresh_tx.send(request).is_err() {
            println!("Failed to send refresh request - channel closed");
        }
    });
}

/// Setup app runtime with select_app callback and timer loop
///
/// Returns (timer, refresh_tx) - timer must be kept alive, refresh_tx used for refresh_app callback
fn setup_app_runtime(
    shell: &Shell,
    butler: Arc<Butler>,
    courier_handle: Arc<RwLock<Option<CourierHandle>>>,
    tokio_handle: tokio::runtime::Handle,
    debug_server: Option<Arc<ControlServer>>,
) -> (slint::Timer, std::sync::mpsc::Sender<RefreshAppRequest>) {
    // Channel to send loaded app data from tokio to Slint thread
    let (app_ready_tx, app_ready_rx) = std::sync::mpsc::channel::<(
        PreparedPage,                 // prepared page with shell path
        ActorRef<ScribeMessage>,
    )>();

    // Channel for refresh app requests (from refresh_app callback)
    let (refresh_tx, refresh_rx) = std::sync::mpsc::channel::<RefreshAppRequest>();

    // Store running apps on Slint thread
    let running_apps: Rc<RefCell<Vec<RunningApp>>> = Rc::new(RefCell::new(vec![]));

    // Register select_app callback - launches an app
    let butler_select = butler.clone();
    let tokio_handle_select = tokio_handle.clone();
    let shell_weak = shell.as_weak();
    let app_ready_tx_select = app_ready_tx.clone();
    shell.on_select_app(move |app_id| {
        println!("Loading app: {}", app_id);

        // Get current page_id to open the page for sync
        let current_page_id = if let Some(shell) = shell_weak.upgrade() {
            shell.get_current_page_id().to_string()
        } else {
            println!("Failed to get current page ID");
            return;
        };

        if current_page_id.is_empty() {
            println!("No page selected");
            return;
        }

        let butler = butler_select.clone();
        let app_id_str = app_id.to_string();
        let tx = app_ready_tx_select.clone();

        tokio_handle_select.spawn(async move {
            // Open the page (not individual app) to start sync
            let scribe_ref = match butler.open_page(&current_page_id).await {
                Ok(scribe) => {
                    println!("Page opened, scribe ready for sync");
                    scribe
                }
                Err(e) => {
                    println!("Failed to open page: {}", e);
                    return;
                }
            };

            // Prepare page with specific app
            let prepared = match prepare_page(&butler, &current_page_id, Some(&app_id_str)).await {
                Ok(p) => p,
                Err(e) => {
                    println!("Failed to prepare page: {}", e);
                    return;
                }
            };

            println!(
                "Prepared page '{}' with app '{}', {} total apps",
                prepared.page_name, prepared.app_name, prepared.all_apps.len()
            );

            if tx.send((prepared, scribe_ref)).is_err() {
                println!("Failed to send app data to Slint thread");
            }

            slint::invoke_from_event_loop(|| {}).ok();
        });
    });

    // Timer to check for ready apps and process runtime updates
    let running_apps_timer = running_apps.clone();
    let butler_timer = butler.clone();
    let _courier_handle_timer = courier_handle.clone();  // Preserved for potential future use
    let tokio_handle_timer = tokio_handle.clone();
    let app_ready_tx_timer = app_ready_tx.clone();
    let debug_server_timer = debug_server.clone();
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_millis(100),
        move || {
            // Handle ready apps
            while let Ok((prepared, scribe_ref)) = app_ready_rx.try_recv() {
                // Get app_status from debug_server for AI feedback
                let app_status = debug_server_timer.as_ref().map(|ds| ds.app_status());
                if let Some(running_app) = create_app_runtime(prepared, scribe_ref, butler_timer.clone(), debug_server_timer.clone(), app_status) {
                    running_apps_timer.borrow_mut().push(running_app);
                }
            }

            // Collect tab switch requests (process after iteration to avoid borrow issues)
            let mut tab_switch_requests: Vec<(String, String)> = vec![]; // (page_id, new_app_name)
            // Collect app restart requests (triggered by app layer updates)
            let mut app_restart_requests: Vec<(String, String)> = vec![]; // (page_id, app_name)

            // Process updates for all running apps
            for running_app in running_apps_timer.borrow_mut().iter_mut() {
                // Check for tab switch requests
                while let Ok(new_app_name) = running_app.tab_switch_rx.try_recv() {
                    // Skip if already on this app
                    if new_app_name != running_app.app_name {
                        println!(
                            "Tab switch requested: {} -> {}",
                            running_app.app_name, new_app_name
                        );
                        tab_switch_requests.push((running_app.page_id.clone(), new_app_name));
                    }
                }

                // Forward unified page events to Lua thread
                while let Ok(event) = running_app.page_event_rx.try_recv() {
                    // Check if this is an app layer update (triggers restart)
                    let expected_app_layer = format!("app:{}", running_app.app_name);
                    if event.layer_name == expected_app_layer {
                        if matches!(event.event_type, butler::PageEventType::Updated) {
                            println!(
                                "App layer '{}' updated - scheduling restart",
                                event.layer_name
                            );
                            app_restart_requests.push((
                                running_app.page_id.clone(),
                                running_app.app_name.clone(),
                            ));
                            continue; // Don't forward this event to Lua - we're restarting
                        }
                    }

                    match event.event_type {
                        butler::PageEventType::Created => {
                            // New layer discovered - notify Lua
                            let _ = running_app.lua_tx.try_send(
                                app_runtime::LuaWorkerCommand::LayerDiscovered {
                                    layer_name: event.layer_name.clone()
                                }
                            );
                        }
                        butler::PageEventType::Updated => {
                            // Layer updated - forward change to Lua
                            let _ = running_app.lua_tx.try_send(
                                app_runtime::LuaWorkerCommand::LoroChanged {
                                    layer_name: event.layer_name.clone(),
                                    delta: event.delta.clone(),
                                    full_data: event.full_data.clone(),
                                }
                            );
                        }
                    }
                }

                // Forward ephemeral events (generic payload) to Lua thread
                while let Ok(event) = running_app.ephemeral_event_rx.try_recv() {
                    match event {
                        butler::EphemeralEvent::Data { user_did, payload } => {
                            let _ = running_app.lua_tx.try_send(
                                app_runtime::LuaWorkerCommand::Ephemeral { user_did, payload }
                            );
                        }
                    }
                }

                // Note: Ephemeral broadcasts now go directly via Scribe → PeerActor channels
                // (no longer routed through apps.rs/Courier)

                // Process asset pick requests (triggered by Slint button click)
                while let Ok(request) = running_app.asset_pick_rx.try_recv() {
                    handle_asset_pick_request(
                        &request,
                        &running_app.page_id,
                        &running_app.lua_tx,
                        &butler_timer,
                        &tokio_handle_timer,
                    );
                }

                // Process UI mutations
                if let Err(e) = running_app.slint_runtime.process_ui_mutations() {
                    println!("Failed to process UI mutations: {}", e);
                }

                // Process UI queries (for ui:get from Lua)
                running_app.slint_runtime.process_ui_queries();
            }

            // Process app restart requests (from app layer updates)
            for (page_id, app_name) in app_restart_requests {
                println!("Restarting app '{}' due to app layer update", app_name);
                // Find and close the existing app
                let mut apps = running_apps_timer.borrow_mut();
                if let Some(pos) = apps.iter().position(|a| a.page_id == page_id && a.app_name == app_name) {
                    println!("Closing app '{}' for restart", apps[pos].app_name);
                    // Hide the window
                    let _ = apps[pos].slint_runtime.slint_instance().hide();
                    // Send shutdown to Lua worker
                    let _ = apps[pos].lua_tx.try_send(app_runtime::LuaWorkerCommand::Shutdown);
                    // Remove from running apps
                    apps.remove(pos);
                }
                drop(apps);

                // Queue re-loading the same app (will get updated files from Scribe)
                let butler = butler_timer.clone();
                let tx = app_ready_tx_timer.clone();

                tokio_handle_timer.spawn(async move {
                    // Open the page to get scribe ref (page is already open, this just gets ref)
                    let scribe_ref = match butler.open_page(&page_id).await {
                        Ok(scribe) => scribe,
                        Err(e) => {
                            println!("Failed to open page for app restart: {}", e);
                            return;
                        }
                    };

                    // Prepare page with same app (will extract updated files from layer)
                    let prepared = match prepare_page(&butler, &page_id, Some(&app_name)).await {
                        Ok(p) => p,
                        Err(e) => {
                            println!("Failed to prepare page for app restart: {}", e);
                            return;
                        }
                    };

                    println!("App restart: reloaded app '{}'", prepared.app_name);

                    if tx.send((prepared, scribe_ref)).is_err() {
                        println!("Failed to send app data for restart");
                    }

                    slint::invoke_from_event_loop(|| {}).ok();
                });
            }

            // Process tab switch requests
            for (page_id, new_app_name) in tab_switch_requests {
                // Find and close the existing app for this page
                let mut apps = running_apps_timer.borrow_mut();
                if let Some(pos) = apps.iter().position(|a| a.page_id == page_id) {
                    println!("Closing app '{}' for tab switch", apps[pos].app_name);
                    // Hide the window
                    let _ = apps[pos].slint_runtime.slint_instance().hide();
                    // Send shutdown to Lua worker
                    let _ = apps[pos].lua_tx.try_send(app_runtime::LuaWorkerCommand::Shutdown);
                    // Remove from running apps
                    apps.remove(pos);
                }
                drop(apps);

                // Queue loading the new app
                let butler = butler_timer.clone();
                let tx = app_ready_tx_timer.clone();
                let page_id_clone = page_id.clone();
                let new_app_name_clone = new_app_name.clone();

                tokio_handle_timer.spawn(async move {
                    // Open the page to get scribe ref
                    let scribe_ref = match butler.open_page(&page_id_clone).await {
                        Ok(scribe) => scribe,
                        Err(e) => {
                            println!("Failed to open page for tab switch: {}", e);
                            return;
                        }
                    };

                    // Prepare page with new app
                    let prepared = match prepare_page(&butler, &page_id_clone, Some(&new_app_name_clone)).await {
                        Ok(p) => p,
                        Err(e) => {
                            println!("Failed to prepare page for tab switch: {}", e);
                            return;
                        }
                    };

                    println!("Tab switch: loaded app '{}'", prepared.app_name);

                    if tx.send((prepared, scribe_ref)).is_err() {
                        println!("Failed to send app data for tab switch");
                    }

                    slint::invoke_from_event_loop(|| {}).ok();
                });
            }

            // Process refresh app requests
            while let Ok(request) = refresh_rx.try_recv() {
                println!("Processing refresh request for app: {}", request.app_name);
                // Find ANY running app to get the page_id (all apps on a page share the same Scribe)
                // First try exact match, then fall back to any running app
                let apps = running_apps_timer.borrow();
                let page_id = apps.iter()
                    .find(|a| a.app_name == request.app_name)
                    .or_else(|| apps.first())
                    .map(|a| a.page_id.clone());
                drop(apps);

                if let Some(page_id) = page_id {
                    let butler = butler_timer.clone();
                    let app_name = request.app_name;
                    let app_dir = request.app_dir;

                    tokio_handle_timer.spawn(async move {
                        // Get the scribe for this page
                        match butler.open_page(&page_id).await {
                            Ok(scribe_ref) => {
                                // Send RefreshApp message to Scribe
                                let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
                                if let Err(e) = scribe_ref.cast(ScribeMessage::RefreshApp {
                                    app_name: app_name.clone(),
                                    app_dir,
                                    reply: reply_tx,
                                }) {
                                    println!("Failed to send RefreshApp to Scribe: {}", e);
                                    return;
                                }

                                // Wait for result
                                match reply_rx.await {
                                    Ok(Ok(changed_files)) => {
                                        println!("App '{}' refreshed, {} files changed: {:?}",
                                            app_name, changed_files.len(), changed_files);
                                    }
                                    Ok(Err(e)) => {
                                        println!("RefreshApp failed: {}", e);
                                    }
                                    Err(_) => {
                                        println!("RefreshApp reply channel closed");
                                    }
                                }
                            }
                            Err(e) => {
                                println!("Failed to get scribe for page {}: {}", page_id, e);
                            }
                        }
                    });
                } else {
                    println!("No running apps found - cannot refresh");
                }
            }
        },
    );

    (timer, refresh_tx)
}

/// Create app runtime from prepared page data
///
/// **Context**: Called when an app needs to be loaded
/// **Updates**: app_status with "loaded" on success or "failed" with error on failure
fn create_app_runtime(
    prepared: PreparedPage,
    scribe_ref: ActorRef<ScribeMessage>,
    butler: Arc<Butler>,
    debug_server: Option<Arc<ControlServer>>,
    app_status: Option<Arc<RwLock<AppStatus>>>,
) -> Option<RunningApp> {
    println!(
        "Creating parallel app runtime for page: {} ({}), app: {}",
        prepared.page_name, prepared.page_id, prepared.app_name
    );

    // Get user DID from Butler (sync call - no tokio runtime needed)
    let user_did = butler.identity_data()
        .ok()
        .flatten()
        .map(|data| data.did)
        .unwrap_or_else(|| {
            println!("WARNING: No identity data - using fallback DID (this will break sync!)");
            format!("did:key:unknown-{}", &prepared.page_id[..8])
        });

    // Get role from page's permit (not hardcoded from app name)
    let user_role = butler.get_page(&prepared.page_id)
        .ok()
        .flatten()
        .and_then(|page| page.get_permit().cloned())
        .map(|permit| butler::scribe::permit::extract_role_from_permit(&permit))
        .unwrap_or_else(|| "owner".to_string());  // No permit = owner

    let lua_code = match std::fs::read_to_string(&prepared.lua_path) {
        Ok(code) => code,
        Err(e) => {
            let error_msg = format!("Failed to read Lua code from {:?}: {}", prepared.lua_path, e);
            println!("{}", error_msg);
            if let Some(ref status) = app_status {
                if let Ok(mut s) = status.try_write() {
                    s.status = "failed".to_string();
                    s.error = Some(error_msg);
                }
            }
            return None;
        }
    };

    // Use generated shell that imports the app component
    let slint_path = prepared.shell_path.clone();

    let (ui_tx, ui_rx) = tokio::sync::mpsc::channel::<app_runtime::UiMutation>(256);
    let (query_tx, query_rx) = tokio::sync::mpsc::channel::<app_runtime::UiQuery>(32);
    // Unified page event channel - replaces per-layer loro subscriptions + scribe events
    let (page_event_tx, page_event_rx) = tokio::sync::mpsc::channel::<butler::PageEvent>(64);
    // Ephemeral events (cursor, typing, presence) - received via datagrams from peers
    let (ephemeral_event_tx, ephemeral_event_rx) = tokio::sync::mpsc::channel::<butler::EphemeralEvent>(256);
    let (tab_switch_tx, tab_switch_rx) = std::sync::mpsc::channel::<String>();
    // Asset pick requests (triggered by Slint button click for file upload)
    let (asset_pick_tx, asset_pick_rx) = std::sync::mpsc::channel::<app_runtime::AssetPickRequest>();

    println!("SlintRuntime: spawning Lua worker thread first...");
    println!("  page_id: {}", prepared.page_id);
    println!("  user_did: {}", user_did);
    println!("  user_role: {} (from permit)", user_role);

    let page_id = prepared.page_id.clone();     // Page UUID (e.g., "bf57890c-70ce-...")
    let app_name = prepared.app_name.clone();   // App name (e.g., "Shop Customer")
    let (lua_thread, lua_tx) = match app_runtime::LuaWorker::spawn(
        page_id.clone(),
        app_name.clone(),
        user_did.clone(),
        user_role.clone(),
        lua_code,
        scribe_ref.clone(),
        ui_tx,
        query_tx,
    ) {
        Ok((thread, tx)) => (thread, tx),
        Err(e) => {
            let error_msg = format!("Failed to spawn Lua worker: {}", e);
            println!("{}", error_msg);
            if let Some(ref status) = app_status {
                if let Ok(mut s) = status.try_write() {
                    s.status = "failed".to_string();
                    s.error = Some(error_msg);
                }
            }
            return None;
        }
    };

    // Connect debug server's eval channel to this app's LuaWorker
    if let Some(ref debug_server) = debug_server {
        // Create bridge channel for forwarding DebugEvalRequest -> LuaWorkerCommand
        let (bridge_tx, mut bridge_rx) = tokio::sync::mpsc::channel::<crate::control_server::DebugEvalRequest>(100);

        // Set the bridge sender on debug server
        debug_server.set_eval_channel(bridge_tx);

        // Set the LuaWorker channel for direct commands (e.g., AssetUploaded)
        debug_server.set_lua_worker_channel(lua_tx.clone());

        // Spawn forwarding thread: receives DebugEvalRequest, sends LuaWorkerCommand::DebugEval
        let lua_tx_for_debug = lua_tx.clone();
        std::thread::spawn(move || {
            while let Some(req) = bridge_rx.blocking_recv() {
                // Forward to LuaWorker as DebugEval command
                let cmd = app_runtime::LuaWorkerCommand::DebugEval {
                    code: req.code,
                    response_tx: req.response_tx,
                };
                if lua_tx_for_debug.blocking_send(cmd).is_err() {
                    println!("Debug eval bridge: LuaWorker channel closed");
                    break;
                }
            }
            println!("Debug eval bridge thread exiting");
        });

        println!("Debug eval bridge connected for app: {}", app_name);
    }

    println!("Lua worker thread spawned, creating SlintRuntime...");

    let mut slint_runtime = match app_runtime::SlintRuntime::load(
        slint_path,
        app_name.clone(),  // SlintRuntime uses app_name for logging
        ui_rx,
        query_rx,
        lua_tx.clone(),
    ) {
        Ok(runtime) => runtime,
        Err(e) => {
            let error_msg = format!("Failed to create SlintRuntime: {}", e);
            println!("{}", error_msg);
            if let Some(ref status) = app_status {
                if let Ok(mut s) = status.try_write() {
                    s.status = "failed".to_string();
                    s.error = Some(error_msg);
                }
            }
            return None;
        }
    };

    // Set up tab switch channel before setup_callbacks
    slint_runtime.set_tab_switch_channel(tab_switch_tx);
    // Set up asset pick channel for file upload requests
    slint_runtime.set_asset_pick_channel(asset_pick_tx);

    if let Err(e) = slint_runtime.slint_instance().show() {
        let error_msg = format!("Failed to show app window: {:?}", e);
        println!("{}", error_msg);
        if let Some(ref status) = app_status {
            if let Ok(mut s) = status.try_write() {
                s.status = "failed".to_string();
                s.error = Some(error_msg);
            }
        }
        return None;
    }

    if let Err(e) = slint_runtime.setup_callbacks() {
        let error_msg = format!("Failed to setup shell callbacks: {}", e);
        println!("{}", error_msg);
        if let Some(ref status) = app_status {
            if let Ok(mut s) = status.try_write() {
                s.status = "failed".to_string();
                s.error = Some(error_msg);
            }
        }
        return None;
    }

    // Initialize VecModels for models declared in manifest
    if let Err(e) = slint_runtime.init_models(&prepared.models) {
        println!("Failed to init models from manifest: {}", e);
        // Continue anyway - models can be created on-demand
    }

    // Setup AppAPI global callbacks (generic Event Bus callbacks)
    if let Err(e) = slint_runtime.setup_global_callbacks() {
        let error_msg = format!("Failed to setup global callbacks: {}", e);
        println!("{}", error_msg);
        if let Some(ref status) = app_status {
            if let Ok(mut s) = status.try_write() {
                s.status = "failed".to_string();
                s.error = Some(error_msg);
            }
        }
        return None;
    }

    println!("Lua worker thread spawned successfully");

    // Subscribe to unified page events (replaces per-layer subscriptions + ScribeUI)
    // This single subscription receives all layer changes (Created/Updated)
    if let Err(e) = scribe_ref.cast(ScribeMessage::SubscribeToPageEvents {
        event_tx: page_event_tx,
    }) {
        println!("Failed to subscribe to page events: {}", e);
    } else {
        println!("Subscribed to unified page events for {} layers", prepared.data_layers.len());
    }

    // Subscribe to ephemeral events (cursor, typing, presence from peers via datagrams)
    if let Err(e) = scribe_ref.cast(ScribeMessage::SubscribeEphemeral {
        tx: ephemeral_event_tx,
    }) {
        println!("Failed to subscribe to ephemeral events: {}", e);
    } else {
        println!("Subscribed to ephemeral events (cursor, typing, presence)");
    }

    // Note: Outbound ephemeral broadcasts (cursor, typing) now go directly via
    // Scribe → PeerActor channels (SubscriberInfo.ephemeral_tx)
    // No apps.rs routing needed anymore

    // Send initial load events for all data layers
    let lua_tx_init = lua_tx.clone();
    println!("Sending initial load events to Lua worker for {} layers...", prepared.data_layers.len());
    for layer_name in &prepared.data_layers {
        let _ = lua_tx_init.try_send(app_runtime::LuaWorkerCommand::LoroChanged {
            layer_name: layer_name.clone(),
            delta: None,  // Initial load - no delta, will use full_data
            full_data: serde_json::json!({}),
        });
    }

    println!(
        "App '{}' loaded successfully with parallel Lua worker!",
        prepared.app_name
    );

    // Update status to "loaded" on success
    if let Some(ref status) = app_status {
        if let Ok(mut s) = status.try_write() {
            s.status = "loaded".to_string();
            s.error = None;
            s.loaded_at = Some(chrono::Utc::now().to_rfc3339());
        }
    }

    Some(RunningApp {
        app_name: prepared.app_name,
        page_id: prepared.page_id,
        page_name: prepared.page_name,
        all_apps: prepared.all_apps,
        slint_runtime,
        lua_thread,
        lua_tx,
        page_event_rx,
        ephemeral_event_rx,
        tab_switch_rx,
        asset_pick_rx,
    })
}

/// Handle asset pick request - opens native file dialog and uploads selected file
///
/// **Context**: Called from timer loop when user clicks asset upload button
/// **Security**: Only triggered by Slint button click (user-initiated)
/// **Flow**: File dialog → read file → detect mime → upload via Butler → notify Lua
fn handle_asset_pick_request(
    request: &app_runtime::AssetPickRequest,
    page_id: &str,
    lua_tx: &tokio::sync::mpsc::Sender<app_runtime::LuaWorkerCommand>,
    butler: &std::sync::Arc<Butler>,
    tokio_handle: &tokio::runtime::Handle,
) {
    // Determine file filter extensions
    let filter_extensions: &[&str] = match request.filter.as_str() {
        "images" => &["png", "jpg", "jpeg", "gif", "webp", "bmp", "svg"],
        _ => &["*"],
    };

    let filter_name = match request.filter.as_str() {
        "images" => "Images",
        _ => "All Files",
    };

    // Open native file dialog (blocking - but OK in Slint event loop context)
    let file = rfd::FileDialog::new()
        .set_title("Select Asset")
        .add_filter(filter_name, filter_extensions)
        .pick_file();

    let path = match file {
        Some(p) => p,
        None => {
            println!("Asset pick cancelled by user");
            return;
        }
    };

    println!("Asset selected: {:?}", path);

    // Read file contents
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) => {
            println!("Failed to read asset file {:?}: {}", path, e);
            return;
        }
    };

    // Extract filename
    let filename = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    // Detect MIME type
    let mime_type = mime_guess::from_path(&path)
        .first_or_octet_stream()
        .to_string();

    let size = bytes.len() as u64;

    // Upload asset via Butler's async service using the tokio handle
    // This encrypts the asset and stores it, returning a content hash
    let butler = butler.clone();
    let page_id = page_id.to_string();
    let lua_tx = lua_tx.clone();

    tokio_handle.spawn(async move {
        match butler.upload_asset_async(&page_id, &bytes, &filename, &mime_type).await {
            Ok(hash) => {
                println!(
                    "Asset uploaded: {} ({}, {} bytes) -> {}",
                    filename, mime_type, size, hash
                );

                // Notify Lua of successful upload
                let _ = lua_tx.try_send(app_runtime::LuaWorkerCommand::AssetUploaded {
                    hash,
                    filename,
                    mime_type,
                    size,
                });
            }
            Err(e) => {
                println!("Failed to upload asset: {}", e);
                // TODO: Could send error notification to Lua
            }
        }
    });
}
