//! Slint Runtime - UI layer on main thread
//!
//! Owns: ComponentInstance, VecModels
//! Receives: UiMutation from Lua thread
//! Applies: VecModel operations
//!
//! **Threading**: Must stay on main thread (Rc<ComponentInstance>, Rc<VecModel>)
//! **Pattern**: Pull mutations from channel via process_ui_mutations()

use slint::{VecModel, Model};
use slint_interpreter::{ComponentInstance, Compiler, ComponentDefinition, Value as SlintValue};
use std::rc::Rc;
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::sync::mpsc;
use crate::vecmodel_ops::{UiMutation, VecModelOp, PropertyUpdate};
use crate::lua_worker::LuaWorkerCommand;

/// Slint runtime state (main thread only)
///
/// **Ownership**: Rc-based (NOT Send), stays on Slint event loop thread
/// **Lifetime**: Lives until app window closes
pub struct SlintRuntime {
    /// App/Page identifier
    pub page_id: String,

    /// Slint component instance (UI window)
    slint_instance: Rc<ComponentInstance>,

    /// VecModels by property name (e.g., "messages", "users")
    models: HashMap<String, Rc<VecModel<SlintValue>>>,

    /// Channel to receive UI mutations from Lua thread
    ui_rx: mpsc::Receiver<UiMutation>,

    /// Channel to send commands to Lua thread
    lua_tx: mpsc::Sender<LuaWorkerCommand>,
}

impl SlintRuntime {
    /// Load Slint UI from file and create runtime
    ///
    /// **Parameters**:
    /// - `slint_path`: Path to .slint file
    /// - `page_id`: App/page identifier
    /// - `ui_rx`: Channel to receive UI mutations from Lua
    /// - `lua_tx`: Channel to send commands to Lua
    ///
    /// **Returns**: SlintRuntime ready to process mutations
    pub fn load(
        slint_path: PathBuf,
        page_id: String,
        ui_rx: mpsc::Receiver<UiMutation>,
        lua_tx: mpsc::Sender<LuaWorkerCommand>,
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
            Ok(handle) => {
                tokio::task::block_in_place(|| {
                    handle.block_on(compiler.build_from_path(&slint_path))
                })
            }
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
                result.component_names().next().and_then(|name| result.component(&name))
            })
            .ok_or_else(|| "No component found in .slint file")?;

        // Create component instance
        let slint_instance = definition.create()
            .map_err(|e| format!("Failed to create instance: {:?}", e))?;

        tracing::debug!(
            page_id = %page_id,
            "Slint component instance created"
        );

        // Initialize models HashMap
        let models = HashMap::new();

        let mut runtime = Self {
            page_id,
            slint_instance: Rc::new(slint_instance),
            models,
            ui_rx,
            lua_tx,
        };

        // Auto-detect array properties and create VecModels
        runtime.init_models()?;

        Ok(runtime)
    }

    /// Get reference to Slint instance (for showing window, etc.)
    pub fn slint_instance(&self) -> &Rc<ComponentInstance> {
        &self.slint_instance
    }

    /// Initialize VecModels for array properties
    ///
    /// **Pattern**: Iterate over component properties, create VecModel for arrays
    /// **Registered**: Set as property on ComponentInstance
    fn init_models(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Get all properties from component definition
        let definition = self.slint_instance.definition();

        for (prop_name, _prop_type) in definition.properties() {
            // Check if property is a model (array type in Slint)
            // For now, we'll try to get the property value and check if it's a model
            // This is a simplified approach - in production, you'd check the type
            if let Ok(value) = self.slint_instance.get_property(&prop_name) {
                if matches!(value, SlintValue::Model(_)) {
                    // Create empty VecModel for this property
                    let model = Rc::new(VecModel::<SlintValue>::default());

                    // Set as property
                    self.slint_instance.set_property(
                        &prop_name,
                        SlintValue::Model(model.clone().into())
                    )?;

                    // Store in models HashMap
                    self.models.insert(prop_name.clone(), model);

                    tracing::debug!(
                        page_id = %self.page_id,
                        property = %prop_name,
                        "VecModel initialized"
                    );
                }
            }
        }

        tracing::info!(
            page_id = %self.page_id,
            model_count = self.models.len(),
            "VecModels initialized"
        );

        Ok(())
    }

    /// Process pending UI mutations (called from timer or event loop)
    ///
    /// **Pattern**: Non-blocking drain of ui_rx channel
    /// **Frequency**: Call this in Slint timer (e.g., every 100ms)
    pub fn process_ui_mutations(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let mut mutation_count = 0;

        // Drain all pending mutations (non-blocking)
        while let Ok(mutation) = self.ui_rx.try_recv() {
            // Apply property updates
            for prop in mutation.properties {
                self.apply_property_update(prop)?;
            }

            // Apply VecModel operations
            for op in mutation.model_ops {
                self.apply_vecmodel_op(op)?;
            }

            mutation_count += 1;
        }

        if mutation_count > 0 {
            tracing::trace!(
                page_id = %self.page_id,
                count = mutation_count,
                "Processed UI mutations"
            );
        }

        Ok(())
    }

    /// Apply property update to ComponentInstance
    ///
    /// **Triggers**: Property change notification in Slint
    fn apply_property_update(&self, update: PropertyUpdate) -> Result<(), Box<dyn std::error::Error>> {
        let slint_val = json_to_slint_value(&update.value)?;

        self.slint_instance.set_property(&update.key, slint_val)?;

        tracing::trace!(
            page_id = %self.page_id,
            property = %update.key,
            "Property updated"
        );

        Ok(())
    }

    /// Apply single VecModel operation
    ///
    /// **Triggers**: ModelNotify events (row_added, row_removed, row_changed)
    fn apply_vecmodel_op(&self, op: VecModelOp) -> Result<(), Box<dyn std::error::Error>> {
        use VecModelOp::*;

        match op {
            Push { model_name, item } => {
                let model = self.models.get(&model_name)
                    .ok_or_else(|| format!("Model not found: {}", model_name))?;
                let slint_val = json_to_slint_value(&item)?;
                model.push(slint_val);

                tracing::trace!(
                    page_id = %self.page_id,
                    model = %model_name,
                    "VecModel::push"
                );
            }
            Insert { model_name, index, item } => {
                let model = self.models.get(&model_name)
                    .ok_or_else(|| format!("Model not found: {}", model_name))?;
                let slint_val = json_to_slint_value(&item)?;
                model.insert(index, slint_val);

                tracing::trace!(
                    page_id = %self.page_id,
                    model = %model_name,
                    index = index,
                    "VecModel::insert"
                );
            }
            Remove { model_name, index } => {
                let model = self.models.get(&model_name)
                    .ok_or_else(|| format!("Model not found: {}", model_name))?;
                if index < model.row_count() {
                    model.remove(index);

                    tracing::trace!(
                        page_id = %self.page_id,
                        model = %model_name,
                        index = index,
                        "VecModel::remove"
                    );
                } else {
                    tracing::warn!(
                        page_id = %self.page_id,
                        model = %model_name,
                        index = index,
                        len = model.row_count(),
                        "Remove index out of bounds"
                    );
                }
            }
            Set { model_name, index, item } => {
                let model = self.models.get(&model_name)
                    .ok_or_else(|| format!("Model not found: {}", model_name))?;
                if index < model.row_count() {
                    let slint_val = json_to_slint_value(&item)?;
                    model.set_row_data(index, slint_val);

                    tracing::trace!(
                        page_id = %self.page_id,
                        model = %model_name,
                        index = index,
                        "VecModel::set"
                    );
                } else {
                    tracing::warn!(
                        page_id = %self.page_id,
                        model = %model_name,
                        index = index,
                        len = model.row_count(),
                        "Set index out of bounds"
                    );
                }
            }
            Clear { model_name } => {
                let model = self.models.get(&model_name)
                    .ok_or_else(|| format!("Model not found: {}", model_name))?;
                let count = model.row_count();
                for _ in 0..count {
                    model.remove(0);
                }

                tracing::trace!(
                    page_id = %self.page_id,
                    model = %model_name,
                    removed = count,
                    "VecModel::clear"
                );
            }
        }

        Ok(())
    }

    /// Setup UI callbacks (button clicks, etc.)
    ///
    /// **Pattern**: Slint callback → send LuaWorkerCommand::UiCallback
    pub fn setup_callbacks(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Setup "send" callback (button click)
        // Read draft property and pass it to Lua
        let lua_tx = self.lua_tx.clone();
        let instance = self.slint_instance.clone();

        self.slint_instance.set_callback("send", move |_args| {
            // Read current draft from Slint property
            let draft_value = instance.get_property("draft").unwrap_or(SlintValue::String(Default::default()));

            // Convert to JSON for Lua
            let draft_json = match draft_value {
                SlintValue::String(s) => serde_json::Value::String(s.to_string()),
                _ => serde_json::Value::String(String::new()),
            };

            let _ = lua_tx.try_send(LuaWorkerCommand::UiCallback {
                callback_name: "on_send".to_string(),
                args: vec![draft_json],
            });
            SlintValue::Void
        })?;

        tracing::debug!(
            page_id = %self.page_id,
            "UI callbacks configured"
        );

        Ok(())
    }
}

/// Convert JSON to Slint value
///
/// **Note**: This is similar to lua_to_slint_value in slint_model_bindings.rs
fn json_to_slint_value(value: &serde_json::Value) -> Result<SlintValue, Box<dyn std::error::Error>> {
    use slint_interpreter::Value;

    match value {
        serde_json::Value::Null => Ok(Value::Void),
        serde_json::Value::Bool(b) => Ok(Value::Bool(*b)),
        serde_json::Value::Number(n) => {
            if let Some(f) = n.as_f64() {
                Ok(Value::Number(f))
            } else {
                Err(format!("Invalid number: {}", n).into())
            }
        }
        serde_json::Value::String(s) => {
            Ok(Value::String(slint::SharedString::from(s.as_str())))
        }
        serde_json::Value::Array(arr) => {
            // Convert as Slint Model
            let items: Result<Vec<_>, _> = arr.iter()
                .map(json_to_slint_value)
                .collect();
            let items = items?;
            Ok(Value::Model(Rc::new(VecModel::from(items)).into()))
        }
        serde_json::Value::Object(obj) => {
            // Convert as Slint Struct
            let fields: Result<Vec<_>, _> = obj.iter()
                .map(|(k, v)| {
                    json_to_slint_value(v).map(|val| (k.clone(), val))
                })
                .collect();
            let fields = fields?;
            Ok(Value::Struct(slint_interpreter::Struct::from_iter(fields).into()))
        }
    }
}
