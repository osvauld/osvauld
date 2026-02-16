//! Single-app test runner
//!
//! Creates a headless LuaRuntime with MockScribeHandle + BufferedUiBindings
//! for testing Lua app logic without the full stack.

use std::path::Path;
use std::sync::{Arc, Mutex};

use serde_json::Value as JsonValue;
use tokio::sync::mpsc;

use lua_runtime::{
    BufferedUiState, LuaCommand, LuaRuntime, LuaRuntimeConfig, MockScribeHandle, MockScribeState,
    StepResult, UiEventType,
};

/// Single-app test runner
///
/// Wraps a headless LuaRuntime with MockScribeHandle for easy testing.
pub struct AppTestRunner {
    runtime: LuaRuntime,
    cmd_tx: mpsc::Sender<LuaCommand>,
    mock_state: Arc<Mutex<MockScribeState>>,
    #[allow(dead_code)]
    buffered_ui: Arc<std::sync::Mutex<BufferedUiState>>,
    page_id: String,
}

impl AppTestRunner {
    /// Load an app from a directory
    ///
    /// Reads manifest.json + app.lua (or entry_logic from manifest).
    /// Creates MockScribeHandle + headless LuaRuntime.
    pub fn load(app_dir: &Path, role: &str, did: &str, name: &str) -> Result<Self, String> {
        // Read manifest
        let manifest_path = app_dir.join("manifest.json");
        let manifest_str = std::fs::read_to_string(&manifest_path)
            .map_err(|e| format!("Failed to read manifest.json: {}", e))?;
        let manifest: serde_json::Value = serde_json::from_str(&manifest_str)
            .map_err(|e| format!("Failed to parse manifest.json: {}", e))?;

        // Get entry logic file name
        let entry_logic = manifest
            .get("entry_logic")
            .and_then(|v| v.as_str())
            .unwrap_or("app.lua");

        // Read Lua code
        let lua_path = app_dir.join(entry_logic);
        let lua_code = std::fs::read_to_string(&lua_path)
            .map_err(|e| format!("Failed to read {}: {}", entry_logic, e))?;

        // Set package.path so require() finds .lua modules in the app directory
        // (matches production behavior in renderer_slint)
        let app_dir_abs = app_dir
            .canonicalize()
            .map_err(|e| format!("Failed to resolve app dir: {}", e))?;
        let package_path_preamble = format!(
            "package.path = '{}/?.lua;' .. package.path\n",
            app_dir_abs.display()
        );

        // Also check for init.lua (derivation rules)
        let init_path = app_dir.join("init.lua");
        let full_code = if init_path.exists() {
            let init_code = std::fs::read_to_string(&init_path)
                .map_err(|e| format!("Failed to read init.lua: {}", e))?;
            format!("{}{}\n{}", package_path_preamble, init_code, lua_code)
        } else {
            format!("{}{}", package_path_preamble, lua_code)
        };

        let app_name = manifest
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("test-app")
            .to_string();

        Self::from_code(&full_code, &app_name, role, did, name)
    }

    /// Create runner from raw Lua code
    pub fn from_code(
        lua_code: &str,
        app_name: &str,
        role: &str,
        did: &str,
        name: &str,
    ) -> Result<Self, String> {
        let page_id = format!("test-page-{}", app_name.replace(' ', "-").to_lowercase());
        Self::from_code_with_page(lua_code, app_name, &page_id, role, did, name)
    }

    /// Create runner from raw Lua code with explicit page_id
    pub fn from_code_with_page(
        lua_code: &str,
        app_name: &str,
        page_id: &str,
        role: &str,
        did: &str,
        name: &str,
    ) -> Result<Self, String> {
        let mock_state = MockScribeHandle::shared_state();
        // Set page_id for layer name normalization (matches real Scribe behavior)
        mock_state.lock().unwrap().set_page_id(page_id);
        let scribe = MockScribeHandle::with_shared_state(mock_state.clone());

        let config = LuaRuntimeConfig {
            page_id: page_id.to_string(),
            app_name: app_name.to_string(),
            scribe,
            user_did: did.to_string(),
            user_name: name.to_string(),
            user_role: role.to_string(),
            lua_code: lua_code.to_string(),
            ui_enabled: false,
            ui_tx: None,
            query_tx: None,
            navigate_tx: None,
        };

        let (runtime, cmd_tx) = LuaRuntime::new_headless(config)?;

        // Access the BufferedUiBindings state (it's set as a global in headless mode)
        // For now, create a separate tracking state
        let buffered_ui = Arc::new(std::sync::Mutex::new(BufferedUiState {
            properties: std::collections::HashMap::new(),
            mutations: Vec::new(),
            models: std::collections::HashMap::new(),
        }));

        Ok(Self {
            runtime,
            cmd_tx,
            mock_state,
            buffered_ui,
            page_id: page_id.to_string(),
        })
    }

    /// Create runner with shared MockScribeState (for multi-peer testing)
    pub fn from_code_with_shared_state(
        lua_code: &str,
        app_name: &str,
        page_id: &str,
        role: &str,
        did: &str,
        name: &str,
        shared_state: Arc<Mutex<MockScribeState>>,
    ) -> Result<Self, String> {
        // Ensure page_id is set for layer name normalization
        shared_state.lock().unwrap().set_page_id(page_id);
        let scribe = MockScribeHandle::with_shared_state(shared_state.clone());

        let config = LuaRuntimeConfig {
            page_id: page_id.to_string(),
            app_name: app_name.to_string(),
            scribe,
            user_did: did.to_string(),
            user_name: name.to_string(),
            user_role: role.to_string(),
            lua_code: lua_code.to_string(),
            ui_enabled: false,
            ui_tx: None,
            query_tx: None,
            navigate_tx: None,
        };

        let (runtime, cmd_tx) = LuaRuntime::new_headless(config)?;

        let buffered_ui = Arc::new(std::sync::Mutex::new(BufferedUiState {
            properties: std::collections::HashMap::new(),
            mutations: Vec::new(),
            models: std::collections::HashMap::new(),
        }));

        Ok(Self {
            runtime,
            cmd_tx,
            mock_state: shared_state,
            buffered_ui,
            page_id: page_id.to_string(),
        })
    }

    // -- Lifecycle --

    /// Run on_init
    pub fn call_on_init(&mut self) {
        self.runtime.call_on_init();
    }

    /// Process a single step (fire timers + try one command)
    pub fn tick(&mut self) -> StepResult {
        self.runtime.step()
    }

    /// Process N steps
    pub fn tick_n(&mut self, n: usize) {
        for _ in 0..n {
            self.runtime.step();
        }
    }

    /// Process steps until idle (no more commands to process)
    pub fn tick_until_idle(&mut self, max_steps: usize) -> usize {
        let mut steps = 0;
        for _ in 0..max_steps {
            match self.runtime.step() {
                StepResult::Processed => steps += 1,
                StepResult::Idle | StepResult::Shutdown => break,
            }
        }
        steps
    }

    // -- Command injection --

    /// Fire a UI callback by name with JSON args
    pub fn fire_callback(&self, name: &str, args: Vec<JsonValue>) -> Result<(), String> {
        self.cmd_tx
            .try_send(LuaCommand::UiCallback {
                callback_name: name.to_string(),
                args,
            })
            .map_err(|e| format!("Failed to send callback: {}", e))
    }

    /// Fire a key press event
    pub fn fire_key(&self, key: &str) -> Result<(), String> {
        self.cmd_tx
            .try_send(LuaCommand::UiEvent {
                event: UiEventType::KeyPressed {
                    key: key.to_string(),
                },
            })
            .map_err(|e| format!("Failed to send key: {}", e))
    }

    /// Fire a text input event
    pub fn fire_text_input(&self, text: &str) -> Result<(), String> {
        self.cmd_tx
            .try_send(LuaCommand::UiEvent {
                event: UiEventType::TextChanged {
                    element: String::new(),
                    text: text.to_string(),
                },
            })
            .map_err(|e| format!("Failed to send text: {}", e))
    }

    /// Inject a layer change (simulates remote write arriving)
    pub fn inject_loro_change(
        &self,
        layer_name: &str,
        full_data: Option<JsonValue>,
    ) -> Result<(), String> {
        self.cmd_tx
            .try_send(LuaCommand::LayerChanged {
                layer_name: layer_name.to_string(),
                created: false,
                delta: None,
                full_data,
            })
            .map_err(|e| format!("Failed to inject loro change: {}", e))
    }

    /// Inject an ephemeral message (simulates remote peer sending)
    pub fn inject_ephemeral(
        &self,
        from_did: &str,
        func: &str,
        args: JsonValue,
    ) -> Result<(), String> {
        self.cmd_tx
            .try_send(LuaCommand::StructuredEphemeral {
                from_did: from_did.to_string(),
                func: func.to_string(),
                args,
            })
            .map_err(|e| format!("Failed to inject ephemeral: {}", e))
    }

    /// Inject peer joined event
    pub fn inject_peer_joined(&self, did: &str) -> Result<(), String> {
        self.cmd_tx
            .try_send(LuaCommand::PeerJoined {
                user_did: did.to_string(),
            })
            .map_err(|e| format!("Failed to inject peer joined: {}", e))
    }

    /// Inject peer left event
    pub fn inject_peer_left(&self, did: &str) -> Result<(), String> {
        self.cmd_tx
            .try_send(LuaCommand::PeerLeft {
                user_did: did.to_string(),
            })
            .map_err(|e| format!("Failed to inject peer left: {}", e))
    }

    // -- Evaluation --

    /// Evaluate Lua code and return JSON result
    pub fn eval(&self, code: &str) -> Result<JsonValue, String> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.cmd_tx
            .try_send(LuaCommand::DebugEval {
                code: code.to_string(),
                response_tx: tx,
            })
            .map_err(|e| format!("Failed to send eval: {}", e))?;

        // Need to step to process the command
        // The caller should call tick() after this
        // Return a placeholder - actual result comes after tick
        // This design is slightly awkward; let's provide eval_sync instead
        drop(rx);
        Ok(JsonValue::Null)
    }

    /// Evaluate Lua code synchronously (sends command + ticks to process it)
    pub fn eval_sync(&mut self, code: &str) -> Result<JsonValue, String> {
        let (tx, mut rx) = tokio::sync::oneshot::channel();
        self.cmd_tx
            .try_send(LuaCommand::DebugEval {
                code: code.to_string(),
                response_tx: tx,
            })
            .map_err(|e| format!("Failed to send eval: {}", e))?;

        // Step to process
        self.runtime.step();

        rx.try_recv()
            .map_err(|e| format!("Eval response not ready: {}", e))?
            .map_err(|e| format!("Eval error: {}", e))
    }

    // -- Inspection --

    /// Get the page ID
    pub fn page_id(&self) -> &str {
        &self.page_id
    }

    /// Get layer data from mock scribe
    pub fn layer_data(&self, name: &str) -> Option<JsonValue> {
        self.mock_state.lock().unwrap().get_layer_data(name)
    }

    /// Get all layer names
    pub fn layer_names(&self) -> Vec<String> {
        self.mock_state.lock().unwrap().layer_names()
    }

    /// Get sent ephemerals
    pub fn sent_ephemerals(&self) -> Vec<Vec<u8>> {
        self.mock_state.lock().unwrap().get_sent_ephemerals()
    }

    /// Clear sent ephemerals
    pub fn clear_ephemerals(&self) {
        self.mock_state.lock().unwrap().clear_ephemerals();
    }

    /// Inject data directly into a list layer (simulates peer write)
    pub fn inject_list_data(&self, layer_name: &str, items: Vec<JsonValue>) {
        self.mock_state
            .lock()
            .unwrap()
            .inject_list_data(layer_name, items);
    }

    /// Inject data directly into a map layer (simulates peer write)
    pub fn inject_map_data(
        &self,
        layer_name: &str,
        data: std::collections::HashMap<String, JsonValue>,
    ) {
        self.mock_state
            .lock()
            .unwrap()
            .inject_map_data(layer_name, data);
    }

    /// Get the shared mock state (for advanced inspection)
    pub fn mock_state(&self) -> Arc<Mutex<MockScribeState>> {
        self.mock_state.clone()
    }

    /// Get the command sender (for advanced use)
    pub fn cmd_tx(&self) -> &mpsc::Sender<LuaCommand> {
        &self.cmd_tx
    }
}
