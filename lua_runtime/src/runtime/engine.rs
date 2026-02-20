use super::*;
use domains::ClockSource;
use mlua::ObjectLike;

impl LuaRuntime {
    /// Run the event loop (blocking)
    pub(super) fn run(&mut self) {
        info!(page_id = %self.page_id, "Lua runtime event loop started");

        self.call_on_init();

        loop {
            match self.step() {
                StepResult::Shutdown => return,
                StepResult::Processed => {}
                StepResult::Idle => {
                    let sleep_ms = {
                        let scheduler = self.scheduler.lock();
                        scheduler
                            .time_until_next()
                            .min(std::time::Duration::from_millis(16))
                    };
                    std::thread::sleep(sleep_ms.min(std::time::Duration::from_millis(1)));
                }
            }
        }
    }

    /// Check if a Lua function is defined
    pub(super) fn has_function(&self, name: &str) -> bool {
        self.lua.globals().get::<Function>(name).is_ok()
    }

    /// Call a Lua function with arguments.
    pub(super) fn call_handler<A: mlua::IntoLuaMulti>(
        &self,
        name: &str,
        args: A,
    ) -> Result<(), String> {
        let func: Function = self
            .lua
            .globals()
            .get(name)
            .map_err(|e| format!("Function '{}' not found: {}", name, e))?;

        func.call::<()>(args)
            .map_err(|e| format!("Error calling '{}': {}", name, e))
    }

    /// Handle a command. Returns true if shutdown was requested.
    pub(super) fn handle_command(&mut self, cmd: LuaCommand) -> bool {
        match cmd {
            LuaCommand::Shutdown => {
                if self.handler_cache.on_shutdown {
                    if let Err(e) = self.call_handler("on_shutdown", ()) {
                        warn!(page_id = %self.page_id, error = %e, "on_shutdown failed");
                    }
                }
                info!(page_id = %self.page_id, "Shutdown requested");
                true
            }

            LuaCommand::LayerChanged {
                layer_name,
                created,
                delta,
                full_data,
                dynamic_ref,
            } => {
                let result = if created {
                    self.handle_layer_discovered(&layer_name, dynamic_ref.as_ref())
                } else {
                    self.handle_loro_change(&layer_name, delta, full_data)
                };
                if let Err(e) = result {
                    warn!(page_id = %self.page_id, layer = %layer_name, error = %e, "Layer change error");
                }
                false
            }

            LuaCommand::TimerFired { timer_id } => {
                self.fire_timer(timer_id);
                false
            }

            LuaCommand::Ephemeral { user_did, payload } => {
                self.handle_ephemeral(&user_did, &payload);
                false
            }

            LuaCommand::StructuredEphemeral {
                from_did,
                func,
                args,
            } => {
                self.handle_structured_ephemeral(&from_did, &func, &args);
                false
            }

            LuaCommand::PeerJoined { user_did } => {
                self.handle_peer_joined(&user_did);
                false
            }

            LuaCommand::PeerLeft { user_did } => {
                self.handle_peer_left(&user_did);
                false
            }

            LuaCommand::AssetUploaded {
                hash,
                filename,
                mime_type,
                size,
            } => {
                self.handle_asset_uploaded(&hash, &filename, &mime_type, size);
                false
            }

            LuaCommand::UiCallback {
                callback_name,
                args,
            } => {
                if let Err(e) = self.handle_ui_callback(&callback_name, args) {
                    warn!(page_id = %self.page_id, callback = %callback_name, error = %e, "UI callback error");
                }
                false
            }

            LuaCommand::UiEvent { event } => {
                if let Err(e) = self.handle_ui_event(event) {
                    warn!(page_id = %self.page_id, error = %e, "UI event error");
                }
                false
            }

            LuaCommand::Validate { ctx, response_tx } => {
                let result = self.validate_ops(&ctx);
                let _ = response_tx.send(result);
                false
            }

            LuaCommand::RebuildDerivation => {
                if let Ok(derivation) = self.lua.globals().get::<mlua::AnyUserData>("derivation") {
                    if let Err(e) = derivation.call_method::<()>("rebuild_all", ()) {
                        warn!(error = %e, "derivation:rebuild_all() failed");
                    }
                }
                false
            }

            LuaCommand::DebugEval { code, response_tx } => {
                let result = self.lua.load(&code).eval::<Value>();
                let json_result = match result {
                    Ok(v) => lua_to_json(&v).map_err(|e| e.to_string()),
                    Err(e) => Err(e.to_string()),
                };
                let _ = response_tx.send(json_result);
                false
            }

            LuaCommand::DebugGetState { response_tx } => {
                let state = DebugState {
                    globals: super::collect_lua_globals(&self.lua),
                    timers: self.scheduler.lock().active_timer_count(),
                    has_on_init: self.has_function("on_init"),
                };
                let _ = response_tx.send(state);
                false
            }

            LuaCommand::SetTime {
                unix_seconds,
                response_tx,
            } => {
                let result = self.set_time(unix_seconds);
                let _ = response_tx.send(result);
                false
            }

            LuaCommand::AdvanceTime {
                seconds,
                response_tx,
            } => {
                let result = self.advance_time(seconds);
                let _ = response_tx.send(result);
                false
            }
        }
    }

    fn handle_loro_change(
        &self,
        layer_name: &str,
        delta: Option<LoroDelta>,
        full_data: Option<JsonValue>,
    ) -> Result<(), String> {
        trace!(
            page_id = %self.page_id,
            layer = %layer_name,
            has_delta = delta.is_some(),
            has_full_data = full_data.is_some(),
            "handle_loro_change ENTRY"
        );

        let bindings_processed =
            self.process_bindings(layer_name, full_data.as_ref(), delta.as_ref());

        if bindings_processed {
            self.trigger_derivation(layer_name);
            return Ok(());
        }

        self.trigger_derivation(layer_name);
        Ok(())
    }

    pub(super) fn trigger_derivation(&self, layer_name: &str) {
        if let Ok(derivation) = self.lua.globals().get::<mlua::AnyUserData>("derivation") {
            let prefixed = format!("{}/{}", self.page_id, layer_name);
            if let Err(e) = derivation.call_method::<()>("on_source_change", prefixed) {
                warn!(page_id = %self.page_id, layer = %layer_name, error = %e, "derivation:on_source_change() failed");
            }
        }
    }

    fn aggregate_wildcard_binding_data(&self, expanded_pattern: &str) -> Result<JsonValue, String> {
        let layer_names = self.scribe.list_layers(expanded_pattern)?;
        let mut aggregated = Vec::new();

        for layer in &layer_names {
            if let Some(data) = self.scribe.get_layer_json(layer)? {
                match data {
                    JsonValue::Array(items) => aggregated.extend(items),
                    JsonValue::Object(map) => {
                        for value in map.into_values() {
                            aggregated.push(value);
                        }
                    }
                    _ => {}
                }
            }
        }

        aggregated.sort_by(|a, b| {
            let ta = a.get("timestamp").and_then(|v| v.as_i64()).unwrap_or(0);
            let tb = b.get("timestamp").and_then(|v| v.as_i64()).unwrap_or(0);
            ta.cmp(&tb)
        });

        Ok(JsonValue::Array(aggregated))
    }

    pub(super) fn process_bindings(
        &self,
        layer_name: &str,
        full_data: Option<&JsonValue>,
        delta: Option<&LoroDelta>,
    ) -> bool {
        if !self.ui_enabled {
            return false;
        }

        let normalized_layer = self.normalize_layer_name_for_lookup(layer_name);
        let manager = self.binding_manager.lock();
        let bindings = manager.get_bindings_for_layer(&normalized_layer);

        if bindings.is_empty() {
            let all_bindings: Vec<_> = manager
                .all_bindings()
                .map(|b| format!("{}→{}", b.expanded_pattern, b.ui_property))
                .collect();
            debug!(
                page_id = %self.page_id,
                layer = %layer_name,
                registered_bindings = ?all_bindings,
                "No binding found for layer"
            );
            return false;
        }

        for binding in bindings {
            if !binding.is_wildcard && binding.expanded_pattern != normalized_layer {
                debug!(
                    page_id = %self.page_id,
                    layer = %layer_name,
                    normalized_layer = %normalized_layer,
                    binding_layer = %binding.expanded_pattern,
                    ui_property = %binding.ui_property,
                    "Skipping non-matching exact binding"
                );
                continue;
            }

            let ui_property = &binding.ui_property;

            if binding.is_wildcard {
                let aggregated =
                    match self.aggregate_wildcard_binding_data(&binding.expanded_pattern) {
                        Ok(data) => data,
                        Err(e) => {
                            warn!(
                                page_id = %self.page_id,
                                layer = %layer_name,
                                ui_property = %ui_property,
                                error = %e,
                                "Failed to aggregate wildcard binding data"
                            );
                            continue;
                        }
                    };

                let processed =
                    match process_binding_data(&self.lua, binding, &aggregated, Some(layer_name)) {
                        Ok(data) => data,
                        Err(e) => {
                            warn!(
                                page_id = %self.page_id,
                                layer = %layer_name,
                                ui_property = %ui_property,
                                error = %e,
                                "Failed to process wildcard binding data"
                            );
                            continue;
                        }
                    };

                if let Some(ref ui_tx) = self.ui_tx {
                    if let Some(mutation) =
                        data_to_ui_mutation(&self.page_id, ui_property, processed)
                    {
                        if let Err(e) = ui_tx.try_send(mutation) {
                            warn!(
                                page_id = %self.page_id,
                                ui_property = %ui_property,
                                error = %e,
                                "Failed to send wildcard binding UI mutation"
                            );
                        } else {
                            debug!(
                                page_id = %self.page_id,
                                layer = %layer_name,
                                ui_property = %ui_property,
                                "Wildcard binding auto-synced to UI"
                            );
                        }
                    }
                }

                continue;
            }

            if let Some(delta) = delta {
                if !binding.is_wildcard {
                    let layer_for_transform = if binding.is_wildcard {
                        Some(layer_name)
                    } else {
                        None
                    };
                    let ops = crate::bindings::binding::convert_delta_for_binding(
                        &self.lua,
                        binding,
                        delta,
                        layer_for_transform,
                    );
                    if !ops.is_empty() {
                        let mutation = UiMutation {
                            app_id: self.page_id.clone(),
                            properties: vec![],
                            model_ops: ops,
                        };
                        if let Some(ref ui_tx) = self.ui_tx {
                            if let Err(e) = ui_tx.try_send(mutation) {
                                warn!(page_id = %self.page_id, ui_property = %ui_property, error = %e, "Failed to send delta binding mutation");
                            } else {
                                debug!(page_id = %self.page_id, layer = %layer_name, ui_property = %ui_property, "Delta binding synced to UI");
                            }
                        }
                        continue;
                    }
                }
            }

            let data = match full_data {
                Some(data) => data,
                None => {
                    debug!(page_id = %self.page_id, layer = %layer_name, ui_property = %ui_property,
                        "No full_data available for fallback Replace");
                    continue;
                }
            };

            let processed = match process_binding_data(
                &self.lua,
                binding,
                data,
                if binding.is_wildcard {
                    Some(layer_name)
                } else {
                    None
                },
            ) {
                Ok(data) => data,
                Err(e) => {
                    warn!(page_id = %self.page_id, layer = %layer_name, ui_property = %ui_property, error = %e,
                        "Failed to process binding data");
                    continue;
                }
            };

            if let Some(ref ui_tx) = self.ui_tx {
                if let Some(mutation) = data_to_ui_mutation(&self.page_id, ui_property, processed) {
                    if let Err(e) = ui_tx.try_send(mutation) {
                        warn!(page_id = %self.page_id, ui_property = %ui_property, error = %e,
                            "Failed to send binding UI mutation");
                    } else {
                        debug!(page_id = %self.page_id, layer = %layer_name, ui_property = %ui_property,
                            "Binding auto-synced to UI (Replace fallback)");
                    }
                }
            }
        }

        true
    }

    /// Validate operations against app's validation logic
    pub fn validate_ops(&self, ctx: &ValidationContext) -> Result<ValidationResult, String> {
        if !self.has_function("validate_ops") {
            return Ok(ValidationResult::default());
        }

        let ops_json = serde_json::to_value(&ctx.ops)
            .map_err(|e| format!("Failed to serialize ops: {}", e))?;
        let ops_lua =
            json_to_lua(&self.lua, &ops_json).map_err(|e| format!("Ops to Lua: {}", e))?;

        let func: Function = self.lua.globals().get("validate_ops").unwrap();
        let result: bool = func
            .call((
                ctx.layer_name.clone(),
                ops_lua,
                ctx.from_did.clone(),
                ctx.role.clone(),
                ctx.page_id.clone(),
            ))
            .map_err(|e| format!("validate_ops error: {}", e))?;

        Ok(ValidationResult {
            valid: result,
            error: if result {
                None
            } else {
                Some("Validation failed".into())
            },
            failed_op_index: None,
        })
    }

    /// Set runtime time (ManualClock only)
    fn set_time(&self, unix_seconds: i64) -> Result<i64, String> {
        // Try to downcast to ManualClock
        let manual_clock = self
            .clock
            .as_any()
            .downcast_ref::<domains::ManualClock>()
            .ok_or_else(|| "Runtime not in test mode (ManualClock required)".to_string())?;

        manual_clock.set_unix(unix_seconds);
        Ok(manual_clock.now_unix())
    }

    /// Advance runtime time (ManualClock only)
    fn advance_time(&self, seconds: u64) -> Result<i64, String> {
        // Try to downcast to ManualClock
        let manual_clock = self
            .clock
            .as_any()
            .downcast_ref::<domains::ManualClock>()
            .ok_or_else(|| "Runtime not in test mode (ManualClock required)".to_string())?;

        manual_clock.advance(std::time::Duration::from_secs(seconds));
        Ok(manual_clock.now_unix())
    }
}
