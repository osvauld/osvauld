use crate::asset_image::{decode_image_thumbnail_bytes, ImageLoadRequest, ImageLoadResponse};
use crate::prepare::prepare_page;
use crate::{
    extract_panic_message, AppStatus, AppUiCommand, AssetPickRequest, DragState, LaunchedApp,
    PreparedPage, RecordedFrame, RecordingState, RunningSlintApp, SlintRuntime, UiMouseButton,
    WindowGeometry, MAX_RECORDING_FRAMES,
};
use butler::{Butler, PageUpdate, ScribeMessage};
use lua_runtime::{
    ActorScribeHandle, LuaCommand, LuaRuntime, LuaRuntimeConfig, UiMutation, UiQuery,
};
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
    image_req_tx: tokio::sync::mpsc::Sender<ImageLoadRequest>,
    image_req_rx: tokio::sync::mpsc::Receiver<ImageLoadRequest>,
    image_resp_tx: tokio::sync::mpsc::Sender<ImageLoadResponse>,
    image_resp_rx: tokio::sync::mpsc::Receiver<ImageLoadResponse>,
}

impl AppChannels {
    fn new() -> Self {
        let (ui_tx, ui_rx) = tokio::sync::mpsc::channel::<UiMutation>(4096);
        let (query_tx, query_rx) = tokio::sync::mpsc::channel::<UiQuery>(32);
        let (page_update_tx, page_update_rx) = tokio::sync::mpsc::channel::<PageUpdate>(4096);
        let (tab_switch_tx, tab_switch_rx) = std::sync::mpsc::channel::<String>();
        let (asset_pick_tx, asset_pick_rx) = std::sync::mpsc::channel::<AssetPickRequest>();
        // Image pipeline bounded to avoid unbounded memory growth from large images.
        let (image_req_tx, image_req_rx) = tokio::sync::mpsc::channel::<ImageLoadRequest>(64);
        let (image_resp_tx, image_resp_rx) = tokio::sync::mpsc::channel::<ImageLoadResponse>(64);
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
            image_req_tx,
            image_req_rx,
            image_resp_tx,
            image_resp_rx,
        }
    }
}

pub fn create_slint_app(
    prepared: PreparedPage,
    scribe_ref: ActorRef<ScribeMessage>,
    butler: Arc<Butler>,
    app_status: Option<Arc<RwLock<AppStatus>>>,
    clock: Arc<dyn domains::ClockSource>,
) -> Option<RunningSlintApp> {
    create_slint_app_inner(prepared, scribe_ref, butler, app_status, clock, None)
}

/// Internal constructor that accepts an optional tokio handle for spawning the image task.
///
/// `tokio_handle` is `None` only in unit tests; production callers always pass `Some`.
fn create_slint_app_inner(
    prepared: PreparedPage,
    scribe_ref: ActorRef<ScribeMessage>,
    butler: Arc<Butler>,
    app_status: Option<Arc<RwLock<AppStatus>>>,
    clock: Arc<dyn domains::ClockSource>,
    tokio_handle: Option<tokio::runtime::Handle>,
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
        clock,
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

    // Spawn the background image loader; keeps decryption + decode off the UI thread.
    let (image_req_tx, image_resp_rx) = if let Some(ref handle) = tokio_handle {
        let mut image_req_rx = channels.image_req_rx;
        let image_resp_tx = channels.image_resp_tx.clone();
        let butler_clone = butler.clone();
        handle.spawn(async move {
            spawn_image_loader(&mut image_req_rx, image_resp_tx, butler_clone).await;
        });

        (Some(channels.image_req_tx), Some(channels.image_resp_rx))
    } else {
        (None, None)
    };

    let mut slint_runtime = match SlintRuntime::load(
        prepared.shell_path.clone(),
        prepared.app_name.clone(),
        channels.ui_rx,
        channels.query_rx,
        lua_tx.clone(),
        image_req_tx,
        image_resp_rx,
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
    clock: Arc<dyn domains::ClockSource>,
) -> Option<LaunchedApp> {
    let running = create_slint_app_inner(
        prepared,
        scribe_ref,
        butler.clone(),
        app_status.clone(),
        clock.clone(),
        Some(tokio_handle.clone()),
    )?;
    let lua_tx_out = running.lua_tx.clone();

    // UI automation channel: drained on the slint thread by the timer below.
    let (app_ui_tx, app_ui_rx) = tokio::sync::mpsc::channel::<AppUiCommand>(64);
    let app_ui_rx: Rc<RefCell<tokio::sync::mpsc::Receiver<AppUiCommand>>> =
        Rc::new(RefCell::new(app_ui_rx));
    let active_drag: Rc<RefCell<Option<DragState>>> = Rc::new(RefCell::new(None));
    let recording: Rc<RefCell<Option<RecordingState>>> = Rc::new(RefCell::new(None));
    let app_ui_tx_out = app_ui_tx.clone();

    let running: Rc<RefCell<Option<RunningSlintApp>>> = Rc::new(RefCell::new(Some(running)));
    let ready_rx: Rc<
        RefCell<Option<std::sync::mpsc::Receiver<(PreparedPage, ActorRef<ScribeMessage>)>>>,
    > = Rc::new(RefCell::new(None));
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
                        if let Some(new_app) = create_slint_app_inner(
                            new_prepared,
                            new_scribe,
                            butler.clone(),
                            app_status.clone(),
                            clock.clone(),
                            Some(tokio_handle.clone()),
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
                            dynamic_ref,
                            ..
                        } => {
                            let expected_app_layer = format!("app:{}", running_app.app_name);
                            let has_ops = ops.as_ref().map(|o| !o.is_empty()).unwrap_or(false);
                            if layer == expected_app_layer && !created && has_ops {
                                tracing::info!(layer = %layer, "App layer updated - restarting");
                                let tx = begin_app_replacement(
                                    running_app,
                                    &pending_geometry,
                                    &ready_rx,
                                );
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
                                dynamic_ref,
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
                running_app.slint_runtime.apply_loaded_images();

                drain_ui_commands(running_app, &app_ui_rx, &active_drag, &recording);
                advance_active_drag(running_app, &active_drag);
                capture_recording_frame(running_app, &recording);
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
        app_ui_tx: app_ui_tx_out,
    })
}

/// Drain pending UI automation commands and dispatch them on the slint thread.
///
/// Drag commands are stored in `active_drag`; motion is stepped across
/// subsequent ticks by `advance_active_drag`.
fn drain_ui_commands(
    running_app: &RunningSlintApp,
    app_ui_rx: &Rc<RefCell<tokio::sync::mpsc::Receiver<AppUiCommand>>>,
    active_drag: &Rc<RefCell<Option<DragState>>>,
    recording: &Rc<RefCell<Option<RecordingState>>>,
) {
    let mut rx = app_ui_rx.borrow_mut();
    while let Ok(cmd) = rx.try_recv() {
        match cmd {
            AppUiCommand::MouseMove { x, y, response_tx } => {
                let result = dispatch_pointer_moved(running_app, x, y);
                let _ = response_tx.send(result);
            }
            AppUiCommand::MousePress {
                x,
                y,
                button,
                response_tx,
            } => {
                // Slint's hit-tester needs the cursor location set before a press;
                // dispatching `Pressed` cold hit-tests at the previous cursor pos.
                let _ = dispatch_pointer_moved(running_app, x, y);
                let result = dispatch_pointer_pressed(running_app, x, y, button);
                let _ = response_tx.send(result);
            }
            AppUiCommand::MouseRelease {
                x,
                y,
                button,
                response_tx,
            } => {
                let result = dispatch_pointer_released(running_app, x, y, button);
                let _ = response_tx.send(result);
            }
            AppUiCommand::Drag {
                from,
                to,
                steps,
                button,
                response_tx,
            } => {
                if active_drag.borrow().is_some() {
                    let _ = response_tx.send(Err("drag already in progress".to_string()));
                    continue;
                }
                // Move-then-press: slint's hit-tester needs the cursor located
                // before a press can register on a TouchArea.
                let _ = dispatch_pointer_moved(running_app, from.0, from.1);
                let press = dispatch_pointer_pressed(running_app, from.0, from.1, button);
                if let Err(e) = press {
                    let _ = response_tx.send(Err(format!("press failed: {}", e)));
                    continue;
                }
                *active_drag.borrow_mut() = Some(DragState {
                    from,
                    to,
                    total_steps: steps.max(1),
                    current_step: 0,
                    button,
                });
                let _ = response_tx.send(Ok(()));
            }
            AppUiCommand::Screenshot { path, response_tx } => {
                let result = take_screenshot(running_app, &path);
                let _ = response_tx.send(result);
            }
            AppUiCommand::WindowSize { response_tx } => {
                let win = running_app.slint_runtime.slint_instance().window();
                let scale = win.scale_factor();
                let phys = win.size();
                let logical = (
                    phys.width as f32 / scale,
                    phys.height as f32 / scale,
                );
                let _ = response_tx.send(Ok(logical));
            }
            AppUiCommand::RecordStart {
                gif_path,
                states_path,
                captures,
                response_tx,
            } => {
                if recording.borrow().is_some() {
                    let _ = response_tx.send(Err("recording already in progress".to_string()));
                    continue;
                }
                *recording.borrow_mut() = Some(RecordingState {
                    gif_path,
                    states_path,
                    captures,
                    started_at: std::time::Instant::now(),
                    frames: Vec::new(),
                    frames_dropped: 0,
                    target_fps: 10,
                    last_capture_t_ms: 0,
                });
                let _ = response_tx.send(Ok(()));
            }
            AppUiCommand::RecordStop { response_tx } => {
                let Some(state) = recording.borrow_mut().take() else {
                    let _ = response_tx.send(Err("no recording in progress".to_string()));
                    continue;
                };
                // Flush off the slint thread: GIF encoding takes seconds and any
                // blocking here freezes input (synthetic releases go undelivered,
                // leaving the window with stuck pointer state).
                std::thread::Builder::new()
                    .name("ui-record-flush".into())
                    .spawn(move || {
                        let result = flush_recording(state);
                        let _ = response_tx.send(result);
                    })
                    .ok();
            }
        }
    }
}

/// Advance an in-flight drag by one step.
///
/// Releases on the tick *after* reaching the final position: slint propagates
/// the new cursor into DragController.hover-target via DropTargets, which
/// takes a tick to settle; releasing on the same tick would fire `dropped`
/// against a stale hover-target.
fn advance_active_drag(
    running_app: &RunningSlintApp,
    active_drag: &Rc<RefCell<Option<DragState>>>,
) {
    let mut slot = active_drag.borrow_mut();
    let Some(state) = slot.as_mut() else {
        return;
    };
    state.current_step += 1;

    if state.current_step <= state.total_steps {
        let (x, y) = state.position();
        let _ = dispatch_pointer_moved(running_app, x, y);
    } else {
        // Settle tick: hover-target has had a tick to update; release now.
        let final_pos = (state.to.0, state.to.1);
        let button = state.button;
        let _ = dispatch_pointer_released(running_app, final_pos.0, final_pos.1, button);
        *slot = None;
    }
}

/// Mirror the synthetic pointer position into the `VirtualPointer` global so
/// the on-screen cursor sprite tracks it. Best-effort: silently no-ops for
/// apps that don't re-export VirtualPointer.
fn update_virtual_pointer(running_app: &RunningSlintApp, x: f32, y: f32, visible: bool) {
    use slint_interpreter::Value as SlintValue;
    let instance = running_app.slint_runtime.slint_instance();
    let _ = instance.set_global_property("VirtualPointer", "x", SlintValue::Number(x as f64));
    let _ = instance.set_global_property("VirtualPointer", "y", SlintValue::Number(y as f64));
    let _ = instance.set_global_property(
        "VirtualPointer",
        "visible",
        SlintValue::Bool(visible),
    );
}

fn ui_button_to_slint(button: UiMouseButton) -> slint::platform::PointerEventButton {
    match button {
        UiMouseButton::Left => slint::platform::PointerEventButton::Left,
        UiMouseButton::Right => slint::platform::PointerEventButton::Right,
        UiMouseButton::Middle => slint::platform::PointerEventButton::Middle,
    }
}

fn dispatch_pointer_pressed(
    running_app: &RunningSlintApp,
    x: f32,
    y: f32,
    button: UiMouseButton,
) -> Result<(), String> {
    let win = running_app.slint_runtime.slint_instance().window();
    let scale = win.scale_factor();
    let phys = win.size();
    tracing::info!(
        x, y, ?button,
        scale, phys_w = phys.width, phys_h = phys.height,
        "ui: dispatch PointerPressed"
    );
    win.dispatch_event(slint::platform::WindowEvent::PointerPressed {
        position: slint::LogicalPosition::new(x, y),
        button: ui_button_to_slint(button),
    });
    update_virtual_pointer(running_app, x, y, true);
    Ok(())
}

fn dispatch_pointer_released(
    running_app: &RunningSlintApp,
    x: f32,
    y: f32,
    button: UiMouseButton,
) -> Result<(), String> {
    let win = running_app.slint_runtime.slint_instance().window();
    tracing::info!(x, y, ?button, "ui: dispatch PointerReleased");
    win.dispatch_event(slint::platform::WindowEvent::PointerReleased {
        position: slint::LogicalPosition::new(x, y),
        button: ui_button_to_slint(button),
    });
    // Leave the cursor visible at the release point so humans can see where
    // the drag ended; next test step (or RecordStop) will hide it.
    update_virtual_pointer(running_app, x, y, true);
    Ok(())
}

fn dispatch_pointer_moved(
    running_app: &RunningSlintApp,
    x: f32,
    y: f32,
) -> Result<(), String> {
    let win = running_app.slint_runtime.slint_instance().window();
    tracing::debug!(x, y, "ui: dispatch PointerMoved");
    win.dispatch_event(slint::platform::WindowEvent::PointerMoved {
        position: slint::LogicalPosition::new(x, y),
    });
    update_virtual_pointer(running_app, x, y, true);
    Ok(())
}

/// Capture one recording frame (state JSON + pixel buffer), bounded by
/// `MAX_RECORDING_FRAMES`. Drops are counted but the recording stays open
/// so `RecordStop` still flushes what was captured.
fn capture_recording_frame(
    running_app: &RunningSlintApp,
    recording: &Rc<RefCell<Option<RecordingState>>>,
) {
    let mut slot = recording.borrow_mut();
    let Some(state) = slot.as_mut() else {
        return;
    };

    if state.frames.len() >= MAX_RECORDING_FRAMES {
        state.frames_dropped += 1;
        return;
    }

    // Throttle to target_fps: take_snapshot does a multi-MB glReadPixels
    // and would starve the slint thread if done every tick.
    let now_ms = state.started_at.elapsed().as_millis() as u64;
    let min_interval_ms = if state.target_fps == 0 {
        100
    } else {
        1000 / (state.target_fps as u64).max(1)
    };
    if state.last_capture_t_ms != 0
        && now_ms.saturating_sub(state.last_capture_t_ms) < min_interval_ms
    {
        return;
    }

    // Screen capture temporarily disabled: femtovg's `take_snapshot` reads
    // from a GL buffer that ends up blank under our setup. Capture only the
    // per-frame state JSON (assertion-grade); GIF is nice-to-have for humans.
    let instance = running_app.slint_runtime.slint_instance();
    let mut state_obj = serde_json::Map::with_capacity(state.captures.len());
    for cap in &state.captures {
        let value = read_global_value(instance, &cap.global, &cap.prop);
        state_obj.insert(cap.name.clone(), value);
    }

    let t_ms = now_ms;
    state.last_capture_t_ms = t_ms;
    state.frames.push(RecordedFrame {
        t_ms,
        width: 0,
        height: 0,
        rgba: Vec::new(),
        state: serde_json::Value::Object(state_obj),
    });
}

fn read_global_value(
    instance: &slint_interpreter::ComponentInstance,
    global: &str,
    prop: &str,
) -> serde_json::Value {
    use slint_interpreter::Value as SlintValue;
    match instance.get_global_property(global, prop) {
        Ok(v) => match v {
            SlintValue::Number(n) => {
                serde_json::Number::from_f64(n)
                    .map(serde_json::Value::Number)
                    .unwrap_or(serde_json::Value::Null)
            }
            SlintValue::String(s) => serde_json::Value::String(s.to_string()),
            SlintValue::Bool(b) => serde_json::Value::Bool(b),
            other => serde_json::Value::String(format!("{:?}", other)),
        },
        Err(e) => serde_json::Value::String(format!("<err: {}>", e)),
    }
}

/// Flush a recording: encode collected frames as GIF and write the
/// per-frame state log as JSONL. Returns the two paths.
fn flush_recording(state: RecordingState) -> Result<(std::path::PathBuf, std::path::PathBuf), String> {
    use image::codecs::gif::GifEncoder;
    use image::{Delay, Frame};
    use std::io::Write;

    if state.frames_dropped > 0 {
        tracing::warn!(
            dropped = state.frames_dropped,
            captured = state.frames.len(),
            "recording cap reached — some frames dropped"
        );
    }

    if let Some(parent) = state.states_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }

    let frame_count = state.frames.len();
    let any_pixels = state.frames.iter().any(|f| !f.rgba.is_empty());

    let mut states_out = std::fs::File::create(&state.states_path)
        .map_err(|e| format!("create states file: {}", e))?;

    // Skip GIF encoding when frames are state-only (screen capture disabled).
    let gif_path_returned = if any_pixels {
        if let Some(parent) = state.gif_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let gif_file = std::fs::File::create(&state.gif_path)
            .map_err(|e| format!("create gif file: {}", e))?;
        let mut encoder = GifEncoder::new(gif_file);
        encoder
            .set_repeat(image::codecs::gif::Repeat::Infinite)
            .map_err(|e| format!("set_repeat: {}", e))?;

        let mut prev_t: u64 = 0;
        for (idx, frame) in state.frames.iter().enumerate() {
            let delay_ms = frame.t_ms.saturating_sub(prev_t).max(33) as u32;
            prev_t = frame.t_ms;

            let img: image::RgbaImage = match image::ImageBuffer::from_raw(
                frame.width,
                frame.height,
                frame.rgba.clone(),
            ) {
                Some(b) => b,
                None => continue,
            };
            let gif_frame =
                Frame::from_parts(img, 0, 0, Delay::from_numer_denom_ms(delay_ms, 1));
            encoder
                .encode_frame(gif_frame)
                .map_err(|e| format!("encode_frame[{}]: {}", idx, e))?;
        }
        drop(encoder);
        state.gif_path.clone()
    } else {
        std::path::PathBuf::new()
    };

    for (idx, frame) in state.frames.into_iter().enumerate() {
        let line = serde_json::json!({
            "t_ms": frame.t_ms,
            "frame_idx": idx,
            "state": frame.state,
        });
        writeln!(states_out, "{}", line)
            .map_err(|e| format!("write state line: {}", e))?;
    }

    tracing::info!(
        gif = %gif_path_returned.display(),
        states = %state.states_path.display(),
        frames = frame_count,
        gif_skipped = !any_pixels,
        "recording flushed"
    );

    Ok((gif_path_returned, state.states_path))
}

/// Snapshot the live window and write a PNG to `path`.
fn take_screenshot(
    running_app: &RunningSlintApp,
    path: &std::path::Path,
) -> Result<std::path::PathBuf, String> {
    let win = running_app.slint_runtime.slint_instance().window();
    let buf = win
        .take_snapshot()
        .map_err(|e| format!("take_snapshot failed: {}", e))?;
    let width = buf.width();
    let height = buf.height();
    let bytes = buf.as_bytes().to_vec();
    diagnose_buffer("ui_screenshot", width, height, &bytes);
    let img: image::RgbaImage = image::ImageBuffer::from_raw(width, height, bytes)
        .ok_or_else(|| "snapshot buffer size mismatch".to_string())?;
    img.save(path)
        .map_err(|e| format!("save png failed: {}", e))?;
    Ok(path.to_path_buf())
}

/// Log diagnostics about a captured RGBA buffer (non-white pixel count, sample)
/// to detect whether `take_snapshot` actually returned rendered content.
fn diagnose_buffer(label: &str, width: u32, height: u32, bytes: &[u8]) {
    let total_px = (width as usize) * (height as usize);
    let mut non_bg = 0usize;
    let mut min_r = 255u8;
    let mut min_g = 255u8;
    let mut min_b = 255u8;
    for chunk in bytes.chunks_exact(4) {
        let (r, g, b) = (chunk[0], chunk[1], chunk[2]);
        if r < 250 || g < 250 || b < 250 {
            non_bg += 1;
        }
        if r < min_r { min_r = r; }
        if g < min_g { min_g = g; }
        if b < min_b { min_b = b; }
    }
    let pct = if total_px == 0 {
        0.0
    } else {
        (non_bg as f64) / (total_px as f64) * 100.0
    };
    tracing::info!(
        label,
        width,
        height,
        total_px,
        non_bg,
        pct = format!("{:.2}%", pct),
        darkest_rgb = format!("({},{},{})", min_r, min_g, min_b),
        "snapshot buffer diagnostic"
    );
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
        window.set_position(slint::WindowPosition::Physical(
            slint::PhysicalPosition::new(geom.x, geom.y),
        ));
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
    ready_rx: &Rc<
        RefCell<Option<std::sync::mpsc::Receiver<(PreparedPage, ActorRef<ScribeMessage>)>>>,
    >,
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

/// Background task: receive `ImageLoadRequest`s, decrypt + decode, send
/// `ImageLoadResponse`s. Exits when the request channel closes.
async fn spawn_image_loader(
    image_req_rx: &mut tokio::sync::mpsc::Receiver<ImageLoadRequest>,
    image_resp_tx: tokio::sync::mpsc::Sender<ImageLoadResponse>,
    butler: Arc<Butler>,
) {
    while let Some(req) = image_req_rx.recv().await {
        tracing::debug!(
            page_id = %req.page_id,
            hash = %req.hash,
            model = %req.model_name,
            row = req.row_index,
            "Image load task: fetching asset bytes"
        );

        // Retry: assets may arrive slightly later than message rows on a newly synced peer.
        let mut bytes_opt = None;
        for attempt in 1..=5 {
            match butler.assets().get_bytes(&req.page_id, &req.hash).await {
                Ok(b) => {
                    bytes_opt = Some(b);
                    break;
                }
                Err(e) => {
                    tracing::debug!(
                        page_id = %req.page_id,
                        hash = %req.hash,
                        attempt = attempt,
                        error = %e,
                        "Image load task: asset bytes not ready yet"
                    );
                    tokio::time::sleep(Duration::from_millis(300)).await;
                }
            }
        }

        let Some(bytes) = bytes_opt else {
            tracing::warn!(
                page_id = %req.page_id,
                hash = %req.hash,
                "Image load task: giving up after retries"
            );
            let _ = image_resp_tx
                .send(ImageLoadResponse {
                    hash: req.hash,
                    rgba_bytes: Vec::new(),
                    width: 0,
                    height: 0,
                    model_name: req.model_name,
                    row_index: req.row_index,
                })
                .await;
            continue;
        };

        // Decode on a blocking thread: CPU-heavy and the result must be Send
        // (slint::Image isn't), so we hand back raw RGBA8 bytes.
        let hash_clone = req.hash.clone();
        let decode_result =
            tokio::task::spawn_blocking(move || decode_image_thumbnail_bytes(&bytes, 400)).await;

        let (rgba_bytes, width, height) = match decode_result {
            Ok(Ok(tuple)) => tuple,
            Ok(Err(e)) => {
                tracing::warn!(
                    page_id = %req.page_id,
                    hash = %hash_clone,
                    error = %e,
                    "Image load task: failed to decode image"
                );
                let _ = image_resp_tx
                    .send(ImageLoadResponse {
                        hash: req.hash,
                        rgba_bytes: Vec::new(),
                        width: 0,
                        height: 0,
                        model_name: req.model_name,
                        row_index: req.row_index,
                    })
                    .await;
                continue;
            }
            Err(e) => {
                tracing::warn!(
                    page_id = %req.page_id,
                    hash = %hash_clone,
                    error = %e,
                    "Image load task: spawn_blocking panicked"
                );
                let _ = image_resp_tx
                    .send(ImageLoadResponse {
                        hash: req.hash,
                        rgba_bytes: Vec::new(),
                        width: 0,
                        height: 0,
                        model_name: req.model_name,
                        row_index: req.row_index,
                    })
                    .await;
                continue;
            }
        };

        let resp = ImageLoadResponse {
            hash: req.hash.clone(),
            rgba_bytes,
            width,
            height,
            model_name: req.model_name,
            row_index: req.row_index,
        };

        if image_resp_tx.send(resp).await.is_err() {
            tracing::debug!(
                page_id = %req.page_id,
                hash = %req.hash,
                "Image load task: response channel closed — stopping"
            );
            break;
        }

        tracing::debug!(
            page_id = %req.page_id,
            hash = %req.hash,
            "Image load task: image decoded and response sent"
        );
    }

    tracing::debug!("Image load task: request channel closed — exiting");
}
