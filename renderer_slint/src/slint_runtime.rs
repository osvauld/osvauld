//! Slint Runtime — UI layer on the main thread.
//!
//! Owns the `ComponentInstance` and `VecModel`s; receives `UiMutation`s from
//! the Lua thread and applies them via the `AppAPI` global. Must stay on the
//! Slint event-loop thread (Rc-based, !Send).

use crate::asset_image::{image_from_rgba_bytes, ImageCache, ImageLoadRequest, ImageLoadResponse};
use crate::value_convert::{json_to_slint_value, slint_value_to_json};
use lua_runtime::{LuaCommand, PropertyUpdate, UiMutation, UiQuery, VecModelOp};
use slint::{Model, VecModel};
use slint_interpreter::{Compiler, ComponentInstance, Value as SlintValue};
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use tokio::sync::mpsc;

/// Request to open native file picker for asset upload.
///
/// Only user-initiated UI actions (Slint button click via `pick_asset_file`) trigger this.
#[derive(Debug)]
pub struct AssetPickRequest {
    /// Filter type: "images" for image files, "all" for all files.
    pub filter: String,
}

/// Apps define their public interface via `export global AppAPI`.
const GLOBAL_API_NAME: &str = "AppAPI";

/// Slint runtime state, pinned to the Slint event-loop thread.
pub struct SlintRuntime {
    pub page_id: String,
    slint_instance: Rc<ComponentInstance>,
    /// VecModels by property name in AppAPI global.
    global_models: HashMap<String, Rc<VecModel<SlintValue>>>,
    ui_rx: mpsc::Receiver<UiMutation>,
    query_rx: mpsc::Receiver<UiQuery>,
    lua_tx: mpsc::Sender<LuaCommand>,
    tab_switch_tx: Option<std::sync::mpsc::Sender<String>>,
    asset_pick_tx: Option<std::sync::mpsc::Sender<AssetPickRequest>>,
    image_cache: ImageCache,
    image_req_tx: Option<mpsc::Sender<ImageLoadRequest>>,
    image_resp_rx: Option<mpsc::Receiver<ImageLoadResponse>>,
}

impl SlintRuntime {
    /// Compile and load Slint UI from `slint_path` and create the runtime.
    pub fn load(
        slint_path: PathBuf,
        page_id: String,
        ui_rx: mpsc::Receiver<UiMutation>,
        query_rx: mpsc::Receiver<UiQuery>,
        lua_tx: mpsc::Sender<LuaCommand>,
        image_req_tx: Option<mpsc::Sender<ImageLoadRequest>>,
        image_resp_rx: Option<mpsc::Receiver<ImageLoadResponse>>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        tracing::info!(
            page_id = %page_id,
            path = %slint_path.display(),
            "Loading Slint UI"
        );

        let compiler = Compiler::default();

        // Block on async compilation: we run on the main thread, not on Tokio.
        let result = match tokio::runtime::Handle::try_current() {
            Ok(handle) => tokio::task::block_in_place(|| {
                handle.block_on(compiler.build_from_path(&slint_path))
            }),
            Err(_) => {
                let rt = tokio::runtime::Runtime::new()
                    .map_err(|e| format!("Failed to create runtime: {}", e))?;
                rt.block_on(compiler.build_from_path(&slint_path))
            }
        };

        let diagnostics: Vec<_> = result
            .diagnostics()
            .filter(|d| d.level() == slint_interpreter::DiagnosticLevel::Error)
            .collect();
        if !diagnostics.is_empty() {
            let errors: Vec<String> = diagnostics.iter().map(|d| d.to_string()).collect();
            return Err(format!("Slint compilation errors: {}", errors.join("; ")).into());
        }

        let definition = result
            .component("App")
            .or_else(|| {
                result
                    .component_names()
                    .next()
                    .and_then(|name| result.component(&name))
            })
            .ok_or_else(|| "No component found in .slint file")?;

        let slint_instance = definition
            .create()
            .map_err(|e| format!("Failed to create instance: {:?}", e))?;

        tracing::debug!(
            page_id = %page_id,
            "Slint component instance created"
        );

        let global_models = HashMap::new();

        let runtime = Self {
            page_id,
            slint_instance: Rc::new(slint_instance),
            global_models,
            ui_rx,
            query_rx,
            lua_tx,
            tab_switch_tx: None,
            asset_pick_tx: None,
            image_cache: ImageCache::new(),
            image_req_tx,
            image_resp_rx,
        };

        // Models are initialized via init_models() after load, passing manifest.models.
        Ok(runtime)
    }

    /// Pre-create VecModels for array properties declared in manifest.
    pub fn init_models(
        &mut self,
        model_names: &[String],
    ) -> Result<(), Box<dyn std::error::Error>> {
        for model_name in model_names {
            if let Some(_model) = self.get_or_create_model(model_name) {
                tracing::info!(
                    page_id = %self.page_id,
                    model = %model_name,
                    "VecModel initialized from manifest"
                );
            } else {
                tracing::warn!(
                    page_id = %self.page_id,
                    model = %model_name,
                    "Model declared in manifest but not found in AppAPI"
                );
            }
        }
        Ok(())
    }

    pub fn slint_instance(&self) -> &Rc<ComponentInstance> {
        &self.slint_instance
    }

    pub fn set_tab_switch_channel(&mut self, tx: std::sync::mpsc::Sender<String>) {
        self.tab_switch_tx = Some(tx);
    }

    pub fn set_asset_pick_channel(&mut self, tx: std::sync::mpsc::Sender<AssetPickRequest>) {
        self.asset_pick_tx = Some(tx);
    }

    /// Get or create a VecModel for an `AppAPI` global property.
    pub fn get_or_create_model(&mut self, prop_name: &str) -> Option<Rc<VecModel<SlintValue>>> {
        if let Some(model) = self.global_models.get(prop_name) {
            return Some(model.clone());
        }

        if let Ok(value) = self
            .slint_instance
            .get_global_property(GLOBAL_API_NAME, prop_name)
        {
            if matches!(value, SlintValue::Model(_)) {
                let model = Rc::new(VecModel::<SlintValue>::default());

                if let Err(e) = self.slint_instance.set_global_property(
                    GLOBAL_API_NAME,
                    prop_name,
                    SlintValue::Model(model.clone().into()),
                ) {
                    tracing::warn!(
                        page_id = %self.page_id,
                        global = GLOBAL_API_NAME,
                        property = prop_name,
                        error = %e,
                        "Failed to set global model property"
                    );
                    return None;
                }

                self.global_models
                    .insert(prop_name.to_string(), model.clone());
                tracing::info!(
                    page_id = %self.page_id,
                    global = GLOBAL_API_NAME,
                    property = prop_name,
                    "VecModel created on-demand"
                );
                return Some(model);
            }
        }

        None
    }

    /// Non-blocking drain of pending UI mutations; call each timer tick.
    pub fn process_ui_mutations(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let mut mutation_count = 0;
        let mut error_count = 0;

        while let Ok(mutation) = self.ui_rx.try_recv() {
            for prop in mutation.properties {
                if let Err(e) = self.apply_property_update(prop) {
                    tracing::warn!(
                        page_id = %self.page_id,
                        error = %e,
                        "Property update failed"
                    );
                    error_count += 1;
                }
            }

            for op in mutation.model_ops {
                if let Err(e) = self.apply_vecmodel_op(op) {
                    tracing::warn!(
                        page_id = %self.page_id,
                        error = %e,
                        "VecModel operation failed"
                    );
                    error_count += 1;
                }
            }

            mutation_count += 1;
        }

        if mutation_count > 0 {
            tracing::debug!(
                page_id = %self.page_id,
                count = mutation_count,
                errors = error_count,
                "Processed UI mutations"
            );
        }

        Ok(())
    }

    /// Non-blocking drain of pending UI queries; call alongside `process_ui_mutations`.
    pub fn process_ui_queries(&mut self) {
        while let Ok(query) = self.query_rx.try_recv() {
            let result = self.read_property(&query.prop_name);
            // Lua may have timed out; ignore send errors.
            let _ = query.response_tx.send(result);
        }
    }

    fn read_property(&self, prop_name: &str) -> Option<serde_json::Value> {
        match self
            .slint_instance
            .get_global_property(GLOBAL_API_NAME, prop_name)
        {
            Ok(value) => Some(slint_value_to_json(&value)),
            Err(e) => {
                tracing::warn!(
                    page_id = %self.page_id,
                    global = GLOBAL_API_NAME,
                    property = %prop_name,
                    error = %e,
                    "Failed to read global property"
                );
                None
            }
        }
    }

    fn apply_property_update(
        &self,
        update: PropertyUpdate,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let slint_val = json_to_slint_value(&update.value)?;

        if let Err(e) =
            self.slint_instance
                .set_global_property(GLOBAL_API_NAME, &update.key, slint_val)
        {
            tracing::warn!(
                page_id = %self.page_id,
                global = GLOBAL_API_NAME,
                property = %update.key,
                error = %e,
                "Failed to set global property"
            );
            return Err(e.into());
        }

        tracing::debug!(
            page_id = %self.page_id,
            global = GLOBAL_API_NAME,
            property = %update.key,
            "Global property updated"
        );

        Ok(())
    }

    fn apply_vecmodel_op(&mut self, op: VecModelOp) -> Result<(), Box<dyn std::error::Error>> {
        use VecModelOp::*;

        match op {
            Push { model_name, item } => {
                let model = self.ensure_model(&model_name, "Push")?;
                let slint_val = json_to_slint_value(&item)?;
                let row_index = model.row_count(); // index after push
                model.push(slint_val.clone());

                tracing::debug!(
                    page_id = %self.page_id,
                    global = GLOBAL_API_NAME,
                    model = %model_name,
                    row_count = model.row_count(),
                    "VecModel::push"
                );

                self.check_row_for_attachment(&model_name, row_index, &slint_val);
            }
            Insert {
                model_name,
                index,
                item,
            } => {
                let model = self.ensure_model(&model_name, "Insert")?;
                let slint_val = json_to_slint_value(&item)?;
                model.insert(index, slint_val.clone());

                tracing::trace!(
                    page_id = %self.page_id,
                    global = GLOBAL_API_NAME,
                    model = %model_name,
                    index = index,
                    "VecModel::insert"
                );

                // Check for attachment_hash and enqueue image load if needed
                self.check_row_for_attachment(&model_name, index, &slint_val);
            }
            Remove { model_name, index } => {
                let model = self.ensure_model(&model_name, "Remove")?;
                if index < model.row_count() {
                    model.remove(index);

                    tracing::trace!(
                        page_id = %self.page_id,
                        global = GLOBAL_API_NAME,
                        model = %model_name,
                        index = index,
                        "VecModel::remove"
                    );
                } else {
                    tracing::warn!(
                        page_id = %self.page_id,
                        global = GLOBAL_API_NAME,
                        model = %model_name,
                        index = index,
                        len = model.row_count(),
                        "Remove index out of bounds"
                    );
                }
            }
            Set {
                model_name,
                index,
                item,
            } => {
                let model = self.ensure_model(&model_name, "Set")?;
                if index < model.row_count() {
                    let slint_val = json_to_slint_value(&item)?;
                    model.set_row_data(index, slint_val.clone());

                    tracing::trace!(
                        page_id = %self.page_id,
                        global = GLOBAL_API_NAME,
                        model = %model_name,
                        index = index,
                        "VecModel::set"
                    );

                    // Check for attachment_hash and enqueue image load if needed
                    self.check_row_for_attachment(&model_name, index, &slint_val);
                } else {
                    tracing::warn!(
                        page_id = %self.page_id,
                        global = GLOBAL_API_NAME,
                        model = %model_name,
                        index = index,
                        len = model.row_count(),
                        "Set index out of bounds"
                    );
                }
            }
            Clear { model_name } => {
                let model = self.ensure_model(&model_name, "Clear")?;
                let count = model.row_count();
                model.set_vec(vec![]);

                tracing::debug!(
                    page_id = %self.page_id,
                    global = GLOBAL_API_NAME,
                    model = %model_name,
                    removed = count,
                    "VecModel::clear"
                );
            }
            Replace { model_name, items } => {
                let model = self.ensure_model(&model_name, "Replace")?;

                let slint_items: Vec<SlintValue> = items
                    .into_iter()
                    .filter_map(|item| json_to_slint_value(&item).ok())
                    .collect();

                let count = slint_items.len();

                // `set_vec` is an atomic replacement (single UI update).
                model.set_vec(slint_items.clone());

                tracing::debug!(
                    page_id = %self.page_id,
                    global = GLOBAL_API_NAME,
                    model = %model_name,
                    items = count,
                    "VecModel::replace (atomic)"
                );

                for (row_index, slint_val) in slint_items.iter().enumerate() {
                    self.check_row_for_attachment(&model_name, row_index, slint_val);
                }
            }
        }

        Ok(())
    }

    /// Inspect a row for `attachment_hash` and enqueue (or inject cached) image.
    fn check_row_for_attachment(&mut self, model_name: &str, row_index: usize, value: &SlintValue) {
        let fields = match value {
            SlintValue::Struct(s) => s.clone(),
            _ => return,
        };

        let hash = match fields.get_field("attachment_hash") {
            Some(SlintValue::String(s)) if !s.is_empty() => s.to_string(),
            _ => return,
        };

        if let Some(cached_image) = self.image_cache.get(&hash) {
            let cached_image = cached_image.clone();
            self.inject_image_into_row(model_name, row_index, &hash, cached_image);
            return;
        }

        if self.image_cache.is_loading(&hash) {
            tracing::trace!(
                page_id = %self.page_id,
                model = %model_name,
                row = row_index,
                hash = %hash,
                "Image already loading — will inject on response"
            );
            return;
        }

        self.enqueue_image_load(hash, model_name.to_string(), row_index);
    }

    /// Send an `ImageLoadRequest` to the background task and mark the hash as loading.
    fn enqueue_image_load(&mut self, hash: String, model_name: String, row_index: usize) {
        let Some(ref tx) = self.image_req_tx else {
            tracing::debug!(
                page_id = %self.page_id,
                hash = %hash,
                "Image load requested but no image channel configured"
            );
            return;
        };

        let req = ImageLoadRequest {
            hash: hash.clone(),
            page_id: self.page_id.clone(),
            model_name,
            row_index,
        };

        match tx.try_send(req) {
            Ok(_) => {
                self.image_cache.mark_loading(hash.clone());
                tracing::debug!(
                    page_id = %self.page_id,
                    hash = %hash,
                    "Image load request enqueued"
                );
            }
            Err(e) => {
                tracing::warn!(
                    page_id = %self.page_id,
                    hash = %hash,
                    error = %e,
                    "Failed to enqueue image load request"
                );
            }
        }
    }

    /// Write a decoded `slint::Image` into the `attachment_image` field of a model row.
    fn inject_image_into_row(
        &self,
        model_name: &str,
        row_index: usize,
        hash: &str,
        image: slint::Image,
    ) {
        let Some(model) = self.global_models.get(model_name) else {
            tracing::warn!(
                page_id = %self.page_id,
                model = %model_name,
                hash = %hash,
                "Cannot inject image — model not found"
            );
            return;
        };

        if row_index >= model.row_count() {
            tracing::warn!(
                page_id = %self.page_id,
                model = %model_name,
                row = row_index,
                len = model.row_count(),
                hash = %hash,
                "Cannot inject image — row index out of bounds"
            );
            return;
        }

        let current = model.row_data(row_index);
        let updated = match current {
            Some(SlintValue::Struct(mut s)) => {
                s.set_field("attachment_image".into(), SlintValue::Image(image));
                SlintValue::Struct(s)
            }
            Some(other) => {
                tracing::warn!(
                    page_id = %self.page_id,
                    model = %model_name,
                    row = row_index,
                    "Row is not a struct — cannot inject attachment_image"
                );
                other
            }
            None => {
                tracing::warn!(
                    page_id = %self.page_id,
                    model = %model_name,
                    row = row_index,
                    "Row data is None — cannot inject attachment_image"
                );
                return;
            }
        };

        model.set_row_data(row_index, updated);

        tracing::debug!(
            page_id = %self.page_id,
            model = %model_name,
            row = row_index,
            hash = %hash,
            "attachment_image injected into row"
        );
    }

    /// Drain decoded image responses and inject them into matching VecModel rows.
    pub fn apply_loaded_images(&mut self) {
        let Some(ref mut rx) = self.image_resp_rx else {
            return;
        };

        // Collect first, to avoid holding a borrow on self while injecting.
        let mut responses = Vec::new();
        while let Ok(resp) = rx.try_recv() {
            responses.push(resp);
        }

        for resp in responses {
            tracing::debug!(
                page_id = %self.page_id,
                hash = %resp.hash,
                model = %resp.model_name,
                row = resp.row_index,
                "Image load response received"
            );

            if resp.rgba_bytes.is_empty() || resp.width == 0 || resp.height == 0 {
                self.image_cache.clear_loading(&resp.hash);
                tracing::debug!(
                    page_id = %self.page_id,
                    hash = %resp.hash,
                    "Image load failed; cleared loading state"
                );
                continue;
            }

            // Reconstruct on the main thread; slint::Image is !Send.
            let image = image_from_rgba_bytes(&resp.rgba_bytes, resp.width, resp.height);

            self.image_cache.insert(resp.hash.clone(), image.clone());

            // Collect model names first to avoid borrow conflicts during injection.
            let model_names: Vec<String> = self.global_models.keys().cloned().collect();
            for model_name in model_names {
                let row_count = self
                    .global_models
                    .get(&model_name)
                    .map(|m| m.row_count())
                    .unwrap_or(0);

                for row_index in 0..row_count {
                    let row_hash = self
                        .global_models
                        .get(&model_name)
                        .and_then(|m| m.row_data(row_index))
                        .and_then(|v| match v {
                            SlintValue::Struct(s) => match s.get_field("attachment_hash") {
                                Some(SlintValue::String(h)) if !h.is_empty() => Some(h.to_string()),
                                _ => None,
                            },
                            _ => None,
                        });

                    if row_hash.as_deref() == Some(resp.hash.as_str()) {
                        self.inject_image_into_row(
                            &model_name,
                            row_index,
                            &resp.hash,
                            image.clone(),
                        );
                    }
                }
            }
        }
    }

    fn ensure_model(
        &mut self,
        model_name: &str,
        operation: &str,
    ) -> Result<Rc<VecModel<SlintValue>>, Box<dyn std::error::Error>> {
        if !self.global_models.contains_key(model_name) {
            self.get_or_create_model(model_name);
        }

        match self.global_models.get(model_name) {
            Some(model) => Ok(model.clone()),
            None => {
                tracing::error!(
                    page_id = %self.page_id,
                    global = GLOBAL_API_NAME,
                    model = %model_name,
                    operation = %operation,
                    available_models = ?self.global_models.keys().collect::<Vec<_>>(),
                    "Model not found for VecModel operation"
                );
                Err(format!("Model not found in {}: {}", GLOBAL_API_NAME, model_name).into())
            }
        }
    }

    /// Wire shell-level callbacks. Currently `select-tab` is forwarded to the
    /// tab-switch channel; `add-app` is a no-op placeholder.
    pub fn setup_callbacks(&self) -> Result<(), Box<dyn std::error::Error>> {
        let definition = self.slint_instance.definition();

        let mut callback_count = 0;
        for callback_name in definition.callbacks() {
            if callback_name == "select-tab" || callback_name == "select_tab" {
                if let Some(ref tx) = self.tab_switch_tx {
                    let tx = tx.clone();
                    self.slint_instance
                        .set_callback(&callback_name, move |args| {
                            if let Some(tab_name) = args.first() {
                                if let SlintValue::String(name) = tab_name {
                                    tracing::info!(
                                        tab = %name,
                                        "Tab switch requested"
                                    );
                                    let _ = tx.send(name.to_string());
                                }
                            }
                            SlintValue::Void
                        })?;
                    callback_count += 1;
                }
            }
        }

        tracing::debug!(
            page_id = %self.page_id,
            callback_count = callback_count,
            "Shell callbacks configured"
        );

        Ok(())
    }

    /// Wire generic AppAPI callbacks (event-bus style) to the Lua thread.
    /// Slint `AppAPI.callback foo(args)` → `LuaCommand::UiCallback { foo, args }`.
    pub fn setup_global_callbacks(&self) -> Result<(), Box<dyn std::error::Error>> {
        let generic_callbacks = [
            "on_click",
            "on_field_changed",
            "on_modal_action",
            "on_pointer_event", // For canvas/drawing apps that need x,y coordinates
            "on_scroll",        // For canvas/drawing apps that need scroll/zoom
            "on_hover",         // For cursor sync without drag (timer-based polling)
            "on_key_pressed",   // For games/interactive apps that need keyboard input
            "on_submit",        // For form submissions (chat, search, etc.)
            "on_text_input",    // For text input changes (typing indicators, etc.)
            "on_drop",          // For drop events from the drag engine (zone, payload, kind, y_intent, x_intent, y_frac, x_frac)
        ];

        let mut callback_count = 0;
        for callback_name in generic_callbacks {
            let lua_tx = self.lua_tx.clone();
            let callback_name_owned = callback_name.to_string();

            // Not all apps define every callback; missing ones are fine.
            match self.slint_instance.set_global_callback(
                GLOBAL_API_NAME,
                callback_name,
                move |args| {
                    let json_args: Vec<serde_json::Value> =
                        args.iter().map(slint_value_to_json).collect();

                    tracing::trace!(
                        global = GLOBAL_API_NAME,
                        callback = %callback_name_owned,
                        arg_count = json_args.len(),
                        "AppAPI callback triggered"
                    );

                    let _ = lua_tx.try_send(LuaCommand::UiCallback {
                        callback_name: callback_name_owned.clone(),
                        args: json_args,
                    });
                    SlintValue::Void
                },
            ) {
                Ok(_) => {
                    callback_count += 1;
                    tracing::debug!(
                        page_id = %self.page_id,
                        global = GLOBAL_API_NAME,
                        callback = callback_name,
                        "AppAPI callback configured"
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        page_id = %self.page_id,
                        global = GLOBAL_API_NAME,
                        callback = callback_name,
                        error = %e,
                        "AppAPI callback registration failed"
                    );
                }
            }
        }

        tracing::info!(
            page_id = %self.page_id,
            global = GLOBAL_API_NAME,
            callback_count = callback_count,
            "AppAPI callbacks configured"
        );

        // Asset file picker: must be user-initiated (Slint button click) for security.
        if let Some(ref tx) = self.asset_pick_tx {
            let tx = tx.clone();
            let page_id = self.page_id.clone();
            match self.slint_instance.set_global_callback(
                GLOBAL_API_NAME,
                "pick_asset_file",
                move |args| {
                    let filter = args
                        .first()
                        .and_then(|v| v.clone().try_into().ok())
                        .and_then(|v: slint::SharedString| Some(v.to_string()))
                        .unwrap_or_else(|| "all".to_string());

                    tracing::info!(
                        page_id = %page_id,
                        filter = %filter,
                        "Asset pick requested via AppAPI callback"
                    );

                    let _ = tx.send(AssetPickRequest { filter });
                    SlintValue::Void
                },
            ) {
                Ok(_) => {
                    tracing::debug!(
                        page_id = %self.page_id,
                        global = GLOBAL_API_NAME,
                        callback = "pick_asset_file",
                        "Asset picker callback configured"
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        page_id = %self.page_id,
                        global = GLOBAL_API_NAME,
                        callback = "pick_asset_file",
                        error = %e,
                        "Asset picker callback registration failed (app may not define it)"
                    );
                }
            }
        }

        Ok(())
    }
}
