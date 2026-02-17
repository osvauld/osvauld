use crate::prepare::prepare_page;
use crate::{
    extract_panic_message, AppStatus, AssetPickRequest, LaunchedApp, PreparedPage, RunningSlintApp,
    SlintRuntime, WindowGeometry,
};
use butler::{Butler, PageUpdate, ScribeMessage};
use lua_runtime::{ActorScribeHandle, LuaCommand, LuaRuntime, LuaRuntimeConfig, UiMutation, UiQuery};
use ractor::ActorRef;
use slint::ComponentHandle;
use std::cell::{Cell, RefCell};
use std::panic::AssertUnwindSafe;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

struct AppChannels {
    ui_tx: tokio::sync::mpsc::Sender<UiMutation>,
    ui_rx: tokio::sync::mpsc::Receiver<UiMutation>,
    query_tx: tokio::sync::mpsc::Sender<UiQuery>,
    query_rx: tokio::sync::mpsc::Receiver<UiQuery>,
    page_update_tx: tokio::sync::mpsc::Sender<PageUpdate>,
    page_update_rx: tokio::sync::mpsc::Receiver<PageUpdate>,
    tab_switch_tx: std::sync::mpsc::Sender<String>,
    tab_switch_rx: std::sync::mpsc::Receiver<String>,
    asset_pick_tx: std::sync::mpsc::Sender<AssetPickRequest>,
    asset_pick_rx: std::sync::mpsc::Receiver<AssetPickRequest>,
}

impl AppChannels {
    fn new() -> Self {
        let (ui_tx, ui_rx) = tokio::sync::mpsc::channel::<UiMutation>(4096);
        let (query_tx, query_rx) = tokio::sync::mpsc::channel::<UiQuery>(32);
        let (page_update_tx, page_update_rx) = tokio::sync::mpsc::channel::<PageUpdate>(4096);
        let (tab_switch_tx, tab_switch_rx) = std::sync::mpsc::channel::<String>();
        let (asset_pick_tx, asset_pick_rx) = std::sync::mpsc::channel::<AssetPickRequest>();
        Self {
            ui_tx,
            ui_rx,
            query_tx,
            query_rx,
            page_update_tx,
            page_update_rx,
            tab_switch_tx,
            tab_switch_rx,
            asset_pick_tx,
            asset_pick_rx,
        }
    }
}

pub fn create_slint_app(
    prepared: PreparedPage,
    scribe_ref: ActorRef<ScribeMessage>,
    butler: Arc<Butler>,
    app_status: Option<Arc<RwLock<AppStatus>>>,
) -> Option<RunningSlintApp> {
    tracing::info!(
        page_id = %prepared.page_id,
        page_name = %prepared.page_name,
        app_name = %prepared.app_name,
        "Creating Slint app runtime"
    );

    let app_ctx = butler.app_context(&prepared.page_id);
    let user_did = app_ctx.user_did;
    let user_name = app_ctx.user_name;
    let user_role = app_ctx.user_role;

    let raw_lua_code = match std::fs::read_to_string(&prepared.lua_path) {
        Ok(code) => code,
        Err(e) => {
            let error_msg = format!("Failed to read Lua code: {}", e);
            tracing::error!(%error_msg);
            update_status_failed(&app_status, error_msg);
            return None;
        }
    };

    let lua_code = format!(
        "package.path = '{}/?.lua;' .. package.path\n{}",
        prepared.temp_dir.display(),
        raw_lua_code
    );

    let channels = AppChannels::new();

    let config = LuaRuntimeConfig {
        page_id: prepared.page_id.clone(),
        app_name: prepared.app_name.clone(),
        scribe: ActorScribeHandle::new(scribe_ref.clone()),
        user_did,
        user_name,
        user_role,
        lua_code,
        ui_enabled: true,
        ui_tx: Some(channels.ui_tx.clone()),
        query_tx: Some(channels.query_tx.clone()),
        navigate_tx: Some(channels.tab_switch_tx.clone()),
    };

    let (lua_thread, lua_tx) = match LuaRuntime::spawn(config) {
        Ok((thread, tx)) => (thread, tx),
        Err(e) => {
            let error_msg = format!("Failed to spawn Lua worker: {}", e);
            tracing::error!(%error_msg);
            update_status_failed(&app_status, error_msg);
            return None;
        }
    };

    let mut slint_runtime = match SlintRuntime::load(
        prepared.shell_path.clone(),
        prepared.app_name.clone(),
        channels.ui_rx,
        channels.query_rx,
        lua_tx.clone(),
    ) {
        Ok(runtime) => runtime,
        Err(e) => {
            let error_msg = format!("Failed to create SlintRuntime: {}", e);
            tracing::error!(%error_msg);
            update_status_failed(&app_status, error_msg);
            return None;
        }
    };

    if let Err(e) = configure_runtime_and_show(
        &mut slint_runtime,
        channels.tab_switch_tx.clone(),
        channels.asset_pick_tx.clone(),
        prepared.restore_geometry.as_ref(),
        &prepared.models,
    ) {
        let error_msg = format!("Failed to configure Slint runtime: {}", e);
        tracing::error!(%error_msg);
        update_status_failed(&app_status, error_msg);
        return None;
    }

    if let Err(e) = scribe_ref.cast(ScribeMessage::SubscribeToPageUpdates {
        tx: channels.page_update_tx,
    }) {
        tracing::warn!("Failed to subscribe to page updates: {}", e);
    }

    if let Some(ref status) = app_status {
        if let Ok(mut s) = status.try_write() {
            s.page_id = Some(prepared.page_id.clone());
            s.app_name = Some(prepared.app_name.clone());
            s.status = "loaded".to_string();
            s.error = None;
            s.loaded_at = Some(chrono::Utc::now().to_rfc3339());
            s.version = Some(prepared.version.display.clone());
        }
    }

    Some(RunningSlintApp {
        app_name: prepared.app_name,
        page_id: prepared.page_id,
        page_name: prepared.page_name,
        all_apps: prepared.all_apps,
        slint_runtime,
        lua_thread,
        lua_tx,
        page_update_rx: channels.page_update_rx,
        tab_switch_rx: channels.tab_switch_rx,
        asset_pick_rx: channels.asset_pick_rx,
        version: prepared.version,
    })
}

pub fn launch_slint_app(
    prepared: PreparedPage,
    scribe_ref: ActorRef<ScribeMessage>,
    butler: Arc<Butler>,
    tokio_handle: tokio::runtime::Handle,
    app_status: Option<Arc<RwLock<AppStatus>>>,
) -> Option<LaunchedApp> {
    let running = create_slint_app(prepared, scribe_ref, butler.clone(), app_status.clone())?;
    let lua_tx_out = running.lua_tx.clone();

    let running: Rc<RefCell<Option<RunningSlintApp>>> = Rc::new(RefCell::new(Some(running)));
    let ready_rx: Rc<RefCell<Option<std::sync::mpsc::Receiver<(PreparedPage, ActorRef<ScribeMessage>)>>>> =
        Rc::new(RefCell::new(None));
    let pending_geometry: Rc<RefCell<Option<WindowGeometry>>> = Rc::new(RefCell::new(None));
    let crashed: Rc<Cell<bool>> = Rc::new(Cell::new(false));

    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(100),
        move || {
            if crashed.get() {
                return;
            }

            let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                if let Some(ref rx) = *ready_rx.borrow() {
                    while let Ok((mut new_prepared, new_scribe)) = rx.try_recv() {
                        if let Some(geom) = pending_geometry.borrow_mut().take() {
                            new_prepared.restore_geometry = Some(geom);
                        }
                        if let Some(new_app) = create_slint_app(
                            new_prepared,
                            new_scribe,
                            butler.clone(),
                            app_status.clone(),
                        ) {
                            *running.borrow_mut() = Some(new_app);
                        }
                    }
                }

                let mut running_ref = running.borrow_mut();
                let Some(running_app) = running_ref.as_mut() else {
                    return;
                };

                while let Ok(update) = running_app.page_update_rx.try_recv() {
                    match update {
                        PageUpdate::LayerChanged {
                            layer,
                            ops,
                            delta,
                            full_data,
                            created,
                            ..
                        } => {
                            let expected_app_layer = format!("app:{}", running_app.app_name);
                            let has_ops = ops.as_ref().map(|o| !o.is_empty()).unwrap_or(false);
                            if layer == expected_app_layer && !created && has_ops {
                                tracing::info!(layer = %layer, "App layer updated - restarting");
                                let tx = begin_app_replacement(running_app, &pending_geometry, &ready_rx);
                                let butler = butler.clone();
                                let page_id = running_app.page_id.clone();
                                let app_name = running_app.app_name.clone();
                                *running_ref = None;
                                schedule_app_prepare(
                                    tokio_handle.clone(),
                                    butler,
                                    page_id,
                                    app_name,
                                    tx,
                                    "restart",
                                );
                                return;
                            }

                            let _ = running_app.lua_tx.try_send(LuaCommand::LayerChanged {
                                layer_name: layer,
                                created,
                                delta,
                                full_data,
                            });
                        }
                        PageUpdate::Ephemeral {
                            user_did, payload, ..
                        } => {
                            let _ = running_app
                                .lua_tx
                                .try_send(LuaCommand::Ephemeral { user_did, payload });
                        }
                        PageUpdate::PeerSubscribed { did, .. } => {
                            let _ = running_app
                                .lua_tx
                                .try_send(LuaCommand::PeerJoined { user_did: did });
                        }
                        PageUpdate::PeerUnsubscribed { did } => {
                            let _ = running_app
                                .lua_tx
                                .try_send(LuaCommand::PeerLeft { user_did: did });
                        }
                        PageUpdate::QueryUpdated { .. } => {}
                        PageUpdate::StructuredEphemeral {
                            from_did,
                            func,
                            args,
                        } => {
                            let _ = running_app
                                .lua_tx
                                .try_send(LuaCommand::StructuredEphemeral {
                                    from_did,
                                    func,
                                    args,
                                });
                        }
                    }
                }

                while let Ok(new_app_name) = running_app.tab_switch_rx.try_recv() {
                    if new_app_name != running_app.app_name {
                        tracing::info!(
                            from = %running_app.app_name,
                            to = %new_app_name,
                            "Tab switch requested"
                        );

                        let tx = begin_app_replacement(running_app, &pending_geometry, &ready_rx);
                        let butler = butler.clone();
                        let page_id = running_app.page_id.clone();
                        *running_ref = None;
                        schedule_app_prepare(
                            tokio_handle.clone(),
                            butler,
                            page_id,
                            new_app_name,
                            tx,
                            "tab switch",
                        );
                        return;
                    }
                }

                while let Ok(request) = running_app.asset_pick_rx.try_recv() {
                    handle_asset_pick(
                        &request,
                        &running_app.page_id,
                        &running_app.lua_tx,
                        &butler,
                        &tokio_handle,
                    );
                }

                if let Err(e) = running_app.slint_runtime.process_ui_mutations() {
                    tracing::warn!(error = %e, "Failed to process UI mutations");
                }
                running_app.slint_runtime.process_ui_queries();
            }));

            if let Err(panic_payload) = result {
                let msg = extract_panic_message(&panic_payload);
                let app_name = running
                    .borrow()
                    .as_ref()
                    .map(|r| r.app_name.clone())
                    .unwrap_or_else(|| "unknown".to_string());

                tracing::error!(app = %app_name, panic = %msg, "App panicked — closing window");
                if let Some(app) = running.borrow_mut().take() {
                    let _ = app.slint_runtime.slint_instance().hide();
                    let _ = app.lua_tx.try_send(LuaCommand::Shutdown);
                }
                crashed.set(true);
                if let Some(ref status) = app_status {
                    if let Ok(mut s) = status.try_write() {
                        s.status = "crashed".to_string();
                        s.error = Some(format!("App panicked: {}", msg));
                    }
                }
            }
        },
    );

    Some(LaunchedApp {
        timer,
        lua_tx: lua_tx_out,
    })
}

pub fn handle_asset_pick(
    request: &AssetPickRequest,
    page_id: &str,
    lua_tx: &tokio::sync::mpsc::Sender<LuaCommand>,
    butler: &Arc<Butler>,
    tokio_handle: &tokio::runtime::Handle,
) {
    let filter_extensions: &[&str] = match request.filter.as_str() {
        "images" => &["png", "jpg", "jpeg", "gif", "webp", "bmp", "svg"],
        _ => &["*"],
    };
    let filter_name = if request.filter == "images" {
        "Images"
    } else {
        "All Files"
    };

    let file = rfd::FileDialog::new()
        .set_title("Select Asset")
        .add_filter(filter_name, filter_extensions)
        .pick_file();

    let path = match file {
        Some(p) => p,
        None => {
            tracing::info!("Asset pick cancelled by user");
            return;
        }
    };

    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) => {
            tracing::error!(path = %path.display(), error = %e, "Failed to read asset file");
            return;
        }
    };

    let filename = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string());
    let mime_type = mime_guess::from_path(&path)
        .first_or_octet_stream()
        .to_string();
    let size = bytes.len() as u64;

    let butler = butler.clone();
    let page_id = page_id.to_string();
    let lua_tx = lua_tx.clone();

    tokio_handle.spawn(async move {
        match butler
            .assets()
            .upload(&page_id, &bytes, &filename, &mime_type)
            .await
        {
            Ok(hash) => {
                let _ = lua_tx.try_send(LuaCommand::AssetUploaded {
                    hash,
                    filename,
                    mime_type,
                    size,
                });
            }
            Err(e) => tracing::error!(error = %e, "Failed to upload asset"),
        }
    });
}

fn capture_window_geometry(app: &RunningSlintApp) -> WindowGeometry {
    let window = app.slint_runtime.slint_instance().window();
    let win_pos = window.position();
    let win_size = window.size();
    WindowGeometry {
        x: win_pos.x,
        y: win_pos.y,
        width: win_size.width,
        height: win_size.height,
    }
}

fn configure_runtime_and_show(
    slint_runtime: &mut SlintRuntime,
    tab_switch_tx: std::sync::mpsc::Sender<String>,
    asset_pick_tx: std::sync::mpsc::Sender<AssetPickRequest>,
    restore_geometry: Option<&WindowGeometry>,
    model_names: &[String],
) -> Result<(), Box<dyn std::error::Error>> {
    slint_runtime.set_tab_switch_channel(tab_switch_tx);
    slint_runtime.set_asset_pick_channel(asset_pick_tx);
    slint_runtime.slint_instance().show()?;

    if let Some(geom) = restore_geometry {
        let window = slint_runtime.slint_instance().window();
        window.set_position(slint::WindowPosition::Physical(slint::PhysicalPosition::new(
            geom.x, geom.y,
        )));
        window.set_size(slint::WindowSize::Physical(slint::PhysicalSize::new(
            geom.width,
            geom.height,
        )));
    }

    slint_runtime.setup_callbacks()?;
    if let Err(e) = slint_runtime.init_models(model_names) {
        tracing::warn!(error = %e, "Failed to init models from manifest");
    }
    slint_runtime.setup_global_callbacks()?;
    Ok(())
}

fn begin_app_replacement(
    running_app: &RunningSlintApp,
    pending_geometry: &Rc<RefCell<Option<WindowGeometry>>>,
    ready_rx: &Rc<RefCell<Option<std::sync::mpsc::Receiver<(PreparedPage, ActorRef<ScribeMessage>)>>>>,
) -> std::sync::mpsc::Sender<(PreparedPage, ActorRef<ScribeMessage>)> {
    let geom = capture_window_geometry(running_app);
    *pending_geometry.borrow_mut() = Some(geom);
    let _ = running_app.slint_runtime.slint_instance().hide();
    let _ = running_app.lua_tx.try_send(LuaCommand::Shutdown);
    let (tx, rx) = std::sync::mpsc::channel();
    *ready_rx.borrow_mut() = Some(rx);
    tx
}

fn schedule_app_prepare(
    tokio_handle: tokio::runtime::Handle,
    butler: Arc<Butler>,
    page_id: String,
    app_name: String,
    tx: std::sync::mpsc::Sender<(PreparedPage, ActorRef<ScribeMessage>)>,
    context: &'static str,
) {
    tokio_handle.spawn(async move {
        let scribe_ref = match butler.open_page(&page_id).await {
            Ok(s) => s,
            Err(e) => {
                tracing::error!(error = %e, context = context, "Failed to open page");
                return;
            }
        };

        let prepared = match prepare_page(&butler, &page_id, Some(&app_name), &scribe_ref).await {
            Ok(p) => p,
            Err(e) => {
                tracing::error!(error = %e, context = context, "Failed to prepare page");
                return;
            }
        };

        let _ = tx.send((prepared, scribe_ref));
        slint::invoke_from_event_loop(|| {}).ok();
    });
}

fn update_status_failed(app_status: &Option<Arc<RwLock<AppStatus>>>, error_msg: String) {
    if let Some(ref status) = app_status {
        if let Ok(mut s) = status.try_write() {
            s.status = "failed".to_string();
            s.error = Some(error_msg);
        }
    }
}
