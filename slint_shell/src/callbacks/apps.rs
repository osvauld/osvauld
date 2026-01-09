//! App management callbacks
//!
//! Handles page/app loading, upload, and runtime management.
//! Flow: Spaces → Pages → Apps

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use butler::{Butler, ScribeMessage};
use ractor::ActorRef;
use slint::ComponentHandle;

use crate::app_runner::{prepare_page, PreparedPage, RunningApp};
use crate::Shell;

/// Register app management callbacks on the shell
///
/// Returns the timer that must be kept alive for the app runtime loop.
pub fn register(
    shell: &Shell,
    butler: Arc<Butler>,
    tokio_handle: tokio::runtime::Handle,
) -> slint::Timer {
    // Page-level callbacks (SpaceView shows pages)
    register_request_pages(shell, butler.clone());
    register_select_page(shell, butler.clone());
    register_upload_page(shell, butler.clone());
    register_reload_page(shell, butler.clone());

    // App-level callbacks (PageView shows apps)
    register_request_page_apps(shell, butler.clone());
    register_delete_app(shell);

    // Setup app loading and runtime timer
    setup_app_runtime(shell, butler, tokio_handle)
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

/// Setup app runtime with select_app callback and timer loop
fn setup_app_runtime(
    shell: &Shell,
    butler: Arc<Butler>,
    tokio_handle: tokio::runtime::Handle,
) -> slint::Timer {
    // Channel to send loaded app data from tokio to Slint thread
    let (app_ready_tx, app_ready_rx) = std::sync::mpsc::channel::<(
        PreparedPage,                 // prepared page with shell path
        ActorRef<ScribeMessage>,
    )>();

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
    let tokio_handle_timer = tokio_handle.clone();
    let app_ready_tx_timer = app_ready_tx.clone();
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_millis(100),
        move || {
            // Handle ready apps
            while let Ok((prepared, scribe_ref)) = app_ready_rx.try_recv() {
                if let Some(running_app) = create_app_runtime(prepared, scribe_ref) {
                    running_apps_timer.borrow_mut().push(running_app);
                }
            }

            // Collect tab switch requests (process after iteration to avoid borrow issues)
            let mut tab_switch_requests: Vec<(String, String)> = vec![]; // (page_id, new_app_name)

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

                // Forward Loro events to Lua thread
                while let Ok(event) = running_app.loro_rx.try_recv() {
                    let _ = running_app.lua_tx.try_send(app_runtime::LuaWorkerCommand::LoroChanged {
                        layer_name: event.layer_name.clone(),
                        delta: event.delta.clone(),
                        full_data: event.full_data.clone(),
                    });
                }

                // Forward ScribeEvent (LayerDiscovered) to Lua thread
                while let Ok(event) = running_app.scribe_event_rx.try_recv() {
                    match event {
                        butler::ScribeEvent::LayerDiscovered { layer_name } => {
                            let _ = running_app.lua_tx.try_send(
                                app_runtime::LuaWorkerCommand::LayerDiscovered { layer_name }
                            );
                        }
                        butler::ScribeEvent::LayerUpdated { .. } => {
                            // LayerUpdated is handled via LoroChangeEvent (more detailed)
                        }
                    }
                }

                // Process UI mutations
                if let Err(e) = running_app.slint_runtime.process_ui_mutations() {
                    println!("Failed to process UI mutations: {}", e);
                }
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
        },
    );

    timer
}

/// Create app runtime from prepared page data
fn create_app_runtime(
    prepared: PreparedPage,
    scribe_ref: ActorRef<ScribeMessage>,
) -> Option<RunningApp> {
    println!(
        "Creating parallel app runtime for page: {}, app: {}",
        prepared.page_name, prepared.app_name
    );

    let lua_code = match std::fs::read_to_string(&prepared.lua_path) {
        Ok(code) => code,
        Err(e) => {
            println!("Failed to read Lua code from {:?}: {}", prepared.lua_path, e);
            return None;
        }
    };

    // Use generated shell that imports the app component
    let slint_path = prepared.shell_path.clone();

    let (ui_tx, ui_rx) = tokio::sync::mpsc::channel::<app_runtime::UiMutation>(32);
    let (loro_tx, loro_rx) = tokio::sync::mpsc::channel::<butler::LoroChangeEvent>(32);
    let (scribe_event_tx, scribe_event_rx) = tokio::sync::mpsc::channel::<butler::ScribeEvent>(32);
    let (tab_switch_tx, tab_switch_rx) = std::sync::mpsc::channel::<String>();

    println!("SlintRuntime: spawning Lua worker thread first...");

    let page_id = prepared.page_name.clone();  // Page identifier (e.g., "my-shop")
    let app_name = prepared.app_name.clone();   // App name (e.g., "Shop Customer")
    let (lua_thread, lua_tx) = match app_runtime::LuaWorker::spawn(
        page_id.clone(),
        app_name.clone(),
        lua_code,
        scribe_ref.clone(),
        ui_tx,
    ) {
        Ok((thread, tx)) => (thread, tx),
        Err(e) => {
            println!("Failed to spawn Lua worker: {}", e);
            return None;
        }
    };

    println!("Lua worker thread spawned, creating SlintRuntime...");

    let mut slint_runtime = match app_runtime::SlintRuntime::load(
        slint_path,
        app_name.clone(),  // SlintRuntime uses app_name for logging
        ui_rx,
        lua_tx.clone(),
    ) {
        Ok(runtime) => runtime,
        Err(e) => {
            println!("Failed to create SlintRuntime: {}", e);
            return None;
        }
    };

    // Set up tab switch channel before setup_callbacks
    slint_runtime.set_tab_switch_channel(tab_switch_tx);

    if let Err(e) = slint_runtime.slint_instance().show() {
        println!("Failed to show app window: {:?}", e);
        return None;
    }

    if let Err(e) = slint_runtime.setup_callbacks() {
        println!("Failed to setup shell callbacks: {}", e);
        return None;
    }

    // Setup AppAPI global callbacks (app-specific callbacks)
    if let Err(e) = slint_runtime.setup_global_callbacks() {
        println!("Failed to setup global callbacks: {}", e);
        return None;
    }

    println!("Lua worker thread spawned successfully");

    // Subscribe to Loro changes for all data layers from permit template
    for layer_name in &prepared.data_layers {
        if let Err(e) = scribe_ref.cast(ScribeMessage::SubscribeToLoroChanges {
            layer_name: layer_name.clone(),
            event_tx: loro_tx.clone(),
        }) {
            println!("Failed to subscribe to '{}' layer: {}", layer_name, e);
        } else {
            println!("Subscribed to Loro changes for '{}' layer", layer_name);
        }
    }

    // Subscribe to ScribeEvent for layer discovery notifications
    if let Err(e) = scribe_ref.cast(ScribeMessage::SubscribeUI {
        tx: scribe_event_tx,
    }) {
        println!("Failed to subscribe to ScribeEvent: {}", e);
    } else {
        println!("Subscribed to ScribeEvent (layer discovery)");
    }

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

    Some(RunningApp {
        app_name: prepared.app_name,
        page_id: prepared.page_id,
        page_name: prepared.page_name,
        all_apps: prepared.all_apps,
        slint_runtime,
        lua_thread,
        lua_tx,
        loro_rx,
        scribe_event_rx,
        tab_switch_rx,
    })
}
