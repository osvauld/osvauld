//! Slint Runtime - UI layer on main thread
//!
//! Owns: ComponentInstance, VecModels
//! Receives: UiMutation from Lua thread
//! Applies: VecModel operations via AppAPI global
//!
//! **Threading**: Must stay on main thread (Rc<ComponentInstance>, Rc<VecModel>)
//! **Pattern**: Pull mutations from channel via process_ui_mutations()
//! **Global API**: Uses `set_global_property` and `set_global_callback` for AppAPI

use crate::asset_image::{image_from_rgba_bytes, ImageCache, ImageLoadRequest, ImageLoadResponse};
use crate::value_convert::{json_to_slint_value, slint_value_to_json};
use lua_runtime::{LuaCommand, PropertyUpdate, UiMutation, UiQuery, VecModelOp};
use slint::{Model, VecModel};
use slint_interpreter::{Compiler, ComponentInstance, Value as SlintValue};
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use tokio::sync::mpsc;

/// Request to open native file picker for asset upload
///
/// **Context**: Triggered by Slint button click via `pick_asset_file` callback
/// **Security**: Only user-initiated UI actions can trigger this
#[derive(Debug)]
pub struct AssetPickRequest {
    /// Filter type: "images" for image files, "all" for all files
    pub filter: String,
}

/// Global API name - apps define their public interface via `export global AppAPI`
const GLOBAL_API_NAME: &str = "AppAPI";

/// Slint runtime state (main thread only)
///
/// **Ownership**: Rc-based (NOT Send), stays on Slint event loop thread
/// **Lifetime**: Lives until app window closes
pub struct SlintRuntime {
    /// App/Page identifier
    pub page_id: String,

    /// Slint component instance (UI window)
    slint_instance: Rc<ComponentInstance>,

    /// VecModels by property name in AppAPI global (e.g., "products", "orders")
    global_models: HashMap<String, Rc<VecModel<SlintValue>>>,

    /// Channel to receive UI mutations from Lua thread
    ui_rx: mpsc::Receiver<UiMutation>,

    /// Channel to receive UI queries from Lua thread (for ui:get)
    query_rx: mpsc::Receiver<UiQuery>,

    /// Channel to send commands to Lua thread
    lua_tx: mpsc::Sender<LuaCommand>,

    /// Channel to send tab switch requests (std::sync for Slint callback)
    tab_switch_tx: Option<std::sync::mpsc::Sender<String>>,

    /// Channel to send asset pick requests (std::sync for Slint callback)
    asset_pick_tx: Option<std::sync::mpsc::Sender<AssetPickRequest>>,

    /// In-memory cache of decoded asset images (keyed by blake3 hash)
    image_cache: ImageCache,

    /// Channel to send image load requests to the background tokio task
    image_req_tx: Option<mpsc::Sender<ImageLoadRequest>>,

    /// Channel to receive decoded images from the background tokio task
    image_resp_rx: Option<mpsc::Receiver<ImageLoadResponse>>,
}

impl SlintRuntime {
    /// Load Slint UI from file and create runtime
    ///
    /// **Parameters**:
    /// - `slint_path`: Path to .slint file
    /// - `page_id`: App/page identifier
    /// - `ui_rx`: Channel to receive UI mutations from Lua
    /// - `query_rx`: Channel to receive UI queries from Lua (for ui:get)
    /// - `lua_tx`: Channel to send commands to Lua
    /// - `image_req_tx`: Channel to send image load requests to background task (None = no image loading)
    /// - `image_resp_rx`: Channel to receive decoded images from background task (None = no image loading)
    ///
    /// **Returns**: SlintRuntime ready to process mutations
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

        // Compile Slint component
        let compiler = Compiler::default();

        // Note: This is running on the main thread (not Tokio)
        // We need to block on the async compilation
        let result = match tokio::runtime::Handle::try_current() {
            Ok(handle) => tokio::task::block_in_place(|| {
                handle.block_on(compiler.build_from_path(&slint_path))
            }),
            Err(_) => {
                // No runtime, create a temporary one
                let rt = tokio::runtime::Runtime::new()
                    .map_err(|e| format!("Failed to create runtime: {}", e))?;
                rt.block_on(compiler.build_from_path(&slint_path))
            }
        };

        // Check for compilation errors
        let diagnostics: Vec<_> = result
            .diagnostics()
            .filter(|d| d.level() == slint_interpreter::DiagnosticLevel::Error)
            .collect();
        if !diagnostics.is_empty() {
            let errors: Vec<String> = diagnostics.iter().map(|d| d.to_string()).collect();
            return Err(format!("Slint compilation errors: {}", errors.join("; ")).into());
        }

        // Get the component definition (use default component name)
        let definition = result
            .component("App")
            .or_else(|| {
                // Try to get the first component if "App" doesn't exist
                result
                    .component_names()
                    .next()
                    .and_then(|name| result.component(&name))
            })
            .ok_or_else(|| "No component found in .slint file")?;

        // Create component instance
        let slint_instance = definition
            .create()
            .map_err(|e| format!("Failed to create instance: {:?}", e))?;

        tracing::debug!(
            page_id = %page_id,
            "Slint component instance created"
        );

        // Initialize models HashMap
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

        // Models are initialized via init_models() after load, passing manifest.models
        // This allows apps to declare their models in manifest.json

        Ok(runtime)
    }

    /// Initialize VecModels for array properties declared in manifest
    ///
    /// **Called by**: App loader after parsing manifest.json
    /// **Pattern**: Pre-create VecModels for declared models to enable incremental updates
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

    /// Get reference to Slint instance (for showing window, etc.)
    pub fn slint_instance(&self) -> &Rc<ComponentInstance> {
        &self.slint_instance
    }

    /// Set the channel for tab switch requests
    ///
    /// **Context**: Called from app management to receive tab switch events
    pub fn set_tab_switch_channel(&mut self, tx: std::sync::mpsc::Sender<String>) {
        self.tab_switch_tx = Some(tx);
    }

    /// Set the channel for asset pick requests
    ///
    /// **Context**: Called from app management to receive asset upload requests
    /// **Security**: Channel is triggered by Slint button click (user-initiated)
    pub fn set_asset_pick_channel(&mut self, tx: std::sync::mpsc::Sender<AssetPickRequest>) {
        self.asset_pick_tx = Some(tx);
    }

    /// Get or create a VecModel for a property
    ///
    /// **Pattern**: Dynamically create VecModels when needed
    pub fn get_or_create_model(&mut self, prop_name: &str) -> Option<Rc<VecModel<SlintValue>>> {
        // Return existing model if we have it
        if let Some(model) = self.global_models.get(prop_name) {
            return Some(model.clone());
        }

        // Check if this property exists and is a Model type
        if let Ok(value) = self
            .slint_instance
            .get_global_property(GLOBAL_API_NAME, prop_name)
        {
            if matches!(value, SlintValue::Model(_)) {
                // Create new VecModel
                let model = Rc::new(VecModel::<SlintValue>::default());

                // Set as global property
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

                // Store and return
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

    /// Process pending UI mutations (called from timer or event loop)
    ///
    /// **Pattern**: Non-blocking drain of ui_rx channel
    /// **Frequency**: Call this in Slint timer (e.g., every 100ms)
    pub fn process_ui_mutations(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let mut mutation_count = 0;
        let mut error_count = 0;

        // Drain all pending mutations (non-blocking)
        while let Ok(mutation) = self.ui_rx.try_recv() {
            // Apply property updates (continue on error to process remaining)
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

            // Apply VecModel operations (continue on error to process remaining)
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

    /// Process pending UI queries (called from timer or event loop)
    ///
    /// **Pattern**: Non-blocking drain of query_rx channel
    /// **Frequency**: Call this in Slint timer alongside process_ui_mutations
    pub fn process_ui_queries(&mut self) {
        // Drain all pending queries (non-blocking)
        while let Ok(query) = self.query_rx.try_recv() {
            let result = self.read_property(&query.prop_name);
            // Send response back to Lua thread (ignore send errors - Lua may have timed out)
            let _ = query.response_tx.send(result);
        }
    }

    /// Read a property from AppAPI global and convert to JSON
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

    /// Apply property update to AppAPI global
    ///
    /// **Pattern**: All app properties go through AppAPI global
    /// **Triggers**: Property change notification in Slint
    fn apply_property_update(
        &self,
        update: PropertyUpdate,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let slint_val = json_to_slint_value(&update.value)?;

        // All properties go to AppAPI global
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

    /// Apply single VecModel operation to AppAPI global model
    ///
    /// **Triggers**: ModelNotify events (row_added, row_removed, row_changed)
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

                // Check for attachment_hash and enqueue image load if needed
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

                // Convert all items to Slint values
                let slint_items: Vec<SlintValue> = items
                    .into_iter()
                    .filter_map(|item| json_to_slint_value(&item).ok())
                    .collect();

                let count = slint_items.len();

                // Use set_vec for atomic replacement (single UI update)
                model.set_vec(slint_items.clone());

                tracing::debug!(
                    page_id = %self.page_id,
                    global = GLOBAL_API_NAME,
                    model = %model_name,
                    items = count,
                    "VecModel::replace (atomic)"
                );

                // Check every replaced row for attachment_hash
                for (row_index, slint_val) in slint_items.iter().enumerate() {
                    self.check_row_for_attachment(&model_name, row_index, slint_val);
                }
            }
        }

        Ok(())
    }

    /// Inspect a Slint struct value for `attachment_hash` and enqueue an image load if needed.
    ///
    /// **Context**: Called after Push/Insert/Set/Replace so every new or updated row that
    /// carries an `attachment_hash` gets its image loaded asynchronously.
    ///
    /// **Behaviour**:
    /// - If the hash is already cached → inject `attachment_image` immediately via `set_row_data`.
    /// - If the hash is not cached and not already loading → send `ImageLoadRequest` to the
    ///   background tokio task.
    /// - If the hash is already loading → do nothing (response will arrive later).
    fn check_row_for_attachment(&mut self, model_name: &str, row_index: usize, value: &SlintValue) {
        // Only structs can carry attachment_hash
        let fields = match value {
            SlintValue::Struct(s) => s.clone(),
            _ => return,
        };

        // Extract attachment_hash string field
        let hash = match fields.get_field("attachment_hash") {
            Some(SlintValue::String(s)) if !s.is_empty() => s.to_string(),
            _ => return,
        };

        // If already cached, inject immediately
        if let Some(cached_image) = self.image_cache.get(&hash) {
            let cached_image = cached_image.clone();
            self.inject_image_into_row(model_name, row_index, &hash, cached_image);
            return;
        }

        // If already loading, wait for the response
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

        // Enqueue a new load request
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
    ///
    /// Reads the current row struct, sets `attachment_image`, and calls `set_row_data`.
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

        // Read current row and patch attachment_image
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

    /// Drain decoded image responses and apply them to VecModel rows.
    ///
    /// **Pattern**: Called from the Slint timer loop after `process_ui_mutations` and
    /// `process_ui_queries`.  Non-blocking — drains all pending responses in one pass.
    ///
    /// **On cache hit**: Scans every model row whose `attachment_hash` matches the
    /// newly decoded image and injects `attachment_image` via `set_row_data`.
    pub fn apply_loaded_images(&mut self) {
        let Some(ref mut rx) = self.image_resp_rx else {
            return;
        };

        // Collect all ready responses without holding a borrow on self
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

            // Reconstruct slint::Image from raw RGBA bytes on the main thread.
            // (slint::Image is !Send so it cannot cross thread boundaries.)
            let image = image_from_rgba_bytes(&resp.rgba_bytes, resp.width, resp.height);

            // Cache the decoded image
            self.image_cache.insert(resp.hash.clone(), image.clone());

            // Scan all models for rows with this hash and inject the image.
            // We collect model names first to avoid borrow conflicts.
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

    /// Setup shell-level callbacks (tab switching, add-app)
    ///
    /// **Pattern**: Wire shell callbacks to appropriate handlers
    /// **Special callbacks**:
    /// - `select-tab(name)` → sent to tab_switch_tx for tab switching
    /// - `add-app()` → could be wired to Lua if needed
    pub fn setup_callbacks(&self) -> Result<(), Box<dyn std::error::Error>> {
        let definition = self.slint_instance.definition();

        let mut callback_count = 0;
        for callback_name in definition.callbacks() {
            // Handle select-tab specially - send to tab switch channel
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
            // add-app callback - currently no-op, could wire to Lua
        }

        tracing::debug!(
            page_id = %self.page_id,
            callback_count = callback_count,
            "Shell callbacks configured"
        );

        Ok(())
    }

    /// Setup AppAPI global callbacks (generic Event Bus callbacks)
    ///
    /// **Pattern**: Wire AppAPI global callbacks to Lua
    /// Slint `AppAPI.callback foo(args)` → Lua `foo(args)`
    ///
    /// Only wires 3 generic callbacks - apps use Event Bus pattern:
    /// - on_click(target) - button clicks, selections
    /// - on_field_changed(field, value) - text input changes
    /// - on_modal_action(modal, action) - modal open/close/submit
    pub fn setup_global_callbacks(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Generic Event Bus callbacks - same for ALL apps
        // Apps define these in their AppAPI and call them from UI elements
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
        ];

        let mut callback_count = 0;
        for callback_name in generic_callbacks {
            let lua_tx = self.lua_tx.clone();
            let callback_name_owned = callback_name.to_string();

            // Try to set the global callback - if it doesn't exist, that's OK (not all apps have all callbacks)
            match self.slint_instance.set_global_callback(
                GLOBAL_API_NAME,
                callback_name,
                move |args| {
                    // Convert Slint args to JSON for Lua
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
                    // Log at warn level to debug callback registration issues
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

        // Asset file picker callback - triggers native file dialog
        // This requires a user click in Slint (security: user-initiated only)
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
